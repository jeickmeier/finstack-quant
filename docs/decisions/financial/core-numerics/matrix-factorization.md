# CORE-NUMERICS-07: Positive-definite solves and semidefinite correlation factors

- Decision status: **Accepted — complete matrix-factor/solve/eigen/correlation-admission package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Source inspected; independent exact-rational/matrix reference examples and semantic reviews passed**. Production factor/solve/host/consumer qualification remains pending; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

This record owns math/linalg/mod.rs ordinary SPD factorization/solve, generic
pivoted PSD factors/application, symmetric eigen decomposition, correlation
admission and shape conversions. [Record 08](nearest-correlation.md) owns explicit
nearest-correlation repair and [record 11](covariance-shrinkage.md) owns shrinkage.
[Accepted LM record 06](levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes)
already selects its own augmented QR step; this package does not reopen it.

## Accepted decision: checked matrix factors, solves and correlation admission

Accepted on **2026-11-08** after whole-record review. Runtime,
caller/host reconciliation and independent numerical qualification remain pending;
observed implementation is retained separately below.

1. **Keep distinct dense matrix operations and matched empty identities.**
   Retain ordinary Cholesky for a computed positive-definite solve, pivoted
   tolerance-truncated PSD factors, symmetric eigen decomposition, explicit
   correlation validation and shape helpers. Matrices are row-major n-by-n;
   validate checked shapes/capacities. n=0 with matching empty buffers gives empty
   factors/vectors/eigen pairs and valid empty correlation admission. Required
   controls remain valid before identity shortcuts. Financial nonemptiness belongs
   to the caller. Shape conversion helpers validate only their stated shape and
   preserve element order/values; they do not infer finiteness, symmetry or PSD.
   Keep rows_of valid for rectangular matrices; fix unchecked n*n allocation.

2. **Use one exact symmetric admission rule for full input matrices.**
   Ordinary SPD factorization, pivoted PSD factorization and symmetric eigen
   decomposition require every matrix entry finite and A[i,j]==A[j,i] numerically;
   +0 and -0 compare equal. Reject a mismatch before scaling, pivoting or delegation.
   Do not silently trust one half, average, clamp or repair. This resolves the
   current inconsistent lower-triangle assumptions/full-row pivot swaps/dependency
   scaling. Explicit lower-factor solve/apply operations have a different contract:
   their upper triangle is unconsumed, deliberately ignored and need not be finite.

3. **Remove arbitrary numerical-singularity cutoffs from ordinary Cholesky.**
   Retain the canonical lower-triangular positive-root factor A≈L*Lᵀ and the
   allocating/into-buffer forms. Require each computed Schur pivot strictly
   positive and each generated diagonal positive finite; a zero/negative computed
   pivot fails. Remove SINGULAR_THRESHOLD=1e-10 and its implicit conditioning policy,
   not merely a smaller replacement constant. Use stable scaling/dots/updates and
   checked finite restored output. A small positive pivot is eligible, not proof of
   useful conditioning or an accurate financial inverse. Computed failure is not
   an exact mathematical indefiniteness/rank certificate.

4. **Define provided-factor solves independently from generated factors.**
   cholesky_solve solves L*Lᵀ*x=b by checked forward/back substitution, using only
   the finite lower triangle and finite RHS with matching output size. Every
   supplied diagonal must be nonzero; negative diagonals are valid because the
   same nonsingular real L still defines SPD L*Lᵀ. Remove the relative singular
   cutoff and MIN_POSITIVE floor; do not reject a finite nonsingular factor merely
   for its scale. Preserve signed/zero off-diagonal entries. A generated Cholesky
   factor retains positive diagonals; arbitrary provided factors are not claimed
   to be that canonical square root. No pseudoinverse, inverse matrix or repair.

5. **Name the generic PSD path honestly and make its approximation explicit.**
   Rename cholesky_correlation to cholesky_psd and CorrelationFactor to PsdFactor;
   remove old aliases and reconcile all Rust/host names together. Require explicit
   finite 0<=pivot_tolerance<1; no hidden generic 1e-10 rank policy. Let
   S=max_i(abs(A[i,i])). For S>0 use a checked normalized residual A/S, with
   pivot_tolerance as the fixed threshold in those normalized units (equivalently
   tau=pivot_tolerance*S in original units, without underflowing tau arithmetic).
   When S=0 only an exactly zero full matrix is admissible; any nonzero coupling
   errors. Keep complete diagonal pivoting and symmetric Schur updates. A pivot
   below -tolerance fails; a pivot above tolerance continues. At a largest pivot
   <=tolerance, accept truncation only after checking every remaining diagonal
   >=-tolerance and every remaining residual entry has abs(value)<=tolerance.
   Reject invalid/range arithmetic immediately, without an automatic method switch.

