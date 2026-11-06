# SCENARIOS-014: Realized-forward carry and per-currency failure-isolated aggregation

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: the time-roll report measures realized-forward mark change plus cash received/paid over the interval. It keeps currencies separate and excludes an instrument completely if either endpoint valuation or its cashflow collection fails. This prevents a partial cashflow-only row from contaminating the total.

## Ordered formula and aggregation

For each inventory instrument in input order, compute `PV0=instrument.value(original_market,t0)` and `PV1=instrument.value(rolled_market,t1)`. Subtract with checked `Money`; mismatched currencies or arithmetic failure reject the row. Collect `instrument.dated_cashflows(original_market,t0)` and include only payment dates satisfying `t0<date<=t1`. Sum flows by their own currencies, then add them to the PV-difference currency map:

`Carry_c = 1[c=PV_currency]*(PV1-PV0) + Σ CF_c(date), t0<date<=t1`.

No discounting, reinvestment or funding charge is added to cashflows in this report. No currency conversion is applied; a multi-currency instrument can contribute separate cashflow currencies alongside its PV currency. Notional/position size is whatever the instrument's value and cashflow methods already embed; the scenario inventory contains no separate position quantity multiplier.

Instrument rows are evaluated with the shared ordered parallel map and folded serially in the original order. For each row, aggregate into a clone of the running currency totals and commit only if every currency addition succeeds. On failure, discard the entire row and retain prior totals. Use checked `Money` additions throughout; there is no f64 compensated portfolio sum at this adapter layer.

## Failure semantics and report interpretation

Failures distinguish t0 valuation, t1 valuation, PV subtraction, cashflow collection and aggregation. Successful rows and totals omit failed instruments, while `failed_instruments` retains ID/reason and the engine creates `TimeRollInstrumentFailed` warnings. Empty/no inventory produces empty totals, not an estimated portfolio carry. Duplicate instrument IDs are not aggregated by ID: rows follow inventory entries, and totals include each entry.

**Execution boundary:** the adapter directly maps `instrument.value` over inventory at both endpoints, with a local ordered fold. The call does not receive the engine's `PricingOptions`, active configuration or recalibration provider, although report metadata stamps engine configuration. Accepted [ADR 0022](../../architecture/0022-portfolio-evaluation-state.md) and [the operational contract](../../architecture/0046-portfolio-execution-operational-contract.md) require ordinary scenario/portfolio P&L to use the shared request-scoped executor. This local inventory pricing path differs from that authority; the observed path is not an approved second executor. No configuration-sensitive price difference was executed here.

The as-of/market roll still commits when only row calculations fail. Callers must inspect completeness before treating `total_carry` as whole-portfolio P&L. Date-boundary inclusion prevents counting the same coupon twice across adjacent intervals, but telescoping also depends on instrument state/fixing consistency across the successive market rolls.

## Evidence and alternatives

Owning source: [adapters/time_roll.rs](../../../../finstack-quant/scenarios/src/adapters/time_roll.rs). Inspected `cashflow_failure_excludes_instrument_from_carry` asserts empty successful rows/totals and the retained failure reason. The interval and ordered-fold rules above are implementation evidence; no boundary or cross-target determinism checks were executed; [time-roll integration](../../../../finstack-quant/scenarios/tests/integration/time_roll_test.rs) locates instrument-level carry comparisons. These are identity/lifecycle evidence; using the same instrument valuation kernel at both endpoints is not independent price validation.

Frozen-market theta, funded/reinvested total return, position-weighted portfolio carry, and base-currency aggregation are alternative measures. Their results should not be labeled as this raw report without explicit conversion. No quantity, FX or reinvestment policy is added here.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
