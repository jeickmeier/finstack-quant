# SCENARIOS-014: Realized-forward carry and per-currency failure-isolated aggregation

- Decision status: **Accepted 2026-11-15 — complete optional carry, comparable marks, settlement timing and native aggregation package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent settlement-phase, signed sizing and currency arithmetic checks completed.** Production pricing, row isolation, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for checked scenario roll carry and native aggregation

1. **Elect optional carry explicitly.** An advancing roll with carry
   elected requires supplied inventory. An explicitly empty inventory
   produces a complete empty measurement. An admitted equal-date roll
   produces identity evidence without pricing or cash collection.
   Measure after the admitted roll and any elected horizon requote,
   before subsequent scenario shocks.
   [Accepted time roll](13-market-time-roll.md#accepted-decision-for-checked-scenario-time-rolls-and-horizon-replay).

2. **Price comparable endpoints through the shared executor.** Use
   strict PV-only pricing with distinct opening and rolled states and
   the elected model, pricing options, configuration and provider.
   Both signed marks use the same currency and a comparable economic
   holder basis. Include detached unpaid claims in the closing mark
   or an explicit bridge. Admit terminal zero only when the product
   establishes that every cash, delivery and other claim is extinguished.
   [Shared executor](../../architecture/0046-portfolio-execution-operational-contract.md#decision-and-scope) ·
   [Holder valuation](../valuations-framework/carry-theta-breakeven.md#accepted-decision-for-checked-horizon-theta-carry-and-breakeven).

3. **Count signed settlement cash once.** Use the complete qualified
   ledger from the same scenario path as the closing claim. Include
   opening-date cash when opening PV includes it and closing-date
   cash when closing PV excludes it. Both ex-cash cuts use
   `(open,close]`; both cum-cash cuts use `[open,close)`.
   Resolve mixed cuts independently. Payment date and entitlement
   govern inclusion. Identify any excluded opening acquisition by
   event identity. PIK and write-downs remain balance events.
   [Cash settlement and principal effects](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata).

4. **Measure native-currency carry at the actual inventory size.**
   Add `PV_close−PV_open` in the mark currency and nominal signed
   cash in its native currencies. FX conversion, funding,
   reinvestment and annualization belong to separate elected
   measures. Each inventory slot is one instrument at embedded
   size, identified by slot and instrument ID. Repeated IDs remain
   distinct. A portfolio carry result requires actual position
   IDs and owner-qualified signed sizing applied once to both
   marks and every cash item.
   [Position sizing owner](../portfolio-execution/01-position-sizing-and-validation.md).

5. **Commit all currency contributions for each row together.**
   Collect indexed outcomes from the shared executor and fold them
   serially in inventory order. Use canonical cash-event order
   and checked Money arithmetic. If admitting a row fails in
   any currency, discard its contributions in every currency.
   [Money arithmetic](../core-conventions/012-money-decimal-precision.md).

6. **Expose coverage and typed failures.** Distinguish skipped,
   identity, complete and incomplete outcomes, with requested,
   successful and failed counts. Retain row identity, endpoint
   marks, nominal cash by currency and typed failure stage. An
   optional carry-row error excludes the full row and leaves a
   subtotal of admitted rows. Required roll, fixing, replay and
   publication errors abort the scenario. Rust owns the financial
   result across Python and WASM. Record 017 owns the wire.
   [Atomic execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution) ·
   [Reporting record 017](17-reports-and-replay.md).

## Independent calculation checks

- Ex-cash marks of `100` and `95`, with settlement cash of
  `12` at opening, `3` between cuts and `4` at closing,
  give carry `95−100+7=2`. With cum-cash opening and
  closing marks, the calculation is `99−112+15=2`. The
  mixed cuts give `99−100+3=2` and `95−112+19=2`.
- Opening USD 100, closing USD 98 and period cash of USD 3
  and EUR 2 give carry of USD 1 and EUR 2. Separate lots
  of `+2` and `−0.5` yield combined carry of USD 1.50
  and EUR 3 after applying the signed scales.
- A row adding USD 2 and EUR 0.01 to prior totals of USD 100
  and EUR `10²⁸` loses a minor-unit digit in the EUR addition.
  The checked row fails and both prior currency totals survive.

## Observed choice and rationale at the audit revision

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

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#scenarios).
