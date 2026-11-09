# CORE-NUMERICS-06: Damped nonlinear least squares and termination reporting

- Decision status: **Accepted — complete damped least-squares package, 2026-11-07**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Source inspected; independent mathematical/QR reference examples and semantic reviews passed**. Production solver and consumer qualification remain pending; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

This record owns math/solver_multi.rs dense local nonlinear least squares, its
callback/Jacobian contract, damped linear step, controls, finite arithmetic,
termination and diagnostics. It does not choose financial residual units,
quote weights, bounds/transforms, initial guesses, root/fit selection or product
acceptance. Record 05's accepted scalar Brent contract remains separately owned;
this package does not adopt record 07's general linear-algebra catalogue.

## Accepted decision: checked damped least squares and outcomes

Accepted on **2026-11-07** after whole-record review. Runtime,
caller/host reconciliation and independent numerical qualification remain pending;
observed source behavior is retained separately below.

1. **Retain one dense local damped least-squares method.** The mathematical
   objective is to reduce ||r(p)||₂, equivalently one half its squared norm,
   for n>=1 finite initial coordinates and m>=1 residuals. Keep rectangular,
   underdetermined and rank-deficient raw Jacobians eligible; m>=n does not prove
   rank and damping does not prove parameter identification. Callers supply
   comparable residuals/explicit weights and parameter coordinates. No sparse
   solver, robust loss, native constraint projection, multistart, automatic
   restart, gradient stop or new optimization framework is selected.

2. **Require explicit validated immutable controls.** Use
   LevenbergMarquardtSolver::new(residual_tolerance,step_tolerance,max_iterations)
   ->Result with private fields and read-only accessors where needed. Both norm
   tolerances must be finite strictly positive; max_iterations>=1. Residual
   tolerance is absolute in the supplied combined residual units; step tolerance
   is absolute Euclidean distance in the supplied parameter coordinates.
   Remove Default, no-argument construction, with_* builders and public mutable
   controls, including the hidden 1e-12 step threshold. Validate controls,
   dimensions and checked workspace/count sizes before allocation or callbacks,
   including an initially exact/within-target residual. No new wire/config type.

3. **Use canonical fallible fixed-shape callbacks.** Residuals take
   FnMut(&[f64],&mut[f64])->Result<()> over exactly m output slots. The supplied
   Jacobian entry point takes an explicit m and a fallible callback over exactly
   m*n flat row-major slots J[i*n+j]=d r_i/d p_j. Remove AnalyticalDerivatives,
   residual_count ownership by that trait, Option/None defaults and per-call
   finite-difference fallback. The two existing solve entry points explicitly
   select numerical or supplied derivatives. Before each call poison its output
   buffer internally, require complete finite writes, and preserve the first
   callback Err before any later sample. No stale/zero-filled missing entries,
   price-error penalties, NaN adapter or invalid-trial reject-and-damp recovery.
   Residuals must be deterministic during a solve; supplied J must differentiate
   that same weighted/transformed function. Supplied does not mean mathematically
   exact or analytically derived. Diagnostic callback side effects are not rolled back.

4. **Specify checked central finite differences without hidden switching.**
   Retain nominal h_j=EPSILON^(1/3)*max(1,abs(p_j)) as the fixed numerical scheme
   in the supplied coordinates, not a financial unit choice or universal optimum.
   Require finite distinct p_plus>p_j>p_minus. Evaluate plus then minus, checking
   each immediately, and reuse the valid base residual r0. Let a=p_plus-p_j,
   b=p_j-p_minus. Use the derivative of the three-point interpolant,
   [b/(a+b)]*(r_plus-r0)/a + [a/(a+b)]*(r0-r_minus)/b,
   with stable arithmetic. This reduces to central differences for equal actual
   spacings and avoids fictitious 2*h after rounding. An invalid probe, collapsed
   coordinate or unusable difference is an error; no shrinking, one-sided probe
   or automatic supplied/numerical derivative switch. The owner must provide
   suitable coordinates/domain or an explicit Jacobian and qualify derivative error.

