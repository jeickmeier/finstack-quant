# MODELS-ANALYTIC-27: Nelson–Siegel factor extraction and Diebold–Li VAR forecast

**Decision status:** Accepted (complete package, 2026-11-10).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and host paths inspected. Independent date, exact-rational and high-precision scalar checks were executed for the accepted contract. Production and empirical forecasting qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked Nelson–Siegel factors and VAR forecasts

1. **Admit one complete immutable yield panel.** Each row is
   a cross section of finite signed continuously compounded
   zero yields in annual decimal units. Require at least two
   rows, a nonempty strictly increasing positive maturity
   grid in years and exact rectangular shape. Preserve
   original order. Check counts and allocations. Reject
   nonfinite or missing cells, ragged rows and unordered
   or duplicate maturities. The data owner qualifies a
   comparable curve universe and yield convention over time.
   The shared panel also serves record 28.

2. **Preserve literal observation identities.** If dated,
   require exactly one date per row, strictly increasing
   with no duplicates. Extracting independent cross sections
   does not require regular dates. An undated panel has
   positional identity only. A dated adapter must propagate
   invalid dates and preserve order. In particular, remove
   the DataFrame index-parse fallback to undated data.

3. **Require an explicit forecast cadence.** For a dated VAR
   choose a declared calendar period kind and context, with
   exactly one row per consecutive period identity, or
   choose business-day observations on an explicit resolved
   calendar, with each row at the next eligible business
   date. For calendar slots, reuse CORE-CONVENTIONS-014's
   period membership and checked stepping. Multiple dates
   in one slot, missing slots and unqualified calendar
   coverage fail. The data owner qualifies sampling within
   each period, such as its last observable market close.
   [CORE-CONVENTIONS-014](../core-conventions/014-fiscal-periods-ltm.md)

4. **Make undated VAR sampling an explicit indexed choice.**
   A caller using an undated panel declares complete
   consecutive equally indexed observations. Each adjacent
   pair is one step by that declaration. Dated inputs use
   their checked dated cadence. No inferred sampling rate,
   missing-row fill, implicit date removal or statistical
   annualization supplies a VAR clock.

5. **Require explicit fixed decay.** Lambda is finite and
   strictly positive in inverse maturity-years. Keep it
   fixed across extraction and forecast. Remove the implicit
   default and mixed month-input convention. The published
   illustration `0.0609` per month converts to `0.7308`
   per year for year maturities. Lambda affects maturity
   loadings. Observation cadence governs the VAR.

6. **Share the checked Nelson–Siegel loadings.** For maturity
   `tau` let `x=lambda*tau` and use the row `[1,g(x),h(x)]`
   with `g(x)=-expm1(-x)/x` and `h(x)=g(x)-exp(-x)`.
   Use CORE-MARKET-07's scaled loading and coefficient-aware
   evaluation, including its degree-16 Taylor profile for
   `0<=x<=1/2`. Keep the complete requested scale through
   weighted yields. A detached `x` or `1/lambda` need not
   fit when the requested quantity is representable.
   [CORE-MARKET-07](../core-market/parametric-discount.md)

7. **Use the analytic origin for standalone yields.** The
   standalone evaluator admits finite nonnegative maturities
   in supplied order, including repeated points, after full
   validation. Exact zero maturity uses `[1,1,0]` and yields
   level plus slope. A small positive maturity uses the same
   analytic law. Loading requests and weighted yields each
   have checked output boundaries. Preserve signed level,
   slope and curvature in decimal yield units.

8. **Fit each cross section with equal-tenor OLS.** Require
   at least three tenors and a numerically full-rank
   loading matrix. Fit the three factors in level, slope,
   curvature order. Factorize the common design once
   for the entire panel. Preserve each original-scale
   observed-minus-fitted residual.

9. **Use one scaled full-rank least-squares profile.** Use
   the bounded normalized SVD profile of ANALYTICS-014
   for both designs. Normalize columns by checked L2
   norms and scale each response as needed. Use one
   factorization for the multiple right-hand sides.
   Require all singular values strictly above
   `EPSILON*max(rows,columns)*sigma_max` on the normalized
   design. Use the documented bounded SVD controls.
   Restore original coefficient units. Propagate rank,
   decomposition and range errors. Remove normal equations,
   pseudoinverse truncation and silent regularization.
   [ANALYTICS-014](../analytics/14-svd-and-constrained-regression.md)

