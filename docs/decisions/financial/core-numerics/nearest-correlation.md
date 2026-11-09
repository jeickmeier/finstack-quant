# CORE-NUMERICS-08: Explicit nearest-correlation projection

- Decision status: **Accepted — complete explicit nearest-correlation repair package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Source inspected; independent exact-rational/projection reference examples and semantic reviews passed**. Production iteration/host/consumer qualification remains pending; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

This record owns the explicit nearest_correlation operation and its options,
preprocessing, unweighted projection, stopping/admission and repair-distance
contract. [Accepted record 07](matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission)
owns shared strict structure, numerical PSD admission and bounded symmetric
eigen primitives. [Record 11](covariance-shrinkage.md) owns statistical shrinkage.
No generic repair call authorizes replacing a financial source/estimate by its result.

## Accepted decision: explicit nearest-correlation projection and repair limits

Accepted on **2026-11-08** after whole-record review. Runtime,
caller/host reconciliation and independent numerical/financial qualification
remain pending; observed source behavior is retained separately below.

1. **Select one unweighted full Frobenius nearest-correlation problem.**
   The ideal target is the unique minimizer of ||C-A||F over real symmetric PSD
   C with exact diagonal 1. The full Frobenius norm counts all n*n entries,
   including both off-diagonal halves. Inputs are dimensionless matrix
   coefficients; this is not covariance-to-correlation standardization. Keep
   Higham/Dykstra alternating projections with full symmetric eigendecomposition.
   No weighted objective, fixed-entry/rank/eigenvalue-floor constraint, partial
   spectrum, Newton alternative, method registry or automatic solver fallback.
   The exact-arithmetic limit selects the mathematical target; a finite result
   does not come with a quantitative distance-to-optimum certificate.

2. **Admit any finite square input and define its preprocessing explicitly.**
   Remove hardcoded 1e-6 asymmetry and 1e-3 diagonal-nearness gates. Validate all
   entries and checked n*n shape, then form B=(A+Aᵀ)/2 using stable averages,
   computing each mirrored pair once. Initialize Y0 by setting B's diagonal to1,
   X0=Y0 and the Dykstra correction S0=0. For a symmetric unit-diagonal C,
   input skew and diagonal deviations contribute constants to ||C-A||F², so this
   preserves the unweighted minimizer. It is an explicit projection step, not
   implicit validation, off-diagonal normalization or permission to use repaired
   financial data. Preserve original A for the final full repair-distance gate.

3. **Use one immutable validated explicit options bundle.** Retain
   NearestCorrelationOpts, with new(tolerance,max_iterations,eigen_epsilon,
   eigen_max_iterations,pivot_tolerance,max_distance)->Result and private fields/
   read-only accessors where needed. Require finite 0<tolerance<1, positive outer
   and eigen iteration budgets, finite 0<eigen_epsilon<1, finite
   0<=pivot_tolerance<1 and finite max_distance>=0. Tolerance is the relative
   projection stopping control; eigen controls and PSD tolerance retain accepted
   record 07 meanings. Max distance is an absolute full Frobenius displacement
   limit from original A, dimensionless and dimension-dependent. Remove Default,
   mutable/public struct literals, old tol/max_iter field names and host omission
   defaults. Validate controls/resources before empty or other success exits.
   No generic financial values or persisted options/repair-state schema.

4. **Specify the single Dykstra sequence and accepted state.** For k>=1,
   R_k=Y_(k-1)-S_(k-1), X_k=P_PSD(R_k), S_k=X_k-R_k and
   Y_k=P_unit_diagonal(X_k). Carry the PSD-cone correction only; the other set
   is affine and needs no separate stored correction. Cache prior X/Y for the
   tests below, keeping every required matrix finite and exactly mirrored.
   The unit-diagonal projection overwrites only diagonal entries with1.
   Remove the pivoted-Cholesky early-return shortcut: approximate PSD admission
   does not establish a converged nearest point, and swallowed numerical errors
   must not select a different path. Apart from the empty identity, all success
   candidates follow this one sequence; no implicit warm restart or private retry.

5. **Define full spectral PSD projection without changing its objective.**
   Use accepted record 07's bounded symmetric eigen helper with explicit epsilon/
   iteration controls. For R=Q diag(lambda) Qᵀ form
   X=Q diag(max(lambda,0)) Qᵀ using stable checked spectral sums, mirroring each
   computed entry once. Zero only computed negative eigenvalues as the defining
   PSD-cone projection; no additional positive eigenvalue floor, rank threshold,
   unit-diagonal rescaling or clipping of R's entries. This projection is specific
   to the explicitly requested repair, not an automatic shared eigen/factor rule.
   Preserve eigen failure context/category, and validate required intermediates.

