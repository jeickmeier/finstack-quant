# CALIBRATION-FRAMEWORK-019: Rate quote shocks and qualified replay

- Decision status: **Accepted 2026-11-15 — complete exact quote targeting, source qualification and target replay package**.
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent quote identity, shock-unit, overlay and dependency checks verified.** Production replay, publication, restoration and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for exact rate quote replay and publication

1. **Require the complete construction record.** Preserve original
   quote IDs and executed order, prepared contracts and pillars,
   resolved conventions and calendars, source clocks, interpolation
   and tails, typed dependencies, and effective solver, weighting
   and acceptance settings. Decode and validate the record strictly.
   A change to its effective fit policy requires an explicitly
   requested new calibration.
   [Accepted construction and replay qualification](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

2. **Qualify the original source before applying the action.** Rebuild
   the unbumped fit on its immutable original market. Require the
   original fit and output gates and reproduce the selected
   evaluator's complete financial contract using owner-qualified
   numerical tolerances. Apply qualification at zero too. Reuse the
   accepted reconstruction for a zero move with identical target
   inputs and policy. A zero quote move with changed target dependencies
   requires a target refit.

3. **Select parallel or exact quote targets.** Apply an explicit
   parallel move to all admitted quotes in the selected recipe or
   require a nonempty list of distinct original quote IDs and their
   moves, scoped to the bound recipe. A supplied pillar date verifies
   the prepared quote binding. Retire nearest-tenor matching, date
   fallbacks and repeat-hit accumulation. Match futures against
   their actual prepared reference end.
   [Accepted futures preparation](05-futures-materialization.md#accepted-decision-for-listed-rate-futures-calibration-and-reference-periods).

4. **Apply signed shocks in native units.** For a finite move `b` in bp,
   add `b/10000` to decimal deposit, FRA and swap rates and subtract
   `b/100` from supported IMM futures prices in points. Preserve
   convexity adjustments, contract terms and observed fixings. Reject
   invalid results and nonzero moves that round away. Report the exact
   affected IDs and both requested and represented moves.
   [CME's IMM price convention](https://www.cmegroup.com/content/dam/cmegroup/education/files/sofr-futures-settlement-calculation-methodologies.pdf).

5. **Deliver and report the accepted target refit.** Refit on the
   explicit target market. Require complete native quote repricing,
   source support and applicable curve checks on the delivered
   evaluator. Return that evaluator, its new construction record and
   the full fit report. Retire discount-factor ratio and additive
   forward overlays, source-grid resampling and the ratio-one fallback.
   [Accepted fit reporting](14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted curve validation](16-curve-validation.md#accepted-decision-for-curve-validation-and-publication).

6. **Publish related refits atomically.** Follow the actual typed
   dependency graph, stage the target inputs for each dependent fit,
   and publish the complete group after delivered-market acceptance.
   Reject unsupported cycles. Preserve stage-specific errors and
   Rust-owned outcomes through existing valuation, schema and host
   paths. Direct curve-parameter risk is a separately elected basis.
   [Accepted linked OIS replay](20-linked-ois-replay.md#accepted-decision-for-linked-single-curve-ois-replay-and-publication) owns linked single-curve OIS
   replay and [record 023](23-recalibration-cache.md#accepted-decision-for-exact-request-local-recalibration-caching) owns cache policy.
   [Accepted calibration execution](03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution) ·
   [Accepted rate risk bases](../valuations-framework/finite-difference-rates.md).

## Independent calculation checks

- A synthetic June 18 target is 91 days from a future's March 19
  expiry and two days from a deposit's June 20 pillar. The former
  nearest-date selector picks the deposit even though the future's
  prepared reference end is June 18. A +2 bp move takes 4.00% to
  4.02% and 96.00 IMM points to 95.98.
- A recorded one-year deposit quote of 2.00%, stored factor implying
  2.02%, and +1 bp shock give an old overlay factor of
  `51000000/52035301`. Its implied rate is about 2.030002% and
  exceeds the 2.01% target by about 2.000196 bp. Both intermediate
  refits can price their own quotes exactly.
- In a two-payment par swap with equal accruals, strike 2%, first
  forward fixed at 1% and first payment DF .99, a second payment DF
  change from .98 to .90 moves the fitted second forward from
  `59/1960` to .031 with zero quote shock.

## Observed choice and rationale at the audit revision

Replay preserves the stored source curve's shape and metadata while applying the movement produced by recalibrating its original quotes. This differs from returning the newly calibrated curve itself. It is an observed risk convention, not a claim of exact repricing on every source grid.

## Observed algorithm and conventions

Require a nonempty typed rate recipe with the appropriate Discount or Projection role. Reconstruct quotes using recipe fields, method, curve day count and optional OIS compounding. Parallel bp bumps add bp/10000 to deposit/FRA/swap rates and subtract bp/100 from futures price. Tenor bumps independently select the nearest quote; repeated targets can accumulate on the same observation. Proximity uses Act365F from curve base date with raw tenor added to base, explicit date unchanged and futures **expiry**, not the builder's spot-start/adjusted/payment-lag/reference-end pillar. Resolution failure falls back to base date/time zero; ties follow observation order.

For discount replay, use source ID/base date/interpolation/extrapolation; discount pricing role becomes source ID, projection role comes from recipe or explicit OIS override. Create a fresh default config, preserving source `allows_non_monotonic` in target final controls. Calibrate bumped quotes and enforce report acceptance; calibrate unbumped quotes too for DeltaOverlay. At every source knot set `D_new(t)=D_source(t)×D_bumped(t)/D_replayed_base(t)`. A nonpositive denominator uses ratio1. Rebuild using source builder, retaining its curve validation policy, then attach the bumped recipe quotes. The original recipe's other fields are retained.

Forward replay builds bumped observations on the target dependency market and unbumped observations on the source market. On original forward knots set `F_new(t)=F_source(t)+F_bumped(t)−F_replayed_base(t)`. Preserve base date, tenor, reset lag, day count, interpolation/extrapolation and FX policy. Keep the source projection grid if present, otherwise use unbumped replay grid. Its intermediate solves pass through runtime validation and strict report acceptance.

## Observed limits and alternatives

Unlike hazard replay, the rate recipe does not replay the original full calibration policy; fresh solver, weighting, scan, acceptance and validation defaults apply. Source curve metadata/forward-floor preservation is not preservation of that optimizer policy. Discount replay calls the target directly while forward replay uses the generic runtime validator; final overlays are not repriced through a full quote report. Resampling can alter off-knot behavior and residuals. Zero-shock overlay identity is intentionally stronger than a refit to the stored shape.

Alternatives are direct return of refitted curves or lossless replay on a shared expanded support grid, each changing shape/risk semantics. Existing replay-versus-direct tests share pricing/calibration kernels and are consistency evidence, not independent accuracy certification.

## Source and validation evidence

- Implementation: [recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — quote selection, fresh policy, discount ratio and forward delta overlays.
- Implementation: [recalibration/mod.rs](../../../../finstack-quant/calibration/src/recalibration/mod.rs) — strict fit gate.
- Implementation: [build/rates.rs](../../../../finstack-quant/calibration/src/build/rates.rs) — actual pillar date contrast.
- Test evidence: [src/recalibration/tests/rates_review.rs](../../../../finstack-quant/calibration/src/recalibration/tests/rates_review.rs) — direct versus replay bumped swaps, composition and zero-shock identity; inspected or located, not executed.
- Test evidence: [src/recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — source grid/floor/FX metadata and mixed quote restoration regressions; inspected or located, not executed.
- Independent examples above check identity selection, native bump arithmetic
  and the effect of source overlays and changed target dependencies.
  Production replay and host qualification remain pending.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