10. **Make constant-tenor R² unavailable.** For each tenor
    with positive observed centered sum of squares, report
    `1-SSR/SST`, using its actual time-series residual
    and centered observation norms in checked scale.
    Preserve negative scores. When the observed series is
    constant, its R² is absent even with zero residuals.
    Remove the dispersion floor and fabricated perfect fit.
    Keep residuals available. Report the arithmetic mean
    across all tenor scores only when all are defined.

11. **Use one affine one-step VAR law.** For extracted
    factor vector `beta_t` fit
    `beta_(t+1)=c+Phi*beta_t+eta_(t+1)` by joint
    unweighted OLS with an explicit intercept column.
    Each transition joins adjacent validated observations
    on the declared cadence. The model iterates this
    single one-step fit for every requested horizon.

12. **Require positive residual degrees of freedom.** If
    T is the observation count, fit T−1 transitions against
    four design columns `[1,beta1,beta2,beta3]`. Require
    T≥6 and full numerical rank four. Residual degrees of
    freedom are exactly T−5. Five rows can identify a
    square regression with zero residual degrees of freedom.
    Remove the denominator floor.

13. **Use the stated innovation covariance estimator.** Form
    each transition residual `e_t=beta_(t+1)-c-Phi*beta_t`
    in the original factor units. Select
    `Q=sum_t(e_t*e_t')/(T-5)`. This is a residual
    degrees-of-freedom convention. It carries no general
    finite-sample unbiasedness guarantee for fitted VAR
    dynamics. Admit singular and exactly zero innovation
    covariance.

14. **Represent innovations by a checked covariance root.**
    Use unpivoted orthogonal triangularization of the
    residual rows scaled by `1/sqrt(T-5)`, yielding a
    lower-triangular factor G with `Q=G*G'`. Use fixed
    factor order. Make the first nonzero entry of each
    root column positive and canonicalize zero entries.
    Keep the root in
    fitted state and form the symmetric covariance only
    for its checked getter. Preserve usable root scale
    even when detached squared entries are out of range.
    No positive-definiteness floor or eigenvalue repair.

15. **Require strict stability of the fitted transition.**
    Admit only transitions whose spectral radius is
    strictly below one. Include both signs of real
    unit roots and complex unit-modulus roots in
    rejection. Stable nonnormal matrices may still cause
    large transient terms, which the forecast evaluates
    with separate checked arithmetic.

16. **Decide stability on the represented 3×3 matrix.** Form
    the characteristic polynomial `p(z)=z^3+a*z^2+b*z+d`
    with `a=-trace(Phi)`, b the sum of its three principal
    second-order minors, and `d=-det(Phi)`. Use exact
    dyadic arithmetic on the finite input entries. Under
    `z=(1+s)/(1-s)`, multiply by `(1-s)^3` to obtain
    the coefficients in ascending powers of s:
    `q0=1+a+b+d`, `q1=3+a-b-3*d`,
    `q2=3-a-b+3*d` and `q3=1-a+b-d`.
    Require each q positive and `q2*q1>q3*q0`.
    This fixed-size Cayley and cubic Routh–Hurwitz test
    certifies stability of the represented coefficients,
    independently of eigenvalue rounding. Apply it on
    fit and restore without repairing Phi or introducing
    a general exact-arithmetic forecast solver.

17. **Keep the intercept canonical.** Store c as the fitted
    one-step intercept with Phi and the innovation root.
    A long-run mean is a derived diagnostic from the
    checked solve `(I-Phi)*mu=c`. Expose it under its
    actual name and meaning. Failure to represent that
    diagnostic does not determine availability of a
    finite-horizon forecast.

18. **Use a portable positive observation-step horizon.**
    Accept whole h from 1 through `u32::MAX` in Rust,
    Python and WASM. Return the terminal h-step forecast
    with the stored cadence and last observation identity
    as its origin. A dated target is expressed relative
    to that origin by h declared slots. Market snapshot
    dates and intraperiod sampling remain with the data
    owner. Maturity years in the output are unchanged
    tenor coordinates.

19. **Propagate the affine conditional mean and covariance.**
    Start at the last extracted factors with zero
    conditional forecast covariance. The law is
    `m_(j+1)=c+Phi*m_j` and
    `Sigma_(j+1)=Phi*Sigma_j*Phi'+Q`.
    Assume independent zero-mean Gaussian innovations
    with the fitted covariance for the interval calculation.
    The affine forecast does not need a stationary
    covariance or mean solve.

20. **Compose forecast blocks by binary powering.** For an
    inner block `(A1,b1,G1)` followed by outer block
    `(A2,b2,G2)`, use
    `(A2*A1,b2+A2*b1,root([A2*G1,G2]))`.
    The root operation uses unpivoted QR of the
    transpose of the concatenated columns with the
    same orientation and sign convention. Start with
    the identity block and use the horizon bits from
    least to most significant. Carry only the composed
    terminal block and needed powers. At most 32 bit
    visits are required.

