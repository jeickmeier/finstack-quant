# CALIBRATION-FRAMEWORK-015: Finite-difference sensitivities and condition estimates

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

Diagnostics are optional additional evaluations after fitting. They describe local conditioning and residual sensitivity; they are not an alternative acceptance criterion or statistical parameter uncertainty estimate.

## Algorithm and equations

Base relative bump is `h0=max(discount_curve.jacobian_step_size, EPSILON^(1/3))`; parameter j uses `h_j=h0(1+|p_j|)`. Bootstrap identifies the solved knot by sorted quote position after initial knots and retrieves the final residual by target key. Perturb that knot in a solver-built curve. Use central `(r+−r−)/(2h)` when both evaluate; forward `(r+−r)/h` or backward `(r−r−)/h` when one fails; when both fail log a warning and return zero. Expose absolute sensitivity, no full Jacobian or condition number.

Global diagnostics form an unweighted residual Jacobian on all solved parameters. Both successful bumps yield central differences; up-only succeeds with forward difference. If up fails, the code marks the **entire Jacobian** unusable, even when down succeeded. Any unusable column makes every per-quote sensitivity zero, omits Jacobian and condition number. Otherwise sensitivity is the maximum absolute entry of each row, not a gradient norm.

For condition number, multiply rows by sqrt residual weights, normalize all weighted entries by their largest magnitude, accumulate symmetric `JᵀWJ`, and eigendecompose. Return `lambda_max/lambda_min` only if all dimensions and weights are valid, positive scale exists and `lambda_min > EPSILON × max(n_params,n_quotes) × lambda_max`. Singular/near-rank-deficient or failed eigensolves return None. This is conditioning of the **normal equations**, the square of the Jacobian's 2-norm condition where applicable. It depends on parameter and residual units.

## Failure semantics and alternatives

Build/pricing `Ok` is used as bump success; separate nonfinite derivative checks are not applied before publishing every Jacobian. The condition helper rejects nonfinite weighted entries, but a Jacobian can still be present while its condition is None. Bootstrap's both-failed zero and global's blanket zero can be mistaken for true insensitivity; None conflates singularity with unavailable evidence.

Use explicit per-column/row availability, symmetric one-sided fallback, bound-aware step choice or automatic/analytic derivatives as alternatives. None has been adopted. Existing exact-matrix tests independently verify the condition formula, while pricing sensitivities remain same-kernel finite-difference evidence.

## Source and validation evidence

- Implementation: [solver/bootstrap.rs](../../../../finstack-quant/calibration/src/solver/bootstrap.rs) — knot-to-quote sensitivity association and three finite-difference branches.
- Implementation: [solver/global.rs](../../../../finstack-quant/calibration/src/solver/global.rs) — global Jacobian failure handling and normalized normal equations.
- Implementation: [solver/helpers.rs](../../../../finstack-quant/calibration/src/solver/helpers.rs) — shared bump scale.
- Implementation: [report.rs](../../../../finstack-quant/calibration/src/report.rs) — diagnostic row/summary wire semantics.
- Test evidence: [src/solver/global.rs](../../../../finstack-quant/calibration/src/solver/global.rs) — exact correlated/diagonal matrix condition, scaling invariance and rank deficiency tests; inspected or located, not executed.
- Test evidence: [src/solver/bootstrap.rs](../../../../finstack-quant/calibration/src/solver/bootstrap.rs) — unsorted quote/key association and one-sided sensitivities; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
