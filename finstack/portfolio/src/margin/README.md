# Portfolio Margin Aggregation

Portfolio-level margin aggregation for `finstack-portfolio`, built on the
instrument-level margin interfaces in `finstack-margin` and
`finstack-valuations`.

## What It Does

- Groups positions into netting sets using instrument-provided margin metadata.
- Nets SIMM sensitivities within each netting set before computing initial
  margin.
- Computes variation margin from netting-set mark-to-market values.
- Aggregates per-netting-set results into a portfolio-wide report in the
  portfolio base currency.

## Main Types

- `PortfolioMarginAggregator`: main orchestration entry point.
- `NettingSet` and `NettingSetManager`: grouping and aggregation containers.
- `NettingSetMargin`: result for one netting set.
- `PortfolioMarginResult`: portfolio-wide margin summary.

## Core Conventions

- Initial margin is computed per netting set, not on a gross portfolio basis.
- Variation margin is the net mark-to-market of positions in the set.
- Cross-currency netting-set results must be FX-converted explicitly when they
  are not already in the portfolio base currency.
- Positions whose SIMM sensitivities or margin MTM cannot be computed are
  recorded in `degraded_positions` instead of being silently ignored.

## Minimal Example

```rust,no_run
use finstack_portfolio::PortfolioMarginAggregator;
use finstack_core::market_data::context::MarketContext;
use time::macros::date;

# fn main() -> finstack_portfolio::Result<()> {
# let portfolio: finstack_portfolio::Portfolio = unimplemented!("Provide a portfolio");
# let market: MarketContext = unimplemented!("Provide market data");
let mut aggregator = PortfolioMarginAggregator::from_portfolio(&portfolio);
let result = aggregator.calculate(&portfolio, &market, date!(2025-01-15))?;

println!("Total IM: {}", result.total_initial_margin);
println!("Total VM: {}", result.total_variation_margin);
println!("Netting sets: {}", result.netting_set_count());
# Ok(())
# }
```

## Current Scope

- Netting-set organization and reporting.
- SIMM-style sensitivity aggregation.
- Portfolio-level IM and VM totals.
- Cleared-vs-bilateral splits via `PortfolioMarginResult::cleared_bilateral_split`.

## Known Limits

- This layer reports margin requirements; it does not track posted or received
  collateral inventory.
- Accuracy depends on the underlying instrument implementations of
  `as_marginable`, `simm_sensitivities`, and `mtm_for_vm`.
- Clearing-house methodologies are represented through the available
  `ImMethodology` surface; venue-specific external CCP integrations are outside
  this crate.

## Tests

```bash
cargo test -p finstack-portfolio --test margin_aggregation --test margin_serialization
```

## References

- `docs/REFERENCES.md#isda-simm`
