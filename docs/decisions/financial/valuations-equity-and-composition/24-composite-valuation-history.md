# VALUATIONS-EQUITY-AND-COMPOSITION-24: Composite valuation, nested risk aggregation and financed return history

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-24`.
- **Decision status:** Accepted 2026-11-14 — complete held valuation, nested exposure and financed P&L history package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent examples cover ordered nested FX, strict matched risk, small nonzero trade changes and a cross-currency coupon and rebalance history. Production valuation, historical cash data and host qualification remain pending.

## Accepted decision for checked composite valuation and financed history

1. **Use one valuation tree for the headline and its breakdown.**
   Use [record 23's frozen holdings](23-composite-sizing.md#accepted-decision-for-checked-composite-sizing-and-rebalance-dates)
   on a common valuation and information cut. Apply each
   signed quantity once along its edge. Carry each native
   child value through its actual ordered, dated FX
   conversions to the root. Use the identical path basis
   for exposure reports. Bind the actual mark, measure
   and FX dependencies. Show signed net and absolute
   gross quantities and values by exact primitive identity
   and lot. Resolve child-owned price controls and inherit
   metric defaults only for controls the child leaves unset.
   Reuse only identical prepared valuation requests.
   [FX conventions](../core-conventions/013-fx-resolution-and-shocks.md).

2. **Require compatibility and completeness for risk totals.**
   Aggregate owner-qualified linear quantities on the same
   factor, direction, normalization, units, horizon and value
   basis. Use the declared measure currency. Show confirmed
   structural inapplicability. Missing or failed applicable
   contributions prevent a strict total. Composite FX01
   reprices the whole held value with matched relative +1%
   and −1% moves on the declared ordered FX direction.
   Bind each active source once and include affected child
   pricing and reporting conversions. Report
   `(V_up - V_down) / 2` in reporting currency per positive
   percentage point. Translated native risk retains its own
   frozen-FX basis.
   [Metric aggregation](../valuations-framework/metric-orchestration.md) ·
   [FX01 convention](../valuations-framework/option-sensitivities.md#8-keep-fx01-a-fixed-directional-scenario).

3. **Net proposed trade changes on the exact primitive lot.**
   Validate the prior and target trees and reject conflicting
   definitions on the same ID even if quantities cancel.
   Accumulate signed target less prior path quantities with
   checked arithmetic before finalization. Omit exact zero
   and retain reliably representable nonzero changes. Remove
   the absolute cutoff. Publish signed quantity proposals
   with stable primitive identity. Execution prices, fills and
   costs require their own evidence.

4. **Require settled cash and exact rebalance closes in history.**
   Match point-in-time marks and complete owner-qualified
   cash actually settled for the held lots during
   `(previous close, current close]`. Translate cash at its
   settlement-date FX and use an aligned ending mark so
   each payment counts once. Require confirmed zero cash
   if the held lots produced none. Placeholder exports and
   endpoint projection cannot establish settled coverage.
   Require an observation on every eligible scheduled
   rebalance after the initial state's effective date through
   the final observation, with no deferral to a later close.
   Warmup supplies sizing inputs only. The prior holdings
   earn P&L through the rebalance close and the new holdings
   apply to the next interval.

5. **Publish the fixed-capital financed P&L index.** For each
   interval add eligible settled cash to the old holdings'
   closing value less the preceding post-rebalance opening
   value. Divide by fixed positive reporting-currency
   capital. The first row has zero period P&L and return
   and an index of 100. Compound by `1 + return`. Require
   finite positive growth factors and resulting index levels.
   At a rebalance publish the pre-trade and post-trade marks
   at the same close and the modeled financing transfer of
   post-trade minus pre-trade value. Positive amounts are
   contributions and negative amounts are releases. Use the
   new mark as the next opening value. Treat settlements as
   cash paid in or out and use zero strategy-level execution,
   slippage and financing costs. Fund time-weighted
   performance has a separate accounting basis.
   [GIPS handbook](https://www.gipsstandards.org/standards/gips-standards-for-firms/gips-standards-handbook-for-firms/).

6. **Reconcile exported values and validate restored reports in
   Rust.** A value breakdown can be available with a limited
   cashflow capability. A complete contractual export
   requires the eligible leaf components to reconcile on
   the selected headline basis, including the agreed
   conversion and adjustment boundaries. Report settled cash,
   dates, capital, pre- and post-trade marks and financing
   so the history arithmetic can be reproduced. Validate
   restored identities, chronology and derivable arithmetic
   through the existing Python and WASM routes.
   [Valuation cashflow export](../valuations-framework/cashflow-export.md).

## Observed behavior

### Choice and rationale

Composite PV is the reporting-currency sum of held leg quantities times native instrument values. Nested exposures flatten to primitive paths for additive risk and execution netting. History reports period P&L divided by fixed configured capital and resets the next comparison value after each rebalance to exclude the rebalance financing amount from market P&L.

### Scope and input domain

Validated resolved state matching leg IDs/order, finite quantities and inputs. Composite-level instrument pricing overrides are rejected: quotes/model inputs belong on legs. Only additive metrics may be aggregated. Market histories must increase strictly; warmup precedes output, and initial state cannot become effective after the first observation.

### Algorithm

1. Value held legs with their native pricing behavior, convert at as-of FX and multiply by resolved quantities; base PV uses compensated Neumaier summation. Flatten nested quantities by multiplying along each path. Child metric overrides take precedence over inherited parent defaults.
2. Price applicable additive metrics for each primitive, convert monetary results and scale by path quantity. Unsupported legs contribute no value for that metric; reject a requested metric only when every primitive lacks it. Preserve path reports, and aggregate by instrument ID into net sums and absolute gross quantities/values/measures. Reused definitions must match hashes; identical definitions/defaults share request-local valuation cache.
3. Execution trades flatten prior/current holdings and subtract prior quantities by primitive ID, rejecting conflicting definitions across states. Suppress changes of magnitude≤1e-12.
4. History prices the held state at each observation. Cashflows are primitive exports in `(previous_date,current_date]`, projected from the current endpoint market and converted on each flow date. `PnL=current_value−previous_comparison_value+cashflows`; `period_return=PnL/configured_capital`; index starts 100 and compounds `index×(1+period_return)`.
5. Record pre-rebalance value/exposures, then rebalance when any scheduled date is after state's effective date and on/before current observation. Rebalance once using that observation's available history; set next comparison value to the new holdings' value at the same market. Missed scheduled dates are therefore serviced at the next observed date, not replayed through unobserved markets.

### Selection, controls and conventions

Risk does not recalculate quantities. History initialization from spec may use warmup; first row has zero P&L/return. Fixed capital denominator supports long/short portfolios whose PV may be near zero. Composite's direct cashflow schedule is a placeholder, while the history engine explicitly flattens primitive cashflows. Rebalance state retains overrides and records weighting inputs/trades.

### Limitations, failures and alternatives

The return index is compounded fixed-capital P&L, not conventional NAV-denominated time-weighted return. The reset assumes financing of changed holdings but does not maintain a cash balance, funding cost, slippage or transaction-cost ledger. Cashflow completeness depends on each primitive's export and endpoint projection. Summing additive local-currency Greeks after FX conversion does not add FX translation risk or covariance effects. A fully self-financing wealth account would require a separate accepted accounting method.

### Evidence

The [composite tests](../../../../finstack-quant/valuations/src/instruments/composite/tests.rs) locate nested net/gross exposure, cross-currency valuation, frozen quantities, override precedence and conflicting execution-definition checks. The source [history.rs](../../../../finstack-quant/valuations/src/instruments/composite/history.rs) and its inline `scheduled_rebalance_is_close_effective_and_principal_is_not_pnl` test establish chronology and the financing reset on a simple equity example; independent historical total-return/cash-ledger validation was not established. No backtest was executed.

## Independent numerical checks

- A USD root holds twice a GBP child with three lots of
  EUR 100 and one lot of GBP 40, plus a direct short of
  two of the EUR lots. EUR to GBP at .80 and GBP to USD
  at 1.25 give a child contribution of USD 700. Direct
  EUR to USD at 1.10 gives USD −220 and a total of 480.
  The direct FX shortcut on the nested EUR lots gives
  USD 540. The EUR lot's native measure of EUR 5 per
  common bump yields path contributions of USD +30 and
  −11, with signed net 19 and absolute gross 41.
  This gives a complete measure if the GBP lot is confirmed
  inapplicable. Bumping GBP to USD by ±1% on this basis
  gives root values USD 487 and 473, hence FX01 +7.
- Signed path quantities `[4, −3]` and
  `[4, −3 + 2^-43]` give one trade of `+2^-43` lots.
- On USD 1,000 fixed capital, a cross-currency coupon case
  opens at 170. Its old holdings are worth 187.20 at the
  rebalance close and receive 9.20 at settlement-date FX.
  First interval P&L is 26.40. The new close value is 21.60
  and the signed transfer is −165.60. The following close
  is worth 24.25, giving P&L 2.65. Summed P&L 29.05
  equals `24.25 − 170 + 9.20 − (−165.60)`. The final
  index is `100 * 1.0264 * 1.00265 = 102.911996`.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [history.rs](../../../../finstack-quant/valuations/src/instruments/composite/history.rs)
- [instrument.rs](../../../../finstack-quant/valuations/src/instruments/composite/instrument.rs)
- [reporting.rs](../../../../finstack-quant/valuations/src/instruments/composite/reporting.rs)
- [spec_support.rs](../../../../finstack-quant/valuations/src/instruments/composite/spec_support.rs)