6. **Disclose what PSD truncation does and does not establish.**
   With positive tolerance, small positive or negative residual directions can be
   discarded. F*Fᵀ approximates A and may change variances/correlations; a successful
   call does not certify exact PSD or exact reproduction. If k coordinates remain
   and their exact residual block is entrywise bounded by tau, the ideal error
   has max-entry bound tau and spectral/Frobenius bounds k*tau, not a universal
   eigenvalue error bound tau. Floating arithmetic adds error; these are not
   certified computed bounds. Tolerance zero means strict computed-pivot/block
   handling, which can reject a mathematically rank-deficient PSD input through
   rounding. Keep the existing factor result rather than a new rank/condition/
   residual-report framework. Callers requiring an actual reconstruction or risk
   error budget must check the realized F*Fᵀ against their own requirements.

7. **Fix ordering and shock orientation, including pivot ties.**
   Pick the largest remaining diagonal; break exact ties by the smallest original
   variable index. If P maps pivot coordinates back to original coordinates,
   return F=sqrt(S)*P*L_pivot*Pᵀ when L_pivot factors the normalized A/S,
   restoring the source matrix units and preserving both row and innovation-column
   labels. The exactly zero S=0 case returns a zero factor. Truncated
   pivot columns remain zero in their corresponding original innovation positions.
   F is generally dense and must not be interpreted as a lower-triangular factor.
   A row-only permutation has the same covariance but changes fixed-shock/common-
   random-number paths and is not selected. Pivot/tie changes can change loadings;
   no pathwise continuity or bitwise factor identity across methods is promised.

8. **Keep one immutable finite factor value with checked application.**
   Use PsdFactor::new(factor,n)->Result for any finite square dense factor, private
   stored state and a derived exact-triangular fast-path flag. Rename factor_matrix
   to get_factor; no duplicate compatibility accessor. A raw F defines covariance
   F*Fᵀ in real arithmetic, not unit correlation, numerical rank, reconstruction
   of some unstored source or guaranteed representable covariance entries.
   PsdFactor::apply uses F*z in original innovation order; apply_lower_triangular
   deliberately reads only its lower factor. Validate every consumed factor/shock
   and dimension, then use checked stable dot products and finite output checks.
   Raw factors may be signed/singular/zero. Skip only exact zero upper entries in
   the internal triangular path; equivalence is mathematical/qualified numerical,
   not an unconditional bit-for-bit promise for zeros or compensated accumulation.

9. **Make symmetric eigen solving bounded and its output convention explicit.**
   Require explicit finite 0<epsilon<1 and max_iterations>=1, including n=0.
   Use nalgebra SymmetricEigen::try_new with that positive finite budget instead
   of unlimited new; preserve symmetric scaling and validate returned values and
   vectors. Epsilon controls the dependency's iterative deflation, not a true
   eigenvalue accuracy tolerance; the budget counts its iterations, not all
   preprocessing work. Sort eigenvalues ascending with matching vector columns;
   preserve returned order for exact ties. Output vectors remain row-major, with
   column k paired to value k. Signed eigenvalues are retained; do not clip,
   normalize to PSD or silently change methods. Eigenvector sign and the basis
   within repeated eigenspaces are not unique, and sorting does not fix them.
   Convergence exhaustion is a computation error, not partial eigen success.

10. **Validate financial correlation structure separately and strictly.**
    validate_correlation_matrix requires finite full symmetry, exact diagonal 1
    and every coefficient in [-1,1], plus the explicitly supplied pivot_tolerance
    and the same operational PSD gate. Remove DIAGONAL_TOLERANCE,
    SYMMETRY_TOLERANCE and the correlation-bound slack; no averaging, diagonal
    reset, bound clipping or automatic repair. Positive PSD tolerance still means
    numerical admission with the limitations in decision 6, not exact PSD proof.
    The validator is read-only and returns no repaired matrix. Owners can apply
    separately accepted explicit normalization/repair before calling it. Generic
    covariance factorization and arbitrary factor construction do not imply this
    financial correlation contract.