5. **Replace normal equations with one augmented column-pivoted QR step.**
   For positive lambda and positive diagonal entries d_j, solve
   min_s ||[J; sqrt(lambda)*diag(d)]*s - [-r;0]||₂.
   Use the existing nalgebra dependency's Householder column-pivoted QR, apply
   Qᵀ to the right-hand side, solve the leading n-by-n triangular system and
   undo the column permutation. Its square-only solve method must not be called
   on the tall augmented matrix. Remove the LM normal-equation/Cholesky path;
   do not form JᵀJ or add a Cholesky/QR/SVD fallback registry, pseudoinverse,
   explicit inverse or raw-J full-rank admission test. This choice is scoped to
   LM's private step, not the adoption of general linear-algebra APIs.

6. **Define the damping metric without squared column norms.** Let
   c_j=stable ||J_col_j||₂ and C=max_j c_j. If C=0 use every d_j=1; if m<n use
   every d_j=C; otherwise use d_j=max(c_j,1e-6*C). This preserves the existing
   metric in real arithmetic, where the normal matrix receives lambda*d_j²,
   including the stronger underdetermined floor. Construct strictly positive
   finite regularization stably; a floor lost to underflow is not a zero penalty.
   Positive damping regularizes this step, not the raw inverse problem or a
   unique/minimum-norm financial parameter solution. The all-zero-J step is zero.

7. **Retain and disclose the simple damping/acceptance heuristic.** Start
   lambda=1e-3; after a valid strict residual-norm decrease accept the trial,
   invalidate its old Jacobian and, if more work is needed, lower lambda by 10
   with floor 1e-15. A valid nonimproving trial leaves accepted parameters/residuals
   unchanged, keeps their Jacobian and raises lambda by 10 with ceiling 1e15.
   A rejected trial already at the ceiling returns DampingLimit rather than
   repeat an identical saturated attempt. Invalid callbacks, arithmetic or QR
   failure return errors, not damping retries. These constants are explicit
   project numerical policy; no MINPACK/Ceres gain-ratio/trust-region parity,
   automatic financial retry or mathematical convergence theorem is claimed.

8. **Make required arithmetic finite and resistant to range loss.** Use stable
   scaled/hypot residual, step and column norms, not sqrt(sum(x*x)). Check candidate
   parameters before small-step handling or callbacks; the step norm uses the
   actual represented p_trial-p, not an unexecuted algebraic step. Use checked
   common normalization of the augmented matrix/right-hand side where necessary
   before library QR, stable differences/triangular operations and finite checked
   outputs. Raw nalgebra Householder code uses squared norms; choosing QR alone
   does not make arbitrary finite raw magnitudes safe. Detectably unusable scaling,
   zeroed required damping, zero/nonfinite triangular pivots, nonfinite steps or
   a required unrepresentable norm cause computation/range failure. No arbitrary
   absolute pivot cutoff, clipping, false zero norm or universal all-finite range
   guarantee. Ordinary harmless rounding is not claimed to be detectable.

9. **Keep an inspectable outcome separate from fitted tolerance.** Retain
   LmSolution and ConvergedResidualNorm, StepTooSmall and MaxIterations, adding
   DampingLimit. Only ConvergedResidualNorm asserts computed ||r||₂<residual_tolerance.
   Check that gate initially and after each accepted trial, including the final
   permitted attempt. After a finite candidate is formed, an effective step norm
   <step_tolerance returns StepTooSmall at the prior accepted point without a
   trial callback. Otherwise evaluate/validate the trial before comparing norms.
   Nonimprovement at the lambda ceiling selects DampingLimit; otherwise attempt
   exhaustion selects MaxIterations. Ok means a valid finite outcome, not an
   acceptable calibration, optimality or a root. A legitimate nonzero-residual
   least-squares minimum can stop StepTooSmall. No new ftol/gtol success modes.

