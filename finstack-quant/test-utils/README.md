# finstack-quant-test-utils

Shared test utilities for the `finstack-quant` workspace.

This crate keeps golden-test loading and comparison helpers out of
`finstack-quant-core`'s production library surface while preserving a common
framework for workspace test suites.

## Usage

Add `finstack-quant-test-utils` as a `dev-dependency` in your crate's
`Cargo.toml`:

```toml
[dev-dependencies]
finstack-quant-test-utils = { path = "../test-utils" }
```

Then use the golden module in tests:

```rust
use finstack_quant_test_utils::golden;

let fixture = golden::load_fixture("my_test_case")?;
```

## Modules

| Module | Purpose |
|--------|---------|
| `golden` | Golden-test fixture loading and comparison helpers |

## License

Dual-licensed under MIT or Apache-2.0.
