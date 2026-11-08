# ANALYTICS-013: Multifactor OLS, return-kind conversion and residual risk

**Decision status:** Accepted for the complete multifactor-regression package below (2026-11-07).

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: multifactor regression

**Accepted:** 2026-11-07.

This package covers the complete record-13 statistical model,
public input ownership, return interpretation, availability, output and
numerical-failure contract. Existing explicit frequency/cash and investment-
history decisions remain constraints. Record 14 separately owns the shared
solver's exact normalization/rank controls and constrained-regression method.

### Statistical inputs and investment-panel boundary

- Expose the existing raw-array `multi_factor_greeks` calculation as the
  canonical statistical function in Rust/Python/WASM, using one Rust estimator.
  It accepts a response vector, factor-major aligned vectors, explicit
  periods-per-year `A` and an explicit `ReturnKind`. Remove the implicit
  `Excess` default. This makes finite already-excess responses available
  without constructing investment wealth from them.
- Keep `Performance.multi_factor_greeks` as a thin active-investment-panel
  adapter to that same function. Its stored investment returns are total
  returns: require explicit annual-effective cash and delegate the `Total`
  response case. Remove the panel method's excess/total reinterpretation
  switch. Already-excess raw observations belong in the statistical function,
  without a new panel mode, synthetic wealth or alternate implementation.
- Require at least one factor and exactly `N` finite values in every column.
  Preserve factor input order in the loadings. The raw API pairs by position;
  callers own exact period/date alignment. The panel adapter uses only the
  selected ticker's active span, not benchmark overlap; callers must slice
  every factor to those same observations. Reject mismatches/nonfinite values,
  rather than truncate, drop, fill or automatically join data.
- Factor columns are already prepared explanatory observations. For financial
  return-factor interpretation, supply matching decimal excess asset returns
  or zero-investment/long-short return differences as intended by the model.
  Do not subtract cash from every factor, compound factor wealth, enforce an
  investment-return lower bound on factors, or silently standardize units.
  Other finite numeric predictors retain their stated units and statistical
  interpretation; beta units are response units per predictor unit.
- Validate raw `Total` histories and panel investment histories under record
  02 before conversion or active selection. A valid terminal `-1` is included
  normally; no percentage continuation is permitted. Raw `Excess` responses
  and converted excess responses need only be finite statistical observations
  and may be below `-1`; they are never validated or compounded as wealth.
  The raw statistical API provides this unrestricted excess-response domain.

Caller responsibilities include factor identity/construction, timestamps and
observation intervals, decimal-versus-percent units, currencies, cash basis,
fee treatment and intended exposure interpretation. No currency conversion,
factor construction, hedging or empirical data-quality certification is
introduced. Frequency is explicit, not inferred from dates. Validate inputs
and required parameters before model/sample availability decisions.

### Explicit response kind and cash conversion

For the raw statistical function:

| Explicit response kind | Accepted interpretation |
| --- | --- |
| `Excess` | Use supplied `y` unchanged. No cash argument, dummy zero or second subtraction. |
| `Total { risk_free_rate: R }` | Require finite annual-effective decimal `R > -1`, including deliberately supplied zero. Set `q = expm1(log1p(R) / A)` and `y_i = total_i - q`. |

Require finite positive `A`. Omitted/unknown kinds, omitted required cash and
cash supplied to `Excess` are invalid requests. Replace the unused-zero label
helper contract rather than preserving it as a compatibility path. Only the
response is converted; factors remain exactly as supplied.

The intercept is Jensen-style only for the intended excess-factor model.
To recover record 10's one-market-factor Jensen alpha, supply factor
`b_i - q` and response `p_i - q`; supplying raw total benchmark `b_i` instead
defines a different intercept. With fixed factors and frequency, subtracting
constant `q` from the response leaves betas, residuals and fit statistics
unchanged and reduces annual alpha by `A*q`.

The constant annual-effective input `R` implies a constant per-observation
cash rate `q` at the declared frequency.
For a time-varying cash series, callers prepare the excess response and
intended factor series themselves, then choose `Excess`. The library does not
claim that a single annual rate reproduces a published time-varying RF series.

