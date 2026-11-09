# CORE-NUMERICS-11: Ledoit–Wolf covariance shrinkage toward a scaled identity

- Decision status: **Accepted — complete scaled-identity covariance-shrinkage package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent exact-rational reference examples and read-only formula/host/caller reviews were completed on 2026-11-08; these do not certify the production implementation or risk model.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

This record owns ledoit_wolf_shrinkage in math/linalg/mod.rs: centered finite
T-by-N observations, population-normalized covariance and the Ledoit–Wolf
scaled-identity target/intensity/result. [Accepted record 09](sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty)
owns shared shifted/scaled centered calculations and checked aggregation;
[record 07](matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission)
owns downstream factorization/numerical admission. Core does not select a
portfolio/factor risk model, return transform, annualization or missing-row policy.

## Accepted decision: checked scaled-identity covariance shrinkage

Accepted on **2026-11-08** after whole-record review. Runtime,
caller/host reconciliation and independent numerical/statistical/financial
qualification remain pending; observed source behavior is retained separately below.

1. **Retain one explicit finite rectangular matrix-estimator domain.** Require
   T>=2 observations and N>=1 variables, row-major with exactly T*N finite entries.
   Validate checked shapes, dimensions, all data and resources before special-case
   returns. T<2 or N=0 is an input/domain error, not an empty covariance, scalar
   availability sentinel or partial estimate; this is an explicit model-input
   domain distinct from record 09 scalar availability. No T>=N, full-rank or
   statistical-sufficiency requirement is inferred. Keep literal row/column order;
   no pairwise deletion, alignment, weighting, row imputation or variable selection.

2. **Select a fixed centered population-covariance profile and target.**
   Estimate each column mean from these same T observations, always demean, and
   define S=(1/T)*sum_t(x_t*x_tᵀ), where x_t is the centered row. Use divisor T,
   not T-1; no ddof or assume-centered flag. Let mu=trace(S)/N and target F=mu*I.
   This is covariance shrinkage in the supplied coordinate units, not correlation
   shrinkage or automatic independent standardization. Numerical per-axis scaling
   may preserve information internally but must evaluate the estimator in the
   original relative column units. Owners choose economically comparable units.

3. **Fix the full Frobenius intensity estimator.** Let
   d²=(1/N)*sum_ij(S_ij-mu*I_ij)² and
   bbar²=(1/(N*T²))*sum_t sum_ij(x_ti*x_tj-S_ij)².
   Set b²=min(bbar²,d²) and, when d²>0, delta=b²/d²;
   Sigma=delta*mu*I+(1-delta)*S. Count both off-diagonal halves in those norms.
   Retain the nonnegative deviation form, not a cancellation-prone subtraction
   of large fourth-moment totals. No target selector, manual shrinkage override,
   OAS/robust/nonlinear alternative, minimum-ridge knob or model registry.

4. **Define genuine degeneracies and the estimator cap separately from errors.**
   A genuine d²=0 means S=mu*I and all coefficient choices give the same matrix;
   return S with canonical delta0. N=1 returns its population variance with that
   convention; all-constant input returns zero covariance. The intensity is not
   uniquely identified in that tie.
   If d²>0 and bbar²=0, return delta0/sample S; if bbar²>=d², return delta1/target.
   The upper min is part of the statistical estimator, not arbitrary numerical
   repair. Do not classify underflowed squared distance as isotropy, add an epsilon
   tie, clamp a negative/nonfinite moment or silently raise delta to enable solving.
   Some constant columns can acquire positive diagonal variance from the target;
   do not drop them or force their returned diagonal back to0.

5. **Center in private shifted/scaled coordinates before moment evaluation.**
   Reuse accepted09-quality centered means/compensation, retaining local mean
   information instead of restoring a rounded global mean and subtracting it.
   Adjacent values around a large offset remain genuinely distinct. Preserve
   exact constant-column evidence and mathematical translation behavior without
   a public centering/state/cache framework. Do not normalize raw observations
   by one large constant column before centering, or infer that ordinary rounded
   subtraction correctly resolved a small centered spread. An unresolved required
   numerical calculation errors; it is not a zero-variance or isotropy convention.

6. **Preserve original-unit second/fourth information across mixed scales.**
   Use checked compensated/scaled products, per-entry/column exponents where needed
   and stable nonnegative norms; one materialized common-normalized array is not
   sufficient if it erases a varying column or cross moment. Independent numerical
   scales do not change the model target or introduce column standardization.
   Do not require raw centered values, S entries, trace(S), mu, d², bbar² or scale²
   to be representable when the requested final covariance/intensity can be.
   Restore means/entries/weights with divisors and exponents combined, avoiding
   premature overflow/zero and false constant/covariance classifications. No
   arbitrary precision or promise that every finite panel succeeds is selected.

