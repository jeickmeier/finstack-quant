# FACTOR-010: Signed VaR budgeting and report orchestration

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

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

Strictly normalized targets, gross-risk budgets, ES budgets and absolute component utilization would answer different policy questions; the last would incorrectly count hedges as consumers under the current signed model. A business-specific target-sum tolerance should be explicit.

## Implementation and evidence

- [src/risk/budget.rs](../../../../finstack-quant/factor/src/risk/budget.rs) — inspected implementation.
- [src/risk/report.rs](../../../../finstack-quant/factor/src/risk/report.rs) — inspected implementation.

[src/risk/budget.rs](../../../../finstack-quant/factor/src/risk/budget.rs) tests locate diversifier signs, sentinels, zero targets, bad sums/IDs and wrapper agreement. [src/risk/report.rs](../../../../finstack-quant/factor/src/risk/report.rs) contains dispatch/report-shape checks. These verify budgeting arithmetic, not adequacy of targets or the 120% threshold.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