21. **Project mean and standard deviation in original units.**
    At each retained maturity, use `ell*m_h` and
    `norm(G_h'*ell')` for the mean and standard
    deviation. Use the same weighted Nelson–Siegel
    evaluation for the mean and each root column, with
    checked scale through matrix products and norms.
    A positive standard deviation
    can fit even when a separately requested variance
    cannot. Remove the nonnegative-variance clamp and
    propagate unresolved range or precision failures.

22. **Keep fixed 95% marginal prediction intervals.** For
    each fitted Nelson–Siegel yield use its mean plus and
    minus the standard deviation times the standard
    normal 0.975 quantile, approximately 1.95996398454.
    These are pointwise central Gaussian intervals for
    future fitted yields given the last extracted factors
    and fixed fitted parameters.

23. **Check interval endpoints and disclose uncertainty scope.**
    Zero projected innovation gives a point interval.
    Require a representable positive width and endpoints
    strictly enclosing the returned mean when the width
    is positive. Propagate loss of range or resolution.
    The interval covers factor innovations under the
    fitted law. Curve-wide coverage, observed-yield
    residuals, current factor extraction error and
    parameter or decay uncertainty require a separate
    model and qualification.

24. **Use atomic validated fit phases.** Construct immutable
    panel, extraction and result state. Extraction
    replaces the factor history and invalidates previous
    dynamics. Fit or restore a complete state for the
    applicable phase, including compatible shapes,
    provenance, tenor order, cadence, finite quantities,
    nullable scores, root and stability. Derive redundant
    diagnostics from canonical fields. Reject malformed
    state and unknown fields. Return no partial results
    on failed fit or forecast.

25. **Keep the historical forecasting boundary explicit.**
    The selected model is an iterated one-step VAR on
    extracted Nelson–Siegel factors. Diebold and Li's
    published comparison estimated direct horizon-specific
    regressions. The result is a conditional historical
    forecast. Market discount curve construction, pricing
    measure and arbitrage restrictions belong to their
    respective owners.

26. **Reconcile existing host and result surfaces.** Route
    Rust, Python and WASM extraction and forecast through
    the same admission, cadence and error paths. Preserve
    typed Python results and checked WASM result objects.
    Update the DataFrame adapter, state, declarations,
    schemas, parity and the dynamic term-structure
    notebook with the selected intercept, derived mean
    and uncertainty meaning.

27. **Qualify numerical and dated behavior independently.**
    Cover short and long maturity loadings, signed
    yields, constant and negative R², missing slots,
    rank failures, residual degrees of freedom, stable
    and unstable complex modes, singular innovations
    and large horizons. Compare against independent
    regression, exact stability and forecast fixtures.
    Reconcile the existing dated success fixture whose
    fitted VAR is explosive.