7. **Evaluate intensity and its complementary weight without false branches.**
   Equivalently let D=||S-mu*I||F and
   B=sqrt(sum_t||x_t*x_tᵀ-S||F²)/T, omitting their common 1/sqrt(N), which cancels.
   Compare their scaled representations: genuine D0 gives the tie above;
   B>=D gives delta1; otherwise delta=(B/D)². Do not materialize T² or tiny/huge
   squared norms just to divide them. Preserve the complementary sample weight
   w=(d²-bbar²)/d² on the interior branch with stable residual/difference arithmetic,
   rather than 1-rounded_delta. Genuine cap has w0; the tie has delta0,w1.
   Both returned fields are rounded descriptions of the same mathematical
   estimator; a reported delta can round to1 while a small sample contribution
   still matters after restoring units. Users cannot reconstruct covariance
   bitwise by subtracting the reported binary64 intensity from1. Detectably lost
   required positive intensity/covariance output or unresolved branch arithmetic
   is computation/range failure, not a no-shrink/full-target fallback.

8. **Keep a simple complete result and explicit numerical postconditions.**
   Retain `Result<LedoitWolfResult>` with covariance and shrinkage fields, no Option,
   result class hierarchy, new constructors/serde restore or repair report. On Ok,
   covariance has exactly N*N finite entries, exact numerical mirrored symmetry
   and nonnegative diagonal; shrinkage is finite in[0,1]. Compute each mirrored
   entry once; preserve signed covariances and genuine zero, with zero optionally
   normalized to+0. Only requested intensity and the full final covariance need
   finite representation. Negative computed diagonal, invalid intensity or failed
   restoration is an error, not clipping. A rounded matrix is not certified
   exactly PSD or guaranteed to pass every downstream record07 tolerance.

9. **Separate mathematical PSD/conditioning from finite output and suitability.**
   In real arithmetic S and mu*I are PSD, so the defined convex estimator is PSD.
   It is positive definite when delta*mu>0, but need not be otherwise: two centered
   rows v,-v have identical outer products, estimated delta0 and rank<=1 S.
   Tiny positive shrinkage supplies no guarantee of adequate conditioning or
   binary64 invertibility. Trace is preserved and eigenvalues move toward mu in the ideal
   formula, not as a bitwise reconstruction/trace certificate. Do not add a generic
   O(N³) PSD/eigen/factor post-gate to this O(T*N²) estimator or hidden jitter,
   diagonal loading, nearest-correlation repair, projection or inverse fallback.
   Owners select/qualify subsequent factorization, conditioning and risk acceptance.

10. **State invariance and statistical claims within their actual limits.**
    Adding column constants, uniformly multiplying all columns by a nonzero c,
    and relabelling columns have the corresponding mathematical translation,
    c²-covariance and permutation effects; intensity is invariant under common
    scaling. Independent column scaling generally changes mu*I and the intensity.
    These are real-arithmetic properties, not all-range/bitwise promises or unit
    standardization. Replace unconditional "optimal" with the estimated intensity
    for this selected profile. The 2004 paper's quadratic-risk optimality is
    asymptotic in a specified linear-shrinkage class under IID/moment/dimension
    conditions; finite observations alone do not establish them. Estimated
    demeaning is the chosen practical plug-in and proves no finite-sample oracle,
    universal forecast/risk improvement or unbiased population covariance result.

11. **Check exact-count/resource admission and preserve failure categories.**
    Bound both integer T,N by min(usize::MAX,2^53) for exact denominator conversion,
    while ordinary floating ratios still round. Check T*N, N*N, workspace byte/
    capacity and work-counter arithmetic before allocation; use fallible boundaries
    where available without claiming recovery from all dependency/global OOM.
    Keep a fixed serial observation/column fold with compensated arithmetic;
    no thread/block preset or different parallel-reduction policy. Bad shape,
    unsupported counts/dimensions and nonfinite data are input/domain failures.
    Required centering, moment/norm/complement, range/restoration and output
    failures are computation errors with stage and available row/column context.
    No NaN passed to min/max, fabricated matrix, partial covariance or all-errors-
    as-validation adapter. Inputs are immutable; errors return no successful result.

12. **Reconcile existing host/caller shapes without new wrappers.** Keep Rust
    canonical name and fields; Python accepts logical rectangular two-dimensional
    data (including NumPy strides) and returns its existing nested covariance/
    intensity pair. WASM keeps flat row-major input with explicit T,N and flat
    typed-array/intensity pair. No new standalone result class or optional/default
    target, weights, annualization or host-only calculation. Correct rectangular
    input docs/messages and "optimal"/range claims; update stubs, declarations,
    examples and parity with canonical runtime/error changes. Map domain failures
    to Python ValueError/domain and calculation failures to RuntimeError, with
    corresponding centralized WASM categories. Covariance values remain in
    per-observation input product units; there is no hidden annualization.

