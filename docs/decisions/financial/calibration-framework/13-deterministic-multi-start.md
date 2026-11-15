# CALIBRATION-FRAMEWORK-013: Deterministic Halton restart selection

- Decision status: **Accepted 2026-11-15 — complete Halton proposal, ordered execution, final selection and all-start work package**.
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent Halton, fit selection and work-accounting checks completed.** Production calibration, replay and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for deterministic multi-start and final selection

1. **Elect multi-start and freeze its coordinates.** Preserve the
   primary seed at index zero and the elected additional proposal
   count `R`. Support one through ten ordered free solver
   coordinates with distinct Halton bases `2, 3, 5, 7, 11, 13, 17,
   19, 23, 29` assigned once in order. Require a finite positive
   half-width for each free coordinate. Remove cyclic base reuse.
   The target owns the actual free-coordinate map, proposal count
   and widths. Hull–White's effective settings remain with its
   [rate-model fit record](../calibration-rate-models/06-two-parameter-fit.md).
   [Halton's original construction](https://link.springer.com/article/10.1007/BF01386213).

2. **Generate exactly the elected proposals in solver coordinates.**
   For index `j = 1…R`, use the unscrambled radical inverse
   `H_b(j)` and form `z_i' = z_i + s_i(2H_b(j) − 1)` in the
   selected coordinate units. Use checked arithmetic and the
   accepted parameter map without clipping. Normalize signed zero
   and skip exact duplicate solver vectors, retaining the earlier
   index. Record invalid or unrepresentable proposals at their
   original indices and generate no replacements. A finite
   prefix can be asymmetric. Qualify the width and count on
   the actual target.
   [Accepted parameter coordinates](12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates).

3. **Attempt each distinct admitted start once.** Prepare immutable
   shared inputs and effective controls, and isolate each start's
   working state. Honor serial execution. Authorize independent
   native parallel starts through the enclosing execution policy.
   Use serial restart execution within an already parallel plan
   batch. WASM uses the same indexed list serially. Let all elected
   starts finish independently of the first fit and collect results
   by start index. Shared admission and publication failures apply
   at the step level.
   [Accepted execution](03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution) ·
   [Rayon indexed execution](https://docs.rs/rayon/latest/rayon/iter/trait.IndexedParallelIterator.html).

4. **Apply all final gates before ranking starts.** Rebuild and
   reprice the exact final artifact for each returned candidate.
   Apply the target's financial fit, parameter-domain, post-fit
   validation and complete-output gates. For Hull–White this
   includes implied volatility in the elected quote convention.
   Rank passing candidates by the final weighted norm on the
   elected objective basis. Break exact ties by earlier start
   index. Preserve typed start-local failures. If every start
   fails, report the earliest indexed failure and retain bounded
   diagnostic evidence.

5. **Bound and report the entire run.** Check proposal counts and
   aggregate work before starting. Reserve deterministic per-start
   allowances. Count actual generation, solver work, provider
   probes and final reprices across every start. Preserve effective
   controls and indexed outcomes for direct execution, plans,
   host reports and qualified replay. Publish only the admitted
   winner.
   [Accepted calibration provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

## Independent calculation checks

- For seed `(−3, −4)` and half-width `0.5` on both coordinates,
  the first two proposals are `(−3, −25/6)` and `(−13/4, −23/6)`.
  In one dimension, the first proposal exactly duplicates the
  primary seed.
- In a synthetic ATM Black quote with 10 years to expiry,
  volatility 0.30 and tolerance 0.001, model volatility 0.3010001
  has a vega-scaled premium error of about 0.000999724602
  and fails with quote error 0.0010001. Model volatility 0.2990001
  has a vega-scaled premium error of about −0.001000274602
  and passes with quote error −0.0009999.
- A start-order check with two equal-score passing candidates
  selects the earlier index regardless of completion order.
  With individual residual call counts 7, 4, 8 and 3, including
  a failed start, the reported total is 22 and the winner's
  count is four.

## Observed choice and rationale at the audit revision

Multi-start uses a deterministic low-discrepancy perturbation rather than random seeds. This makes repeatable exploration possible, while ordered collection resolves ties reproducibly. The perturbation lives in **the target parameter units**, not as a fraction of the initial value.

## Algorithm and selection

Always attempt the primary initial guess. For restart index j and parameter dimension i, compute radical inverse `H_(j+1)(base_i)` using bases [2,3,5,7,11,13,17,19,23,29] cyclically. Set `x'_i = x_i + scale × (2H−1)` then clip to target lower/upper bounds. A target using logarithmic parameters consequently receives multiplicative movement after exponentiation; for scale 0.5 this is about exp(-0.5) to exp(0.5), not ±50% in physical units.

Native restart computation uses Rayon regardless of plan `use_parallel`; WASM runs the same ordered restart sequence sequentially. Results collect in restart order. Keep any successful single solve whose weighted L2 is strictly smaller than current best; ties retain the earlier winner (primary first). Failed attempts are skipped; if all fail, return primary error where available. Only the winning solve's optimizer stats and evaluation-error counts are published, with total configured restarts and winner label.

The public plan has no generic restart setting. Targets explicitly choose whether to use this path and supply count/scale. HW's exact configuration is in [rate-model calibration](../calibration-rate-models/06-two-parameter-fit.md).

## Observed limits and alternatives

This is deterministic sampling, not global optimization. Beyond ten dimensions, identical base streams repeat and correlate perturbations. Additive scale is not normalized by parameter range, nor by |initial|. No shared validation of a negative/nonfinite scale or pathological restart count is shown here; current callers must supply valid constants. Perturbation clipping followed by inward solver projection can duplicate starts.

Alternatives include a target-scaled Sobol design, stratified bounded restarts or optimizer-specific basin searches. These increase control complexity and need evidence before adoption.

## Source and validation evidence

- Implementation: [solver/multi_start.rs](../../../../finstack-quant/calibration/src/solver/multi_start.rs) — radical-inverse bases and additive clipped perturbations.
- Implementation: [solver/global.rs](../../../../finstack-quant/calibration/src/solver/global.rs) — parallel restart collection, strict improvement and winning-stat selection.
- Test evidence: [src/solver/multi_start.rs](../../../../finstack-quant/calibration/src/solver/multi_start.rs) — known Halton values, near-zero movement, scale independence and bounds; inspected or located, not executed.
- Test evidence: [src/solver/global.rs](../../../../finstack-quant/calibration/src/solver/global.rs) — deterministic perturbation regression located; inspected or located, not executed.
- Independent arithmetic checks cover fixed proposal indices,
  target fit versus objective ranking and all-start accounting.
  Production target and host qualification remain open.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
