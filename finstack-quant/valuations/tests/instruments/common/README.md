# Common Module Tests

Unit tests for `instruments/common`: shared pricing models, metrics helpers, traits, and conventions.

## Layout

```
common/
├── mod.rs
├── test_helpers.rs
├── test_traits.rs
├── helpers/
├── models/
├── metrics/
├── parameters/
├── test_discountable.rs
└── test_pricing.rs
```

## Scope

- **Models**: binomial/trinomial trees, Black–Scholes/Black-76, SABR, short-rate and multi-factor trees.
- **Metrics**: theta utilities and related helpers.
- **Traits**: `CashflowProvider`, `Priceable`, and `Discountable` contracts via mock instruments.
- **Fixtures**: shared curves, tolerances, and reference formulas in `test_helpers.rs`.

## Running Tests

```bash
cargo test --lib common
cargo test --lib test_binomial_tree
cargo test --lib common -- --nocapture
```

Tests use tolerance-based floating-point comparisons and standard parity/bounds checks where applicable.