13. **Keep credit/portfolio model policy and diagnostics with their owners.**
    The actual credit adapter selects complete cases, annualizes once and derives
    correlation from the returned covariance. Reconcile its scale-safe diagonal
    normalization and computation error propagation; never form overflowing/
    underflowing variance products or hide bad ratios through a blanket clamp.
    Zero-variance correlation conventions, sample selection, factor universe,
    ordering, panel frequency/A, suitability and conditioning/forecast criteria
    remain owner decisions. Do not adopt the separate FullSampleRepaired default
    or enable LW as a fallback on failed factorization. Existing calibration
    artifacts retain effective estimator/sample-count/ordering/intensity and
    relevant input/output provenance where needed, without a new generic report
    framework. Formula/reference/scale fixtures do not qualify production
    numerical range, out-of-sample performance or a financial risk model.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| T<2,N=0, wrong/overflowing shape, nonfinite entry or invalid count domain | Input/domain/resource error; no empty/partial matrix or missing-statistic sentinel. |
| N=1 with valid sample | Population variance and canonical intensity0; target and sample coincide. |
| All columns genuinely constant | Zero covariance/intensity0, without a ridge. |
| One constant column, other values(1,-1,2,-2) | Intensity9/50; diagonal(9/40,91/40): the target deliberately fills the constant direction. |
| Rows(1,1),(-1,-1),(2,-2),(-2,2) | Intensity17/18; diagonal5/2, off-diagonal-1/12. |
| Same panel uniformly multiplied by2 | Same intensity and four times covariance, subject to representation. |
| First column alone multiplied by2 | Intensity353/738, not17/18; no independent unit invariance. |
| Two rows v,-v | Outer products agree; intensity0 and possibly singular sample covariance. |
| One-column values[1e16,1e16+2] | Population covariance1, not2 from rounded-global-mean subtraction. |
| Two rows+/- (2^500,2^-500) | Intensity0 and covariance[[2^1000,1],[1,2^-1000]]; no erased small diagonal or invalid zero-based substitute. |
| epsilon=2^-600, rows(1,0),(-1,0),(epsilon,1),(-epsilon,-1) | Positive target distance must not underflow into a tie; the cap selects intensity1. |
| Rows(a,0),(-a,0),(0,0),(0,0), a=1.9e154 | Intensity1/2 and finite covariance diag(.75V,.25V), V=a²/2, despite unrepresentable unshrunk V. |
| Rounded reported intensity1 but consequential positive sample complement | Preserve the complement in covariance construction; no bitwise reconstruction claim from1-reported_delta. |
| Required restored covariance/intensity unrepresentable or negative/nonfinite output | Computation/range error, no floor/clip/repair or successful NaN. |
| Positive intensity with small eigen-direction | No automatic conditioning, exact rounded-PSD or invertibility certificate. |
| Credit adapter converting covariance to correlation | Use scale-safe ratios and propagate failures; owner chooses constant-factor policy and financial acceptance. |

## Observed choice and rationale

The explicit estimator shrinks a demeaned population-normalized sample covariance toward its mean diagonal. Common scaling before second/fourth moments mitigates some overflow; uniform-scaling invariance is a real-arithmetic property, not an all-range implementation guarantee. Finite normalization can erase small components. Identity-target suitability is an adoption choice; heterogeneous units are not standardized independently.

## Observed algorithm

1. For each column, divide observations by that column's maximum absolute value, compute a Neumaier mean, restore the scale, and subtract the mean. Reject nonfinite centered observations.
2. Divide all centered values by one common maximum absolute centered value. The common scale intends to retain relative column units rather than standardize them; finite normalization and subsequent products can lose small variances or cross moments.
3. Compute `S=XᵀX/T`, `mu=trace(S)/N`, `d²=Σ_ij(S_ij−mu I_ij)²/N`, and `bbar²=Σ_t Σ_ij(x_ti x_tj−S_ij)²/(N T²)`.
4. Set `b²=min(bbar²,d²)`, `delta=b²/d²` if d²>0 else 0. Form `Σ*=delta mu I+(1−delta)S`.
5. Restore each entry through `(normalized*scale)*scale`, avoiding a separately overflowing scale². Reject nonfinite restored covariance. Return covariance and delta.

## Observed selection, controls and conventions

This is a direct API, not a fallback on failed Cholesky. All arithmetic and folds are serial; there is no configurable target, shrinkage override or degrees-of-freedom adjustment. S divides by T, unlike the sample covariance helper's T−1. Row order and column order are caller-supplied.

## Observed limitations, failure and fallback

