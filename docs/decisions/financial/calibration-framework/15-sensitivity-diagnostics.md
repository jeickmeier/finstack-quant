# CALIBRATION-FRAMEWORK-015: Finite-difference sensitivities and condition estimates

- Decision status: **Accepted 2026-11-15 — complete final-artifact probes, signed derivatives, partial availability, direct conditioning and bounded work package**.
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent stencil, final-basis and conditioning checks completed.** Production diagnostic pricing, restoration, replay and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for calibration sensitivities and conditioning

1. **Diagnose the admitted final artifact on explicit election.** Freeze
   exact quote identities, valuation cut, market bindings, weights and
   residual definitions. Use the same final construction and pricing
   rules for the base and each probe. Change one represented coordinate
   per probe and hold all other parameters fixed without recalibration.
   Use a base residual only when it has the same basis and authority.

2. **Name the differentiated rows and coordinates.** For bootstrap
   report the signed derivative of each quote's residual with respect
   to its exact fitted knot value at the fixed pillar. For global
   calibration report signed partials of the unweighted optimizer
   residual in the elected free solver coordinates. Fixed parameters
   have no columns. Preserve exact coordinate identities and units.
   Label the optimizer Jacobian separately when the financial gate
   uses a different residual, as with Hull–White implied volatility.
   Retire the generic per-quote maximum across coordinates.
   [Accepted sequential bootstrap](10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing) ·
   [Accepted free coordinates and weighting](12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates).

