# CALIBRATION-FRAMEWORK-010: Sequential knot bootstrapping and final repricing

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

The generic bootstrap fixes each new knot using the observations ordered by target-supplied time. Curve construction is delegated separately for temporary solver states and the final deliverable. This keeps sequential solving independent of instrument family while making final repricing necessary.

## Algorithm, domains and controls

Quote times must be finite and strictly positive. Sort ascending with original index as tie breaker. Starting from supplied initial knots, reject a time before the previous maximum or within 1e-10 of it. For each quote obtain a target initial guess and scan grid. If the grid is empty, build a symmetric exponentially expanding grid around the guess (nonfinite center becomes zero), starting at `max(scan_grid_step × (1+|center|),1e-8)`, doubling up to half-width `100 × max(1,|center|)`. Remove nonfinite points, include finite initial guess, sort and deduplicate within 1e-12.

Each scalar objective copies existing knots, appends the trial, builds a solver curve and prices the quote residual. Construction/pricing errors become NaN during search and retain the first error for diagnostics. Target opt-in selects nearest-first versus exhaustive scan. [Scalar root search](11-scalar-root-search.md) returns a point plus root-certification diagnostics using solver tolerance; final acceptance uses the separately supplied target tolerance.

A sign-change-certified point may be accepted; an uncertified near-zero point requires `allow_approximate_knots`, default false. No-bracket best points still need finite residual within acceptance tolerance. Validate the solved knot with the target, append it, rebuild and reprice; on failure pop it. Residuals must be finite, below penalty threshold 5e5, and within acceptance tolerance. Approximate knot times are explicitly recorded.

After all knots, build the **final** curve and reprice every quote. Construct a report from those final residuals, with sequential method, target units and total objective evaluations in the `iterations` field. Optional traces and finite-difference diagnostics do not change knot solving. The final report can fail even if every local knot passed, because final interpolation or later knots affect prior observations; the engine enforces its fit policy.

## Alternatives

Bootstrap is fast and local but requires one strictly increasing support time per observation. Global least squares supports duplicate-time observations after target grouping, at higher cost and coupling. There is no generic runtime validation of initial-knot order/value domain beyond what the target curve builder enforces. Final residual collection uses a `BTreeMap` without the explicit duplicate-key guard used by global solve: colliding target keys overwrite prior residuals. Public envelope uniqueness limits this risk; complete observation coverage is not established for generic target or replay callers.

## Source and validation evidence

- Implementation: [solver/bootstrap.rs](../../../../finstack-quant/calibration/src/solver/bootstrap.rs) — sorting, scan construction, per-knot acceptance, rollback and final repricing.
- Implementation: [solver/traits.rs](../../../../finstack-quant/calibration/src/solver/traits.rs) — target contracts and default no approximate knots.
- Implementation: [constants.rs](../../../../finstack-quant/calibration/src/constants.rs) — duplicate and penalty thresholds.
- Test evidence: [src/solver/bootstrap.rs](../../../../finstack-quant/calibration/src/solver/bootstrap.rs) — order invariance, nonmonotone root, approximate opt-in, infeasible trials and f-space tolerance fixtures; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
