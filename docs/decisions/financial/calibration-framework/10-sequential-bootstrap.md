# CALIBRATION-FRAMEWORK-010: Sequential knot bootstrapping and final repricing

- Decision status: **Accepted 2026-11-15 — complete sequential bootstrap admission, local knot and final fit package**.
- Implementation status: **Observed.** Runtime reconciliation to the accepted package is pending.
- Validation status: **Independent coordinate, root-evidence and final repricing checks completed.** Production bootstrap, calibration, replay and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for sequential bootstrap and final repricing

1. **Admit the entire quote set and fixed prefix before solving.** Require
   one exact unique observation identity and one distinct finite,
   positive represented coordinate per new knot in the target's units.
   Validate fixed anchors under the target's anchor rules in their
   supplied order. Reject a quote at or before the fixed prefix end,
   equal represented coordinates and colliding report identities.
   Remove the generic `1e−10` spacing cutoff. Distinct close coordinates
   still require valid construction and evaluation. Solve in ascending
   coordinate order and retain authored report order and quote identity.
   [Accepted execution and preflight](03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution).

2. **Use explicit bounded search and the accepted Brent kernel.** Validate
   the guess, bounds, coordinate tolerance and work limits on the actual
   solve parameter. Remove hidden fallback scans and universal residual
   magnitude thresholds. Distinguish target-declared infeasible
   locations from pricing, data and numerical failures. Preserve typed
   errors and reject brackets across known infeasible gaps.
   Record 011 owns scan order, feasible-interval discovery and
   bracket selection.
   [Accepted Brent contract](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search) ·
   [Accepted scalar root search](11-scalar-root-search.md#accepted-decision-for-bounded-scalar-search-and-candidate-evidence).

3. **Bind root evidence to the actual returned candidate.** A finite
   computed zero or successful Brent termination under the selected
   valid bracket qualifies the returned coordinate for financial
   checks. A sign change elsewhere cannot certify the candidate.
   Allow an approximate knot only under an explicitly elected,
   target-qualified policy after completed bounded search without a
   root. Preserve the approximate status and require the same local
   and final fit checks. Solver failure and work exhaustion remain
   failures.

4. **Commit the knot privately after pricing its quote.** Build the
   proposed prefix and use the target's actual represented knot and
   pricing basis to evaluate a finite signed residual. Require valid
   target domain and the declared financial tolerance independently
   of coordinate convergence. Advance the private prefix only after
   those checks pass.
   [Accepted controls and fit policy](02-controls-and-market-context.md#accepted-decision-for-calibration-controls-freshness-and-initial-market).

5. **Rebuild and reprice the final artifact before admission.** Evaluate
   every consumed quote with its prepared inputs on the exact final
   artifact. Require exactly one finite signed unweighted residual in
   declared units per quote. Apply the independent financial fit,
   post-fit validation and complete-output gates before installation
   or replay. Exploratory failure may retain bounded diagnostics.
   Elect and report a private sequential seed followed by a coupled
   solve explicitly where that workflow is used.
   [Ametrano and Bianchetti, section 5.8](https://www.bianchetti.org/finance/bootstrappingtheilliquidity-v1.0.pdf#page=22).

6. **Preserve actual outcomes and work.** Carry exact quote and knot
   identities, coordinates and the root or approximate status. Distinguish
   local and final residuals. Count actual search evaluations, local
   checks and final reprices using checked counts. Keep optional
   diagnostics under their own work limits. Represent unavailable
   sensitivities as unavailable.
   [Accepted fit reports](14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted sensitivity diagnostics](15-sensitivity-diagnostics.md#accepted-decision-for-calibration-sensitivities-and-conditioning).

## Independent calculation checks

- With `D(0)=1` and `D(1)=0.96`, the two-node interpolation gives
  `D(0.5)+D(1)=1.94`. Adding `D(2)=0.88` under PCHIP leaves the
  new knot fitted and moves `D(0.5)` to `1181/1200`. The first
  quote's final residual is `+1/240`, outside a `0.0001`
  tolerance despite both passing local checks.
  [Accepted PCHIP rules](../core-numerics/value-interpolation.md#accepted-decision-validated-scalar-interpolation-and-boundary-semantics).
- The binary64 successor of `1` differs by `2^−52`. It is a distinct
  represented coordinate and falls within the observed absolute cutoff.
- The residual `(x−1)^2(x−3)−10^−10` at `x=1` passes a
  `10^−8` financial budget. The sampled neighbors `0.99` and
  `1.01` also have negative residuals. The crossing in `[2,4]`
  provides evidence only for that bracket.

## Observed choice and rationale at the audit revision

The generic bootstrap fixes each new knot using the observations ordered by target-supplied time. Curve construction is delegated separately for temporary solver states and the final deliverable. This keeps sequential solving independent of instrument family while making final repricing necessary.

## Algorithm, domains and controls

Quote times must be finite and strictly positive. Sort ascending with original index as tie breaker. Starting from supplied initial knots, reject a time before the previous maximum or within 1e-10 of it. For each quote obtain a target initial guess and scan grid. If the grid is empty, build a symmetric exponentially expanding grid around the guess (nonfinite center becomes zero), starting at `max(scan_grid_step × (1+|center|),1e-8)`, doubling up to half-width `100 × max(1,|center|)`. Remove nonfinite points, include finite initial guess, sort and deduplicate within 1e-12.

Each scalar objective copies existing knots, appends the trial, builds a solver curve and prices the quote residual. Construction/pricing errors become NaN during search and retain the first error for diagnostics. Target opt-in selects nearest-first versus exhaustive scan. The [observed scalar root search](11-scalar-root-search.md#ordered-algorithm) returns a point plus root-certification diagnostics using solver tolerance; final acceptance uses the separately supplied target tolerance.

A sign-change-certified point may be accepted; an uncertified near-zero point requires `allow_approximate_knots`, default false. No-bracket best points still need finite residual within acceptance tolerance. Validate the solved knot with the target, append it, rebuild and reprice; on failure pop it. Residuals must be finite, below penalty threshold 5e5, and within acceptance tolerance. Approximate knot times are explicitly recorded.

After all knots, build the **final** curve and reprice every quote. Construct a report from those final residuals, with sequential method, target units and total objective evaluations in the `iterations` field. Optional traces and finite-difference diagnostics do not change knot solving. The final report can fail even if every local knot passed, because final interpolation or later knots affect prior observations; the engine enforces its fit policy.

## Observed limitations

Bootstrap is fast and local but requires one strictly increasing support time per observation. Global least squares supports duplicate-time observations after target grouping, at higher cost and coupling. There is no generic runtime validation of initial-knot order/value domain beyond what the target curve builder enforces. Final residual collection uses a `BTreeMap` without the explicit duplicate-key guard used by global solve: colliding target keys overwrite prior residuals. Public envelope uniqueness limits this risk; complete observation coverage is not established for generic target or replay callers.

## Source and validation evidence

- Implementation: [solver/bootstrap.rs](../../../../finstack-quant/calibration/src/solver/bootstrap.rs) — sorting, scan construction, per-knot acceptance, rollback and final repricing.
- Implementation: [solver/traits.rs](../../../../finstack-quant/calibration/src/solver/traits.rs) — target contracts and default no approximate knots.
- Implementation: [constants.rs](../../../../finstack-quant/calibration/src/constants.rs) — duplicate and penalty thresholds.
- Test evidence: [src/solver/bootstrap.rs](../../../../finstack-quant/calibration/src/solver/bootstrap.rs) — order invariance, nonmonotone root, approximate opt-in, infeasible trials and f-space tolerance fixtures; inspected or located, not executed.
- The independent examples check represented spacing, bracket evidence
  and local versus final residuals. Production pricing, solver and
  replay qualification remain open.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
