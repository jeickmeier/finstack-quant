# Finstack Quant Portfolio

`finstack-quant-portfolio` builds portfolios from entities and positions, values them in a
base currency, aggregates risk metrics and cashflows, applies scenarios, and runs
margin, factor-risk, and optimization workflows on top of `finstack-quant-core` and
`finstack-quant-valuations`.

## Capabilities

- Entity-aware portfolios with optional dummy entities for standalone instruments.
- Position scaling via `PositionUnit` (`Units`, `Notional`, `FaceValue`, `Percentage`).
- Base-currency valuation with per-position drill-down.
- Metric aggregation, attribute/book grouping, cashflow ladders, margin, factor risk,
  liquidity scoring, performance attribution, and tabular exports (`TableEnvelope`).
- Scenario stress via `apply_and_revalue` (uses `finstack-quant-scenarios`).

## Conventions

- `Portfolio::base_ccy` is the reporting currency for totals and portfolio-level analytics.
- `Position::quantity` is interpreted by `PositionUnit`; it is not always a share count.
- Summable risk metrics are FX-converted to base currency before aggregation.
- Selective repricing (`revalue_affected`) uses each instrument's declared market
  dependencies; unresolved dependencies trigger full repricing.

## Quick start

```rust
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::valuation::value_portfolio;
use finstack_quant_portfolio::PortfolioBuilder;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use std::sync::Arc;
use time::macros::date;

# fn main() -> finstack_quant_portfolio::Result<()> {
let as_of = date!(2024-01-01);
let market = MarketContext::new();
let config = FinstackConfig::default();

let deposit = Deposit::builder()
    .id("DEP_1M".into())
    .notional(Money::new(1_000_000.0, Currency::USD))
    .start_date(as_of)
    .maturity(date!(2024-02-01))
    .day_count(finstack_quant_core::dates::DayCount::Act360)
    .discount_curve_id("USD".into())
    .build()
    .expect("example deposit should build");

let position = Position::new(
    "POS_001",
    "ACME_FUND",
    "DEP_1M",
    Arc::new(deposit),
    1.0,
    PositionUnit::Units,
)?
.with_text_attribute("asset_class", "cash");

let portfolio = PortfolioBuilder::new("MY_FUND")
    .base_ccy(Currency::USD)
    .as_of(as_of)
    .entity(Entity::new("ACME_FUND"))
    .position(position)
    .build()?;

let valuation = value_portfolio(&portfolio, &market, &config, &Default::default())?;
println!("Portfolio total: {}", valuation.total_base_ccy);
# Ok(())
# }
```

## Workflows

| Task | Entry point |
|------|-------------|
| Valuation | `value_portfolio` |
| Metric rollup | `aggregate_metrics` |
| Grouping | `aggregate_by_attribute`, `aggregate_by_multiple_attributes`, `aggregate_by_book` |
| Partial repricing | `revalue_affected` |
| Scenario + revalue | `scenarios::apply_and_revalue` |
| Margin | `PortfolioMarginAggregator` |
| Factor risk | `factor_model` module |
| Optimization | `optimization` module |
| Cashflows | `cashflows` module |
| Tables | `positions_to_table`, `metrics_to_table`, … |

## `PositionUnit`

- `Units`: scale by share or contract count.
- `Notional(Option<Currency>)`: scale by notional; use `1.0` when the instrument PV
  already reflects its configured notional.
- `FaceValue`: scale by held face amount.
- `Percentage`: percentage points (`50.0` → 50% → `0.50` internally).

## FX and reporting

- Position values are stored in native and base currency.
- Portfolio totals and summable metrics use base currency.
- Cashflow FX helpers use spot-equivalent rates for all dates; use explicit forward FX
  outside this crate when forward-sensitive reporting is required.
- Attribution separates instrument FX risk from base-currency translation effects.

## Serialization

`Portfolio::to_spec` / `Portfolio::from_spec` produce JSON-friendly specs. Round-trip
reconstruction requires each instrument to implement `to_instrument_json()`. Positions
with `instrument_spec: None` need an external instrument registry on load.

## Parallelism

Valuation and metric collection use Rayon; there is no feature flag to disable it.

## Examples and tests

```bash
cargo run -p finstack-quant-portfolio --example portfolio_optimization
cargo test -p finstack-quant-portfolio
cargo test -p finstack-quant-portfolio --doc
```

## References

Quantitative references: [`docs/REFERENCES.md`](../../docs/REFERENCES.md).
