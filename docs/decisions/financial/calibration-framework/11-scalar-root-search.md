# CALIBRATION-FRAMEWORK-011: Domain-aware scalar scan and root certification

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

The shared scalar search explicitly distinguishes a low residual from evidence of a root, and avoids treating a failed pricing point as a bracket endpoint. It is a custom composite search, **not simply Brent**. The observed rationale is to handle nonlinear residuals and feasible subdomains while preserving hard fit acceptance.

## Ordered algorithm

Require at least eight supplied scan points. Evaluate initial guess once; exact zero returns immediately as certified. A valid evaluation is finite with absolute residual below 1e5 (one tenth of the 1e6 generic penalty). Exhaustive mode evaluates the authoritative grid; nearest-first orders by distance from initial guess, then value, stopping on exact zero or a sign change among valid points.

If no near-zero residual or sign change is found and some evaluations were invalid, bisect adjacent valid/invalid boundaries, nearest midpoint to guess first, for 50–64 iterations (clamped from configured maximum). Newly feasible points join the observations. Sort valid observations; choose the sign-changing adjacent pair whose midpoint is nearest the initial guess.

A sampled best point below solver tolerance is returned before bracket iteration. Certification is any observed exact zero/sign change, or local two-sided probes around the best point. Local probes begin at `max(1e-6|x|,1e-9)`, widen tenfold up to eight times, and stay within scan bounds; widening stops on a nonflat failed sign test.

Without a bracket, start secant from the best point and nearest other feasible observation (or a small positive probe), clamp iterates to scan bounds, and run 50–200 steps. Track encountered sign changes and require residual tolerance. With a bracket, bisect for at least 50 iterations, stopping on residual tolerance or width `max(1e-14,1e-12 max(|a|,|b|))`; invalid midpoints stop that phase. If residual convergence remains absent, try Illinois false-position with halved retained-end residuals and midpoint safeguards, again for at least 50 steps. Return diagnostics/best observation or no result on failure; the bootstrap owner performs final validation.

## Limits and alternatives

Configured `max_iterations` is a phase control rather than a hard total-work ceiling. Invalid observations are removed before adjacent valid pairs are formed; a proposed bracket can cross an invalid gap, though invalid midpoint evaluation prevents silent convergence there. Local sign probes and secant crossing checks do not apply the same valid-magnitude filter at every certification point. Also, any sign change elsewhere in the scan can certify the selected best near-zero point, so certification is not a proof of uniqueness or of a local crossing at that point.

Brent/TOMS748 on an explicitly continuous valid bracket would offer standard convergence properties; trust-region scalar least squares would accept tangent minima more naturally but change root semantics. Existing tests establish selected cases, not global guarantees.

## Source and validation evidence

- Implementation: [solver/helpers.rs](../../../../finstack-quant/calibration/src/solver/helpers.rs) — all scan, boundary, secant, bisection and Illinois branches.
- Implementation: [solver/bootstrap.rs](../../../../finstack-quant/calibration/src/solver/bootstrap.rs) — classification/acceptance of returned points.
- Implementation: [constants.rs](../../../../finstack-quant/calibration/src/constants.rs) — valid-evaluation magnitude.
- Test evidence: [src/solver/helpers.rs](../../../../finstack-quant/calibration/src/solver/helpers.rs) — analytic-root, signed-zero, sparse-grid, feasible-boundary, quantized-flat and tangent-minimum tests; inspected or located, not executed.
- Reference context: [Brent 1973](../../../REFERENCES.md#brent-1973) — alternative bracketed-root context; the implementation here is a custom composite, not claimed Brent equivalence; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
