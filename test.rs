//! # Note to Marker
//! These are self written autotests. Working on CSE requires the BINARY_PATH, and REMOTE_PORT to be updated accordingly.
//! **IMPORTANT:** If working on CSE, be sure to start birdie and set its port accoringly.
//!
//! # Requirements
//! The only requirements to use these tests are the following:
//! - you have a valid ssh key for cse located at `~/.ssh/cs6991/cs6991-id`
//! - `pars_libs` is a dependency, so that must be included in you `cargo.toml`. If it is not, add the following
//! to you cargo.toml, under dependencies: `pars_libs = "0.1.3".
//!
//! # Instructions
//! Update the host constant to include your zid as the prefix.
//!
//! Place this testing file in the `/src/` folder of you binary crate. Then
//! run `cargo test --bin pars` (or `<Crate name>` if you crate is not called pars).
//!
//! After you have run the tests using the above command, you should be able to
//! run repeat tests with just `cargo test`.
#![allow(warnings, unused)]

// ----- CSE Params -----
const BINARY_PATH: &str = "~/pars/target/debug/pars";
const CARGO_CMD: &str = "6991";
const REMOTE_PORT: u16 = 1234;

const KEY_PATH: &str = "~/.ssh/cs6991/cs6991-id";
const HOST: &str = "localhost";

use std::{
    error,
    fmt::{format, Debug},
    io::{BufRead, BufReader, Read, Stderr, Stdin, Stdout, Write},
    num::ParseIntError,
    os::unix::process,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    str::FromStr,
    sync::mpsc::{channel, Sender},
    thread::{self, JoinHandle},
};

use bstr::io::BufReadExt;
use pars_libs::Remote;

// use pars_lib::{Distribution, TerminationType};
#[derive(Debug, PartialEq)]
pub enum Distribution {
    // Local number of threads
    Local(u32),
    // Vector tuples (Remote, num_threads)
    Remote(Vec<RemoteHost>),
}
impl Distribution {
    /// Gets total number of threads that work is distributed across
    pub fn num_threads(&self) -> u32 {
        match self {
            Distribution::Local(n) => *n,
            Distribution::Remote(remote) => remote.len() as u32,
        }
    }
}
#[derive(PartialEq)]
pub enum TerminationType {
    Never,
    Lazy,
    Eager,
}
impl FromStr for TerminationType {
    type Err = std::fmt::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use TerminationType::*;
        match s {
            "never" => Ok(Never),
            "lazy" => Ok(Lazy),
            "eager" => Ok(Eager),
            _ => Err(std::fmt::Error),
        }
    }
}
impl Debug for TerminationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Never => write!(f, "never"),
            Self::Lazy => write!(f, "lazy"),
            Self::Eager => write!(f, "eager"),
        }
    }
}

struct ParsProgram {
    child: Child,
}
impl ParsProgram {
    pub fn new_local(distribution: Distribution, term_type: Option<TerminationType>) -> Self {
        let mut cmd = Command::new(CARGO_CMD);
        cmd.args(["cargo", "run", "--"]);
        let dist_args = match distribution {
            Distribution::Local(num) => vec!["-J".to_string(), format!("{num}")],
            Distribution::Remote(remotes) => {
                let mut args = Vec::new();
                for x in remotes {
                    args.push("--remote".to_string());
                    let fmt_host = format!("{}:{}/{}", x.remote.addr, x.remote.port, x.threads);
                    args.push(fmt_host);
                }

                args
            }
        };
        cmd.args(dist_args);

        if let Some(term) = term_type {
            cmd.args(["--halt".to_string(), format!("{:?}", term)]);
        }

        // Set up stdin and stdout as separate streams
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());

        let mut _child = cmd.spawn().unwrap();
        Self {
            child: cmd.spawn().unwrap(),
        }
    }

    pub fn run_cmds(&mut self, commands: Vec<&str>) {
        // Get stdin of the child process

        for command in commands {
            let stdin = self.child.stdin.as_mut().unwrap();
            let buf = format!("{}\n", command); // Add a newline to simulate Enter key
            stdin.write_all(buf.as_bytes()).unwrap();
            stdin.flush().unwrap();
        }
    }

    pub fn get_stdout(&mut self) -> Vec<String> {
        // Wait for the child process to complete
        let status = self.child.wait().expect("Failed to wait for child process");

        let stdout = self.child.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        let mut output = Vec::new();

        for line in reader.lines() {
            if let Ok(line) = line {
                output.push(line);
            }
        }

        output
    }

    pub fn kill(mut self) {
        self.child.kill();
    }
}

#[derive(Debug, PartialEq)]
pub struct RemoteHost {
    pub remote: Remote,
    pub threads: u32,
}
impl RemoteHost {
    pub fn new(host: String, port: u16, threads: u32) -> Self {
        Self {
            remote: Remote { addr: host, port },
            threads,
        }
    }
}
impl FromStr for RemoteHost {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.split([':', '/']).into_iter();