Shape multiplication is checked; all observations must be finite. All-constant data yields zero covariance/delta=0. Computed d²=0 returns S without establishing that the mathematical distance is zero. A PSD estimate is not necessarily strictly positive definite (for example all-zero data or centered two-row rank-one data); no condition-number guarantee or confidence interval is returned. No matrix repair is invoked.

## Observed alternatives and tradeoffs

Raw sample covariance is retained at delta=0; identity target dominates at delta=1. Correlation shrinkage, constant-correlation targets, factor models and robust estimates are not choices of this API. Its O(TN²) dense computation is appropriate only within the caller's size budget.

## Evidence

- **Canonical source and located tests.** [Implementation and inline `ledoit_wolf_tests`](../../../../finstack-quant/core/src/math/linalg/mod.rs) expose the two-field result, restored global-mean centering, common-normalized moments and direct squared-norm intensity. `ledoit_wolf_matches_hand_worked_two_factor_example` checks delta=17/18 and off-diagonal −1/12 (1e-14/1e-13 tolerances). Uniform extreme-rescaling, column-shift and unrepresentable-covariance tests were inspected/located, not executed; they do not cover every mixed-axis/offset/norm/complement boundary or statistical performance.
- **Independent exact-rational references.** Nine bounded reference cases were recomputed on 2026-11-08 using Python Fraction arithmetic, independently of the Rust kernel: base 17/18 and −1/12; uniform scaling by2; one-column-only scaling with delta353/738; constant-column delta9/50 and diagonal(9/40,91/40); adjacent-offset population variance1; mixed 2^500/2^-500 covariance; positive distance whose square rounds to0; finite shrunk output despite an unrepresentable unshrunk variance; and a consequential complement when the reported intensity rounds to1. All exact formula assertions and stated finite-output checks passed. These are reference calculations, not production execution, statistical experiments, broad-range accuracy certification or new committed fixtures.
- **Complement and branch references.** For epsilon=2^-600 in the boundary panel, d²=epsilon²/4+epsilon⁴/16>0 and bbar²=1/16+epsilon⁴/32 select the true cap even though binary64 d² rounds to0. For rows ±(1,1), ±(b,-b), b=8700286382685973/2^52, the sample complement is about4.56607446535e-17: delta rounds to1 while covariance off-diagonal remains about−6.23737371524e-17, or about−6.6834e284 after common scaling by2^500. These examples motivate scaled branch/complement handling rather than a numerical tie or a public-intensity reconstruction guarantee.
- **Primary provenance and limits.** The existing [Ledoit–Wolf 2004 reference](../../../REFERENCES.md#ledoitwolf2004) links the primary paper and DOI. Its known-zero-mean IID, moment and dimensional assumptions support asymptotic quadratic-risk claims within the stated linear-shrinkage class. The selected estimated-mean population plug-in is also evidenced by the [maintained scikit-learn implementation](https://raw.githubusercontent.com/scikit-learn/scikit-learn/main/sklearn/covariance/_shrunk_covariance.py); this is profile/formula corroboration, not an independent range oracle, and does not adopt its flags, implementation arithmetic or other estimators. Read-only independent reviews checked the full formula, numerical boundaries and owner distinctions.
- **Actual host/error surfaces.** [Python](../../../../finstack-quant-py/src/bindings/core/math/linalg.rs) accepts rectangular logical 2D data and returns nested covariance/intensity, despite its shared extractor's misleading square-input fallback message. [WASM](../../../../finstack-quant-wasm/src/api/core/math.rs) returns a flat typed-array/intensity pair with explicit T,N; the [parity ledger](../../../../finstack-quant-py/parity_contract.toml) intentionally retains these pairs. Current computed centering/restoration errors are Rust Validation; [central Python mapping](../../../../finstack-quant-py/src/errors.rs) already distinguishes Computation→RuntimeError from Validation→ValueError. Canonical error correction and corresponding [WASM mapping](../../../../finstack-quant-wasm/src/utils/mod.rs), docs and examples remain reconciliation work, not a new result wrapper.
- **Actual credit consumer.** [Calibration statistics](../../../../finstack-quant/factor/src/credit/calibration/statistics.rs) selects complete rows, flattens core failures to validation, annualizes once, and normalizes via sqrt(var_i*var_j) plus a blanket clamp. It discards intensity and complete-row count. Existing [calibration/model artifacts](../../../../finstack-quant/factor/src/credit/hierarchy.rs) retain frequency, config/order, covariance/correlation and diagnostics but currently lack those LW diagnostic fields; [forecasting](../../../../finstack-quant/factor/src/credit/forecast.rs) scales the retained authoritative covariance. The accepted package requires owner reconciliation of stable ratios, errors and needed provenance; it does not certify the observed risk strategy/defaults or introduce a generic reporting framework.

The mathematical implementation alone does not approve a portfolio or factor risk model.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
