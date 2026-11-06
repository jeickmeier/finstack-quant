# CORE-NUMERICS-07: Positive-definite solves and semidefinite correlation factors

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/linalg/mod.rs`: row-major square finite matrices, vectors, ordinary Cholesky solves, pivoted semidefinite factors, symmetric eigenvalues/eigenvectors, and correlation validation. Financial correlation matrices require the explicit validator; low-level routines do not infer that requirement.

## Choice and rationale

Separate factorization paths preserve rank-deficient correlation inputs without adding jitter, while linear solves require nonsingularity. Matrix-scale normalization and relative pivot thresholds avoid an absolute-unit failure for small covariance matrices. Dense factor application preserves permutation back into original factor coordinates.

## Algorithm

1. Ordinary Cholesky validates shape/finiteness, scales A by `max|A_ii|`, and computes lower-triangular entries from `L_ij=(A_ij/scale-ΣL_ikL_jk)/L_jj`; diagonal is the square root of the Schur residual. Reject negative residual or normalized diagonal below `1e-10`, then multiply L by `sqrt(scale)`.
2. `cholesky_solve` validates dimensions and finite RHS/lower triangle. Forward solve `Ly=b`, then backward solve `Lᵀx=y`; diagonal threshold is `1e-10 max(max|L_ii|,MIN_POSITIVE)`. Nonfinite intermediate solution errors. Unused upper triangle is deliberately ignored.
3. `cholesky_correlation` uses complete diagonal pivoting on the residual matrix, selecting its largest remaining diagonal. Threshold is `1e-10 |max initial diagonal|`. Negative pivot below -threshold errors. A pivot≤threshold ends the numerical rank only after checking every remaining diagonal and nonnegligible residual coupling; a coupled near-zero block errors.
4. Swap matrix rows/columns and prior factor rows at each pivot, compute L column, perform symmetric rank-one residual update. Unpermute both rows and innovation columns into the original coordinates; remaining rank columns are zero. `CorrelationFactor::apply` uses lower-triangular multiplication only if the returned factor is exactly lower triangular, otherwise a full matrix-vector multiply.
5. `validate_correlation_matrix` checks unit diagonal within `1e-10`, symmetry within `1e-10`, entries within [-1-1e-12,1+1e-12], then uses pivoted factorization for PSD.
6. `symmetric_eigen` delegates to nalgebra `SymmetricEigen`, converts columns to row-major storage and handles empty matrices. Shape conversion utilities validate rectangular input lengths but do not themselves establish symmetry or PSD.

## Selection and controls

Callers explicitly choose ordinary solve factorization versus `cholesky_correlation`. `CorrelationFactor::from_parts` accepts any finite n×n factor and detects its triangular shape; it does not validate implied correlation. `nearest_correlation` is an explicit [separate repair](nearest-correlation.md), never an automatic fallback here. nalgebra is the existing dependency.

## Conventions

Matrices and outputs retain caller ordering; for independent unit-variance innovations z, `Fz` has covariance `FFᵀ`. Ordinary matrix coefficients/RHS can carry arbitrary consistent units. Low-level factorization uses the lower triangle; callers must enforce symmetry if needed.

## Limitations, failure and fallback

Small eigen-directions below the pivot threshold are truncated in the PSD path, while ordinary Cholesky rejects singularity. Shape/finiteness checks do not prove symmetry. `apply_lower_triangular` validates shapes but not finite factor/shocks, and factor `apply` does not validate shocks. No pivot error bar or condition-number estimate is returned. Symmetric eigen decomposition exposes no user iteration/convergence control.

## Alternatives and tradeoffs

Both nonpivoted PD and pivoted PSD paths are implemented. Eigen square roots, jitter, diagonal loading, or SVD pseudoinverse would change factor/risk behavior and are not automatic fallback options. Explicit correlation repair is available when the caller authorizes matrix modification.

## Evidence

- [Implementation and inline tests](../../../../finstack-quant/core/src/math/linalg/mod.rs); [correlation boundary tests](../../../../finstack-quant/core/tests/math/correlation.rs).
- `test_cholesky_2x2`, known solves and `symmetric_eigen_vectors_satisfy_av_equals_lambda_v`: formula/reconstruction evidence. Pivoted tests cover rank deficiency, original shock order, coupled zero blocks, nonfinite input and scale invariance. Fast/dense application comparisons are implementation equivalence only.
- [Golub–Van Loan](../../../REFERENCES.md#golub-van-loan-matrix-computations), [Higham](../../../REFERENCES.md#higham-accuracy-and-stability).

Independent conditioning/error sweeps are not established by reconstruction tests alone.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