28. **Qualify empirical use on a declared history.** Assess
    point error and marginal coverage on dated held-out
    samples without look-ahead, using the declared
    cadence and forecast horizon. Compare suitable simple
    forecast baselines on the same data and convention.
    Fitting a stable model and passing formula checks
    establish the selected calculation's contract.
    Predictive performance needs its own evidence.
    [Forecast references](../../../REFERENCES.md#checked-nelsonsiegel-factors-and-var-forecasts)

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| Month-end samples 2024-01-31, 2024-02-29 and 2024-03-31 | Calendar gaps are 29 and 31 days. Both transitions are one step on a declared monthly clock. |
| `x=2^-60` with a curvature factor of 1e18 | The curvature contribution is about 0.433680869. Forming the curvature by subtracting two rounded ones loses it. |
| Repeated rows (0.01, 0.02, 0.02, 0.01), maturities (1,2,4,8), lambda=ln(2) | Per-tenor dispersion is zero, yet exact equivalent OLS gives residuals about (−3.35418e-5, 8.82025e-5, −8.64633e-5, 3.18026e-5). All tenor R² values are absent. |
| Transition diagonal (1.2, 0.5, 0.2) | I−Phi is invertible and the transition is explosive. The exact stability check rejects it. |
| Complex pair from the represented block with diagonal 0.8 and off-diagonal −0.6, 0.6 | The exact represented squared modulus exceeds one even though the binary64 squared sum rounds to one. The test rejects it. Moving 0.8 down one float gives a stable pair. |
| Six factor rows (1,0,1), (0,1,1), (1,1,−1), (0,−1,1), (−1,−1,−1), (0,0,−1) | The exact full-rank fit has intercept (0.1,0,−0.4), a strictly stable transition and rank-one innovation covariance with one residual degree of freedom. |
| Phi rows (0.5,0.25,0), (0,−0.25,0.125), (0,0,0.75), intercept (0.01,−0.002,0.001), last factors (0.02,0.01,−0.004), innovation root column (0.001,−0.002,0.001) | Exact binary composition and direct recursion agree through 17 steps. At lambda=1 and maturity one year the one-step projected innovation is zero. At two steps the yield is 0.0192357588823, SD 0.0005932561875 and the 95% interval is [0.0180729981212, 0.0203985196435]. |

The checks cover date identities, exact rational regression and
stability, and high-precision forecast projection. Production
host behavior and empirical forecast qualification remain pending.

## Observed implementation and evidence

### Scope and input domain

Historical decimal yield panels with positive ascending tenor years, at least2 rows and3 tenors; VAR needs≥5 observations. Optional dates are length-checked but ordering/equal spacing is not enforced.

### Choice and rationale

Cross-sectional OLS extracts three Nelson–Siegel factors at fixed λ, followed by unconstrained VAR(1) with intercept. It is a historical forecast, not a risk-neutral no-arbitrage pricing model.

### Algorithm

Loadings X=[1,(1−exp(−λτ))/(λτ),(1−exp(−λτ))/(λτ)−exp(−λτ)], with small λτ limit. Solve (X′X)β=X′y by Cholesky for each row. Report per-tenor R²=1−SSR/SST; SST<1e−30 reports1 regardless SSR. Fit βt+1=c+Φβt by Cholesky normal equations with[1,βt] design. Solve μ=(I−Φ)⁻¹c by LU; singular/nonfinite mean errors. Q=residual′residual/max(nobs−4,1). Forecast βh=μ+Φ^h(βlast−μ); covariance Σh=Σj=0..h−1 Φ^j Q(Φ^j)′. Yields=Xβh; marginal95% bands ±1.96√max(XiΣhXi′,0).

### Selection and controls

Default λ=.7308 inverse years; explicit positive finite override. Factor extraction resets prior fitted dynamics. Standalone Nelson–Siegel yields allows zero tenor; panel tenors strictly positive. Serde validates finite shapes and joint fit fields but not stationarity or covariance PSD. No λ optimization or shrinkage.

### Conventions

Horizon is count of adjacent observations, not calendar years; dates are metadata. Yield units determine factor units and output bands. Forecast bands omit cross-sectional residual noise and parameter-estimation uncertainty.

### Limitations, failure and fallback

Normal equations can amplify conditioning errors; singular matrices reject. Invertible I−Φ does not imply stable VAR, so explosive forecasts are possible. Minimum5 rows can leave zero residual degrees of freedom, with denominator floored1. Constant-series R²=1 can hide a nonzero cross-sectional fitting error. Public panel fields allow invalid mutation after construction.

### Alternatives and tradeoffs

Kalman state-space DNS, stationary/shrunk VAR, λ optimization or Svensson are unimplemented alternatives. Separate PCA describes changes rather than imposing NS economic factor loadings.

### Implementation and validation evidence

[diebold_li.rs](../../../../finstack-quant/models/src/rates/dtsm/diebold_li.rs) and [types.rs](../../../../finstack-quant/models/src/rates/dtsm/types.rs). Known-NS factor recovery is synthetic formula evidence; var_mean_rejects_unit_root checks singular mean. [dtsm_serde.rs](../../../../finstack-quant/models/tests/dtsm_serde.rs) checks shape contracts. References: [Diebold–Li](../../../REFERENCES.md#diebold-li-2006), [Nelson–Siegel](../../../REFERENCES.md#nelson-siegel-1987).

The [Python bindings](../../../../finstack-quant-py/src/bindings/models/rates/dtsm.rs)
discard DataFrame dates on index-parse failure. The existing
[dated Python fixture](../../../../finstack-quant-py/tests/models/test_rates_dtsm.py)
has a fitted spectral radius about 1.241037 in an independent
regression. The [WASM binding](../../../../finstack-quant-wasm/src/api/models/rates/dtsm.rs)
documents the derived long-run mean as the intercept. The
[model schema registry](../../../../finstack-quant/models/src/schema.rs)
and [dynamic term-structure notebook](../../../../finstack-quant-py/examples/notebooks/01_foundations/market_data/dynamic_term_structure.ipynb)
carry the same fitted state and forecast concepts.

## Follow-up tracking

Implementation and validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
