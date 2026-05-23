---
trigger: always_on
description:
globs:
---
- For rust code changes, always run `mise run rust-lint` after each set of changes and ensure 100% green.
- For wasm code, always run `mise run wasm-lint` after each set of changes.
- For python code changes, always run `mise run python-lint` after each set of changes.
- Run targeted tests focused on each code change and not the full test suites as it takes too long to run.
- Fix any errors that are present from lints and tests before moving on to next task.
- If you change the rust library, you will need to rebuild the python and wasm bindings before using in python/wasm. Use `mise run python-build` for python and `mise run wasm-build` for typescript/js/wasm
- DO NOT OVER-ENGINEER THE SOLUTIONS. AIM FOR SIMPLICITY.
- DO NOT RUN cargo test DIRECTLY, WE DON"T WANT TO RUN RUST DOC TESTS
