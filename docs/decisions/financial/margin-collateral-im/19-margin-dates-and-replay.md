# MARGIN-COLLATERAL-IM-019 — VM call schedules and agreed-collateral replay

**Decision status:** Accepted 2026-11-17 for checked VM call schedules, settlement and replay.

**Implementation status:** Existing behavior is described below. Dated schedule, transfer state, cashflow and caller reconciliation remain pending.

**Validation status:** Source and primary operating references inspected. Independent examples checked effective date boundaries, settlement-day units and partial settlement. Runtime and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked VM schedules and settlement replay

1. **Require explicit timing terms.** Bind each run to the effective agreement,
   account and currency. Require elected call and settlement calendars, time zone
   and relevant cutoffs. Preserve the valuation date, information cutoff,
   contractual anchor, effective call slot, contractual due date and actual
   settlement date. A date-only event requires a declared phase that establishes
   its order. Retain notification, response, dispute and grace terms with their
   authority. An unresolved material deadline makes timing unavailable.
   [Contractual terms](01-registry-and-defaults.md#accepted-decision-for-explicit-margin-parameter-snapshots-and-contractual-terms) ·
   [Financial clocks](../conventions/0045-financial-clocks-and-date-boundaries.md) ·
   [Published VM timing example, Article 12](https://eur-lex.europa.eu/legal-content/EN/TXT/PDF/?uri=CELEX%3A02016R2251-20230214).
2. **Use one inclusive window of effective call dates.** Daily schedules use open
   call-calendar days. Weekly schedules advance by seven calendar days from the
   original anchor. Monthly schedules preserve the elected day, clipped in
   shorter months, or use explicit month-end. Support explicitly elected Following
   for periodic calls. Roll once, retain the raw anchors and combine anchors that
   resolve to the same scheduled slot. Include an earlier anchor that rolls into
   the window and filter by the effective dates. Reject reversed windows and
   date arithmetic failures. On-demand calls require supplied, authorized requests.
   [Core date arithmetic](../core-conventions/007-tenor-date-arithmetic.md).
3. **Calculate the contractual due date on the settlement calendar.** Count a
   positive elected business-day lag strictly after the effective call date.
   Zero lag requires same-day settlement eligibility under the calendar and
   cutoff. A closure or missed cutoff requires a qualified due-date rule or an
   explicit timing error. Report the elected business-day lag and elapsed
   calendar days as distinct values.
4. **Replay authorized observations in chronological order.** Require an eligible
   exposure observation for each periodic slot. Admit distinct intraday and
   on-demand events with explicit authorization and identity. Validate event
   order, including an explicit sequence for ties, and use evidence available at
   each cut. Reject duplicate or out-of-order events. Missing required exposure
   or balance evidence makes that result unavailable. Each eligible slot uses
   the [accepted signed VM calculation](18-variation-margin.md#accepted-decision-for-checked-signed-variation-margin-and-transfers).
5. **Track proposals, agreed pending transfers and actual settlements.** Require
   explicit opening settled and pending state, including explicit zero or empty
   balances. A proposal has zero balance effect. Agreement evidence creates a
   signed pending transfer for the authorized amount, including any agreed
   portion of a dispute. Confirmed settlement moves the verified amount from
   pending to settled once, with partial settlement supported. An overdue
   transfer stays outstanding until reconciled. Record failures, disputes and
   cancellations with their evidence. Credited collateral is settled collateral
   alone or settled plus eligible agreed pending collateral, as elected.
   [Credited collateral](15-collateral-eligibility-and-haircuts.md#accepted-decision-for-checked-collateral-eligibility-and-credited-value) ·
   [ISDA collateral operating practices](https://www.isda.org/a/ZeOgE/2021-Best-Practices-for-the-OTC-Derivatives-Collateral-Process-Updated-082024.pdf).
6. **Carry the same validated status through Rust, Python and WASM.** Give each
   nonzero proposal a positive magnitude, signed direction, scope, dates and
   evidence. Keep zero and suppressed calculations as diagnostics. Cash
   forecasts require an explicit cash basis and use the contractual due date.
   Realized cash rows use the actual signed cash amount and settlement date.
   Securities transfers stay on the collateral ledger. Record 022 owns the
   wire format. [Cashflow semantics](../core-conventions/015-cashflow-semantics.md) ·
   [Wire and provenance](22-wire-tables-and-provenance.md).

A Friday call due Monday on an elected weekdays-only calendar has a
one-business-day lag and three elapsed calendar days. For an agreed receipt of
100 with 60 confirmed, the balances are 60 settled and 40 pending. Credited
collateral is 60 under settled-only treatment or 100 if the remainder is eligible.

## Choice and rationale

Observed date generation and exposure-series replay are separate APIs. Replay processes each provided observation without filtering to generated call frequency. The accepted contract uses scheduled slots and authorized events to determine call coverage and credit at each cut.

## Input domain and conventions

Dates use the registered CSA holiday calendar. Default mapping: USD usny, EUR target2, GBP gblo, JPY jpto, CHF chzh, CAD cato, AUD auce; others weekends_only. Settlement lag is nonnegative u32 limited to i32 range by CSA VM validation.

## Ordered algorithm

1. Single calculation rolls valuation date Following, then adds settlement_lag business days; zero lag uses the rolled call date.
2. Daily schedule starts at Following(start), advances one business day and includes dates≤unadjusted end.
3. Weekly/monthly schedules retain original unadjusted start as the anchor: start+n weeks or start.add_months(n), then roll each date Following and deduplicate adjacent equal adjusted dates. A contractual date≤end may roll beyond end. Period-count overflow errors.
4. OnDemand returns adjusted start and adjusted end (one if equal); if adjusted start>adjusted end it returns empty. This endpoint convention differs from periodic schedules and is not continuous monitoring.
5. Exposure replay iterates input order without sort/dedup. Each observation calls the VM calculator against the current signed collateral. A computed post immediately subtracts from the balance and a computed collect adds to it. Two observations rolling to the same business day still each produce a call.
6. Only nonzero transfers emit MarginCall events. The call's settlement date is used by cashflow conversion. `MarginCall::days_to_settle` returns calendar elapsed days.

## Selection, controls and limitations

No settled-versus-pending ledger, intraday deadline/timezone, failed settlement, disputes or collateral interest is simulated. Margin call constructors do not independently validate nonnegative amounts or settlement chronology. A caller can use replay with irregular or decreasing dates; no chronology check is enforced here.

## Alternatives and tradeoffs

The accepted dated transfer state distinguishes amounts available at a cut from amounts still pending. Validate supplied event order and retain distinct authorized intraday events. Apply the contractual cadence through effective scheduled slots.

## Source and validation evidence

Inspected/located tests cover weekend/holiday settlement, weekly anchor retention, month-end anchors, on-demand equal endpoints, signed collateral replay and positive zero. They exercise calendar/replay behavior, not operational agreement enforcement.

Implementation and data: [src/calculators/vm.rs](../../../../finstack-quant/margin/src/calculators/vm.rs), [src/types/call.rs](../../../../finstack-quant/margin/src/types/call.rs), [src/types/mod.rs](../../../../finstack-quant/margin/src/types/mod.rs), [src/types/csa.rs](../../../../finstack-quant/margin/src/types/csa.rs).

Existing evidence: [src/calculators/vm.rs](../../../../finstack-quant/margin/src/calculators/vm.rs), [src/types/mod.rs](../../../../finstack-quant/margin/src/types/mod.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
