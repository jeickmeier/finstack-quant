# ANALYTICS-010: Single-factor beta, Jensen alpha, fit statistics and beta uncertainty

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice, input domain and rationale

Single-factor benchmark regressions are equal-weight OLS with an intercept, estimated from online sample covariance. `Performance` first intersects each ticker's dates with its benchmark and the active window. Standalone public `beta(portfolio,benchmark)` instead pairs by position and silently uses the shorter input length; it has no date information and does not perform panel domain validation. These are distinct alignment contracts.

## Algorithm and selection

Accumulate paired means μp, μb, variances and covariance. The slope is `β=Cov(p,b)/Var(b)`; zero benchmark variance gives NaN. `beta_only` requires two observations and is used by Treynor; public `BetaResult` requires at least three because it also estimates uncertainty. Below that threshold it returns NaN beta/SE/bounds plus the actual paired count and .95 confidence.

For full beta uncertainty, form raw-return intercept `a=μp−β*μb` and residuals `e_i=p_i−a−β*b_i`. Residual sample variance uses N−1; slope standard error is `sqrt(sample_var(e)/((N−2)*sample_var(b)))`, equivalent to the OLS residual degrees-of-freedom formula. The two-sided fixed 95% interval is β ± `t_inv(.975,N−2)*SE`. If the core Student-t inverse fails, critical value is NaN; confidence is not caller-configurable.

`greeks` requires at least two pairs and returns annualized Jensen alpha `(μp−q−β*(μb−q))*A`, beta, R² and adjusted R². q is annual effective cash converted to per-observation cash. R² is squared sample correlation; either zero portfolio variance or zero benchmark variance makes it NaN. Adjusted R² is `1−(1−R²)*(N−1)/(N−2)` only when N>2 and R² is finite. Alpha is an annual arithmetic return fraction, beta/R² are dimensionless. A constant response can have estimable slope zero yet undefined R².

## Conventions, limitations and alternatives

This is historical linear exposure, not derivative price delta despite the name `greeks`. Cash affects alpha, not the raw-return slope; no time-varying cash vector is consumed. Sample observations are treated as equally spaced at frequency A even if dates are irregular. OLS intervals use a homoskedastic independent-residual model; heteroskedastic/autocorrelation-consistent covariance, bootstrap intervals and errors-in-variables corrections are not implemented. NaN results preserve per-ticker outcomes rather than failing the entire panel. `BetaResult`/`GreeksResult` explicitly serialize nonfinite fields.

Full-rank multivariate alternatives use [record 13](13-multifactor-regression.md); rolling point estimates use [record 12](12-rolling-regression.md). They do not make the single-factor uncertainty interval robust to serial dependence.

## Implementation and validation evidence

- [Benchmark kernel/result types](../../../../finstack-quant/analytics/src/benchmark.rs), [Performance alignment/callers](../../../../finstack-quant/analytics/src/performance/benchmark.rs), and [common span construction](../../../../finstack-quant/analytics/src/performance/mod.rs).
- Located inline tests cover exact linear relationships, two-versus-three observations, truncated standalone inputs, Jensen alpha, adjusted R², degenerate-field JSON roundtrips and Student-t intervals. Some interval tests invert the same core Student-t distribution and are self-consistency evidence. [Correctness regressions](../../../../finstack-quant/analytics/tests/correctness_regressions.rs) cover empty overlap/constant benchmarks; [audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) cover undefined constant-response fit statistics.
- Canonical context: [active portfolio management](../../../REFERENCES.md#grinoldKahn1999ActivePortfolio) and [matrix computations](../../../REFERENCES.md#golub-van-loan-matrix-computations). These are not a newly authenticated reference dataset or a justification of every confidence-interval assumption.

No statistical fits or test runs were executed.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
