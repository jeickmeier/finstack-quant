# ANALYTICS-010: Single-factor beta, Jensen alpha, fit statistics and beta uncertainty

**Decision status:** Accepted for the complete single-factor regression,
alignment, field availability, fit, uncertainty and numerical/output package
below (2026-11-07). Relative metrics, rolling regression and multifactor
policies retain their owning scope.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice, input domain and rationale

Single-factor benchmark regressions are equal-weight OLS with an intercept, estimated from online sample covariance. `Performance` first intersects each ticker's dates with its benchmark and the active window. Standalone public `beta(portfolio,benchmark)` instead pairs by position and silently uses the shorter input length; it has no date information and does not perform panel domain validation. These are distinct alignment contracts.

## Accepted decision: single-factor benchmark regression

**Accepted:** 2026-11-07. **Implementation:** Partial; selected source
inspected at `827ebb75b9`. Standalone alignment/validation, two-pair beta
availability, scale-safe fit arithmetic and explicit calculation-error
propagation require reconciliation. No runtime estimator checks were executed.

This package covers single-factor estimator, alignment, field availability,
fit, uncertainty and numerical/output choices. Accepted explicit cash and
frequency conventions remain constraints; records 11–13 retain their own
relative-metric, rolling and multifactor policies.

### Model, alignment and units

Fit equal-weight OLS with an intercept, with benchmark observations `b_i` as
the explanatory variable and portfolio observations `p_i` as the response:

```text
Sxx = sum((b_i - mean(b))^2)
Sxy = sum((b_i - mean(b)) * (p_i - mean(p)))
beta = Sxy / Sxx
a = mean(p) - beta * mean(b)
e_i = p_i - a - beta * b_i
```

