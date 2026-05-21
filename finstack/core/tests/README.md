# finstack-core integration tests

Integration tests for the `finstack-core` crate, organized by domain module.

## Layout

Each domain has a root file (e.g. `cashflow.rs`, `dates.rs`) that documents the
suite and includes submodules via `#[path = "..."]`:

```
tests/
├── common/mod.rs          # Shared helpers (dates, approx_eq)
├── cashflow.rs + cashflow/
├── dates.rs + dates/
├── expr.rs + expr/
├── infrastructure.rs + infrastructure/
├── market_data.rs + market_data/
├── math.rs + math/
├── money.rs + money/
├── serde.rs + serde/
├── types.rs + types/
├── golden/                # Reference-value fixtures (see golden/README.md)
├── golden_tests.rs        # Golden suite entry point
├── canonical_api.rs       # Public API shape checks
└── simplicity_parity.rs   # Cross-crate parity guards
```

Unit tests live in `#[cfg(test)]` blocks inside source files. This directory
tests public API behavior and cross-module interactions.

## Test helpers

**Global** (`common/mod.rs`):

- `test_date()` — 2025-01-15
- `sample_base_date()` — 2024-01-01
- `make_date(year, month, day)`
- `approx_eq(a, b, tol)`

**Module-specific** (`<module>/test_helpers.rs` or `common.rs`):

- Domain tolerance constants
- Fixtures (curves, surfaces, etc.)

## Tolerance conventions

| Constant | Value | Use case |
|----------|-------|----------|
| `RATE_TOLERANCE` | 1e-10 | IRR, discount factors, unitless rates |
| `FACTOR_TOLERANCE` | 1e-12 | Year fractions, day-count |
| `XIRR_TOLERANCE` | 1e-6 | XIRR (Excel-compatible precision) |
| `MATH_TOLERANCE` | 1e-12 | General math |
| `SERDE_TOLERANCE` | 1e-12 | Serialization roundtrips |
| `CONTINUITY_TOLERANCE` | 1e-4 | Forward-rate continuity at knots |
| `financial_tolerance(n)` | max(n × 1e-8, 0.01) | Money amounts |

## Running tests

```bash
# All core tests
cargo test -p finstack-core

# One integration target
cargo test -p finstack-core --test cashflow

# Single test by name
cargo test -p finstack-core --test cashflow npv_100_cashflows

# With output
cargo test -p finstack-core -- --nocapture
```

Or via mise:

```bash
mise run rust-test
```

## Adding tests

1. Add a file under the relevant subdirectory
2. Wire it from the domain root with `#[path = "..."]`
3. Document the file's scope in a module doc comment
4. Use domain helpers and the tolerance table above for float comparisons

Test names should describe the scenario and expected outcome, e.g.
`npv_negative_rate_inflates_value`, `calendar_usny_excludes_thanksgiving`.

Each test should set up its own fixtures and not depend on execution order.