10. **Count actual attempted work and report the matching accepted state.**
    max_iterations bounds damped step attempts, not accepted improvements.
    LmStats.iterations counts those attempts; add accepted_steps separately.
    residual_evals counts actual core initial/probe/trial callback calls;
    jacobian_evals counts full numerical assemblies or supplied-provider calls,
    with numerical probes also in residual_evals. Hidden provider-internal work
    is not counted as core residual calls. Cache J only at unchanged accepted
    parameters and do not re-evaluate their base residual unnecessarily. A
    conservative bound is 1+max_iterations*(2*n+1) residual calls for numerical
    J, or 1+max_iterations for supplied J, with at most max_iterations Jacobian
    assemblies/calls. Check count arithmetic. Initial target success has zero
    attempts/Jacobians. final_residual_norm matches returned accepted params;
    replace ambiguous final_step_norm/lambda_final by last_step_norm/last_lambda
    Option values for the actual last attempted step/damping, None before any
    attempt. A rejected candidate never becomes the returned accepted state.

11. **Preserve first errors and validate resources without a second framework.**
    Invalid controls, dimensions or initial data use existing input/domain errors;
    callback Err keeps its source category. Missing/nonfinite callback entries,
    failed finite arithmetic/QR and unusable range are computation failures with
    stage, parameter/probe/component and actual work where known. Do not swallow
    all linear-solver errors as positive-definiteness failures. Validate m*n,
    (m+n)*n, workspace byte/capacity and work-count arithmetic and use fallible
    allocation boundaries. This does not promise recovery from every dependency
    allocation or process-wide out-of-memory failure. No arbitrary fixed dimension
    cap, persisted solver state, new report schema, invalid/rejected candidate
    return or source-error sentinel.

12. **Limit invariance, convergence and accuracy claims to what is supported.**
    The diagonal floor, m<n metric, absolute step tolerance and finite-difference
    scheme depend on parameter coordinates; remove per-parameter scale-invariance
    claims. Uniform residual/J scaling with scaled residual tolerance preserves
    the nondegenerate real-arithmetic step, not necessarily the binary64 path;
    the C=0 branch is degenerate. QR avoids explicitly squaring J's condition
    number but does not cure poor identification, derivative error or conditioning.
    Remove universal quadratic-convergence/robust-global-fit claims. No domain-wide
    ULP, covariance/inference, global optimum or external-solver parity guarantee.

13. **Reconcile actual financial callers and retain the Rust-only boundary.**
    Update configurations, providers, all constructors/callbacks/stats consumers,
    benchmarks and enclosing host reports in the same implementation slice.
    Replace actual pricing-error penalties and provider-failure-to-None adapters,
    while preserving separately reviewed financial weights, transforms, bounds,
    initializations and restart/selection policy. A Jacobian must match the actual
    residual function, including weighting and parameter transforms; clipping's
    derivative semantics require owner reconciliation. Apply finite owner domain
    and repricing/quote-error gates after any returned-coordinate transformation
    for every solver outcome, preserving diagnostics. Do not require the generic
    residual-target status for every usable nonzero-residual least-squares fit,
    or treat any Ok/StepTooSmall/MaxIterations/DampingLimit as automatic acceptance.
    Keep LM Rust-only, with existing enclosing host mappings rather than new
    standalone Python/WASM solver classes. Methodology acceptance does not execute
    or qualify any financial calibration/model.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Zero/negative/NaN/infinite tolerance, zero budget, empty/nonfinite initial or m=0 | Input error before allocation/callback/initial-target shortcut. |