Retain negative and zero beta. No through-origin, weighted, shrunk or robust
estimator is substituted. The construction follows
[NIST's straight-line least-squares definition](https://www.itl.nist.gov/div898/handbook/pmd/section4/pmd431.htm).

`Performance` uses exact ticker/benchmark active-date overlap. Standalone
`beta` requires equal-length, position-paired finite arrays, with economic
alignment supplied by the caller. Reject unequal lengths and nonfinite
observations rather than shortening, dropping or filling inputs. The generic
statistical kernel does not construct investment wealth and must not impose
the nonnegative-wealth return domain on spread/factor observations. Investment
panels retain record 02's domain. For return-exposure interpretation, use
comparable decimal returns over matched periods.

Beta and R-squared require no unused cash or frequency input. Jensen alpha
uses the accepted explicit annual-effective cash rate `R` and periods-per-year
`A`, with `q = expm1(log1p(R) / A)`:

```text
annual_Jensen_alpha = A * (mean(p) - q - beta * (mean(b) - q))
```

This is arithmetic annualized excess-return intercept, not CAGR or derivative
price sensitivity. Constant cash subtraction leaves the fitted slope
unchanged. In return/return fits beta, beta SE, CI bounds and R-squared are
dimensionless; generic statistical beta/SE/bounds carry response units per
benchmark unit. Alpha is an annual decimal return. No time-varying cash input
or automatic serial-correlation correction is introduced.

### Field-specific availability and fit statistics

Preserve an available point estimate when uncertainty or another field is
undefined. Count valid aligned pairs as `N`:

| Paired history | Accepted available fields |
| --- | --- |
| `N < 2` | Estimates and fit/inference fields undefined; retain actual `n_obs`. |
| `N = 2`, varying benchmark | Beta and Jensen alpha available; R-squared available when the response varies. SE, CI and adjusted R-squared undefined. |
| `N >= 3`, varying benchmark | Point estimates and classical beta uncertainty available; R-squared fields available when the response varies. |
| Truly constant benchmark | Beta, Jensen alpha and fit/inference fields undefined because the slope is unidentified. |
| Constant response, varying benchmark | Beta zero and alpha available; R-squared fields undefined. With `N >= 3`, zero residual sum gives SE zero and CI `[0, 0]`. |
| Exact nonconstant affine fit | R-squared one; with `N >= 3`, adjusted R-squared one, SE zero and CI collapsed at beta. |

For nonconstant response and identifiable slope, use centered
`R2 = 1 - SSE / SST`, mathematically equivalent to squared Pearson correlation,
where `SSE = sum(e_i^2)` and `SST = sum((p_i - mean(p))^2)`. A perfect inverse
relationship also has R-squared one; the statistic does not preserve beta's
sign. For `N > 2`, adjusted R-squared is
`1 - (1 - R2) * (N - 1) / (N - 2)`. Retain legitimate negative adjusted values.
Do not replace a constant-response undefined R-squared with zero or one.
[R's linear-model summary](https://stat.ethz.ch/R-manual/R-devel/library/stats/html/summary.lm.html)
documents the centered residual/total sums and residual degrees of freedom.

### Beta uncertainty and interpretation

For `N >= 3` and positive benchmark variation, use:

```text
SE(beta) = sqrt(SSE / ((N - 2) * Sxx))
CI95 = beta +/- t_inv(0.975, N - 2) * SE(beta)
```

Retain a clearly labelled fixed two-sided 95% classical interval and report
`confidence_level = 0.95`, even when bounds are unavailable. No confidence
parameter is introduced. This is an explicit reporting-method choice,
separate from tail-risk confidence, not a universal inference requirement.
Do not annualize beta uncertainty or replace the small-sample Student-t
critical value with a fixed normal critical value.

Computing descriptive OLS point estimates does not require normal residuals.
The usual exact finite-sample Student-t coverage assumes the specified
conditional linear mean and independent, mean-zero, Gaussian errors with
common positive conditional variance. These assumptions are not established
merely by returning an interval. Do not automatically substitute HAC,
bootstrap or other inference. A collapsed sample interval does not establish
future certainty. The construction and model conditions are described in
[Frees' regression text, chapter 2](https://users.ssc.wisc.edu/~ewfrees/ActuarialRegression2009/C2BasicLR.html).

### Numerical, request and output rules

- Validate aligned finite observations and required cash/frequency inputs
  before applying insufficient-history or degenerate-field outcomes.
  Preserve actual `n_obs`, available fields and unavailable-field sentinels
  consistently across standalone/panel calculations, hosts and serialization.
- Distinguish a truly constant benchmark/response and exact zero residuals
  from numerical collapse. Add no small-variance, beta-magnitude or residual
  floor. Nonconstant observations must not become an unidentified or perfect
  fit solely because squared intermediates underflow or cancellation remains.
- Use adequately scaled centered calculations. In particular, correlation
  normalization must not overflow/underflow by multiplying two variances
  before taking a square root. Recompute the same estimator when needed;
  no failed computation may become a plausible zero R-squared or clipped
  endpoint value. Unresolved impossible R-squared/risk states are calculation
  errors, while legitimate negative adjusted R-squared remains available.
- Nonfinite required moments, coefficients, residual calculations, SE/bounds,
  annualized alpha or failed Student-t evaluation are explicit calculation
  errors, not unavailable-history NaNs or substituted Gaussian intervals.
  Errors fail the affected metric call; ordinary insufficient/degenerate
  per-ticker estimates remain field-wise undefined. Whole-summary failure
  behavior remains outside this record.
- Keep canonical result field meanings and confidence/count metadata.
  Preserve finite zero and unavailable fields through the existing nonfinite
  adapters. This package adds no residual-SD output or new estimator family.

For benchmark `[-1%, 0%, +1%]` and portfolio `[-1%, +1%, +3%]`, beta is two,
raw intercept is 1%, R-squared is one and the three-pair CI collapses at two.
At explicit zero annual cash and monthly frequency, annual Jensen alpha is
12%. Keeping only the first and last pairs preserves beta two while SE/CI
and adjusted R-squared are unavailable. These illustrative values do not
qualify implementation accuracy. Independent validation must cover exact and
noisy fits, negative beta, constant inputs, small samples, tiny/high-scale
variation, residual cancellation, Student-t values and host/wire behavior.
Reconciliation and independent qualification are owned by the
[record-10 follow-up](../FOLLOW_UPS.md#analytics-single-factor-regression).

## Algorithm and selection

Accumulate paired means μp, μb, variances and covariance. The slope is `β=Cov(p,b)/Var(b)`; zero benchmark variance gives NaN. `beta_only` requires two observations and is used by Treynor; public `BetaResult` requires at least three because it also estimates uncertainty. Below that threshold it returns NaN beta/SE/bounds plus the actual paired count and .95 confidence.

For full beta uncertainty, form raw-return intercept `a=μp−β*μb` and residuals `e_i=p_i−a−β*b_i`. Residual sample variance uses N−1; slope standard error is `sqrt(sample_var(e)/((N−2)*sample_var(b)))`, equivalent to the OLS residual degrees-of-freedom formula. The two-sided fixed 95% interval is β ± `t_inv(.975,N−2)*SE`. If the core Student-t inverse fails, critical value is NaN; confidence is not caller-configurable.

`greeks` requires at least two pairs and returns annualized Jensen alpha `(μp−q−β*(μb−q))*A`, beta, R² and adjusted R². q is annual effective cash converted to per-observation cash. R² is squared sample correlation; either zero portfolio variance or zero benchmark variance makes it NaN. Adjusted R² is `1−(1−R²)*(N−1)/(N−2)` only when N>2 and R² is finite. Alpha is an annual arithmetic return fraction, beta/R² are dimensionless. A constant response can have estimable slope zero yet undefined R².

The current shorter-prefix pairing, all-fields-undefined two-pair beta,
Student-t failure-to-NaN branch and unqualified covariance/correlation
arithmetic require reconciliation with the accepted package. Expected
unavailable fields must remain distinct from failed numerical calculations.

## Conventions, limitations and alternatives

This is historical linear exposure, not derivative price delta despite the name `greeks`. Cash affects alpha, not the raw-return slope; no time-varying cash vector is consumed. Sample observations are treated as equally spaced at frequency A even if dates are irregular. The accepted classical intervals retain their stated model conditions; heteroskedastic/autocorrelation-consistent covariance, bootstrap intervals and errors-in-variables corrections are not selected. Expected unavailable fields preserve per-ticker outcomes, while request/calculation errors fail the metric call. `BetaResult`/`GreeksResult` explicitly serialize nonfinite fields.

Full-rank multivariate alternatives use [record 13](13-multifactor-regression.md); rolling point estimates use [record 12](12-rolling-regression.md). They do not make the single-factor uncertainty interval robust to serial dependence.

## Implementation and validation evidence

- [Benchmark kernel/result types](../../../../finstack-quant/analytics/src/benchmark.rs), [Performance alignment/callers](../../../../finstack-quant/analytics/src/performance/benchmark.rs), and [common span construction](../../../../finstack-quant/analytics/src/performance/mod.rs).
- Located inline tests cover exact linear relationships, two-versus-three observations, truncated standalone inputs, Jensen alpha, adjusted R², degenerate-field JSON roundtrips and Student-t intervals. Some interval tests invert the same core Student-t distribution and are self-consistency evidence. [Correctness regressions](../../../../finstack-quant/analytics/tests/correctness_regressions.rs) cover empty overlap/constant benchmarks; [audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) cover undefined constant-response fit statistics.
- Canonical context: [active portfolio management](../../../REFERENCES.md#grinoldKahn1999ActivePortfolio) and [matrix computations](../../../REFERENCES.md#golub-van-loan-matrix-computations). These are not a newly authenticated reference dataset or a justification of every confidence-interval assumption.

No statistical fits or test runs were executed.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
