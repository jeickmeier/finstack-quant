# CALIBRATION-FRAMEWORK-011: Domain-aware scalar scan and root certification

- Decision status: **Accepted 2026-11-15 — complete bounded scalar search, feasible bracketing and candidate evidence package**.
- Implementation status: **Observed.** The live helper calls Brent on the bracketed path and retains other search, recovery and evidence paths. Runtime reconciliation is pending.
- Validation status: **Independent arithmetic and isolated current-source checks completed.** Production bootstrap, Student-t calibration, replay and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for bounded scalar search and candidate evidence

1. **Admit the complete search controls before sampling.** Require finite
   strictly ordered bounds, a finite in-bounds seed, explicit in-domain
   scan coordinates, selected search order, coordinate tolerance and
   checked work limits. Canonicalize signed zero and reuse observations by
   exact represented coordinate. Remove the eight-point minimum, epsilon
   merging and residual magnitude thresholds.
   [Accepted numerical controls](02-controls-and-market-context.md#accepted-decision-for-calibration-controls-freshness-and-initial-market).

2. **Select deterministic search order and candidate priority.** Sample
   the seed first and accept a valid computed zero there. Exhaustive
   search visits the remaining grid in increasing coordinate order. Rank
   eligible zeros or brackets by distance to the seed, using the zero
   coordinate or the bracket midpoint. Nearest-first visits by increasing
   seed distance and stops at the first eligible zero or bracket. Break
   distance ties toward the lower coordinate. Use safe distance and
   midpoint arithmetic. The target qualifies its choice of search order
   and its root selection under the actual objective.

3. **Preserve feasibility gaps and hard failures.** Keep target-declared
   infeasible samples in coordinate order. Form a bracket only from
   consecutive feasible samples of opposite signs within one target-qualified
   continuous interval. Compare signs directly. Pricing, data and numerical
   errors terminate with the original cause. When the grid yields no
   candidate, an expressly elected bounded bisection may refine a qualified
   single feasibility boundary. Keep feasible and infeasible sides distinct.
   Admit a computed zero or a new sign-changing bracket entirely on the
   feasible side. Use the supplied resolution and work controls and
   handle representable adjacency without hidden additional work.

4. **Run common fallible Brent on the selected bracket.** Reuse the exact
   endpoint samples. Preserve the returned coordinate, matching residual
   and selected bracket or computed-zero evidence. Remove unbracketed
   secant, extra midpoint retries and Illinois recovery. An unexpected
   infeasible interior sample, callback failure, stagnation or exhausted
   work budget fails the solve. The target supplies the continuity
   assumptions on the admitted interval.
   [Accepted Brent contract](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search) ·
   [Brent's original paper](https://maths-people.anu.edu.au/brent/pd/rpb005.pdf).

5. **Separate search outcomes from the financial gate.** Distinguish
   completed search without a bracket, exhausted work and evaluation
   failure. A completed search without root evidence may offer its
   best feasible sampled point to an expressly elected target-qualified
   approximation policy under record 010. Preserve approximate status
   and apply the required fit gates. Use one search contract in
   sequential bootstrap and Student-t. Reuse a residual only at its
   exact coordinate and keep the target's financial tolerance independent
   of the search tolerance.
   [Accepted sequential bootstrap](10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing) ·
   [Accepted Student-t target](../calibration-targets/16-student-t.md#accepted-decision-for-identified-single-tranche-student-t-calibration).

6. **Enforce and report actual work.** Cap aggregate callbacks and
   elected phase work. Charge before each new evaluation. Count scan,
   boundary and Brent work separately and count each cached evaluation
   once. Report the actual failing stage, coordinate and typed cause.
   Track financial repricing and optional diagnostic work separately.

## Independent calculation and source checks

- With feasible points through `23/64` and residual `x − 17/50`,
  the sampled boundary pair `1/4` and `1/2` has one feasible endpoint.
  Three midpoint probes yield the eligible bracket `[5/16, 11/32]`,
  with residuals `−11/400` and `3/800`.
- An isolated run of the live scan and candidate-selection source returned
  `x=1` and `−1e−13` with a sign-change flag in two examples before
  invoking Brent. In the first, `(x−1)^2(x−3)−1e−13` has a remote
  crossing between 3 and 3.5. In the second, the left feasible component
  has residual `−1e−13`, the middle samples are infeasible and the right
  component has residual `+1`. Both cases used ten samples.
- At binary64 `0.5` and its next higher coordinate, the continuous
  residual `1e14 × (x−0.5)` is respectively zero and about
  `0.01110223024625`. Exact-coordinate pairing matters even within
  the observed `1e−12` Student-t residual reuse tolerance.

## Observed choice and rationale at the audit revision

The shared scalar search explicitly distinguishes a low residual from evidence of a root, and avoids treating a failed pricing point as a bracket endpoint. It is a custom composite search, **not simply Brent**. The observed rationale is to handle nonlinear residuals and feasible subdomains while preserving hard fit acceptance.

## Ordered algorithm

Require at least eight supplied scan points. Evaluate initial guess once; exact zero returns immediately as certified. A valid evaluation is finite with absolute residual below 1e5 (one tenth of the 1e6 generic penalty). Exhaustive mode evaluates the authoritative grid; nearest-first orders by distance from initial guess, then value, stopping on exact zero or a sign change among valid points.

If no near-zero residual or sign change is found and some evaluations were invalid, bisect adjacent valid/invalid boundaries, nearest midpoint to guess first, for 50–64 iterations (clamped from configured maximum). Newly feasible points join the observations. Sort valid observations; choose the sign-changing adjacent pair whose midpoint is nearest the initial guess.

A sampled best point below solver tolerance is returned before bracket iteration. Certification is any observed exact zero/sign change, or local two-sided probes around the best point. Local probes begin at `max(1e-6|x|,1e-9)`, widen tenfold up to eight times, and stay within scan bounds; widening stops on a nonflat failed sign test.

Without a bracket, start secant from the best point and nearest other feasible observation (or a small positive probe), clamp iterates to scan bounds, and run 50–200 steps. Track encountered sign changes and require residual tolerance. With a bracket, bisect for at least 50 iterations, stopping on residual tolerance or width `max(1e-14,1e-12 max(|a|,|b|))`; invalid midpoints stop that phase. If residual convergence remains absent, try Illinois false-position with halved retained-end residuals and midpoint safeguards, again for at least 50 steps. Return diagnostics/best observation or no result on failure; the bootstrap owner performs final validation.

## Limits and alternatives

Configured `max_iterations` is a phase control rather than a hard total-work ceiling. Invalid observations are removed before adjacent valid pairs are formed; a proposed bracket can cross an invalid gap, though invalid midpoint evaluation prevents silent convergence there. Local sign probes and secant crossing checks do not apply the same valid-magnitude filter at every certification point. Also, any sign change elsewhere in the scan can certify the selected best near-zero point, so certification is not a proof of uniqueness or of a local crossing at that point.

Brent/TOMS748 on an explicitly continuous valid bracket would offer standard convergence properties; trust-region scalar least squares would accept tangent minima more naturally but change root semantics. Existing tests establish selected cases, not global guarantees.

## Current source and validation evidence

- Implementation: [solver/helpers.rs](../../../../finstack-quant/calibration/src/solver/helpers.rs) — scan, boundary refinement and candidate evidence. The live bracketed path invokes Brent before its additional midpoint and Illinois recovery.
- Implementation: [solver/bootstrap.rs](../../../../finstack-quant/calibration/src/solver/bootstrap.rs) — classification and acceptance of returned points.
- Implementation: [targets/student_t.rs](../../../../finstack-quant/calibration/src/targets/student_t.rs) — direct scalar search, finite fit and residual reuse.
- Implementation: [constants.rs](../../../../finstack-quant/calibration/src/constants.rs) — valid-evaluation magnitude.
- Test evidence: [src/solver/helpers.rs](../../../../finstack-quant/calibration/src/solver/helpers.rs) — analytic-root, signed-zero, sparse-grid, feasible-boundary, quantized-flat and tangent-minimum tests; inspected or located, not executed.
- Reference context: [Brent 1973](../../../REFERENCES.md#brent-1973) — bracketed numerical root finding and safeguards.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
