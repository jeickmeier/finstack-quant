# MODELS-ANALYTIC-28: Yield-change PCA, reconstruction and standardized scenarios

**Decision status:** Accepted (complete package, 2026-11-11).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and host paths inspected. Independent date, exact-rational and high-precision scalar checks were executed for the accepted distinctions. Production fit, restore and scenario qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked yield-change PCA and scenarios

1. **Use a common zero-yield input contract.** Use the
   immutable yield panel of record 27, with finite signed
   annual decimal continuously compounded zero yields
   and strictly increasing positive maturity-years.
   Require at least three level rows and two tenors.
   Form checked adjacent later-minus-earlier changes in
   original order. The data owner qualifies the same
   yield convention and curve universe across rows.
   [MODELS-ANALYTIC-27](diebold-li-var.md#accepted-decision-for-checked-nelsonsiegel-factors-and-var-forecasts)

2. **Require real maturities for supplied changes.** The
   direct input takes at least two complete finite
   decimal change rows and the actual positive ordered
   tenor grid. Require two or more tenor columns and
   exact rectangular alignment. Remove the synthetic
   maturity sequence. Preserve row order and reject
   missing cells or invalid shapes.

3. **Require one explicit observation interval.** Reuse
   record 27's dated calendar-slot, resolved business-day
   or explicitly indexed cadence. Dated levels and
   dated raw interval endpoints must form consecutive
   steps on the declared cadence. An undated series
   needs the caller's equal-step declaration. No inferred
   frequency, gap fill, day-count weighting or calendar
   annualization changes the rows or scenario scale.

4. **Preserve interval identity.** A dated fit retains
   the ordered endpoints of each change. Raw dated
   changes supply one more ordered boundary than
   change rows. Positional data retains positions
   without fabricated dates. A displayed score may
   use the right endpoint as its date label while the
   fit and view preserve the full interval.

5. **Use equally weighted sample-covariance PCA.** For
   m admitted change rows form the per-tenor mean
   `mu` and centered matrix `C`. The covariance is
   `C'*C/(m-1)`. Retain variation in the original
   yield units across tenors. The estimator uses
   the literal sample denominator, without correlation
   standardization, time decay or shrinkage.

6. **Center in a shifted and scaled representation.**
   Use each column's original first change as its
   reference and compute the checked mean offset
   and centered differences on that scale. Retain
   the reference and offset for reconstruction.
   Expose the checked rounded mean as a diagnostic.
   A required offset below the precision of the
   rounded mean can still change a reconstruction.
   [CORE-NUMERICS-09](../core-numerics/sample-statistics.md)

7. **Use one common scale for decomposition.** Scale
   the centered matrix by a common binary power so
   its largest absolute entry is in [1/2,1).
   Keep the scale as an exponent when the detached
   factor does not fit. Use one scale across all
   tenors and rows. Check required differences,
   sums, conversions and allocations, including
   exact-count and platform limits.

8. **Distinguish constant history from small dispersion.**
   If every change column is exactly constant on
   the admitted rows, return an input error for zero
   dispersion. Constant columns
   within a varying panel are valid. Remove the
   absolute trace floor. Loss of needed nonzero
   variation in computation is a numerical error.

9. **Decompose the scaled centered data directly.** Use a
   bounded thin SVD with both singular-vector matrices.
   Reuse the checked library profile with tolerance
   `5*EPSILON` and at most 1,000 internal iterations.
   Validate factors before sorting descending singular
   values with original solver order breaking exact
   ties. Remove explicit covariance formation and
   negative-eigenvalue clipping.
   [Bounded SVD profile](../analytics/14-svd-and-constrained-regression.md)

10. **Check the decomposition before accepting it.** On
    the common-scaled matrix require finite factors,
    a positive leading singular value, and orthogonality
    and normalized reconstruction checks. Use
    `tau=64*EPSILON*max(m,n)` with n tenors and
    require tau<1. Each Gram entry differs from
    the corresponding identity entry by at most tau.
    Require the Frobenius norm of the reconstruction
    error to be at most tau times the input Frobenius
    norm. Use checked norms and report failed admission
    or convergence explicitly.

11. **Retain numerically resolved components.** Let d be
    the min(m,n) returned common-scaled singular values. Keep
    the leading values strictly above
    `EPSILON*max(m,n)*d[0]`. The retained count must
    be positive and at most `min(m-1,n)` after centering.
    Keep the entire returned spectrum for variance
    accounting. Discarded positive values carry mass.
    This rule identifies resolved numerical directions.
    It does not certify exact rank or economic significance.

12. **Keep variance and standard deviation boundaries
    independent.** With original singular amplitude s
    use `sigma=s/sqrt(m-1)` and eigenvalue
    `sigma^2`. Keep singular amplitudes in scale for
    scenarios and diagnostics. Restore a requested
    absolute variance or standard deviation in its own
    units and range. A usable standard deviation can
    coexist with an unrepresentable variance.

13. **Orient each retained loading deterministically.**
    Make the loading's largest absolute coordinate
    positive. Use the earliest tenor for exact magnitude
    ties. Apply the same sign to its left singular vector
    and scores. Retain that orientation on restore.
    Remove the fixed absolute sign threshold.

14. **Scope repeated and near-repeated directions.** Keep
    the solver-selected axes in the fitted state.
    Exact ties can rotate within their eigenspace.
    Near ties can change axes substantially under small
    data changes. A prefix or shock may select a
    direction within that space on this fit. Compare
    whole subspaces across fits where individual axes
    are poorly separated. No automatic rotation,
    near-tie merging or economic factor labels.

15. **Use original-unit centered scores.** Loadings
    are the retained columns of V. Derive the scores
    from the retained `U*diag(s)`, which equals `C*V`
    in the mathematical decomposition. Keep the
    normalized left singular vectors and common scale
    as canonical state. Score rows align with
    the stored change intervals and use decimal
    yield units.

16. **Use the full returned spectrum for variance shares.**
    The denominator is the sum of the squared
    common-scaled singular values before retention.
    A component share uses its squared singular value
    over that same denominator. Report the positive
    mass of any unresolved tail. These are shares of
    the checked represented decomposition. Extremely
    small exact input variation may be below SVD
    resolution.

17. **Derive cumulative shares from spectral mass.** Compute
    each cumulative share from the prefix squared sum
    over the full denominator. Keep the denominator
    when making a view. Use scaled arithmetic and
    checked rounding for displayed fractions. A displayed
    cumulative share can round to one while a positive
    tail remains. Independent stored fraction fields
    are unnecessary.

18. **Make threshold selection exact on represented mass.**
    Require a finite threshold q in [0,1]. Exact zero
    returns zero components. Otherwise select the
    smallest available prefix whose squared mass is
    at least q times the full squared mass, using exact
    dyadic comparison on the represented normalized
    singular values and threshold. At one, a positive
    omitted tail makes the target unavailable. Report
    an unreachable threshold explicitly.

19. **Use one checked prefix domain.** A view or
    reconstruction takes an integer count from zero
    through the retained count. Zero is a mean-only
    reconstruction or a view with zero loading and
    score columns. Retain original row and tenor
    identities, mean and full-spectrum denominator.
    A prefix through a tied space refers to the
    selected basis of this fit.

20. **Reconstruct yield changes.** For prefix K use
    `mu + S_K*V_K'` for each row, restoring the
    mean once. Accumulate the centered projection with
    the stored mean offset before restoring the reference
    scale. Use the original tenor and interval order.
    Full retained reconstruction is subject to
    the discarded spectrum and numerical error.

21. **Define a one-interval scenario.** Accept finite
    signed shocks `z_k` in standard deviation units
    of this fit, in retained component order. An
    omitted suffix contributes zero. Compute
    `delta = sum_k z_k*sigma_k*v_k` without the
    sample mean or annualization. Empty and all-zero
    shocks yield the exact zero change after admission.

22. **Apply scenarios on the exact fitted maturity grid.**
    Require finite signed base yields and exact grid
    equality in fitted tenor order. Compute the
    complete `base+delta` with the checked weighted
    sum, preserving a representable shifted yield
    when an unrequested standalone delta is out of
    range. Empty and all-zero shocks preserve the
    admitted base values.

23. **Check complete output quantities.** Preserve needed
    scales through norms, products, ordered sums,
    reconstruction and scenario calculation. Reject
    unresolved range or precision, invalid index or
    shape, and nonfinite data with the appropriate
    error. A requested absolute diagnostic can fail
    independently of a valid ratio or scenario.
    Return complete results after validation.

24. **Persist the selected fit and view state.** Store
    immutable cadence, tenor and row identity, the
    centering representation, common binary scale,
    complete represented singular spectrum and
    retained oriented axes and left score directions.
    A view stores its selected prefix with enough of
    that state to reproduce its exact variance denominator.
    Derive scores, eigenvalues and variance shares
    rather than restore independent copies of them.

25. **Validate state numerically and structurally.** Apply
    the same state checks before fit or view publication
    and on restoration. Check finite shapes, count and
    cadence, the complete descending spectrum, the
    retention rule, sign anchors and the loading and
    left-vector Gram checks of point 10. For each
    retained column k require
    `abs(sum_i U[i,k]*d[k]) <= tau*sqrt(m)*d[0]`.
    Reject malformed or unknown fields, including
    incoherent views. This verifies internal consistency.
    Authenticating the historical data requires its provenance.

26. **Reconcile existing Rust, Python and WASM paths.**
    Preserve typed Python fit and view objects and
    checked WASM view data. Update real tenor and
    cadence arguments, dated DataFrame labels, state
    schemas, declarations, facade and parity. Reconcile
    the dynamic term-structure notebook and the
    existing fit, restore and scenario consumers.

27. **Keep the financial interpretation empirical.**
    The scales describe historical zero-yield changes
    for the declared observation interval. Factor
    directions need qualification on the actual curve
    universe and fit. Market scenario selection,
    dated curve construction, repricing and forward
    dynamics remain with their owners.

28. **Qualify the stated domain independently.** Cover
    dated and indexed cadence, very small and large
    scales, thin and rank-deficient data, exact and
    near ties, threshold boundaries, malformed state
    and signed scaled scenarios. Compare covariance,
    subspace projection and reconstruction against
    independent calculations. Qualify the actual
    host and empirical scenario paths.
    [PCA references](../../../REFERENCES.md#checked-yield-change-pca-and-scenarios)

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| Change rows (1,0) and (nextUp(1),0) | Exact centered values are ±2⁻⁵³ and the sample variance is 2⁻¹⁰⁵, about 2.46519e-32. The absolute trace floor rejects this varying sample. |
| Orthogonal columns with changes ±1 and ±2⁻⁵⁵ | The first variance share is `2^110/(2^110+1)`. Its displayed binary64 value is one and the positive omitted mass is about 7.70372e-34. |
| Change rows (1e-200,0) and (−1e-200,0) | The sample variance is about 2e-400 and cannot fit binary64. Its standard deviation is about 1.41421e-200 and is representable. |
| Changes (0.003,0.002), (−0.001,0.002), (0.001,0.003), (0.001,0.001) | The mean is (0.001,0.002). Leading explained mass is 4/5 of the full fit. The +1, −2 standard-deviation scenario is about (+0.00163299,−0.00163299). |
| Two equal-variance orthogonal axes and centered row (0.003,0) | Projection onto the first coordinate gives (0.003,0). Projection onto the rotated leading direction (0.6,0.8) gives (0.00108,0.00144). Both full-space reconstructions agree. |
| Base yield −MAX and mathematical scenario change 2·MAX in one component of the complete shifted vector | The shifted yield is MAX. Materializing the standalone change first loses the usable result. |

The fixtures verify exact sample arithmetic, range distinctions and
subspace behavior. They leave production solver, state and empirical
qualification open.

## Observed implementation and evidence

### Scope and input domain

Historical finite yield panels or explicit changes with≥2 tenors and≥2 changes. Explicit change-only API synthesizes tenor labels1..N instead of accepting real maturities.

### Choice and rationale

Centered sample-covariance PCA uses symmetric eigendecomposition without standardizing tenor variances. Scenarios scale component loadings by square-root eigenvalues; this is empirical change risk, not a calibrated term-structure law.

### Algorithm

Form adjacent level differences, subtract each tenor sample mean, covariance=C′C/(m−1). Reject trace<1e−30 or nonfinite. SymmetricEigen decomposes covariance; sort descending, clamp negative eigenvalues0, orient each eigenvector so its first loading>|1e−12| in magnitude is positive. Scores=centered changes×loadings; explained fractions use sum of clamped eigenvalues. Reconstruction uses first k scores/loadings then restores mean change. Scenario Δy=Σ shock_k√λk loading_k; apply_scenario adds to base yields without mean drift. components_for_threshold returns first cumulative fraction≥threshold, otherwise all.

### Selection and controls

All N components retained internally; truncated view validates1..N. Scenario can use fewer components (including empty); rejects too many/nonfinite shocks. Deserialized view checks finite aligned shapes, descending nonnegative eigenvalues and bounded monotone fractions, but not their algebraic consistency or orthonormality.

### Conventions

Input yields/changes in consistent decimal units; shocks in per-observation standard deviations. No annualization or elapsed-date weighting; arbitrary date gaps are adjacent observations. Reconstruction returns changes, not yield levels.

### Limitations, failure and fallback

All negative eigenvalues clamp without size-based error threshold. Near-equal eigenvalues leave eigenspace orientation unstable despite sign convention. Threshold accepts out-of-range/nonfinite input and can return first/all rather than error. Historical covariance is unweighted; no time decay, missing-data model or forecasting dynamics.

### Alternatives and tradeoffs

Correlation-matrix PCA or exponentially weighted covariance would change factor dominance; not available selectors here. NS/VAR is a separate implemented forecast model with different assumptions.

### Implementation and validation evidence

[pca.rs](../../../../finstack-quant/models/src/rates/dtsm/pca.rs) and [types.rs](../../../../finstack-quant/models/src/rates/dtsm/types.rs). Inline synthetic-factor, reconstruction, raw-change/panel equivalence and scenario checks are algebraic evidence; [dtsm_serde.rs](../../../../finstack-quant/models/tests/dtsm_serde.rs) checks wire state. Reference: [Litterman–Scheinkman](../../../REFERENCES.md#litterman-scheinkman-1991).

The [Python wrapper](../../../../finstack-quant-py/src/bindings/models/rates/dtsm.rs)
publishes typed PCA and view results, with positional scores
in the current DataFrame adapter. The
[WASM wrapper](../../../../finstack-quant-wasm/src/api/models/rates/dtsm.rs)
returns the view through its generated object surface. The
[schema registry](../../../../finstack-quant/models/src/schema.rs),
[Python parity fixtures](../../../../finstack-quant-py/tests/test_models_wasm_parity.py),
[WASM parity fixtures](../../../../finstack-quant-wasm/tests/facade/models_parity.test.mjs)
and [dynamic term-structure notebook](../../../../finstack-quant-py/examples/notebooks/01_foundations/market_data/dynamic_term_structure.ipynb)
carry the observed API and result conventions.

## Follow-up tracking

Implementation and validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
