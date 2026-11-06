# CORE-NUMERICS-06: Damped nonlinear least squares and termination reporting

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/solver_multi.rs`: nonempty finite parameter vector and positive residual count; residual callbacks fill a preallocated vector. Rectangular systems, including underdetermined systems, are supported. Constraints/weights must be encoded by the caller.

## Choice and rationale

Levenberg–Marquardt uses damped normal equations and Cholesky. Diagonal-scaled damping aims to make steps less sensitive to residual units, with a larger floor for underdetermined systems. The result deliberately carries termination status even when tolerance is not met.

## Algorithm

1. Evaluate initial residuals and Euclidean norm `||r||₂`; reject nonfinite initial norm. Start λ=`1e-3`.
2. Use the provided Jacobian if `AnalyticalDerivatives::jacobian` returns `Some(())`; its construction is provider-specific despite the API name. Otherwise centrally difference each parameter with `h=epsilon^(1/3) max(|p_j|,1)`, using two residual calls per column.
3. Assemble `A=JᵀJ`, `b=-Jᵀr`. Let `dmax=max diag(A)`. Damping floor is 1 if dmax≤0, dmax if residual count<parameter count, otherwise `1e-12 dmax`. Solve `[A+λ diag(max(A_ii,floor))]s=b` via core Cholesky.
4. Factorization/solve failure increases λ tenfold capped at `1e15` and retries without recomputing the Jacobian at the same accepted parameters.
5. Compute `p_new=p+s`; if the *effective* floating-point step norm is below `1e-12`, return `StepTooSmall` at the prior accepted parameters. Otherwise evaluate trial norm.
6. Accept only strict norm decrease; then increment accepted-iteration count, invalidate Jacobian, and reduce λ tenfold with floor `1e-15`. Rejected trials increase λ and retain state/Jacobian. Check residual norm `< tolerance` at entry and immediately after acceptance.
7. Return `ConvergedResidualNorm`, `StepTooSmall`, or `MaxIterations` in `LmSolution`, with residual/Jacobian evaluation counters, final norms and damping.

## Selection and controls

Default residual-norm tolerance `1e-8`, loop budget 100; both public fields/builders can override. Damping controls and minimum step are fixed internal constants. `solve_system` uses central differences; `solve_system_with_jacobian` can fall back per call when the provider returns None. There is no bound projection, parameter transform, trust-region gain ratio, multistart or automatic restart here.

## Conventions

Residual norm and tolerance carry the combined residual units; callers need explicit scaling/weights before combining instruments or quotes. Step threshold is absolute in supplied parameter coordinates. Reported `iterations` counts accepted steps, whereas the loop budget also consumes failed/rejected trials.

## Limitations, failure and fallback

`Ok` does **not** imply fitted tolerance: both small-step and exhausted-budget states are successful result containers. Nonfinite trial norms fail the strict improvement comparison and can lead to damping exhaustion. Provided Jacobian dimensions/finiteness are provider contracts; malformed providers may not produce a clear validation error. Tolerance is not checked finite positive. Normal equations square conditioning; no QR/SVD fallback exists.

## Alternatives and tradeoffs

Analytic and finite-difference Jacobians are both supported. QR/SVD least squares, transformed bounded parameters and relative-gradient stopping are unimplemented alternatives whose conditioning/control benefits must be weighed against additional work and API choices.

## Evidence

- [Owning implementation](../../../../finstack-quant/core/src/math/solver_multi.rs), including `solve_lm_core_with_stats` and `solve_normal_equations_into`.
- Inline `test_lm_solution_is_invariant_to_uniform_residual_scale`: linear analytic root across 1e-12,1,1e12 scales with scaled tolerance; `test_lm_stats_reports_max_iterations`: explicitly expects Ok with `MaxIterations`; overdetermined Jacobian tests and final accepted-step regression are implementation evidence.
- [Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes), [matrix conditioning](../../../REFERENCES.md#golub-van-loan-matrix-computations). No specific calibrated market-data benchmark is established by these generic tests.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