11. **Check arithmetic/resources and preserve truthful failures and buffer rules.**
    Use stable scaled compensated products/dots/substitution/Schur arithmetic and
    finite required-output checks. Do not let scaling erase a required positive
    direction or a needed nonzero result and then claim singularity, zero or
    convergence. Detectably unusable scaling, overflow, zero-underflow of a needed
    nonzero result and invalid dependency outputs are computation/range failures;
    ordinary rounding is not generally detectable. Check n*n, workspace bytes and
    capacities before allocation; use fallible boundaries where available without
    claiming recovery from all dependency/process out-of-memory failures. Input
    admission happens before output writes. An into/solve/apply computation error
    may leave output unspecified; callers discard it before committing state.
    Complete finite output is guaranteed only on Ok; do not add hot-path atomic
    matrix copies. Keep existing typed error mechanisms, adding truthful symmetry,
    computed-range and eigen-convergence distinctions rather than mapping every
    failure to invalid input or NotPositiveSemiDefinite with fabricated row 0.

12. **Reconcile actual consumers and existing host surfaces together.**
    Rename the generic PSD primitive/type/constructor/accessor through models,
    factor/portfolio covariance uses, tests, benches, stubs, exports and parity.
    Financial correlation consumers validate the correlation contract explicitly;
    general covariance consumers keep their generic domain. Propagate application
    failures now ignored/debug-only in Monte Carlo callers. Add explicit PSD/eigen
    controls from each owner and preserve the accepted LM augmented-QR boundary.
    Python retains logical nested/2D-array matrices irrespective of strides;
    WASM retains flat row-major matrices. Existing host factor functions return
    generally dense F; no new standalone host factor class. Map input failures to
    Python ValueError/documented CholeskyError subclass, computation failures to
    RuntimeError, and the corresponding existing WASM validation/computation kinds
    through centralized mappings; update actual exception docs in the same slice.
    No string-only all-errors-as-validation adapter or host-only financial logic.

13. **Keep repair, portfolio/risk policy and accuracy qualification with owners.**
    No automatic jitter, identity fallback, diagonal loading, eigenvalue clipping,
    pseudoinverse, nearest-correlation repair or shrinkage is selected here.
    Record 08/11 algorithms, model/covariance-estimator defaults, PCA thresholds,
    common-random-number use and financial admissibility/error budgets stay
    separately owned. Analytics record 15's already accepted removal of automatic
    repair remains unchanged. Reconstruction, orthogonality and solve residual
    checks are useful bounded evidence, not exact PSD/rank, condition estimates,
    forward-error/ULP or bitwise external-library guarantees. Inputs/outputs preserve
    caller units: L and F carry square-root covariance units; innovation z is
    dimensionless when used for random samples, and generic matrix-vector use
    does not assert a distribution. Methodology acceptance does not implement or
    qualify a financial simulation, calibration or portfolio result.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| n=0 with matching empty buffers and valid controls | Empty factor/solve/apply/eigen/correlation identities; caller owns financial nonemptiness. |
| Wrong shape, ragged nested rows or checked size overflow | Input/resource error before allocation or output writes. |
| Full matrix with only upper/lower off-diagonals differing, even slightly | Symmetry error; no authoritative-half interpretation or silent averaging. |
| Symmetric entries differing only as +0/-0 | Equal numerically; no false asymmetry. |
| A=diag(1,1e-24), b=(1,1e-24) | Eligible positive pivots L=diag(1,1e-12), x=(1,1); no fixed1e-10 singularity rejection. |
| SPD path with a zero or negative computed pivot | Factorization failure; no jitter or truncated inverse. |
| Supplied L=diag(-1,2) and b=(1,4) | Eligible solve x=(1,1); generated canonical Cholesky still uses positive roots. |
| Provided lower-factor operation with NaN only in unused upper triangle | Ignore that entry by contract; NaN in consumed lower/shocks errors. |
| PSD factor with zero full diagonal and nonzero off-diagonal | Error; a zero largest diagonal alone does not establish PSD. |
| PSD A=diag(4,1), tolerance0 | Restore scale: F=diag(2,1), not a factor of A/4. |
| PSD tolerance negative/NaN/infinite or >=1; eigen epsilon outside(0,1) or zero budget | Input error before work/empty shortcut. |
| A=diag(1,tau/2), PSD scale1 and positive tolerance tau | Small direction may be dropped; F*Fᵀ need not equal A. |
| A=diag(1,-tau/2), same tolerance | May be numerically admitted/truncated despite mathematical indefiniteness. |
| k-by-k stopped residual with every entry bounded by tau | Ideal spectral error can be as large as k*tau; no universal tau eigenvalue bound. |
| Unit-diagonal 3x3 equicorrelation rho=-1/2-2^-44, tolerance1e-10 | Structural bounds/symmetry hold, but a tiny negative direction can be truncated; success is not exact PSD proof. |
| Exact repeated pivot diagonals | Smallest original index wins; covariance identity does not choose shock orientation. |
| Pivot swap makes F nontriangular | Apply full F*z with both original row/innovation labels. |
| Arbitrary finite F=2I passed to PsdFactor::new | Valid covariance factor; implied4I is not a correlation matrix. |
| Correlation diagonal1+1e-12 or coefficient just above1 | Strict structural validation error; no generic tolerance/clipping. |
| Eigen A=[[2,1],[1,2]] | Values(1,3), matching column eigenvectors, with signs unprescribed. |
| Repeated eigenvalues | Ascending values; no unique vector basis/sign promise. |
| Computed output overflow/nonfinite eigen data or exhausted eigen budget | Computation error; no partial-success or falsePSD classification. |
| Into/apply computation failure after valid admission | Output unspecified/discarded; no owner-state commit from failed data. |

