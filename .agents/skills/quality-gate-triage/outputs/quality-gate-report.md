# Triage Result

### Root cause
`mise pre-commit-run` failed in `cargo clippy` because a binding method used `unwrap()` in non-test PyO3 code. The binding crate denies `clippy::unwrap_used`, and the failure is a real production-path issue because malformed Python input can reach the branch.

### Fix
Replaced the `unwrap()` with explicit error propagation through the binding error mapper and added a targeted regression test for malformed input.

### Verification
- `mise run python-build`: passed
- `mise run rust-lint`: passed for the affected crate
- Targeted binding test: passed

### Residual risk
`mise run all-ci` was not run because the change was scoped to one binding module. Run it before merging a broad branch.
