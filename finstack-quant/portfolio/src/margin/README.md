# Portfolio margin aggregation

Portfolio-level netting-set organization and margin aggregation for
`finstack-quant-portfolio`. It builds on the `Marginable` trait and the
SIMM/schedule/CCP calculators in `finstack-quant-margin`, reaching individual
holdings through `finstack-quant-valuations`, whose instruments implement
`Marginable` and surface it via `Instrument::as_marginable()`. It does not
implement a margin model of its own.

The public surface is `finstack_quant_portfolio::margin`: one function,
`portfolio_margin`, and its two result types. Netting-set grouping is built
per call and stays internal.

## What it does

- Groups positions into netting sets from instrument-provided margin metadata
  (`Marginable::netting_set_id()` and `Marginable::margin_spec()`).
- Extracts per-position SIMM sensitivities, scales them to the held position,
  FX-collapses them into the portfolio base currency, then nets them within
  each netting set before computing initial margin.
- Computes variation margin from netting-set mark-to-market values.
- Aggregates per-netting-set results into a portfolio-wide report in the
  portfolio base currency.

## API

| Item | Role |
|------|------|
| `portfolio_margin(portfolio, market, as_of, current_im_collateral)` | Single entry point: groups netting sets and computes the report |
| `NettingSetMargin` | One netting set's IM, VM, total, methodology, sensitivities, and IM breakdown |
| `PortfolioMarginResult` | Portfolio summary: totals, `by_netting_set`, counts, `degraded_positions` |

Identifiers (`NettingSetId`), specs (`OtcMarginSpec`), methodologies
(`ImMethodology`: `Haircut`, `Simm`, `Schedule`, …) and `SimmSensitivities`
come from `finstack-quant-margin`.

## Conventions

- **Clearing estimate.** The clearing path sums nonnegative standalone proxy IM
  using absolute position scales, with no assumed offsets. Each such netting-set
  result carries `is_approximate = true`; it is not a CCP portfolio-margin replication.
- **Agreement consistency.** `portfolio_margin` returns an error for conflicting
  CSA, clearing, methodology, frequency, or settlement terms within one netting
  set. Trade-specific SIMM credit classifications may differ.

- **Per-netting-set IM.** Initial margin is computed per netting set, never on a
  gross portfolio basis, then summed into the base currency.
- **Signed, scaled sensitivities.** `Marginable::simm_sensitivities` returns
  per-unit values (the same contract as `mtm_for_vm`). Portfolio margin scales
  them by the signed `Position::scale_factor()` before merging, so a short
  position offsets an equal long — which is what ISDA SIMM netting requires.
- **Explicit FX, before netting.** Sensitivities produced in a currency other
  than the portfolio base currency are converted with an explicit spot factor
  before the netting-set merge. Rebasing a set that carries non-empty
  `fx_delta` is refused (`MO-17`) because there is no calculation-currency
  remap policy. VM mark-to-market is FX-converted per position.
- **Registration is opt-in.** Netting-set grouping ignores any instrument whose
  `as_marginable()` is `None` or whose `netting_set_id()` is `None`.
- **Failures are recorded, not dropped.** A position whose sensitivities or VM
  mark-to-market cannot be computed lands in
  `PortfolioMarginResult::degraded_positions`.
  `positions_without_margin` counts the distinct union of unregistered and
  degraded positions. A position present in both groups is counted once.
- **MPOR.** Schedule and CCP-proxy IM are rescaled from the model's margin
  period of risk to the CSA's (`amount * sqrt(target / source)`) when the CSA
  carries IM terms; a zero source MPOR is rejected.
- **Determinism.** Sensitivity extraction fans out over positions with Rayon
  and collects positionally, so the downstream merge order (portfolio order)
  matches the serial path. `NettingSetMargin` and `PortfolioMarginResult` serialize through
  internal `*Wire` types so `HashMap`-backed fields emit in a stable order.
- **Shared sensitivity JSON.** Nested `sensitivities` use the margin crate's
  `SimmSensitivitiesJson` contract: sorted tuple arrays, such as
  `"ir_delta": [["USD", "5Y", 12500.0]]` and
  `"credit_qualifying_delta": [["financial", "BANK_A", "5Y", 725.0]]`.
  This is also the shape returned by `SimmSensitivities::to_json()`. Credit
  qualifying tuples require a sector before the reference name and tenor.

## Example

```rust,no_run
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_portfolio::{Portfolio, margin::portfolio_margin};
use time::macros::date;

fn report(
    portfolio: &Portfolio,
    market: &MarketContext,
) -> finstack_quant_portfolio::Result<()> {
    // No IM collateral posted yet: every CSA balance is zero.
    let collateral = finstack_quant_core::FxHashMap::default();
    let result = portfolio_margin(portfolio, market, date!(2025 - 01 - 15), &collateral)?;

    println!("Total IM: {}", result.total_initial_margin);
    println!("Total VM: {}", result.total_variation_margin);
    println!("Netting sets: {}", result.by_netting_set.len());

    for (position_id, reason) in &result.degraded_positions {
        eprintln!("degraded: {position_id}: {reason}");
    }
    Ok(())
}
```

## Scope and limits

- This layer reports margin *requirements*. The caller supplies the current
  one-way IM collateral balance per CSA (`current_im_collateral`), from which
  `by_csa` derives the required collateral and the transfer; it does not track
  collateral inventory (eligible assets, haircuts on posted securities) or the
  opposite party's segregated account.
- Results are only as good as the instrument implementations of
  `as_marginable`, `simm_sensitivities`, and `mtm_for_vm`.
- Clearing-house treatment is expressed through the available `ImMethodology`
  variants; venue-specific CCP integrations are out of scope for this crate.

## Tests

```bash
cargo nextest run -p finstack-quant-portfolio --test portfolio margin_aggregation
cargo nextest run -p finstack-quant-portfolio --test portfolio margin_serialization
```

## References

- ISDA SIMM: [`docs/REFERENCES.md#isda-simm`](../../../../docs/REFERENCES.md)
- Crate overview: [`../../README.md`](../../README.md)