## Observed choice and rationale

Separate factorization paths preserve rank-deficient correlation inputs without adding jitter, while linear solves require nonsingularity. Matrix-scale normalization and relative pivot thresholds avoid an absolute-unit failure for small covariance matrices. Dense factor application preserves permutation back into original factor coordinates.

## Observed algorithm

1. Ordinary Cholesky validates shape/finiteness, scales A by `max|A_ii|`, and computes lower-triangular entries from `L_ij=(A_ij/scale-ΣL_ikL_jk)/L_jj`; diagonal is the square root of the Schur residual. Reject negative residual or normalized factor diagonal sqrt(residual) below `1e-10`, then multiply L by `sqrt(scale)`.
2. `cholesky_solve` validates dimensions and finite RHS/lower triangle. Forward solve `Ly=b`, then backward solve `Lᵀx=y`; diagonal threshold is `1e-10 max(max|L_ii|,MIN_POSITIVE)`. Nonfinite intermediate solution errors. Unused upper triangle is deliberately ignored.
3. `cholesky_correlation` uses complete diagonal pivoting on the residual matrix, selecting its largest remaining diagonal. Threshold is `1e-10 |max initial diagonal|`. Negative pivot below -threshold errors. A pivot≤threshold ends the numerical rank only after checking every remaining diagonal and nonnegligible residual coupling; a coupled near-zero block errors.
4. Swap matrix rows/columns and prior factor rows at each pivot, compute L column, perform symmetric rank-one residual update. Unpermute both rows and innovation columns into the original coordinates; remaining rank columns are zero. `CorrelationFactor::apply` uses lower-triangular multiplication only if the returned factor is exactly lower triangular, otherwise a full matrix-vector multiply.
5. `validate_correlation_matrix` checks unit diagonal within `1e-10`, symmetry within `1e-10`, entries within [-1-1e-12,1+1e-12], then uses pivoted factorization for PSD.
6. `symmetric_eigen` delegates to nalgebra `SymmetricEigen`, converts columns to row-major storage and handles empty matrices. Shape conversion utilities validate rectangular input lengths but do not themselves establish symmetry or PSD.

## Observed selection and controls

Callers explicitly choose ordinary solve factorization versus `cholesky_correlation`. `CorrelationFactor::from_parts` accepts any finite n×n factor and detects its triangular shape; it does not validate implied correlation. `nearest_correlation` is an explicit [separate repair](nearest-correlation.md), never an automatic fallback here. nalgebra is the existing dependency.

## Observed conventions

Matrices and outputs retain caller ordering; for independent unit-variance innovations z, `Fz` has covariance `FFᵀ`. Ordinary matrix coefficients/RHS can carry arbitrary consistent units. Ordinary factorization reads the lower triangle, but pivoted full-row/column swaps can consume original upper entries and nalgebra eigen scaling examines both halves. There is no uniform checked symmetric admission; callers must currently enforce it.

## Observed limitations, failure and fallback

Small eigen-directions below the pivot threshold are truncated in the PSD path, while ordinary Cholesky rejects singularity. Shape/finiteness checks do not prove symmetry. `apply_lower_triangular` validates shapes but not finite factor/shocks, and factor `apply` does not validate shocks. No pivot error bar or condition-number estimate is returned. Symmetric eigen decomposition exposes no user iteration/convergence control.