| Checked dimension/count/capacity overflow | Error before allocation or callback; no wrapping or panic as admission policy. |
| Initial finite residual already below target | ConvergedResidualNorm; one residual call, zero Jacobian/step attempts; last step/lambda absent. |
| Callback leaves any output slot unwritten | Error from the poisoned missing entry; no stale/zero residual or Jacobian. |
| First plus probe Err/NaN, later minus would be usable | Abort before the minus callback; preserve the first source error. |
| Supplied Jacobian unavailable/invalid | Error; no Option/None fallback to numerical derivatives. |
| Rounded unequal plus/minus spacings for a quadratic residual | Three-point derivative uses actual spacings and the cached base residual. |
| Finite p near MAX whose nominal plus probe overflows | Probe-geometry error; no infinity query or hidden one-sided scheme. |
| Scalar residual 1e200 / 1e-200 | Stable norm remains 1e200 / 1e-200; no squared-norm overflow or false zero. |
| J=[1e200], r=[-2e200], lambda=.001 | Finite augmented entries admit the mathematical damped step 2/1.001; do not form overflowing JᵀJ. |
| J=[1,1], r=[-2], lambda=.001 | Eligible underdetermined step: each coordinate 2/(2+.001); no parameter-identification claim. |
| J=0 with nonzero residual | Zero step gives StepTooSmall, not a fitted target or inferred global optimum. |
| Residuals (p-1,p+1), p=0, target below sqrt(2) | Legitimate nonzero-residual stationary minimum; no residual-target success. |
| Valid trial with unchanged/larger norm | Keep accepted state, raise damping/reuse J; never return rejected parameters. |
| Nonfinite candidate, trial output or QR arithmetic | Immediate error, not ordinary damping rejection. |
| Rejection at lambda=1e15 | DampingLimit without another identical saturated attempt. |
| Final permitted accepted attempt meets residual target | ConvergedResidualNorm; no extra uncounted loop needed. |
| Exhaustion with a finite improved point above target | MaxIterations with that last accepted point, truthful attempts and accepted_steps. |
| Reparameterization activates/deactivates the damping floor | Different mapped steps are permitted; no blanket parameter-scale invariance. |
| Consumer already checks repriced quotes after LM | Preserve and independently qualify that gate; do not claim every current Ok is blindly accepted. |

## Observed choice and rationale

Levenberg–Marquardt uses damped normal equations and Cholesky. Diagonal-scaled damping aims to make steps less sensitive to residual units, with a larger floor for underdetermined systems. The result deliberately carries termination status even when tolerance is not met.

## Observed algorithm

1. Evaluate initial residuals and Euclidean norm `||r||₂`; reject nonfinite initial norm. Start λ=`1e-3`.
2. Use the provided Jacobian if `AnalyticalDerivatives::jacobian` returns `Some(())`; its construction is provider-specific despite the API name. Otherwise centrally difference each parameter with `h=epsilon^(1/3) max(|p_j|,1)`, using two residual calls per column.
3. Assemble `A=JᵀJ`, `b=-Jᵀr`. Let `dmax=max diag(A)`. Damping floor is 1 if dmax≤0, dmax if residual count<parameter count, otherwise `1e-12 dmax`. Solve `[A+λ diag(max(A_ii,floor))]s=b` via core Cholesky.
4. Factorization/solve failure increases λ tenfold capped at `1e15` and retries without recomputing the Jacobian at the same accepted parameters.
5. Compute `p_new=p+s`; if the *effective* floating-point step norm is below `1e-12`, return `StepTooSmall` at the prior accepted parameters. Otherwise evaluate trial norm.
6. Accept only strict norm decrease; then increment accepted-iteration count, invalidate Jacobian, and reduce λ tenfold with floor `1e-15`. Rejected trials increase λ and retain state/Jacobian. Check residual norm `< tolerance` at entry and immediately after acceptance.
7. Return `ConvergedResidualNorm`, `StepTooSmall`, or `MaxIterations` in `LmSolution`, with residual/Jacobian evaluation counters, final norms and damping.

## Observed selection and controls

Default residual-norm tolerance `1e-8`, loop budget 100; both public fields/builders can override. Damping controls and minimum step are fixed internal constants. `solve_system` uses central differences; `solve_system_with_jacobian` can fall back per call when the provider returns None. There is no bound projection, parameter transform, trust-region gain ratio, multistart or automatic restart here.

## Observed conventions

Residual norm and tolerance carry the combined residual units; callers need explicit scaling/weights before combining instruments or quotes. Step threshold is absolute in supplied parameter coordinates. Reported `iterations` counts accepted steps, whereas the loop budget also consumes failed/rejected trials.

## Observed limitations, failure and fallback

`Ok` does **not** imply fitted tolerance: both small-step and exhausted-budget states are successful result containers. Nonfinite trial norms fail the strict improvement comparison and can lead to damping exhaustion. Provided Jacobian dimensions/finiteness are provider contracts; malformed providers may not produce a clear validation error. Tolerance is not checked finite positive. Normal equations square conditioning; no QR/SVD fallback exists.