6. **Require three relative Frobenius convergence quantities.** At each
   completed iteration compute dx=||X_k-X_(k-1)||F/||X_k||F,
   dy=||Y_k-Y_(k-1)||F/||Y_k||F and
   dxy=||Y_k-X_k||F/||Y_k||F, using stable scaled ratios. Require all three<=tolerance,
   not just small successive Y displacement. A zero numerator/zero denominator
   defines0; positive numerator/zero denominator is not converged, without NaN,
   EPS denominator floors or an infinite returned diagnostic. These are the
   author's later Frobenius-code convention; the paper's displayed stopping
   tests use infinity norms. Neither variant is a KKT, objective-gap, true PSD
   error or distance-to-minimizer certificate in finite arithmetic.

7. **Gate the actual candidate with the accepted correlation contract.**
   After the three tests pass, validate Y_k's exact diagonal1, finite mirrored
   entries, strict coefficients in[-1,1] and operational PSD using the explicit
   pivot_tolerance from record 07. Finite PSD/bound nonadmission continues the
   same sequence within the remaining outer budget. Unexpected shape/symmetry/
   diagonal corruption and actual computation/range failures are errors, not
   ordinary feasibility rejection or a permission to repair again. Positive
   pivot tolerance can admit a slightly indefinite stored matrix; it is not an
   exact PSD certificate. Zero tolerance can reject a mathematical boundary
   solution through rounding. No success merely because iteration changes are small.

8. **Return the stopped Y without an extra feasibility transformation.**
   Remove the final extra PSD projection, congruence rescaling and 1e-12 diagonal
   floor. Return exactly the admitted unit-diagonal iterate after the distance
   gate, not a second candidate with different objective/displacement. No bound
   clipping, diagonal ridge, identity substitute, eigensolver switch, tolerance
   relaxation or uncounted final work. Strict coefficient admission can require
   more iterations than convergence metrics alone: for [[1,2],[2,1]], ideal Y_k
   has off-diagonal1+2^-k. A finite budget need not produce an admitted matrix even
   when the limit exists; output refusal is permitted and explicit.

9. **Require the caller-selected repair-distance budget on the returned matrix.**
   Compare computed ||Y_k-A||F<=max_distance against original A, including both
   off-diagonal halves, symmetrization and diagonal changes. Use a stable scaled
   comparison without requiring an unrequested overflowing norm intermediate.
   A distance violation is a distinct input/domain repair-distance refusal with
   available original-input displacement/limit context (Python ValueError/domain
   subclass, WASM validation), separate from numerical/convergence failures.
   Do not keep searching
   for a different non-nearest fit, relax tolerances or reinterpret the budget.
   Zero max_distance is valid and requires a computed zero displacement; a valid
   input can still fail it if eigen reconstruction changes bits. This operational
   comparison is not a certified real-arithmetic error bound. The generic kernel
   selects no materiality threshold or downstream price/risk acceptance policy.

10. **Bound actual work and preserve truthful numerical/resource errors.**
    Every completed Dykstra iteration, including a metric-passed but infeasible
    candidate, consumes the outer budget. Apply the normal gates after the last
    permitted iteration; if unmet, return nonconvergence with actual completed
    work/available residuals. There is at most one bounded eigensolve per outer
    iteration and no extra final projection. Do not claim the dependency exposes
    its actual internal eigen iteration count if it does not. Use stable averages,
    differences, scaled norms/ratios and compensated spectral sums; check all
    required correction/state/output data. Nonfinite input is an input error,
    not EigenDecompositionFailed. Actual eigen, arithmetic/range and exhaustion
    failures preserve computation categories/stages, not generic input or PSD
    failures with fabricated rows. Check n*n, work/counter and capacity/byte
    arithmetic; use fallible allocation boundaries where available without a
    universal dependency/process out-of-memory recovery promise.

11. **Keep immutable input, new matrix output and explicit empty behavior.**
    Return only the row-major matrix through existing Result/error mechanisms;
    do not add a repair report, persisted iteration state, alternate matrix API
    or in-place financial mutation. n=0 with matching empty input and valid options
    returns empty with zero projection/eigen work and zero displacement. Shape
    and all option validation precede that identity. Errors return no partial
    repaired matrix. A mathematical input domain is not a guarantee that every
    finite input/control combination succeeds within binary64 range and budget.

12. **Reconcile existing callers/hosts without adopting their repair policies.**
    Rust callers construct explicit validated options. Python/WASM take one
    required options mapping/object with all six canonical fields (WASM camelCase),
    converted through the Rust constructor. Reject missing/unknown/retired keys,
    nulls and wrong types, including booleans as numerical controls; do not ignore
    host options or coerce strings to numbers. Remove flattened optional/default
    controls and aliases without a new host options class or business logic.
    Preserve existing accepted input matrix forms/logical ordering and flat
    row-major list/typed-array outputs. Update stubs, declarations, exports,
    examples and parity together. Use centralized Python ValueError/domain-input
    versus RuntimeError/computation mapping and matching WASM kinds. Actual credit
    repair strategies must propagate computation failures rather than repair on
    every validator Err. Their default strategy, permission, estimation/ridge
    choices and economic distance/risk limits remain separately owned. Analytics
    record 15's already accepted removal of automatic repair is unchanged.