## Observed alternatives and tradeoffs

Both nonpivoted PD and pivoted PSD paths are implemented. Eigen square roots, jitter, diagonal loading, or SVD pseudoinverse would change factor/risk behavior and are not automatic fallback options. Explicit correlation repair is available when the caller authorizes matrix modification.

## Evidence

- [Implementation and inline tests](../../../../finstack-quant/core/src/math/linalg/mod.rs); [correlation boundary tests](../../../../finstack-quant/core/tests/math/correlation.rs).
- `test_cholesky_2x2`, known solves and `symmetric_eigen_vectors_satisfy_av_equals_lambda_v`: formula/reconstruction evidence. Pivoted tests cover rank deficiency, original shock order, coupled zero blocks, nonfinite input and scale invariance. Fast/dense application comparisons are implementation equivalence only.
- [Factor covariance](../../../../finstack-quant/factor/src/covariance.rs) and
  [portfolio allocation](../../../../finstack-quant/portfolio/src/allocation.rs)
  use the covariance-capable primitive. [Monte Carlo pricing](../../../../finstack-quant/models/src/monte_carlo/engine/pricing.rs)
  factors supplied correlation data directly; [latent factors](../../../../finstack-quant/models/src/correlation/latent_factor.rs)
  validate first. Some simulation/application paths ignore or only debug-check
  current factor-application results. These are concrete reconciliation targets,
  not a reproduced claim of a wrongly accepted financial simulation.
- [Python math bindings](../../../../finstack-quant-py/src/bindings/core/math/linalg.rs)
  currently map every displayed matrix error to the ValueError-derived
  CholeskyError, while [WASM's common mapping](../../../../finstack-quant-wasm/src/utils/mod.rs)
  classifies CholeskyError wholesale as validation. Reconcile the new computed
  range/convergence distinctions through the actual centralized mappings.
  [Python correlation](../../../../finstack-quant-py/src/bindings/models/correlation/mod.rs)
  and [WASM correlation](../../../../finstack-quant-wasm/src/api/models/correlation.rs)
  expose the dense factor primitive rather than a host factor class; preserve
  that shape while correcting names/admission documentation.
- Installed nalgebra 0.35.0 symmetric-eigen source was inspected: try_new accepts
  epsilon/max_niter, zero max_niter means unlimited iteration, eigenvalues are
  unsorted and input scaling examines the full matrix. Its controls/output need
  the explicit bounded/symmetric/finite shared contract above. This is dependency
  capability evidence, not execution of the proposed Rust path.
- [Golub–Van Loan](../../../REFERENCES.md#golub-van-loan-matrix-computations), [Higham](../../../REFERENCES.md#higham-accuracy-and-stability).

[LAPACK DPOTRF](https://netlib.org/lapack/double/dpotrf.f) distinguishes the SPD
factorization domain, while [DPSTRF](https://netlib.org/lapack/double/dpstrf.f)
documents complete pivoting and explicitly does not certify PSD from its stopping
test. [DSYEV](https://netlib.org/lapack/double/dsyev.f) provides a primary reference
for ordered paired eigen output and convergence errors. These sources were
consulted on 2026-11-07. The project's exact full-symmetry admission, explicit
relative truncation control, original innovation-column convention and dependency
controls are scoped choices, not bitwise LAPACK parity or adoption of LAPACK defaults.

Independent exact-rational/reference checks on 2026-11-07 confirmed the small
positive SPD diagonal example, signed-factor solve, normalized scale restoration,
and both-permutation versus row-only shock example. The 3x3 equicorrelation
example has exact minimum eigenvalue -2^-43 (about -1.137e-13) and a final exact
Schur pivot about -3.411e-13, within the 1e-10 operational truncation threshold.
The k*tau error example follows from an exact rank-one discarded block.
NumPy 2.5.1 independently confirmed the bounded factor/covariance/ordering examples
and the known eigenpair: maximum eigen residual 0 and orthogonality error
2.23e-16 under a 1e-12 check threshold. These checks validate formulas/examples,
not the Rust kernels, new controls, host mappings or a certified numerical domain.

Production Rust/host/model tests were located or inspected, not executed.
Independent conditioning/error sweeps and finite-range, pivot/truncation,
eigen-budget, consumer-risk and host qualification remain pending; reconstruction
tests alone establish neither those properties nor financial fitness.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
