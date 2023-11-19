# COMP6991-pars-autotests
By James Appleton (z5310803)

## Installing
1. Copy this test.rs into your `/src/` directory (next to your binary's main.rs)
2. Add `mod test.rs` to the you main.rs to include the tests
3. Check that the port, key-path, and other constants in the tests are appropriate.
4. `cargo test`

## Filtering tests
You can also filter tests using cargo test.

To run only the fist task's test you could run `cargo test test::test_1_1`, for example. Or 
run `cargo test::test_1` to run all the tests for the local case.

It should be noted that all but `test_2_3` will run in parrellel so will be relatively fast, however `test_2_3`
will run serially, which will be slow. It may be a good idea to filter out `test_2_3` if you not 
wanting to test that section.
