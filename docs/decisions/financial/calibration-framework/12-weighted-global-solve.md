# CALIBRATION-FRAMEWORK-012: Weighted Levenberg–Marquardt and bound projection

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

The global driver delegates optimization to the core Levenberg–Marquardt solver and standardizes target weighting, bounds, final validation and reporting. It minimizes a weighted least-squares objective but judges deliverable fit using every unweighted quote residual. These are distinct choices.

## Domains and ordered solve

Targets return parameter times, initial guesses and active observations. Require nonempty observations/parameters, equal time/parameter lengths, strictly increasing finite nonnegative times, finite guesses, and observation count at least parameter count. Target weights must be finite nonnegative with at least one positive weight; zero-weight observations remain in final acceptance. Target bounds, when present, must match dimensions, be finite and ordered; equal bounds are allowed.

The residual closure uses `sqrt(w_i) × r_i`. Before pricing it clamps every trial to an interior margin: two-sided margin `min(1e-8,0.25(bound width))`, one-sided 1e-8, exact fixed bounds retained. Construction/error/nonfinite-residual paths record first error and count and replace the residual vector by a penalty. The floor is 5e5 with parameter-specific `1e6 × d/(1+d)` for distance outside a bound; residual row k uses parameter k modulo parameter count. Penalty returns occur before weighting. Since pricing receives already-clamped parameters, in-bound construction failures often produce a flat 5e5 penalty.

A target-supplied Jacobian is used only if requested and supported. The interface does not require an analytic implementation: the [discount target](../calibration-targets/03-discount-calibration.md) supplies central finite differences. Target build/Jacobian failure returns `None` to core's fallback path. Derivative rows are multiplied by sqrt weights. Otherwise core finite differences the residual closure. The core LM method is separately recorded in [core numerics](../core-numerics/levenberg-marquardt.md).

Clamp final parameters again, rebuild and calculate finite raw residuals. Multi-start selection uses this weighted L2. Build the target's final curve, reprice all observations, reject duplicate residual keys, and gate on unweighted max absolute residual at the target acceptance tolerance. Final clamping forces report failure even if the quote fit is good. Intermediate invalid evaluations alone do not force final failure. Record optimizer termination separately from fit status.

## Limits and alternatives

The target-supplied Jacobian is evaluated at the clamped point without multiplying by the derivative of the clamp map. Outside/interior-margin boundaries this differs from the derivative of the actual residual closure; finite differences of the whole residual closure and target-supplied derivatives may therefore behave differently. This is a confirmed structural mismatch; convergence impact needs reproduction on a target supporting efficient derivatives. Fixed parameters still count toward the generic parameter count and underdetermination check.

True constrained LM, transformed parameters, or bound-aware finite differences are alternatives with different conditioning/cost. Weighting and weighted-L2 restart choice can favor a candidate whose worst quote fails over another candidate satisfying max acceptance. No numerical fix is adopted here.

## Source and validation evidence

- Implementation: [solver/global.rs](../../../../finstack-quant/calibration/src/solver/global.rs) — weighted residuals, projection, penalties, derivative dispatch, final reconstruction and reporting.
- Implementation: [solver/traits.rs](../../../../finstack-quant/calibration/src/solver/traits.rs) — optional efficient derivatives, weights and bounds.
- Implementation: [config.rs](../../../../finstack-quant/calibration/src/config.rs) — core LM tolerance and iteration construction.
- Test evidence: [src/solver/global.rs](../../../../finstack-quant/calibration/src/solver/global.rs) — overdetermined system, duplicate keys, all-zero weights, max-acceptance boundary, invalid evaluation and clamp tests; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
