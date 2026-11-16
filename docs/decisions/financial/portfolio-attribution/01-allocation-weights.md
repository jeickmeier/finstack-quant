# PORTFOLIO-ATTRIBUTION-001: Equal, fixed and inverse-volatility strategy weights

- ID: PORTFOLIO-ATTRIBUTION-001
- Decision status: Accepted 2026-11-16 for Equal, Fixed and InverseVolatility.
- Implementation status: Observed for the stated owning source scope. Runtime and host reconciliation with the accepted contract are pending.
- Validation status: Independent arithmetic examples were checked. Allocation tests were located. Runtime, host and numerical-range qualification of the accepted contract is pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked strategy allocation weights

1. Require the caller to select Equal, Fixed or InverseVolatility and supply a nonempty ordered list of sleeves with unique, nonblank IDs. Preserve the exact IDs and their order. Validate scheme-specific payloads and reject fields belonging to another scheme. Equal and Fixed require only their own inputs. [Record 002](02-risk-budget-solver.md) owns RiskBudget.
2. Equal assigns the mathematical share `1/n` to every sleeve. Use equal scores for capital apportionment even when the displayed fraction rounds. Three sleeves therefore have identical quota inputs.
3. Fixed requires one exact nonnegative decimal fraction per sleeve. Require an exact total of one, preserve exact zero shares and use the accepted fractions directly. Remove the `1e-9` sum tolerance and the separate capital normalization for this scheme. For example, `(0.5000000004, 0.5)` fails admission. On a budget of `10,000,000,000`, the first supplied fraction implies `5,000,000,004`, while dividing by the tolerated sum yields a quota near `5,000,000,002`.
4. InverseVolatility requires a common strictly increasing grid of actual valuation dates, at least two return intervals and one finite decimal simple total return per interval for every sleeve. Require declared sampling, return and cutoff conventions, including applicable currency, fees and flows. Reject missing or misaligned observations and returns below `-1`. Allow an exact `-1` only at the terminal observation under the accepted total-return convention. Use the stricter aligned-history requirement for this allocation. [Accepted analytics grid and panel admission](../analytics/01-panels-windows-and-wire.md#actual-valuation-grid-and-panel-admission).
5. Compute unannualized sample standard deviation on the supplied observation interval with denominator `n-1`, using checked directly scaled [core sample statistics](../core-numerics/sample-statistics.md). Require positive volatility for every sleeve, including a singleton. A validated singleton gets weight one. Form `q_i=min_j(sigma_j)/sigma_i` and normalize in sleeve order with checked arithmetic. Remove absolute volatility and reciprocal floors. Carry scale through the full quotient and preserve representable positive shares. Return a range error when a positive share cannot be retained. Histories `(-0.01, 0.01)` and `(-0.02, 0.02)` give shares `(2/3, 1/3)`. The same shares apply to `(-1e-310, 1e-310)` and `(-2e-310, 2e-310)`.
6. Derive displayed weights and capital quotas from the same effective share ratios. Allow signed or zero capital and run the selected scheme's validation even for zero capital. [Record 003](03-capital-apportionment.md) owns budget representation, rounding and residual assignment. InverseVolatility reports standalone sample volatility. Covariance risk contributions belong to [record 002](02-risk-budget-solver.md).
7. Use one bounded, checked Rust input and output contract for the existing Python and WASM surfaces. Preserve ordered IDs, exact authored decimals, applicable sample provenance and effective shares. Restore through checked source evidence and recompute derived fields. Identify rounded floating-point fields as presentation values.

## Observed choice and domain at the audit revision

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

Implementation and independent validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution).
