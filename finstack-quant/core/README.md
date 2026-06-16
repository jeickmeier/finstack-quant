# finstack-quant-core

Foundational crate for the Finstack Quant workspace: currencies, money, rates, dates,
calendars, market data containers, cashflow primitives, math helpers,
configuration, and the expression engine.

## Coverage

- **Types and money**: currencies, monetary amounts, rates, basis points,
  percentages, credit ratings, phantom-typed IDs
- **Dates and calendars**: business-day conventions, holiday calendars, day
  counts, tenors, period identifiers, schedule utilities
- **Market data**: discount, forward, hazard, inflation, and base-correlation
  curves; volatility surfaces; FX matrices; scalars; `MarketContext`
- **Cashflow primitives**: dated cashflow types, NPV, IRR/XIRR
- **Math and numerics**: interpolation, solvers, integration, statistics,
  summation, volatility helpers
- **Expression engine**: AST-based scalar evaluation for time-series formulas
- **Credit and factor model**: migration matrices, PD/LGD helpers, covariance
  and matching utilities
- **Configuration**: rounding policies and shared runtime settings

## Module docs

Deeper module notes live under `src/`:

- [`src/README.md`](src/README.md)
- [`src/dates/README.md`](src/dates/README.md)
- [`src/market_data/README.md`](src/market_data/README.md)
- [`src/math/README.md`](src/math/README.md)
- [`src/cashflow/README.md`](src/cashflow/README.md)
- [`src/expr/README.md`](src/expr/README.md)
- [`src/money/README.md`](src/money/README.md)
- [`src/types/README.md`](src/types/README.md)

## Cargo features

`finstack-quant-core` defines no crate-local Cargo features. Serde wire formats,
tracing hooks, and golden-test helpers compile unconditionally.

## Usage

Depend on the crate directly:

```toml
[dependencies]
finstack-quant-core = { path = "../finstack-quant/core" }
```

Or through the umbrella crate:

```toml
[dependencies]
finstack-quant = { path = "../finstack-quant" }
```

## Related crates

Use adjacent crates for domain-specific workflows:

- `finstack-quant-cashflows` — schedule construction and accrual
- `finstack-quant-valuations` — pricing, metrics, calibration, attribution
- `finstack-quant-statements` — financial statement modeling
- `finstack-quant-analytics` — return-series performance and risk analytics

## Verification

```bash
cargo fmt -p finstack-quant-core
cargo clippy -p finstack-quant-core --all-targets -- -D warnings
cargo test -p finstack-quant-core
RUSTDOCFLAGS="-D warnings" cargo doc -p finstack-quant-core --no-deps
cargo test -p finstack-quant-core --doc
```

## License

MIT OR Apache-2.0
