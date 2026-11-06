# MARGIN-COLLATERAL-IM-019 — VM call schedules and agreed-collateral replay

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: date generation and exposure-series replay are separate APIs. Replay processes each provided observation without filtering to generated call frequency. Proposed rationale: callers can supply actual observation events while calendar helpers generate contractual dates when needed.

## Input domain and conventions

Dates use the registered CSA holiday calendar. Default mapping: USD usny, EUR target2, GBP gblo, JPY jpto, CHF chzh, CAD cato, AUD auce; others weekends_only. Settlement lag is nonnegative u32 limited to i32 range by CSA VM validation.

## Ordered algorithm

1. Single calculation rolls valuation date Following, then adds settlement_lag business days; zero lag uses the rolled call date.
2. Daily schedule starts at Following(start), advances one business day and includes dates≤unadjusted end.
3. Weekly/monthly schedules retain original unadjusted start as the anchor: start+n weeks or start.add_months(n), then roll each date Following and deduplicate adjacent equal adjusted dates. A contractual date≤end may roll beyond end. Period-count overflow errors.
4. OnDemand returns adjusted start and adjusted end (one if equal); if adjusted start>adjusted end it returns empty. This endpoint convention differs from periodic schedules and is not continuous monitoring.
5. Exposure replay iterates input order without sort/dedup. Each observation calls the VM calculator against the current signed collateral. Post subtracts, collect adds immediately when a call is agreed, even if settlement is later. Two observations rolling to the same business day still each produce a call.
6. Only nonzero transfers emit MarginCall events. The call's settlement date is used by cashflow conversion. `MarginCall::days_to_settle` returns calendar elapsed days.

## Selection, controls and limitations

No settled-versus-pending ledger, intraday deadline/timezone, failed settlement, disputes or collateral interest is simulated. Margin call constructors do not independently validate nonnegative amounts or settlement chronology. A caller can use replay with irregular or decreasing dates; no chronology check is enforced here.

## Alternatives and tradeoffs

A dated settlement queue would distinguish available from agreed collateral; sorting/deduplicating observations would change event semantics and may discard intended intraday changes. Calendar helper integration with replay would impose frequency rather than leave observation selection to the caller.

## Source and validation evidence

Inspected/located tests cover weekend/holiday settlement, weekly anchor retention, month-end anchors, on-demand equal endpoints, signed collateral replay and positive zero. They exercise calendar/replay behavior, not operational agreement enforcement.

Implementation and data: [src/calculators/vm.rs](../../../../finstack-quant/margin/src/calculators/vm.rs), [src/types/call.rs](../../../../finstack-quant/margin/src/types/call.rs), [src/types/mod.rs](../../../../finstack-quant/margin/src/types/mod.rs), [src/types/csa.rs](../../../../finstack-quant/margin/src/types/csa.rs).

Existing evidence: [src/calculators/vm.rs](../../../../finstack-quant/margin/src/calculators/vm.rs), [src/types/mod.rs](../../../../finstack-quant/margin/src/types/mod.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
