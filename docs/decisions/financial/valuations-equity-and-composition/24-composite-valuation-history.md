# Composite valuation, nested risk aggregation and financed return history

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-24`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

Composite PV is the reporting-currency sum of held leg quantities times native instrument values. Nested exposures flatten to primitive paths for additive risk and execution netting. History reports period P&L divided by fixed configured capital and resets the next comparison value after each rebalance to exclude the rebalance financing amount from market P&L.

## Scope and input domain

Validated resolved state matching leg IDs/order, finite quantities and inputs. Composite-level instrument pricing overrides are rejected: quotes/model inputs belong on legs. Only additive metrics may be aggregated. Market histories must increase strictly; warmup precedes output, and initial state cannot become effective after the first observation.

## Algorithm

1. Value held legs with their native pricing behavior, convert at as-of FX and multiply by resolved quantities; base PV uses compensated Neumaier summation. Flatten nested quantities by multiplying along each path. Child metric overrides take precedence over inherited parent defaults.
2. Price applicable additive metrics for each primitive, convert monetary results and scale by path quantity. Unsupported legs contribute no value for that metric; reject a requested metric only when every primitive lacks it. Preserve path reports, and aggregate by instrument ID into net sums and absolute gross quantities/values/measures. Reused definitions must match hashes; identical definitions/defaults share request-local valuation cache.
3. Execution trades flatten prior/current holdings and subtract prior quantities by primitive ID, rejecting conflicting definitions across states. Suppress changes of magnitude≤1e-12.
4. History prices the held state at each observation. Cashflows are primitive exports in `(previous_date,current_date]`, projected from the current endpoint market and converted on each flow date. `PnL=current_value−previous_comparison_value+cashflows`; `period_return=PnL/configured_capital`; index starts 100 and compounds `index×(1+period_return)`.
5. Record pre-rebalance value/exposures, then rebalance when any scheduled date is after state's effective date and on/before current observation. Rebalance once using that observation's available history; set next comparison value to the new holdings' value at the same market. Missed scheduled dates are therefore serviced at the next observed date, not replayed through unobserved markets.

## Selection, controls and conventions

Risk does not recalculate quantities. History initialization from spec may use warmup; first row has zero P&L/return. Fixed capital denominator supports long/short portfolios whose PV may be near zero. Composite's direct cashflow schedule is a placeholder, while the history engine explicitly flattens primitive cashflows. Rebalance state retains overrides and records weighting inputs/trades.

## Limitations, failures and alternatives

The return index is compounded fixed-capital P&L, not conventional NAV-denominated time-weighted return. The reset assumes financing of changed holdings but does not maintain a cash balance, funding cost, slippage or transaction-cost ledger. Cashflow completeness depends on each primitive's export and endpoint projection. Summing additive local-currency Greeks after FX conversion does not add FX translation risk or covariance effects. A fully self-financing wealth account would require a separate accepted accounting method.

## Evidence

The [composite tests](../../../../finstack-quant/valuations/src/instruments/composite/tests.rs) locate nested net/gross exposure, cross-currency valuation, frozen quantities, override precedence and conflicting execution-definition checks. The source [history.rs](../../../../finstack-quant/valuations/src/instruments/composite/history.rs) and its inline `scheduled_rebalance_is_close_effective_and_principal_is_not_pnl` test establish chronology and the financing reset on a simple equity example; independent historical total-return/cash-ledger validation was not established. No backtest was executed.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [history.rs](../../../../finstack-quant/valuations/src/instruments/composite/history.rs)
- [instrument.rs](../../../../finstack-quant/valuations/src/instruments/composite/instrument.rs)
- [reporting.rs](../../../../finstack-quant/valuations/src/instruments/composite/reporting.rs)
- [spec_support.rs](../../../../finstack-quant/valuations/src/instruments/composite/spec_support.rs)
