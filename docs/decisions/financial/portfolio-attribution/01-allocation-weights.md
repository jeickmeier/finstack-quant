# PORTFOLIO-ATTRIBUTION-001: Equal, fixed and inverse-volatility strategy weights

- ID: PORTFOLIO-ATTRIBUTION-001
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

`allocate_weights` explicitly selects Equal, Fixed, InverseVolatility or the separate [risk-budget solver](02-risk-budget-solver.md). This record covers the first three. They allocate across nonempty, uniquely named strategy sleeves; they do not estimate instrument exposures or optimize portfolio PV. All common inputs require finite total capital and at most 12 decimal places. Capital may be negative. Optional fields unused by the selected scheme do not affect that scheme.

The implementation favors simple, reproducible weights with no shrinkage, volatility targeting or correlation estimate in the inverse-volatility branch. That is an observed design, not evidence of financial adoption.

## Algorithm and controls

1. Equal returns `w_i=1/n` in input order.
2. Fixed requires one finite, nonnegative weight per strategy and an overall sum within `1e-9` of one. It returns those accepted weights without renormalizing them. The separate capital allocator normalizes its Decimal copies, so reported weights and capital shares can differ by the accepted sum tolerance.
3. For multiple inverse-volatility sleeves, validate every supplied return as finite and require at least two per sleeve. Use the core sample variance, with denominator `n-1`, and take its square root. This is the volatility of the supplied observation interval; no annualization, date alignment, equal-history-length or return-definition check is applied here.
4. Require each resulting volatility greater than `1e-9`; set `w_i=(1/sigma_i)/sum_j(1/sigma_j)`. Report sample volatilities, weight sum and absolute-weight leverage. Risk contributions are populated only for RiskBudget.
5. For exactly one inverse-volatility sleeve, return weight one and `sample_volatility(strategy).ok()` as optional diagnostic. Missing/short histories and nonfinite returns therefore become `volatility=None` instead of errors. A valid zero-volatility history is also accepted in this branch.
6. All schemes dispatch to the [capital apportionment](03-capital-apportionment.md) stage.

## Limitations, alternatives and tradeoffs

Inverse-volatility weighting ignores correlation and sampling uncertainty. Risk budgeting addresses covariance under a more restrictive solver; minimum-variance or constrained optimization is a distinct problem. A single-sleeve portfolio has trivial weight, but swallowing diagnostic validation errors differs from the multi-sleeve invalid-data contract. No inference is made that this was an approved exception. Finite-input checks do not independently qualify very large return magnitudes or overflow in variance.

## Evidence

[Source and inline tests](../../../../finstack-quant/portfolio/src/allocation.rs) establish branch behavior, input rejection and zero-budget cases. [Allocation integration tests](../../../../finstack-quant/portfolio/tests/weight_allocation.rs) provide formula/invariant examples; these were located, not rerun. The [core sample-statistics record](../core-numerics/sample-statistics.md) owns variance numerics.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
