# ANALYTICS-014: Normalized SVD least squares and single-equality factor-return correction

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice, domain and rationale

The shared numerical solver uses column-normalized nalgebra SVD and rejects rank deficiency. It serves multifactor time-series OLS and a public cross-sectional constrained least-squares function for factor-Brinson attribution. The constrained objective is **unweighted** residual least squares subject to one holdings-weighted return identity `w'Xf=w'r`; the `weights` argument is not a weighted-least-squares loss matrix. Exposures are a row-major assets×factors matrix, returns/holdings aligned per asset; dimensions must be nonzero and finite. Negative or unnormalized holdings are allowed.

## Normalized SVD algorithm

1. For each nonzero design column, find maximum absolute entry. Split its scale into an exact binary exponent scale and a bounded norm calculated with Neumaier sums. Prefer a single full norm if representable; otherwise retain the two-stage scale/norm to avoid squared overflow/underflow.
2. Divide each column by its scale and norm. SVD the normalized matrix with singular vectors.
3. Set tolerance `1e−10*max(largest_singular_value,1)`. Count singular values strictly above it and error if rank is less than the number of columns. No zero-column repair, ridge term or dropped-column fallback occurs.
4. Solve all right-hand sides using that factorization/tolerance, reverse column scaling in coefficient rows, and reject nonfinite coefficients. The helper validates matrix shapes; public callers own input-finiteness boundaries.

## Constrained correction algorithm

Normalize holdings w by their largest absolute value; all-zero w stays zero. Use one SVD for two right-hand sides to compute `f_hat=argmin ||r−Xf||²` and `g=argmin ||w−Xg||²`. Compute OLS fitted values and residuals. First determine whether the equality is already satisfied using a separately response-scaled bound `abs(w'(r−Xf_hat)) <= 1e−12*||w||*(||r||+||Xf_hat||)`. This calculation is performed in normalized return units to avoid a spurious infinite tolerance. All-zero holdings or all-zero returns/fits need no correction.

If unsatisfied, form Xg and denominator D=w'Xg. Reject nonfinite D or `abs(D)<=1e−10*||w||*||Xg||` as unreachable. Otherwise λ=`w'(r−Xf_hat)/D` and f=f_hat+λg; reject nonfinite λ or coefficients. This restores the single identity algebraically when represented accurately. There is no post-correction residual/constraint-tolerance check; numerical satisfaction relies on the formulation and existing tests, not an extra runtime certificate.

## Selection, limits and alternatives

No intercept is inserted by the public constrained function; callers include one only as an exposure column. Multifactor OLS inserts its own intercept before the shared solver. Normalizing holdings preserves the identity under uniform nonzero rescaling, but extreme nonuniform/ill-conditioned designs can still exceed representable precision. The fixed rank, satisfaction and degeneracy tolerances are internal numerical policies.

QR/KKT or null-space constrained solves, regularization and multiple equality/inequality constraints are unimplemented alternatives. A minimum-norm solution would change the current fail-closed rank contract. Existing architecture uses this small numerical function without portfolio-specific conventions; downstream attribution owns exposures/holdings and reconciliation meaning.

## Implementation and validation evidence

- [Owning implementation](../../../../finstack-quant/analytics/src/regression.rs) and [multifactor caller](../../../../finstack-quant/analytics/src/multi_factor.rs). The cited formula rationale is [Jeet–Partani](../../../REFERENCES.md#jeet-partani-2023); [matrix computations](../../../REFERENCES.md#golub-van-loan-matrix-computations) is numerical context.
- Inspected inline `binary_factor_case_matches_hand_closed_form` and `continuous_case_satisfies_constraint_and_matches_hand_derivation` use hand-derived coefficients/weighted identities. The latter explicitly says its expected values are a local derivation, **not** the paper's unreproducible Exhibit 8. Located scale/overflow/orthogonal-weight/rank tests qualify specific numerical cases; none was executed.

The absent post-correction residual gate is a qualification gap, not a reproduced numerical failure.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