        // get hostname
        // reallocate hostname, noting that host_string is freed at the end of this funcion
        let hostname = split.next().expect("Missing hostname").to_string();

        // get port and num threads.
        let port = split.next().expect("Missing port").parse()?;

        let threads = split.next().expect("Missing number of threads").parse()?;

        // get hostname
        Ok(Self {
            remote: Remote {
                addr: s.to_string(),
                port,
            },
            threads,
        })
    }
}
impl ToString for RemoteHost {
    fn to_string(&self) -> String {
        format!("{}:{}/{}", self.remote.addr, self.remote.port, self.threads)
    }
}

mod test_1_1 {

    use super::*;

    #[test]
    fn test_simple_output() {
        let mut program = ParsProgram::new_local(Distribution::Local(1), None);
        program.run_cmds(vec!["echo \"hello world\"", "echo foo", "echo bar", "\r"]);
        let expected = vec!["hello world", "foo", "bar"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_simple_no_output() {
        let mut program = ParsProgram::new_local(Distribution::Local(1), None);
        program.run_cmds(vec!["true"]);

        let expected: Vec<&str> = vec![];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_some_output() {
        let mut program = ParsProgram::new_local(Distribution::Local(1), None);
        program.run_cmds(vec![
            "true", "echo 1", "true", "echo 2", "true", "echo 3", "true", "echo 4", "true",
            "echo 5", "true", "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_with_error_line() {
        let mut program = ParsProgram::new_local(Distribution::Local(1), None);
        program.run_cmds(vec![
            "echo \"hello\"; echo \"world\"",
            "echo \"you can see this\"; /bin/false; echo \"can't see this\"",
            "echo \"cheeky; echo semicolon\"",
            "\r",
        ]);

        let expected = vec![
            "hello",
            "world",
            "you can see this",
            "cheeky; echo semicolon", // stringify producing incorrect output
        ];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_multiple_commands() {
        let mut program = ParsProgram::new_local(Distribution::Local(1), None);
        program.run_cmds(vec![
            "echo 1; echo 2",
            "echo 3; true; echo 4;",
            "echo 5; true",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_empty_file() {
        let mut program = ParsProgram::new_local(Distribution::Local(1), None);
        program.run_cmds(vec![stringify!(cat << EOF)]);
        let expected: Vec<&str> = vec![];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    /// commands should only run one a time and in serial
    fn test_commands_ordering() {
        let mut program = ParsProgram::new_local(Distribution::Local(1), None);
        program.run_cmds(vec![
            "sleep 1; echo 1",
            "echo 2",
            "echo 3",
            "echo 4",
            "echo 5",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    /// test never termination control (default)
    fn test_never_termination() {
        let mut program = ParsProgram::new_local(Distribution::Local(1), None);
        program.run_cmds(vec![
            "echo 1; false; echo 1",
            "echo 2; echo 3; echo 4; echo 5",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }
}

mod test_1_2 {
    use std::time::{Duration, Instant, SystemTime};

    use crate::test::ParsProgram;

    use super::*;
    #[test]
    fn test_simple_2_threads() {
        let mut program = ParsProgram::new_local(Distribution::Local(2), None);
        program.run_cmds(vec!["echo hello", "sleep 1", "echo world", "\r"]);

        let expected = vec!["hello", "world"];
        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_completion_order_2_threads() {
        let mut program = ParsProgram::new_local(Distribution::Local(2), None);
        program.run_cmds(vec!["sleep 1; echo hello", "echo world", "\r"]);

        let output = program.get_stdout();
        let expected = vec!["world", "hello"];

        assert_eq!(output, expected);
    }

    #[test]
    fn test_multiple_commands_at_once() {
        let mut program = ParsProgram::new_local(Distribution::Local(2), None);
        program.run_cmds(vec![
            "echo 1;",
            "sleep 1; echo 5",
            "echo 2;",
            "echo 3;",
            "echo 4;",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    /// output of a line should be buffered
    fn test_output_line_buffering() {
        let mut program = ParsProgram::new_local(Distribution::Local(2), None);
        program.run_cmds(vec![
            "echo 3; sleep 2; echo 4; echo 5",
            "echo 1; echo 2",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    /// never mode: existing lines should finish, new lines should run
    fn test_never_termination() {
        let mut program = ParsProgram::new_local(Distribution::Local(2), None);
        program.run_cmds(vec![
            "sleep 1; echo 4; echo 5",
            "echo 1; false; echo 1",
            "echo 2; echo 3",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    /// job limit / thread number should be respected
    fn test_number_of_threads() {
        let mut program = ParsProgram::new_local(Distribution::Local(4), None);
        program.run_cmds(vec![
            "sleep 1; echo 1",
            "sleep 1.5; echo 3",
            "sleep 2; echo 4",
            "sleep 2.5; echo 5",
            "echo 2;",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }
}

#[cfg(test)]
mod test_1_3 {
    use super::*;

    #[test]
    /// lazy mode: existing lines should finish, new lines should not start
    fn test_lazy_mode_1() {
        let mut program =
            ParsProgram::new_local(Distribution::Local(2), Some(TerminationType::Lazy));
        program.run_cmds(vec![
            "sleep 1; echo 4; echo 5",
            "echo 1; echo 2; echo 3",
            "false",
            "echo 1",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    /// lazy mode: the current line should finish
    fn test_lazy_mode_2() {
        let mut program =
            ParsProgram::new_local(Distribution::Local(2), Some(TerminationType::Lazy));
        program.run_cmds(vec![
            "sleep 1; echo 4; echo 5",
            "echo 1; echo 2; echo 3; false; echo 6; echo 7",
            "echo 1",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }
    #[test]
    /// eager mode: existing commands can finish, no new commands on any line
    /// all lines will halt as soon as the current command finishes
    fn test_eager_mode() {
        let mut program =
            ParsProgram::new_local(Distribution::Local(2), Some(TerminationType::Eager));
        program.run_cmds(vec![
            "echo 5; sleep 1; echo 2",
            "echo 1; echo 2; echo 3; echo 4; false; echo 5",
            "echo 5",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }
    #[test]
    /// never mode: existing lines should finish, new lines should run
    /// given with explicit command line argument
    fn never_mode() {
        let mut program =
            ParsProgram::new_local(Distribution::Local(2), Some(TerminationType::Never));
        program.run_cmds(vec![
            "sleep 1; echo 4; echo 5",
            "echo 1; false; echo 1",
            "echo 2; echo 3",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }
}

#[cfg(test)]
/// Test 1 remote using 1 thread
mod test_2_1 {
    use super::*;
    const PORT: u16 = 1425;
    #[test]
    fn test_simple_connection_one_command() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![RemoteHost::new(
                String::from("localhost"),
                REMOTE_PORT,
                1,
            )]),
            None,
        );

        program.run_cmds(vec!["echo 1; echo 2; echo 3", "\r"]);

        let expected = vec!["1", "2", "3"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_simple_connection_serveral_lines() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![RemoteHost::new(
                String::from("localhost"),
                REMOTE_PORT,
                1,
            )]),
            None,
        );

        program.run_cmds(vec!["echo 1; echo 2; echo 3", "echo 4; echo 5", "\r"]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_halt_never_implicit() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![RemoteHost::new(
                String::from("localhost"),
                REMOTE_PORT,
                1,
            )]),
            None,
        );

        program.run_cmds(vec![
            "echo 1; echo 2; echo 3; false; echo foo",
            "false; echo bar",
            "echo 4; echo 5",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_halt_never_explicit() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![RemoteHost::new(
                String::from("localhost"),
                REMOTE_PORT,
                1,
            )]),
            None,
        );

        program.run_cmds(vec![
            "echo 1; echo 2; echo 3; false; echo foo",
            "false; echo bar",
            "echo 4; echo 5",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_halt_lazy() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![RemoteHost::new(
                String::from("localhost"),
                REMOTE_PORT,
                1,
            )]),
            Some(TerminationType::Lazy),
        );

        program.run_cmds(vec![
            "echo 1; echo 2; echo 3; false; echo foo",
            "false; echo bar",
            "echo 4; echo 5",
            "\r",
        ]);

        let expected = vec!["1", "2", "3"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    /// As this is singled threaded, this should behave the same as halt lazy
    fn test_halt_eager() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![RemoteHost::new(
                String::from("localhost"),
                REMOTE_PORT,
                1,
            )]),
            Some(TerminationType::Eager),
        );

        program.run_cmds(vec![
            "echo 1; echo 2; echo 3; false; echo foo",
            "false; echo bar",
            "\r",
        ]);

        let expected = vec!["1", "2", "3"];

        assert_eq!(program.get_stdout(), expected);
    }
}

#[cfg(test)]
mod test_2_2 {
    use super::*;

    #[test]
    fn test_simple_2_threads() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![RemoteHost::new(
                String::from("localhost"),
                REMOTE_PORT,
                2,
            )]),
            None,
        );

        program.run_cmds(vec![
            "echo 4; echo 5; sleep 1",
            "echo 1; echo 2; echo 3",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_simple_3_threads() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![RemoteHost::new(
                String::from("localhost"),
                REMOTE_PORT,
                3,
            )]),
            None,
        );

        program.run_cmds(vec![
            "echo 5; sleep 2",
            "echo 4; sleep 1",
            "echo 1; echo 2; echo 3",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_multiple_threads_never() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![RemoteHost::new(
                String::from("localhost"),
                REMOTE_PORT,
                3,
            )]),
            Some(TerminationType::Never),
        );

        program.run_cmds(vec![
            "echo 4; sleep 1; false; echo foo",
            "false",
            "echo 1; echo 2; echo 3;",
            "echo 5; sleep 1",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_multiple_threads_lazy() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![RemoteHost::new(
                String::from("localhost"),
                REMOTE_PORT,
                2,
            )]),
            Some(TerminationType::Lazy),
        );

        program.run_cmds(vec![
            "echo 4; echo 5; sleep 1",
            "echo 1; echo 2; echo 3; false",
            "echo foobar",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }

    #[test]
    fn test_multiple_threads_eager() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![RemoteHost::new(
                String::from("localhost"),
                REMOTE_PORT,
                2,
            )]),
            Some(TerminationType::Eager),
        );

        program.run_cmds(vec![
            "echo 4; echo 5; sleep 1; echo hidden",
            "echo 1; echo 2; echo 3; false",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
    }
}

/// Test using multiple remotes (including testing )
///
/// # Note
/// Tests are marked to be run serially as each process is attempting to connect
/// to the same remote client.
///
/// As rust wants to run tests in parrellel, without this different tests would all
/// be trying to talk to the same clients.
#[cfg(test)]
mod test_2_3 {
    use serial_test::serial;

    use super::*;

    #[test]
    #[serial]
    fn test_simple_2_remotes() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![
                RemoteHost::new(String::from("localhost"), REMOTE_PORT, 1),
                RemoteHost::new(String::from("localhost"), REMOTE_PORT, 1),
            ]),
            None,
        );

        program.run_cmds(vec![
            "echo 4; echo 5; sleep 2",
            "echo 1; echo 2; echo 3",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
        program.kill();
    }

    #[test]
    #[serial]
    fn test_remotes_with_errors() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![
                RemoteHost::new(String::from("localhost"), REMOTE_PORT, 1),
                RemoteHost::new(String::from("localhost"), REMOTE_PORT, 1),
            ]),
            None,
        );

        program.run_cmds(vec![
            "echo 4; sleep 1.5",
            "echo 1; echo 2; echo 3; false; echo foo",
            "echo 5; sleep 3",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
        program.kill();
    }

    #[test]
    #[serial]
    fn test_multiple_threads_never() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![
                RemoteHost::new(String::from("localhost"), REMOTE_PORT, 1),
                RemoteHost::new(String::from("localhost"), REMOTE_PORT, 1),
            ]),
            Some(TerminationType::Never),
        );

        program.run_cmds(vec![
            "echo 4; sleep 1; false; echo foo",
            "false",
            "sleep 0.5; echo 1; echo 2; echo 3;",
            "echo 5; sleep 3",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
        program.kill();
    }

    #[test]
    #[serial]
    fn test_multiple_threads_lazy() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![
                RemoteHost::new(String::from("localhost"), REMOTE_PORT, 1),
                RemoteHost::new(String::from("localhost"), REMOTE_PORT, 1),
            ]),
            Some(TerminationType::Lazy),
        );

        program.run_cmds(vec![
            "echo 4; echo 5; sleep 2",
            "sleep 1; echo 1; echo 2; echo 3; false",
            "echo foobar",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
        program.kill();
    }

    #[test]
    #[serial]
    fn test_multiple_threads_eager() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![
                RemoteHost::new(String::from("localhost"), REMOTE_PORT, 1),
                RemoteHost::new(String::from("localhost"), REMOTE_PORT, 1),
            ]),
            Some(TerminationType::Eager),
        );

        program.run_cmds(vec![
            "echo 4; echo 5; sleep 2; echo hidden",
            "sleep 1; echo 1; echo 2; echo 3; false",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
        program.kill();
    }

    #[test]
    #[serial]
    fn test_load_balance_1() {
        let mut program = ParsProgram::new_local(
            Distribution::Remote(vec![
                RemoteHost::new(String::from("localhost"), REMOTE_PORT, 1),
                RemoteHost::new(String::from("localhost"), REMOTE_PORT, 4),
            ]),
            Some(TerminationType::Eager),
        );

        program.run_cmds(vec![
            "echo 2; sleep 1",
            "echo 3; sleep 2",
            "echo 4; sleep 3",
            "echo 5; sleep 4",
            "echo 1",
            "\r",
        ]);

        let expected = vec!["1", "2", "3", "4", "5"];

        assert_eq!(program.get_stdout(), expected);
        program.kill()
    }
    #[test]
    #[serial]
    fn test_load_balance_2() {}
    #[test]
    #[serial]
    fn test_load_balance_3() {}
}
