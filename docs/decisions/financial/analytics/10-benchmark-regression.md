# ANALYTICS-010: Single-factor beta, Jensen alpha, fit statistics and beta uncertainty

**Decision status:** Accepted for the complete single-factor regression,
alignment, field availability, fit, uncertainty and numerical/output package
below (2026-11-07). Relative metrics, rolling regression and multifactor
policies retain their owning scope.

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: single-factor benchmark regression

**Accepted:** 2026-11-07.

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

## Implementation evidence

[benchmark.rs](../../../../finstack-quant/analytics/src/benchmark.rs), [performance/benchmark.rs](../../../../finstack-quant/analytics/src/performance/benchmark.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