13. **Retain source provenance and qualify the claimed result.** Financial
    owners retain original/returned matrices, factor ordering, effective controls,
    repair permission and realized displacement in their existing artifacts where
    they replace source data; do not invent a generic report framework or declare
    existing owner defaults accepted. The mathematical projection limit is unique,
    but finite stopping/admission/distance checks establish neither an exact PSD
    matrix nor a nearest-within-tolerance or downstream risk certificate. Remove
    unsupported product-size/runtime presets and divide-and-conquer/O(n^5) method
    claims; use the actual dependency/method evidence. Repair differs from strict
    validation and from shrinkage. Methodology acceptance does not execute or
    qualify credit calibration, simulation or portfolio use of changed correlations.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Invalid control, zero budget, malformed/overflowing shape or nonfinite input | Input/resource error before work or empty identity; no eigensolver-failure label for bad input. |
| n=0, matching empty input and valid options | Empty result, zero projection/eigen work and distance0. |
| Finite asymmetric/diagonal-distant input | Explicit stable symmetric/unit-diagonal preprocessing; no old near-input gates or off-diagonal normalization. |
| A=[[2,.9],[.1,0]] | Ideal nearest C=[[1,.5],[.5,1]]; original-input distance sqrt(2.32), not zero distance to its preprocessed target. |
| Same fixture, max_distance1.5 /1.6 | Reject / admit only if the actual candidate also meets all stopping/feasibility gates. |
| A=[MAX], max_distance=MAX | Project to[1] without an unnecessary huge-diagonal eigensolve; compare original displacement stably. |
| Unit-diagonal input with a tiny negative eigen-direction | No tolerance-Cholesky early success; run the same projection/stopping contract. |
| Small successive Y change but large X change or X/Y disagreement | Continue; one small scalar change is insufficient. |
| Zero denominator with zero /positive numerator | Defined zero ratio /not converged; no NaN or denominator floor. |
| A=[[1,2],[2,1]], convergence metrics pass while coefficient>1 | Continue and count work; no clip/polish/early success. |
| PSD gate rejects a finite candidate at zero/positive pivot tolerance | Continue within outer budget; no claim the ideal limit is absent. |
| PSD validator/eigensolver has an actual computation failure | Propagate immediately with its cause; do not interpret it as permission to retry/repair. |
| Final permitted iterate passes all stopping, feasibility and distance gates | Success with no extra projection. |
| Outer budget expires while feasibility still fails | Nonconvergence, even if a mathematical nearest matrix exists. |
| Distance limit fails after convergence/admission | Reject returned candidate; no looser budget or alternate matrix. |
| max_distance0 and an originally valid input changed by rounding | Reject if displacement is nonzero; no bitwise fixed-point promise. |
| Three-dimensional equicorrelation rho=-.55 | Ideal nearest off-diagonal-.5; original minimum eigenvalue-.1, not-.165. |
| Any input requesting weighted/fixed-entry/positive-definite-floor repair | Unsupported objective; no interpretation through hidden weights or ridge. |

## Observed choice and rationale

The source identifies Higham alternating projections with a Dykstra correction for the PSD cone. The aim is small Frobenius displacement while enforcing PSD and unit diagonal. These source claims are observed design intent; product-level permission to alter correlation remains pending.

## Observed algorithm

1. Validate shape (including n² overflow), finite values and entry gates; empty input returns empty. Symmetrize `Y=(A+Aᵀ)/2`.
2. Force unit diagonal in a trial copy. If pivoted Cholesky accepts it, return that copy immediately, retaining its off-diagonals.
3. Otherwise initialize correction `S=0`, then repeat `R=Y-S`; eigendecompose R; `X=Q diag(max(λ_i,0)) Qᵀ`; `S=X-R`; set `Y_new=X` with all diagonal entries 1.
4. Stop when absolute Frobenius change `||Y_new-Y||F<tol`. Before returning, project once more to PSD, then rescale `A_ij *= 1/sqrt(max(A_ii,1e-12)) * 1/sqrt(max(A_jj,1e-12))` and set each diagonal to 1.
5. If the iteration budget expires, return `DidNotConverge`; eigendecomposition or nonfinite-input failures use `EigenDecompositionFailed`.

## Observed selection and controls