## Observed alternatives and tradeoffs

Analytic and finite-difference Jacobians are both supported. QR/SVD least squares, transformed bounded parameters and relative-gradient stopping are unimplemented alternatives whose conditioning/control benefits must be weighed against additional work and API choices.

## Evidence

- [Owning implementation](../../../../finstack-quant/core/src/math/solver_multi.rs), including `solve_lm_core_with_stats` and `solve_normal_equations_into`.
- Inline `test_lm_solution_is_invariant_to_uniform_residual_scale`: linear root across 1e-12,1,1e12 scales with scaled tolerance; `test_lm_stats_reports_max_iterations`: explicitly expects Ok with `MaxIterations`; overdetermined Jacobian tests and final accepted-step regression are located implementation evidence, not external numerical qualification or a production pass.
- [Global calibration](../../../../finstack-quant/calibration/src/solver/global.rs)
  uses trial penalties and provider errors mapped to None, but independently
  rebuilds/reprices and checks final quote errors while reporting LM status.
  [SABR](../../../../finstack-quant/models/src/volatility/sabr/calibration.rs)
  has a finite final residual gate and per-quote checks.
  [SVI](../../../../finstack-quant/models/src/volatility/svi.rs)
  validates transformed parameters and applies an RMSE gate; its comparison
  alone is not a finite guard, but no accepted NaN fit was reproduced here.
  [Scenario curve fitting](../../../../finstack-quant/scenarios/src/adapters/curves.rs)
  has a NaN adapter and checks every rebuilt target before writing. These
  consumers require reconciliation, not a claim that every current Ok is accepted.
- The existing [nalgebra dependency](../../../../finstack-quant/core/Cargo.toml)
  is version 0.35. Installed 0.35.0 QR/column-pivoted QR/Householder sources were
  inspected: tall factorization, Q-transpose application, triangular R and
  permutation access are available; solve asserts square shape. Householder
  norm-squared arithmetic establishes the need for checked scaling/range tests,
  not an all-finite safety claim. Dependency-internal allocations limit generic
  out-of-memory recovery claims.
- [Rust-only parity scope](../../../../finstack-quant-py/parity_contract.toml)
  deliberately leaves solver_multi unbound; enclosing financial host surfaces
  retain their own reports and errors.
- [Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes), [matrix conditioning](../../../REFERENCES.md#golub-van-loan-matrix-computations). No specific calibrated market-data benchmark is established by these generic tests.

[Ceres's official solver documentation](https://ceres-solver.readthedocs.io/latest/nnls_solving.html)
supports the augmented least-squares/QR rationale and explains the conditioning
cost of forming normal equations. [MINPACK lmder](https://netlib.org/minpack/lmder.f)
distinguishes termination conditions and derivative/evaluation work;
[MINPACK enorm](https://netlib.org/minpack/enorm.f) illustrates scaled norm
evaluation. These primary sources were consulted on 2026-11-07. The package's
simple damping heuristic, absolute stops and underdetermined eligibility are
explicit project choices, not adoption of MINPACK's separate controls or m>=n limit.

Independent reference checks on 2026-11-07 confirmed the unequal-spacing
three-point derivative exactly on a quadratic, the damping metric/steps for
scalar, underdetermined, rank-deficient and zero-column/zero-J systems, a
parameter-rescaling counterexample and extreme representable norms. Seven
commonly normalized augmented QR cases using NumPy 2.5.1 agreed with the
independent algebraic steps within 4.45e-16 absolute error (1e-12 check threshold),
including scalar uniform scales 1e-200, 1 and 1e200. These bounded examples
validate formulas and limits, not the Rust implementation, nalgebra's whole
range or a financial calibration. Independent semantic reviews passed.

Production Rust/host/model tests were located or inspected, not executed.
Callback/resource/work-limit handling, derivative/conditioning/range and
financial consumer qualification remain pending. Acceptance is not runtime
execution or a calibration accuracy/identification certificate.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
