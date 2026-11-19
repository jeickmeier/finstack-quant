# MARGIN-COLLATERAL-IM-020 — Repo margin balance and interest cashflows

**Decision status:** Accepted 2026-11-17 for checked repo cash-margin balances, transfers and interest.

**Implementation status:** Existing behavior is described below. Agreement-qualified projection, settlement state, interest and caller reconciliation remain pending.

**Validation status:** Source and primary repo references inspected. Independent Decimal examples checked the trigger boundary, full shortfall, eligible pending credit, capped refunds and signed interest. Runtime and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked repo cash margin and interest

1. **Elect the margin route explicitly.** Support explicit no margin or cash-only
   ratio maintenance for one funded, fixed-term repo from the cash lender's
   perspective. Require the actual agreement, account, currency and applicable
   dates. Remove automatic ratio, trigger, timing and interest defaults.
   Tri-party servicing, two-way transfers, GMRA net exposure and securities
   substitution require their own qualified methods.
   [Accepted repo valuation and margin scope](../valuations-rates/15-repo.md#accepted-decision-for-checked-repo-cashflows-collateral-and-risk).
2. **Require a dated collateral target.** The agreement elects purchase principal
   or accrued repurchase cash as the positive reference amount `B`. Require the
   exact basis date and a qualified dirty securities value `V` in the same
   currency. Track cash margin separately. With positive margin ratio `m` and
   trigger fraction `0 ≤ q < 1`, set `T = B × m` and `L = T × (1 − q)` in checked
   Decimal Money. Keep the ratio, haircut and quote on their respective bases.
   [Money arithmetic](../core-conventions/012-money-decimal-precision.md) ·
   [ICMA ERCC Guide, sections 3.12–3.16](https://www.icmagroup.org/assets/ERCC-Guide-to-Best-Practice-March-2025.pdf).
3. **Call the full shortfall once the trigger is crossed.** Let `C` be settled
   cash margin plus any eligible signed pending transfers under the elected
   credit policy. If `V + C < L`, propose a collection of `T − (V + C)`.
   Equality gives zero collection. Apply the comparison before rounding. Elect
   either exact transfers or an increment that rounds collections up and
   refunds down in magnitude.
4. **Refund only settled excess.** Let `A` be settled cash margin less
   outstanding agreed refunds. When the collection gate is closed, the refund
   magnitude is `min(A, max(V + A − T, 0))`, including at the first eligible
   observation. With a 102 million target and a 105 million securities mark,
   zero settled cash gives zero refund. With 2 million settled and zero refunds
   reserved, the refund is capped at 2 million.
5. **Use dated transfer evidence.** Apply record 019's authorized slots,
   cutoffs, calendars and settlement lags. Require explicit opening settled and
   pending amounts. Proposals have zero balance effect. Agreements create
   pending transfers. Confirmed receipts or refunds update the settled balance
   once, including partial settlements. At contractual close, use an explicit
   refund instruction or settlement evidence. Any unreconciled balance stays
   open. [Accepted call and settlement replay](19-margin-dates-and-replay.md#accepted-decision-for-checked-vm-schedules-and-settlement-replay).
6. **Accrue interest on settled cash only.** Elect no interest or simple interest
   with a signed annual rate, complete day-count context and payment dates. Split
   accrual at settled balance changes, rate changes and payment boundaries. The
   lender's signed interest is `−Σ(balance × rate × year fraction)`. Holding
   2 million for seven ACT/360 days at 5% produces a lender payment of 1,944.44
   with elected cent rounding. Require complete inputs and apply rounding once
   per payment. Remove the fixed 0.01 filter. An agreement that credits unpaid
   interest in net exposure needs its qualified exposure calculation.
   [ICMA ERCC Guide, sections 3.52–3.54](https://www.icmagroup.org/assets/ERCC-Guide-to-Best-Practice-March-2025.pdf).
7. **Export signed cash with its settlement status.** A top-up is positive cash
   to the lender and a refund is negative. Forecast cash uses a qualified cash
   instruction and due date. Realized cash uses the confirmed amount and actual
   date. Margin transfers and interest have zero repo principal effect. Reconcile
   existing valuation, portfolio and host consumers with the checked Rust
   contract. Record 022 owns the wire format.
   [Cashflow semantics](../core-conventions/015-cashflow-semantics.md) ·
   [Wire and provenance](22-wire-tables-and-provenance.md).

## Choice and rationale

All observed active repo margin types share one collateral-ratio projection. The accepted cash-only route tracks agreed top-ups and settled cash separately. Interest follows the settled balance and elected payment terms.

## Input domain and conventions

Cash principal is Money; collateral observations are same-unit raw f64 values with dates. Margin ratio and trigger discount are caller-provided decimals, with no validating constructor/domain check here. MarkToMarket, NetExposure and Triparty are labels sharing the active calculation; None suppresses calls.

## Ordered algorithm

1. Defaults: no margining, ratio 1.02, trigger threshold 0.01, daily, T+1, no interest, no substitution. MarkToMarket enables interest/substitution with treasury schedule and caller ratio/threshold; Triparty chooses threshold 0.005, T+0 and BCBS collateral schedule.
2. Helpers compute required=principal×ratio; trigger=required×(1-threshold); margin call iff active and collateral<trigger. Deficit/excess helpers measure distance from required without applying active/trigger checks.
3. Projection converts settlement lag to i32 before None short-circuit. Start margin_balance=0. At each observation in input order, effective collateral=valuation+margin_balance. If below trigger, emit positive VariationMarginPay of required-effective and add to balance immediately.
4. Else if effective>required and observation index>0, emit positive VariationMarginReceive of effective-required and subtract from balance. There is no cap by prior cash margin, and no return at the first observation. Settlement is observation date plus business-day lag, without the VM calculator's explicit Following call-date roll.
5. Interest generation returns empty unless pays_margin_interest and an explicit rate are both present. For each adjacent balance pair accrue prior balance×annual decimal rate×caller day-count fraction, paid on the latter date; emit only if abs(interest)>0.01 major currency units. No end balance close-out or reinvestment is synthesized.
6. In the audited revision, call conversion mapped initial margin, VM post or top-up, VM collect and substitution to `InitialMarginPost`, `VariationMarginPay`, `VariationMarginReceive` and `CollateralSubstitutionOut`, keeping positive call amounts and the scheduled settlement date. The current converter skips in-kind substitutions under core record 015. It still carries the amount and scheduled date for the other call types.

## Selection, controls and limitations

call_frequency, substitution_allowed and eligible_substitutes are not consulted by the projection. The caller owns observation cadence, eligibility and return/cash balance interpretation. No chronology/sorting, rate/ratio domain check, initial margin balance input or implicit market-rate fallback is implemented. Interest uses supplied balance dates, not automatically the generated calls.

## Alternatives and tradeoffs

The accepted route caps cash refunds by the uncommitted settled cash. GMRA net exposure, tri-party operations and securities movements require the qualified terms and evidence of their respective methods.

## Source and validation evidence

Inspected tests check persistent-deficit no-repeat, threshold suppression, a 3 million excess return with zero prior top-up, and interest magnitudes of about 1,944.44 on ACT/360 and 1,917.81 on ACT/365F. A weekend settlement regression was located. These tests check formula and date behavior. Agreement-specific qualification requires its own reference comparison.

Implementation and data: [src/types/repo_margin.rs](../../../../finstack-quant/margin/src/types/repo_margin.rs), [src/types/repo_cashflows.rs](../../../../finstack-quant/margin/src/types/repo_cashflows.rs), [src/types/call.rs](../../../../finstack-quant/margin/src/types/call.rs), [src/schema.rs](../../../../finstack-quant/margin/src/schema.rs).

Existing evidence: [src/types/repo_margin.rs](../../../../finstack-quant/margin/src/types/repo_margin.rs), [src/types/repo_cashflows.rs](../../../../finstack-quant/margin/src/types/repo_cashflows.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