### OLS model, identification and field availability

Fit equal-weight OLS with one automatically inserted intercept:

```text
y_i = a + sum(beta_j * f_ji, j = 1..k) + e_i
X = [ones, factor_1, ..., factor_k]
(a, betas) = argmin sum(e_i^2)
alpha = A * a
nu = N - k - 1
SSE = sum(e_i^2)
SST = sum((y_i - mean(y))^2)
R2 = 1 - SSE / SST
adjusted_R2 = 1 - (1 - R2) * (N - 1) / nu
residual_vol = sqrt(SSE / nu) * sqrt(A)
```

Require `k >= 1`, `N >= k + 1` and full rank of the intercept-inclusive
design under the shared solver's numerical rank contract. A user-supplied
intercept/constant factor or dependent columns make the design deficient.
Reject underdetermined/deficient models explicitly, including constant or
duplicate factors, rather than drop columns, regularize, return arbitrary
loadings or use a rank-truncated minimum-norm fallback. Exact SVD cutoff
policy is separately accepted in [record 14](14-svd-and-constrained-regression.md#accepted-decision-normalized-svd-and-constrained-regression);
its implementation and independent qualification retain that owning scope.

| Identified full-rank model | Accepted available fields |
| --- | --- |
| `N = k + 1` | Finite alpha/betas and actual `n_obs`. Nonconstant-response R-squared is one for the saturated fit; adjusted R-squared and residual volatility are undefined because `nu = 0`. |
| `N >= k + 2`, response varies | Alpha/betas, R-squared, adjusted R-squared and residual volatility, using positive residual degrees of freedom. |
| Constant response `y_i = c` | All slopes zero, alpha `A*c`, both R-squared fields undefined. Residual volatility is zero only when `nu > 0`, otherwise undefined. |
| Exact nonconstant fit | R-squared one; with `nu > 0`, adjusted R-squared one and residual volatility zero. A saturated fit does not establish zero residual risk. |

Do not reject identified coefficients solely because residual-risk estimation
lacks degrees of freedom. An underdetermined or unsupported model is an error,
distinct from an identified model with specific unavailable statistics.

For factors `[-1%, 0%, +1%]` and `[+1%, -2%, +1%]`, excess response
`[-1.5%, +2%, +2.5%]`, and `A = 12`, the three-observation fit has betas
`[2, -0.5]` and annual alpha `12%`. Its R-squared is one, while adjusted
R-squared and residual volatility are undefined (`nu = 0`).

### Units, fit interpretation and invariance

Alpha is the annual arithmetic excess-response intercept in decimal-return
units. Betas are not annualized; return-on-return loadings are dimensionless.
Residual volatility is the annualized, residual-df-adjusted in-sample error
dispersion, not `sqrt(SSE/N)`, active-return tracking error, a causal residual
attribution or a guaranteed out-of-sample risk forecast. Preserve conventional
unadjusted `sqrt(A)` scaling without automatic serial-dependence correction.
The usual error-variance interpretation requires an appropriate linear mean
model and mean-zero uncorrelated errors with common finite variance; numerical
point estimation itself does not require Gaussian errors.

For the identified intercept OLS model, R-squared lies in `[0, 1]` for varying
response; preserve legitimate negative adjusted R-squared. A constant-response
undefined fit is not zero or one. No clipping, epsilon floor or automatic
coefficient significance filter is introduced. No coefficient SEs, confidence
intervals, p-values or confidence input are added.

Nonzero factor rescaling or invertible linear mixing of the same factor span
preserves fitted values, residuals, R-squared and intercept, with loadings
transformed back to the stated units/order. Shifting/centering factors can
change the intercept while preserving the fit; changing the explanatory span
or nonlinear transformations can also change fit. Numerical column scaling
must not change economic estimates. These identities do not guarantee that
every ill-conditioned reparameterization passes the numerical rank gate.
Exposure remains conditional on the selected factors, not causal attribution,
a prescribed Fama–French model or an automatically tradable hedge.

### Numerical evaluation, errors and result carrier

- Retain normalized full-rank SVD and scaled response/residual moment
  evaluation. Use adequate target scaling and stable fitted/residual dot
  products as needed, without explicitly forming normal equations. Preserve
  representable requested results when unscaled intermediate coefficients,
  products, fitted values or squared sums would overflow or underflow.
  Shared solver implementation details remain with record 14.
- Distinguish genuine constant response, exact zero residuals and zero
  coefficients from arithmetic collapse. Tiny positive response dispersion
  or residual risk must not become false undefined fits or perfect fits.
  For `Total`, preserve the constant-cash translation identity: solving the
  original response and shifting only its intercept is equivalent to fitting
  `total - q`. Materializing that subtraction must not erase small response
  variation, betas or residuals through rounding against a large cash level.
  Nonfinite or invalid computed fit statistics require adequate scaled
  same-estimator evaluation, then explicit failure if unresolved; do not
  substitute a new estimator or silently repair values.
- Invalid shape/domain/kind/cash/frequency and unsupported identification are
  explicit request/model errors. Numerical decomposition failure or an
  unrepresentable requested estimate is an explicit calculation error failing
  the metric call. Successful identified fits retain field-wise expected
  unavailability. Intentional infinite coefficients or residual volatility are
  not valid outputs; do not return a partially repaired result.
- Keep `MultiFactorResult` fields `alpha`, `betas`, `r_squared`,
  `adjusted_r_squared`, `residual_vol` and `n_obs`. Preserve factor order and
  actual `N`; derive `k` from beta count and `nu` from `N - k - 1`.
  Retain existing host field naming/conversion conventions. Add residual
  volatility to the existing nonfinite wire metadata/adapter/schema so its
  zero-df undefined value survives Rust/Python/WASM and JSON as `"nan"`.
- On restoration, require nonempty finite betas, finite alpha, integer
  `n_obs >= k + 1`, and nonnegative finite residual volatility for `nu > 0`;
  require undefined residual volatility and adjusted R-squared for `nu = 0`.
  R-squared is finite in `[0, 1]` or undefined. With `nu > 0`, both fit fields
  are finite or both undefined; with `nu = 0`, available R-squared requires
  the saturated-fit value one within independently qualified roundoff
  tolerances, and undefined adjusted R-squared. Reject
  infinities, nulls, unknown fields, unknown sentinel tokens and incompatible
  availability states without repair. An available adjusted R-squared must
  agree with stored R-squared and the count-derived formula to independently
  qualified roundoff tolerances; do not repair inconsistent stored fields. A result
  carrier cannot certify source alignment, factor rank or coefficient values
  without source data. Host factor names are labels in beta order, not factor
  identity inferred by the estimator.

Independent qualification must cover saturated fits, constant/zero responses,
negative adjusted fit, cash/one-factor identities, scaled target/dot-product
overflow, tiny residuals, factor reparameterization and host/wire availability.
Synthetic recovery and production-code self-comparison do not independently
certify statistical accuracy or economic factor provenance. Weighted, robust,
regularized, Bayesian, PCA, rolling multifactor and inference variants remain
outside this package; the separate factor crate retains its own policies.

The [NIST least-squares definition](https://www.itl.nist.gov/div898/handbook/pmd/section4/pmd431.htm)
supports the fitting objective. [R's linear-model summary](https://stat.ethz.ch/R-manual/R-devel/library/stats/html/summary.lm.html)
documents centered R-squared and residual degrees of freedom; its [time-series warning](https://stat.ethz.ch/R-manual/R-devel/library/stats/html/lm.html)
supports explicit prealignment, without adopting its missing-data or singular-
fit defaults. [French's factor definitions](https://mba.tuck.dartmouth.edu/pages/faculty/ken.french/Data_Library/f-f_factors.html)
distinguish market excess return from SMB/HML long-short differences. These
references do not validate this implementation's rank cutoff or numerical
accuracy, or prescribe a factor dataset.

## Implementation evidence

[multi_factor.rs](../../../../finstack-quant/analytics/src/multi_factor.rs), [performance/benchmark.rs](../../../../finstack-quant/analytics/src/performance/benchmark.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
