# MODELS-MONTE-CARLO-24: LSMC bases, SVD cutoff and standardized polynomial reduction

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in pricer/basis.rs, pricer/lsq.rs and pricer/polynomial.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

Univariate LSMC supports raw powers, strike-normalized powers `(S-K)/K`, or unweighted Laguerre polynomials in S/K. Laguerre degree is restricted to 1–4; other basis constructors require degree>0, and normalized basis requires finite nonzero scale. A separate reusable N-dimensional policy supports quadratic/cubic total-degree polynomials with standardized features.

## Algorithm and controls

Least squares forms a row-major design matrix and solves using nalgebra SVD with truncation tolerance `sigma_max*machine_epsilon*max(n,k)`, requiring n≥k. Rank-deficient systems use the pseudoinverse; failure propagates rather than substituting realized future values. Univariate basis columns include constant; Laguerre terms through degree four are expanded explicitly without exponential weighting.

For the standardized policy, compute sample means and sample standard deviations. Drop dimensions whose standard deviation≤1e-12*max(|mean|,1), using scale one for those dimensions. Try candidates in order: cubic with all mixed terms when requested, quadratic with interactions, quadratic without interactions, linear, intercept only. Skip candidates with more terms than observations, and retry on solver error or nonfinite coefficients. The first successful finite solution is retained; rank deficiency by itself may still succeed through SVD, so it does not necessarily trigger simplification. Predict by applying stored standardization and selected terms; nonfinite inputs/results error.

## Failure behavior and alternatives

Low-level SVD accepts unvalidated shape/finite arrays beyond n≥k, relying on matrix constructor/solver contracts; public malformed arrays may fail below the domain error layer. Standardization sums can overflow for extreme finite data. No condition-number threshold, ridge penalty, cross-validation or out-of-sample selection governs candidate choice. Dropping basis terms is an observed fallback, not proof of adequate continuation fit. QR/ridge or cross-validated models are unimplemented alternatives here.

## Evidence

[Bases](../../../../finstack-quant/models/src/monte_carlo/pricer/basis.rs), [SVD](../../../../finstack-quant/models/src/monte_carlo/pricer/lsq.rs), [polynomial policy](../../../../finstack-quant/models/src/monte_carlo/pricer/polynomial.rs). Located tests recover known linear/quadratic/cubic functions, handle constant dimensions and singular designs, and reduce undersampled fits without future-value fallback. These are mathematical reconstruction/invariant tests, not financial exercise validation. [Golub–Van Loan](../../../REFERENCES.md#golub-van-loan-matrix-computations) supplies numerical background; it does not establish financial exercise accuracy for the selected SVD tolerance and model-selection procedure.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
