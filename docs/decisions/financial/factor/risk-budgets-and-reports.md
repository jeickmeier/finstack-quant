# FACTOR-010: Signed VaR budgeting and report orchestration

**Decision status:** Accepted 2026-11-17 for exact VaR budgets, signed utilization and checked report assembly.

**Implementation status:** Signed utilization and report dispatch are implemented. Exact target admission, explicit thresholds, zero-target outcomes and checked reconciliation remain pending.

**Validation status:** All 17 focused budget and report tests passed on 2026-11-17 against current behavior. Independent exact arithmetic verified the signed utilization example. The accepted admission, zero-target and reconciliation rules still need qualification.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Bind targets to exact position IDs. Use nonnegative decimal shares summing to exactly one for a nonempty book and require an explicit positive utilization threshold. Check that signed component VaR reconciles to portfolio VaR on the same risk basis.
2. Negate the signed P&L allocation once to obtain budget consumption. Preserve hedge credits. With portfolio VaR `−100`, components `−80`, `−40` and `+20`, and shares `50%`, `40%` and `10%`, utilization is `160%`, `100%` and `−200%`. The first position exceeds its target by `30` and breaches a `120%` threshold.
3. For a zero target, positive consumption breaches and negative consumption is a hedge with no breach. Zero consumption against zero target has undefined utilization and no breach. For positive targets, breach occurs strictly above the supplied threshold. Report the checked sum of positive excess above target separately from threshold breach status.
4. Use one checked Rust report from the explicitly selected normal or historical source. Add fractional stress detail for historical results and evaluate any requested budget from the same VaR allocation. Carry method, confidence, horizon, units, and any applied targets and threshold. [Normal position risk](position-normal-risk.md) · [Historical risk](position-historical-risk.md) · [Historical stress](tail-stress.md).

## Observed choice and rationale

Evaluate budget consumption in the portfolio-loss direction so diversifiers retain negative utilization, then optionally attach the budget and historical stress view to a position risk report.

## Input domain and conventions

Budget targets are fractions in [0,1], finite, and for nonempty portfolios sum within 0.05 of 1. They are not renormalized. Actual component and total VaR must be finite; position IDs must be unique. The breach threshold must be finite and positive; report/decomposition adapters default it to 1.20.

## Algorithm stages

1. Validate lengths, IDs, numeric domains and approximate target sum. Reject near-zero total magnitude <=1e-15 with any component magnitude >1e-15.
2. Let sign be -1 for negative portfolio VaR, otherwise +1. Consumption_i=sign*actual_i; target_i=fraction_i*abs(total).
3. If |target_i|>1e-15, utilization=consumption_i/target_i. A zero target with nonzero consumption gives signed infinity; zero/zero gives 1.
4. Excess is consumption-target. Sum only positive excess into total_overbudget; breach iff any utilization is strictly greater than threshold. Keep original signed actual values.
5. The report dispatches normal covariance or historical P&L decomposition; only historical adds stress. An optional budget then joins the computed VaR contribution rows to supplied targets.

## Selection and controls

Risk budgets are on VaR, not ES. The standalone budget accepts either consistent positive-loss or negative-P&L sign convention. Nonfinite utilization is encoded using explicit core sentinels. Historical report input is position-major P&L; the internal risk matrix is scenario-major.

## Limitations, failures, and fallbacks

The budget does not require component sums to equal portfolio VaR and does not normalize target sums. Breach threshold and positive excess are separate: an excess can exist below the utilization threshold. Zero/zero utilization of 1 can breach a user threshold below 1. Summed overbudget values have no final overflow check. The wrapper does not convert currencies or scenario horizons.

## Alternatives and tradeoffs

Exact shares allocate the entire portfolio VaR budget. Signed consumption preserves the contribution of diversifiers, and a separate alert threshold allows the caller to choose when excess becomes actionable.

## Implementation and evidence

- [src/risk/budget.rs](../../../../finstack-quant/factor/src/risk/budget.rs) — inspected implementation.
- [src/risk/report.rs](../../../../finstack-quant/factor/src/risk/report.rs) — inspected implementation.

[Budget tests](../../../../finstack-quant/factor/src/risk/budget.rs) and [report tests](../../../../finstack-quant/factor/src/risk/report.rs) passed for the current signed utilization, input validation and dispatch. Qualify exact target closure, zero-target states, threshold equality, overflow and end-to-end report conservation.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
