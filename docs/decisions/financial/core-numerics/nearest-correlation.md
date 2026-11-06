# CORE-NUMERICS-08: Explicit nearest-correlation projection

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/linalg/nearest_correlation.rs`: finite near-symmetric n×n matrix, diagonals within 1e-3 of one and asymmetry no more than 1e-6. This is an explicit data repair operation, not validation-only behavior.

## Choice and rationale

The source identifies Higham alternating projections with a Dykstra correction for the PSD cone. The aim is small Frobenius displacement while enforcing PSD and unit diagonal. These source claims are observed design intent; product-level permission to alter correlation remains pending.

## Algorithm

1. Validate shape (including n² overflow), finite values and entry gates; empty input returns empty. Symmetrize `Y=(A+Aᵀ)/2`.
2. Force unit diagonal in a trial copy. If pivoted Cholesky accepts it, return that copy immediately, retaining its off-diagonals.
3. Otherwise initialize correction `S=0`, then repeat `R=Y-S`; eigendecompose R; `X=Q diag(max(λ_i,0)) Qᵀ`; `S=X-R`; set `Y_new=X` with all diagonal entries 1.
4. Stop when absolute Frobenius change `||Y_new-Y||F<tol`. Before returning, project once more to PSD, then rescale `A_ij *= 1/sqrt(max(A_ii,1e-12)) * 1/sqrt(max(A_jj,1e-12))` and set each diagonal to 1.
5. If the iteration budget expires, return `DidNotConverge`; eigendecomposition or nonfinite-input failures use `EigenDecompositionFailed`.

## Selection and controls

Public options default to 200 iterations and tol=`1e-10`. These can be supplied directly; there is no validation that tol is positive/finite. No automatic use by the strict validator. The final PSD projection/rescaling and early exit are fixed internal behavior.

## Conventions

Absolute Frobenius tolerance is dimensionless and grows differently from an entrywise/relative tolerance as dimension increases. Input and output are row-major correlations; final projection can perturb estimated exposure relationships.

## Limitations, failure and fallback

The stopping criterion monitors iterate displacement, not a separately computed optimization/KKT residual. The final rescaling ensures a practical correlation output but can move away from the exact Frobenius optimum. No maximum repair-distance gate or repair report is returned. The eigensolver and pivot tolerances set effective numerical PSD.

## Alternatives and tradeoffs

Rejecting invalid matrices is already available through `validate_correlation_matrix`. Explicit shrinkage is a distinct statistical estimator, not this nearest-matrix operation. Weighted nearest-correlation objectives and constrained entries are unimplemented alternatives.

## Evidence

- [Implementation](../../../../finstack-quant/core/src/math/linalg/nearest_correlation.rs), [factor/validator](../../../../finstack-quant/core/src/math/linalg/mod.rs).
- Inline `converges_to_frobenius_nearest_matrix` compares a three-dimensional equicorrelation input against analytic optimum off-diagonal -0.5; other tests check fixed points, strict-validation output, gate rejection and exhausted budget. The analytic case does not establish arbitrary-dimension optimality.
- [Higham numerical reference](../../../REFERENCES.md#higham-accuracy-and-stability) exists, but it is **not** the specific 2002 nearest-correlation paper named by the source. That exact canonical bibliography entry is missing.

Convergence, feasibility and optimality are distinct evidence claims.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
