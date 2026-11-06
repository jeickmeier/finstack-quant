# STATEMENTS-CAPITAL-003: Funding face, residual scaling and closing debt/accrual

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed for the source scope and discrepancies stated below.
**Validation status:** Evidence inspected or located, not executed; numerical qualification remains open.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Choice and rationale

Funding face is separated from cash proceeds so withheld OID does not reduce principal owed. Stateful zero balances suppress resurrection of debt already swept. Residual schedule rebuilding is the intended interest engine; a remaining opening-balance ratio is diagnostic but still affects cash amounts. These are observed mechanisms, not approval of every fallback.

## Inputs and calculation

1. A funding row is Notional or RevolvingDraw. If it has an explicit principal delta, only a positive delta establishes funding. Otherwise RevolvingDraw or nonpositive Notional contributes the absolute cash amount. Sum principal by its economic balance date inside `[start,end)`, independently of payment date. Debt funding currency must match the schedule.
2. If there is no funding leg anywhere, opening is zero, and the issue date is in the current period, use nonnegative initial principal as implicit issuance. A funding leg suppresses this inference even when outside the period. Negative implicit initial principal errors.
3. Determine scale `s`: hedges use 1. For zero opening, use 1 if scheduled opening is zero or new funding exists, else 0. For nonzero opening and zero scheduled opening, use 1. Otherwise use `opening/scheduled_opening`. If `abs(s−1)>1e−6`, emit `ResidualScaleMismatch` and a tracing warning **but apply the raw ratio**. The module's claimed `[0,1.10]` clamp is stale prose; there is no such clamp in this branch.
4. Scheduled closing is the absolute latest economic outstanding at/before `end−1`; with no entry it normally falls back to absolute initial principal. The pre-issuance zero fallback requires both a later issue date and a first outstanding entry later than the snapshot.
5. Hedge closing is zero. For nonzero debt opening, use scheduled closing. For zero opening without new funding, remain zero. For zero opening with new funding, use `max(funding + in-period PIK expense − absolute in-period cash repayments,0)`. This last branch uses cash dates for PIK/repayment buckets, whereas the normal branch follows economic principal dates.
6. Debt accrued interest is the shared full-schedule accrual engine at `end−1`, multiplied by `s`; hedges report zero. `AccrualConfig::default()` supplies no frequency, so an ACT/ACT ISMA requirement can fail closed rather than guess. The returned funding is kept separately for waterfall capacity; raw cash proceeds are not rewritten.

## Limitations, controls and alternatives

The scale mismatch is not repaired, and negative stateful opening is not explicitly rejected at this entry point. Financial callers should reconcile residual schedules rather than rely on an undocumented safety bound. A hard error or a bounded repair would have different economics and requires explicit adoption.

**Unreproduced date concern:** newly funded zero-opening debt bypasses the dated scheduled closing and recomputes from cash buckets. An economic principal change and its settlement on opposite sides of a period boundary may therefore give a different closing balance or feed inconsistent dated-waterfall capacity. Existing ordinary issuance/PIK cases do not prove all separated-date combinations.

There is no OID amortization/EIR engine here: preserving funding face is distinct from accounting for issue discount. An economic-ledger-only closing path would provide a simpler date contract, but must also preserve prior full paydown/new-draw behavior.

## Evidence

- **Implementation:** [funding, scale and closing branches](../../../../finstack-quant/statements/src/capital_structure/period_flows.rs), `funding_principal` and `calculate_period_flows`; [state funding handoff](../../../../finstack-quant/statements/src/capital_structure/state.rs).
- **Regression evidence:** [issuance/OID/PIK cases](../../../../finstack-quant/statements/src/capital_structure/tests/senior_review_capital.rs), `oid_funding_preserves_loan_face_and_cash_at_boundaries`, `oid_funding_preserves_custom_bond_face_with_separate_cash_dates`, `schedule_without_funding_books_issuance_once_and_does_not_resurrect_paid_debt`, `newly_funded_loan_preserves_contractual_pik_in_its_first_period`. Internal ledger checks; not executed.

Shared accrual mechanics remain owned by the cashflows scope.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
