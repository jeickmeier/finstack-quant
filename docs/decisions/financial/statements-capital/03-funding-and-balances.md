# STATEMENTS-CAPITAL-003: Funding face, residual scaling and closing debt/accrual

**Decision status:** Accepted for opening principal reconciliation and contractual accrued cash interest (2026-11-15).
**Implementation status:** Opening principal reconciliation and separation of contractual accrual from overdue interest remain pending. The current extraction uses the raw scale ratio and the generic accrual configuration described below.
**Validation status:** Evidence inspected or located, not executed; numerical qualification remains open.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Accepted decision for opening principal reconciliation

For active debt carried into a reporting period, require the canonical dated schedule and the stateful debt ledger to agree on opening principal in the same currency at the same opening economic boundary. Calculate debt service from the reconciled dated schedule. If the amounts disagree, fail the calculation with the instrument, reporting period, currency and both opening amounts. This preserves the dated debt-service amounts from the reconciled schedule.

Explicit dated events govern issuance, redraw and retirement. Hedge cashflows follow their own reporting rules. [Dated principal effects](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata) · [Hedge cashflow classification](02-cashflow-classification.md).

## Accepted decision for contractual accrued cash interest

Accrued interest is contractual cash interest earned through the period-close snapshot and due after that date. Calculate it from the canonical instrument's contractual earning terms, its dated principal and the complete accrual context those terms require. A missing required term, principal state or accrual input fails the calculation. Use the contractual earning basis for this Statements balance. The bond settlement quote has its own basis. [CASHFLOWS-013 quote contract](../cashflows/13-accrued-interest.md#accepted-decision-original-parent-cash-quotation-and-inclusive-first-ex-dates).

Track interest already due and unpaid as overdue arrears. Keep issue-discount amortization and any effective-interest carrying-value calculation separate from both contractual accrued interest and the principal face ledger.

## Audited inputs and calculation

The audited implementation separates funding face from cash proceeds so withheld OID leaves principal owed intact. It uses stateful zero balances to suppress resurrection of debt already swept. Residual schedule rebuilding supplies interest, with the opening-balance ratio still affecting current cash amounts.

1. A funding row is Notional or RevolvingDraw. If it has an explicit principal delta, only a positive delta establishes funding. Otherwise RevolvingDraw or nonpositive Notional contributes the absolute cash amount. Sum principal by its economic balance date inside `[start,end)`, independently of payment date. Debt funding currency must match the schedule.
2. If there is no funding leg anywhere, opening is zero, and the issue date is in the current period, use nonnegative initial principal as implicit issuance. A funding leg suppresses this inference even when outside the period. Negative implicit initial principal errors.
3. Determine scale `s`: hedges use 1. For zero opening, use 1 if scheduled opening is zero or new funding exists, else 0. For nonzero opening and zero scheduled opening, use 1. Otherwise use `opening/scheduled_opening`. If `abs(s−1)>1e−6`, emit `ResidualScaleMismatch` and a tracing warning **but apply the raw ratio**. The module's claimed `[0,1.10]` clamp is stale prose; there is no such clamp in this branch.
4. Scheduled closing is the absolute latest economic outstanding at/before `end−1`; with no entry it normally falls back to absolute initial principal. The pre-issuance zero fallback requires both a later issue date and a first outstanding entry later than the snapshot.
5. Hedge closing is zero. For nonzero debt opening, use scheduled closing. For zero opening without new funding, remain zero. For zero opening with new funding, use `max(funding + in-period PIK expense − absolute in-period cash repayments,0)`. This last branch uses cash dates for PIK/repayment buckets, whereas the normal branch follows economic principal dates.
6. Debt accrued interest uses the shared full-schedule accrual engine at `end−1`, multiplied by `s`. Hedges report zero. `AccrualConfig::default()` supplies no frequency, so required ACT/ACT ISMA context can be missing. The waterfall also adds current unpaid cash-interest shortfalls to reported accrued interest. Returned funding remains a separate input to waterfall capacity. [Waterfall shortfall reporting](../../../../finstack-quant/statements/src/capital_structure/waterfall/mod.rs).

## Limitations, controls and alternatives

The current path still uses the raw ratio after a mismatch warning. Negative stateful openings can reach this branch. The accepted hard failure awaits implementation.

**Unreproduced date concern:** newly funded zero-opening debt bypasses the dated scheduled closing and recomputes from cash buckets. An economic principal change and its settlement on opposite sides of a period boundary may therefore give a different closing balance or feed inconsistent dated-waterfall capacity. Existing ordinary issuance/PIK cases do not prove all separated-date combinations.

There is no OID amortization/EIR engine here: preserving funding face is distinct from accounting for issue discount. An economic-ledger-only closing path would provide a simpler date contract, but must also preserve prior full paydown/new-draw behavior.

## Evidence

- **Implementation:** [funding, scale and closing branches](../../../../finstack-quant/statements/src/capital_structure/period_flows.rs), `funding_principal` and `calculate_period_flows`; [state funding handoff](../../../../finstack-quant/statements/src/capital_structure/state.rs).
- **Regression evidence:** [issuance/OID/PIK cases](../../../../finstack-quant/statements/src/capital_structure/tests/senior_review_capital.rs), `oid_funding_preserves_loan_face_and_cash_at_boundaries`, `oid_funding_preserves_custom_bond_face_with_separate_cash_dates`, `schedule_without_funding_books_issuance_once_and_does_not_resurrect_paid_debt`, `newly_funded_loan_preserves_contractual_pik_in_its_first_period`. Internal ledger checks; not executed.

Shared accrual mechanics remain owned by the cashflows scope.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
