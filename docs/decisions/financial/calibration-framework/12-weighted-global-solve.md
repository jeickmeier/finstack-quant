# CALIBRATION-FRAMEWORK-012: Weighted Levenberg–Marquardt and parameter coordinates

- Decision status: **Accepted 2026-11-15 — complete weighted global solve, parameter coordinates, derivatives and final candidate gate package**.
- Implementation status: **Observed.** Runtime reconciliation to the accepted package is pending.
- Validation status: **Independent weighting, derivative and final-fit arithmetic checks completed.** Production global solves, calibration, replay and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for weighted global solve and final candidate gates

1. **Freeze observations, native residuals and weights.** Require unique
   quote identities, declared signed residual units and one finite
   nonnegative weight per row. Fit the positive-weight rows using
   `√w_i × r_i`. Keep zero-weight rows in final repricing and the
   unweighted financial gate. For quote-only optimization require
   at least as many positive-weight rows as free parameters. The
   target owns identification given its actual observations and model.
   [NIST weighted least squares](https://itl.nist.gov/div898/handbook/pmd/section4/pmd432.htm).

2. **Use one checked map from free coordinates to native parameters.**
   Use identity for unbounded coordinates, an exponential offset
   for one-sided open bounds and a stable logistic map for two-sided
   open bounds. A target may qualify a coupled map for its domain.
   Apply the same map and target-domain checks to seeds, probes,
   trials, final pricing and replay. Treat equal bounds and
   deliberately selected endpoints as exact fixed native values
   outside the LM vector. If all parameters are fixed, reprice
   and apply the final gates with zero LM work. Exact endpoint
   discovery requires a qualified endpoint-capable method. Fail
   when a required interior representation or transform derivative
   cannot be resolved. Remove hidden margins, model-level clipping
   and final clamping.
   [CERN on fixed parameters and bound transformations](https://root.cern/root/html534/guides/minuit2/Minuit2Letter.pdf#page=13).

3. **Use the accepted fallible augmented-QR kernel.** Require explicit
   residual-norm tolerance, step tolerance and attempt budget. Keep
   complete, finite writes through the target and calibration scratch
   buffers. Preserve the first typed construction, pricing,
   derivative or arithmetic error. Remove synthetic penalty residuals
   and use checked, scaled norms for objective and candidate comparisons.
   [Accepted Levenberg–Marquardt](../core-numerics/levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes) ·
   [Accepted calibration controls](02-controls-and-market-context.md#accepted-decision-for-calibration-controls-freshness-and-initial-market).

4. **Elect the actual derivative method before solving.** Use core
   central differences in solver coordinates or a qualified supplied
   Jacobian. For native parameters `p = T(z)`, the supplied derivative
   of the weighted objective is
   `diag(√w) × J_native × DT(z)` on the active rows. A supplied provider
   may use finite differences. Any skipped row derivative needs
   qualification from actual pricing dependencies under the elected
   construction. Unsupported providers and failed derivative
   evaluations return typed errors.

5. **Apply the final candidate gate before ranking.** Build the exact
   final artifact for each returned candidate and reprice every
   consumed quote. Apply the unweighted financial gate, independent
   post-fit validation and complete-output checks. Preserve the
   optimizer's actual stop reason. A finite iterate stopped by a step,
   damping or attempt limit can qualify if it passes the full gate.
   Rank only passing candidates by final weighted norm. Record 013
   owns restart generation and ties. If every candidate fails, retain
   only bounded diagnostic evidence.
   [Accepted execution gates](03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution) ·
   [Accepted multi-start selection](13-deterministic-multi-start.md#accepted-decision-for-deterministic-multi-start-and-final-selection).

6. **Report actual work on the exact result.** Count attempted and
   accepted steps, residual and Jacobian calls, provider-internal
   probes, final reprices and optional diagnostics separately. Use
   the same final acceptance contract for direct calibration, plan
   execution and replay. Preserve fitted parameters, method, units
   and typed outcomes through the existing host reports.
   [Accepted fit reports](14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted sensitivity diagnostics](15-sensitivity-diagnostics.md#accepted-decision-for-calibration-sensitivities-and-conditioning).

## Independent calculation checks

- With weights `(1, 0.1)` and a `0.01` per-quote fit tolerance,
  residuals `(0, 0.02)` yield weighted squared error `0.00004`
  and fail. Residuals `(0.007, 0.007)` yield `0.0000539` and pass.
- For native residual `p−1`, weight four and a clipped physical
  interval `[0,1]`, the weighted residual is flat above the upper
  clip while the supplied native derivative scaled by the weight
  is two. A logistic map on the same open range has
  `dp/dz=1/4` at `z=0`, giving the matching weighted derivative
  `1/2`. A separately fixed native `p=1` gives exact zero residual.
- A one-entry write to a two-entry zero-filled residual can
  fabricate a zero norm. A missing-slot check rejects that output.
  A finite residual of `1e200` has a representable norm although
  its unscaled square overflows.

## Observed choice and rationale at the audit revision

The global driver delegates optimization to the core Levenberg–Marquardt solver and standardizes target weighting, bounds, final validation and reporting. It minimizes a weighted least-squares objective but judges deliverable fit using every unweighted quote residual. These are distinct choices.

## Domains and ordered solve

Targets return parameter times, initial guesses and active observations. Require nonempty observations/parameters, equal time/parameter lengths, strictly increasing finite nonnegative times, finite guesses, and observation count at least parameter count. Target weights must be finite nonnegative with at least one positive weight; zero-weight observations remain in final acceptance. Target bounds, when present, must match dimensions, be finite and ordered; equal bounds are allowed.

The residual closure uses `sqrt(w_i) × r_i`. Before pricing it clamps every trial to an interior margin: two-sided margin `min(1e-8,0.25(bound width))`, one-sided 1e-8, exact fixed bounds retained. Construction/error/nonfinite-residual paths record first error and count and replace the residual vector by a penalty. The floor is 5e5 with parameter-specific `1e6 × d/(1+d)` for distance outside a bound; residual row k uses parameter k modulo parameter count. Penalty returns occur before weighting. Since pricing receives already-clamped parameters, in-bound construction failures often produce a flat 5e5 penalty.

A target-supplied Jacobian is used only if requested and supported. The interface does not require an analytic implementation: the [discount target](../calibration-targets/03-discount-calibration.md) supplies central finite differences. Target build/Jacobian failure returns `None` to core's fallback path. Derivative rows are multiplied by sqrt weights. Otherwise core finite differences the residual closure. The core LM method is separately recorded in [core numerics](../core-numerics/levenberg-marquardt.md).

Clamp final parameters again, rebuild and calculate finite raw residuals. Multi-start selection uses this weighted L2. Build the target's final curve, reprice all observations, reject duplicate residual keys, and gate on unweighted max absolute residual at the target acceptance tolerance. Final clamping forces report failure even if the quote fit is good. Intermediate invalid evaluations alone do not force final failure. Record optimizer termination separately from fit status.

## Observed limits and alternatives

The target-supplied Jacobian is evaluated at the clamped point without multiplying by the derivative of the clamp map. Outside/interior-margin boundaries this differs from the derivative of the actual residual closure; finite differences of the whole residual closure and target-supplied derivatives may therefore behave differently. This is a confirmed structural mismatch; convergence impact needs reproduction on a target supporting efficient derivatives. Fixed parameters still count toward the generic parameter count and underdetermination check.

True constrained LM, transformed parameters, or bound-aware finite differences are alternatives with different conditioning/cost. Weighting and weighted-L2 restart choice can favor a candidate whose worst quote fails over another candidate satisfying max acceptance. No numerical fix is adopted here.

## Source and validation evidence

- Implementation: [solver/global.rs](../../../../finstack-quant/calibration/src/solver/global.rs) — weighted residuals, projection, penalties, derivative dispatch, final reconstruction and reporting. The live supplied provider call propagates its error. The adapter still clamps the objective and can silently elect numerical derivatives when the requested provider is unavailable.
- Implementation: [solver/traits.rs](../../../../finstack-quant/calibration/src/solver/traits.rs) — optional efficient derivatives, weights and bounds.
- Implementation: [config.rs](../../../../finstack-quant/calibration/src/config.rs) — core LM tolerance and iteration construction.
- Test evidence: [src/solver/global.rs](../../../../finstack-quant/calibration/src/solver/global.rs) — overdetermined system, duplicate keys, all-zero weights, max-acceptance boundary, invalid evaluation and clamp tests; inspected or located, not executed.
- Independent examples cover the selected formulas and failure cases.
  Production target, pricing, replay and host qualification remain open.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