3. **Use actual represented steps in the selected stencil.** Require
   finite positive elected bump sizes in the declared coordinate units.
   With `a = x₊ − x₀ > 0` and `b = x₀ − x₋ > 0`, compute the
   centered derivative
   `[b(r₊ − r₀)/a + a(r₀ − r₋)/b] / (a + b)`
   using checked arithmetic. Allow a signed forward or backward
   quotient only on a side qualified before pricing at a domain
   boundary. Check the actual displacement in each quotient.
   Remove the borrowed discount-step floor and automatic fallback
   after an evaluation error.
   [Fornberg on finite differences on arbitrary grids](https://doi.org/10.1090/S0025-5718-1988-0935077-0).

4. **Preserve typed partial availability.** Failed, nonfinite or
   collapsed probes make affected derivatives unavailable with a
   typed cause. A whole callback error invalidates that probe's
   output. Retain independently valid entries and preserve actual
   zero derivatives. Attach diagnostics by exact quote and parameter
   identity to the authoritative report, preserving fit rows.
   [Accepted fit report contract](14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

5. **Condition the weighted Jacobian directly.** Use a bounded,
   singular-values-only SVD of `A = diag(√w)J` on positive-weight
   rows and free columns. Apply a common checked scale to protect
   the calculation against range loss. With explicit relative cutoff
   `0 < τ < 1`, count values satisfying `σ_i/σmax > τ`, with
   zero rank for an all-zero matrix. Report
   `κ₂ = σmax/σmin` only when the complete basis resolves every
   free direction. Distinguish incomplete probes, structural
   underdetermination, estimated rank loss and numerical failure.
   [LAPACK singular values and numerical rank](https://netlib.org/lapack/explore-html/db/d6a/dgelsd_8f_source.html).

6. **Bound and report optional work independently.** Check dimensions,
   workspace and elected limits. Count actual builds, reprices and
   decomposition work, including failed attempts. Use the same
   Rust-owned finite or unavailable results across Python, WASM
   and restoration. Optional diagnostic outcomes leave fit status,
   candidate selection and publication eligibility unchanged.

## Independent calculation checks

- For `r(x)=x²−3x` at `x=1` and probes at `3/4` and `3/2`,
  the represented three-point derivative is −1. The endpoint secant
  gives −3/4. A qualified backward quotient at the upper boundary
  `x=1` gives −5/4.
- With final residual `4−3p`, trial residual `−3p`, base `p=1`
  and one-sided step `1/4`, a mixed final baseline and trial probe
  gives −19. Two evaluations under the final construction give −3.
- The weighted diagonal `(2,6)` gives Jacobian condition 3 and
  normal-equation condition 9. For diagonal `(1,2^−30)` an explicit
  relative cutoff `2^−40` retains both directions, while the
  observed Gram threshold would discard the second direction.

## Observed choice and rationale at the audit revision

Diagnostics are optional additional evaluations after fitting. They describe local conditioning and residual sensitivity; they are not an alternative acceptance criterion or statistical parameter uncertainty estimate.

## Algorithm and equations

Base relative bump is `h0=max(discount_curve.jacobian_step_size, EPSILON^(1/3))`; parameter j uses `h_j=h0(1+|p_j|)`. Bootstrap identifies the solved knot by sorted quote position after initial knots and retrieves the final residual by target key. Perturb that knot in a solver-built curve. Use central `(r+−r−)/(2h)` when both evaluate; forward `(r+−r)/h` or backward `(r−r−)/h` when one fails; when both fail log a warning and return zero. Expose absolute sensitivity, no full Jacobian or condition number.

Global diagnostics form an unweighted residual Jacobian on all solved parameters. Both successful bumps yield central differences; up-only succeeds with forward difference. If up fails, the code marks the **entire Jacobian** unusable, even when down succeeded. Any unusable column makes every per-quote sensitivity zero, omits Jacobian and condition number. Otherwise sensitivity is the maximum absolute entry of each row, not a gradient norm.

For condition number, multiply rows by sqrt residual weights, normalize all weighted entries by their largest magnitude, accumulate symmetric `JᵀWJ`, and eigendecompose. Return `lambda_max/lambda_min` only if all dimensions and weights are valid, positive scale exists and `lambda_min > EPSILON × max(n_params,n_quotes) × lambda_max`. Singular/near-rank-deficient or failed eigensolves return None. This is conditioning of the **normal equations**, the square of the Jacobian's 2-norm condition where applicable. It depends on parameter and residual units.

## Observed failure semantics and alternatives

Build/pricing `Ok` is used as bump success; separate nonfinite derivative checks are not applied before publishing every Jacobian. The condition helper rejects nonfinite weighted entries, but a Jacobian can still be present while its condition is None. Bootstrap's both-failed zero and global's blanket zero can be mistaken for true insensitivity; None conflates singularity with unavailable evidence.

Alternatives considered at the audit revision included explicit per-column/row availability, symmetric one-sided fallback, bound-aware step choice and automatic or analytic derivatives. Existing exact-matrix tests independently verify the observed condition formula, while pricing sensitivities remain same-kernel finite-difference evidence.

## Source and validation evidence

- Implementation: [solver/bootstrap.rs](../../../../finstack-quant/calibration/src/solver/bootstrap.rs) — knot-to-quote sensitivity association and three finite-difference branches.
- Implementation: [solver/global.rs](../../../../finstack-quant/calibration/src/solver/global.rs) — global Jacobian failure handling and normalized normal equations.
- Implementation: [solver/helpers.rs](../../../../finstack-quant/calibration/src/solver/helpers.rs) — shared bump scale.
- Implementation: [report.rs](../../../../finstack-quant/calibration/src/report.rs) — diagnostic row/summary wire semantics.
- Test evidence: [src/solver/global.rs](../../../../finstack-quant/calibration/src/solver/global.rs) — exact correlated/diagonal matrix condition, scaling invariance and rank deficiency tests; inspected or located, not executed.
- Test evidence: [src/solver/bootstrap.rs](../../../../finstack-quant/calibration/src/solver/bootstrap.rs) — unsorted quote/key association and one-sided sensitivities; inspected or located, not executed.
- Independent examples cover the selected derivative and condition
  arithmetic. Production target, decomposition and host qualification
  remain open.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
