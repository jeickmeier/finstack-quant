# VALUATIONS-EQUITY-AND-COMPOSITION-04: Equity total-return futures projection

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-04`.
- **Decision status:** Accepted 2026-11-14 — complete total-return futures quote, spread clock, clearing and risk package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Source and primary venue rules inspected. Independent spread-clock, quote, risk, final cash and published example arithmetic checked. Venue, runtime and host qualification pending.

## Accepted decision for checked equity total-return futures

1. **Bind the exact cash-settled contract and live quote authority.** Require
   the venue and effective rule version, exact contract and underlying series,
   point denomination, currency, side, positive contract quantity and cash
   per point. Separate last trading, underlying fixing, publication, clearing
   booking and payment. Elect a qualified observed futures-points mark or
   the spread-to-points conversion. Require the reference of that mode and
   venue. Eurex Trade at Close uses the prescribed close and Trade at Market
   uses the agreed custom reference. Treat a fixed execution reference as
   history. Missing or mismatched required data makes the selected result
   unavailable. Apply venue tick and cash rounding at their authorized stages.
   [Eurex contract specifications, §§1.22 and 1.26](https://www.eurex.com/resource/blob/1637254/c61cfea99bd9081a4295f5314b4d70b5/data/2026_11_02_eurex_d_kontraktspezifikationen%20_en.pdf),
   [accepted equity futures lifecycle](03-equity-future.md#accepted-decision-for-checked-equity-futures-carry-quanto-and-clearing).

2. **Convert spread to points on a matched component basis.** Use
   `P=I+D−A+I s τ`, where `I` is the positive underlying level, `D`
   accrued distribution points, `A` accrued funding points, and
   `s=spread_bp/10000` the signed annual decimal spread. Require authorized
   accumulator snapshots on the same contract, series, origin, point and
   publication basis. Preserve signed funding and fractional or negative
   spreads. A price index uses qualified distribution points. For a contract
   on a total-return index with distributions already incorporated, the
   selected rule gives zero separate `D`. Use supplied accumulators and an
   explicit publication phase, including any venue-authorized snapshot
   choice. Funding-history reconstruction and implicit stale lookup
   fallbacks remain outside this calculation. Dividend event inputs require
   owner qualification before populating an accumulator. Check point and
   output arithmetic in the selected domain.
   [Eurex clearing §2.22.2](https://www.eurex.com/resource/blob/2923634/27024f33382c118fc4cf457466718626/data/2026_11_02_ecag_clearing_conditions_chapter_02_en.pdf#page=66),
   [CME AIR comparison](https://www.cmegroup.com/trading/equity-index/files/air-total-return-futures-explained-t-1.pdf),
   [identified dividend events](../core-market/dividend-events.md).

3. **Use the contractual spread clock.** Bind the prescribed settlement-
   adjusted endpoints, calendar, offsets and annualization for `τ`. For
   the cited Eurex index convention, count actual days between the trading
   day and expiry after adjusting each by the applicable settlement-day
   offset, then divide by the specified annualization. Keep these endpoints
   separate from the official fixing and payment. Reject invalid chronology
   or data coverage. A qualified zero fraction gives exactly zero spread
   adjustment and uses no inactive spread input. Zero spread time alone
   establishes no lifecycle event.
   [Eurex spread clock §1.22.6.5](https://www.eurex.com/resource/blob/1637254/c61cfea99bd9081a4295f5314b4d70b5/data/2026_11_02_eurex_d_kontraktspezifikationen%20_en.pdf#page=87).

4. **Share the mark, reference and actual clearing lifecycle.** Report
   `futures_price` on the selected mark. The live interval references the
   fill for a new position or the last applied official clearing mark for a
   carried one. Compute signed undiscounted live variation from the point
   difference and contracted size. Track applied, pending, settled and
   cumulative amounts distinctly. After cutoff require the eligible official
   final mark or confirmed clearing cash. Where the venue sets final spread
   contribution to zero, `final reference + final distributions − final
   funding` is a reconciliation check on qualified final inputs. Value
   included same-day unpaid cash at factor one and future fixed cash on its
   actual dated settlement basis. Require explicit overdue treatment and
   proved closure before returning an empty residual.
   [Accepted listed-product lifecycle](../valuations-framework/listed-products.md),
   [Eurex final settlement §2.22.3](https://www.eurex.com/resource/blob/2923634/27024f33382c118fc4cf457466718626/data/2026_11_02_ecag_clearing_conditions_chapter_02_en.pdf#page=68).

5. **Publish sensitivities on the actual moving basis.** With
   `M=side × contracts × cash_per_point`, live futures-mark point delta
   is `M`. On the selected component route, hold the other inputs fixed
   and use underlying delta `M(1+sτ)` per underlying point and
   `spread01=M I τ/10000` per positive bp of annualized spread. Apply each
   only on its qualified movable source. A frozen observed points mark
   has structural zero model spot and spread dependence. Confirmed fixed
   cash has no live quote risk and may retain discount risk while unpaid.
   Retain the actual units, contract reference and source roles. Unresolved
   active data or calculation failures propagate.

6. **Export confirmed variation and preserve its provenance.** Accrued
   distribution and funding points enter the mark once. Report the
   confirmed unpaid variation as dated cash and use `NoResidual` only
   for a proved empty cash scope. A horizon roll crossing fixing or clearing
   requires qualified actual or hypothetical accumulator and cash state.
   Reconcile Rust pricing and metrics, portfolio, existing Python and WASM
   routes, and generated schemas with the same prepared state.
   [Cashflow export](../valuations-framework/cashflow-export.md),
   [horizon valuation](../valuations-framework/carry-theta-breakeven.md).

## Observed behavior

### Choice and rationale

The projected total-return-future quote combines observed index points, accumulated distributions/funding and a simple remaining-term spread adjustment. Accrued components are caller-supplied market scalars, not recomputed financing histories.

### Scope and input domain

Listed total-return-future terms with finite point scalars, positive spot, a unitless spread quoted in basis points and an explicit spread day count. The result is in the terms' settlement currency and quote points.

### Algorithm

Compute `τ=max(year_fraction(as_of,settlement),0)` and `s=spread_bp×10^-4`. The model quote is `S + accrued_distributions − accrued_funding + S s τ`. The model spot derivative is `1+sτ`; spread derivative per basis point is `Sτ×10^-4`. Multiply derivatives by signed listed-contract point delta. Resolved mark/PV delegate to shared listed-future terms, so quoted marks and settlement observations may replace the projection.

### Selection, controls and conventions

The three metrics are resolved FuturesPrice, model Delta and model Spread01. There is no discount curve dependency. Point scalars accept monetary values only in settlement currency or unitless amounts; spread specifically requires unitless data. No residual cashflow schedule is exported.

### Limitations, failures and alternatives

No term-structure integration or funding/day-by-day index reconstruction occurs. A fixed mark can suppress repricing risk while analytic model derivatives remain nonzero. An explicit term-integrated spread or exchange accrual engine would be a different unimplemented method.

### Evidence

Inline tests in [types.rs](../../../../finstack-quant/valuations/equity/src/equity_total_return_future/types.rs) locate projection and metric identity cases. [Hull futures reference](../../../REFERENCES.md#hull-options-futures) supplies general futures context, not independent verification of a specific exchange total-return-index formula.

### Relationship to the accepted distribution source boundary

[CORE-MARKET-13's complete accepted package](../core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates) distinguishes actual
cash-event identity/ex/pay dates from accumulated distribution model inputs.
Any owner deriving this product's accrued-distributions scalar from events must
qualify share/point units, currency, entitlement, payment phase and provenance;
a source schedule is not itself that accumulated scalar or evidence of payment.
CORE-MARKET-13's source package was accepted on **2026-11-08**, with
reconciliation and qualification pending.

## Independent arithmetic and qualification

- For a stipulated weekday settlement calendar and two-day offset,
  trading on 2026-12-14 and expiry on 2026-12-18 give spread endpoints
  2026-12-16 and 2026-12-22. At `τ=6/360`, level 5,000, distributions 12,
  funding 9 and spread `−37.5` bp, the mark is `5002.6875` points. Three
  short contracts at 10 cash per point give spot delta `−29.998125` and
  spread sensitivity `−.25` per bp. A final mark of 5,010 versus the last
  applied 5,004 gives `−180` pending and `−179.64` PV at factor `.998`.
- An independent calculation on CME's separate income-inclusive profile
  reproduces its published `10,578.73` points using index `11,402.10`,
  funding `866.45`, `+67` bp and `203/360`. [CME comparison, Table 3](https://www.cmegroup.com/trading/equity-index/files/air-total-return-futures-explained-t-1.pdf#page=10).

Qualify effective venue conventions, time and publication boundaries,
accumulator authority, confirmed cash and host/portfolio integration.
Production runtime and host suites remain unrun.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/equity_total_return_future/types.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/equity_total_return_future/metrics.rs)