Public options default to 200 iterations and tol=`1e-10`. These can be supplied directly; there is no validation that tol is positive/finite. No automatic use by the strict validator. The final PSD projection/rescaling and early exit are fixed internal behavior.

## Observed conventions

Absolute Frobenius tolerance is dimensionless and grows differently from an entrywise/relative tolerance as dimension increases. Input and output are row-major correlations; final projection can perturb estimated exposure relationships.

## Observed limitations, failure and fallback

The stopping criterion monitors iterate displacement, not a separately computed optimization/KKT residual. The final rescaling is intended as a feasibility polish but can move away from the exact Frobenius optimum; the returned matrix has no independent final finite/structure/PSD gate. The approximate factorization shortcut can also return a slightly indefinite matrix without iteration. No maximum repair-distance gate or repair report is returned. The eigensolver and pivot tolerances set effective numerical PSD.

## Observed alternatives and tradeoffs

Rejecting invalid matrices is already available through `validate_correlation_matrix`. Explicit shrinkage is a distinct statistical estimator, not this nearest-matrix operation. Weighted nearest-correlation objectives and constrained entries are unimplemented alternatives.

## Evidence

- [Implementation](../../../../finstack-quant/core/src/math/linalg/nearest_correlation.rs), [factor/validator](../../../../finstack-quant/core/src/math/linalg/mod.rs).
- Inline `converges_to_frobenius_nearest_matrix` compares a three-dimensional equicorrelation input against analytic optimum off-diagonal -0.5; other tests check fixed points, strict-validation output, gate rejection and exhausted budget. The analytic case does not establish arbitrary-dimension optimality.
- The [specific Higham 2002 nearest-correlation reference](../../../REFERENCES.md#higham-2002-nearest-correlation)
  now records the paper and author implementation. Source comments currently
  point to the different [Higham numerical-analysis book](../../../REFERENCES.md#higham-accuracy-and-stability);
  reconcile that citation alongside the unsupported algorithm/performance claims.
- [Credit covariance assembly](../../../../finstack-quant/factor/src/credit/calibration/assemble.rs)
  repairs on any failed validation in Ridge/FullSampleRepaired using default
  options. [Its configuration](../../../../finstack-quant/factor/src/credit/calibration/config.rs)
  selects FullSampleRepaired by default. These financial strategy/permission
  choices remain unadopted here; distinguish numerical failure from data-repair
  eligibility and retain final model validation. [Credit calibration artifacts](../../../../finstack-quant/factor/src/credit/calibration/calibrator.rs)
  need owner-side provenance/distance reconciliation; no generic report is added.
  [Analytics aggregation](../../../../finstack-quant/analytics/src/performance/aggregation.rs)
  contains automatic repair whose removal is already accepted in analytics 15.
- [Python](../../../../finstack-quant-py/src/bindings/models/correlation/mod.rs)
  and [WASM](../../../../finstack-quant-wasm/src/api/models/correlation.rs)
  currently expose optional flattened controls and default construction, returning
  a new flat matrix. Reconcile explicit options, preserved shapes and actual
  typed input/distance versus computation/convergence mapping together.

The [paper](https://eprints.maths.manchester.ac.uk/232/1/paper3.pdf) supports the
mathematical problem and modified alternating projections; the
[author's later code](https://nhigham.com/2013/02/13/the-nearest-correlation-matrix/)
supports the selected three relative Frobenius tests. These primary sources were
consulted on 2026-11-08. The input-domain extension, unit-diagonal initialization,
explicit controls, strict post-admission and original-distance refusal are scoped
project decisions/inferences, not copied defaults or a paper accuracy certificate.

Independent exact-rational checks on 2026-11-08 confirmed the skew/diagonal
constant decomposition, the asymmetric fixture's ideal matrix and original
distance sqrt(2.32)≈1.5231546212, and the rho=-.55 equicorrelation optimum/distance
sqrt(.015) with input minimum eigenvalue -.1. They confirmed the ideal 2x2
sequence's coefficient1+2^-k: at k=40, all three stopping quantities can pass
1e-10 while the coefficient still exceeds1. NumPy 2.5.1 independently checked
that closed projection step within 2.23e-16 absolute error (1e-12 threshold),
and demonstrated 7.86e-17 displacement of a valid matrix through spectral
reconstruction. MAX/subnormal-average examples demonstrate unsafe naive formulas,
not a production range pass. Independent semantic reviews passed.

The source's 60-dimensional smoke fixture is already SPD (independent NumPy
minimum eigenvalue about .205471), so its early-return path is not evidence of
iterative/eigen performance at that size. The source
comment giving the rho=-.55 example's minimum eigenvalue as -.165 is incorrect.
Production Rust/host/model tests were located or inspected, not executed.
Convergence, finite feasibility, nearest-objective error and financial repair/risk
acceptance remain distinct, with production qualification pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
