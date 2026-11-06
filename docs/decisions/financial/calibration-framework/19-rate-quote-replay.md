# CALIBRATION-FRAMEWORK-019: Rate quote shocks and source-grid overlays

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

Replay preserves the stored source curve's shape and metadata while applying the movement produced by recalibrating its original quotes. This differs from returning the newly calibrated curve itself. It is an observed risk convention, not a claim of exact repricing on every source grid.

## Algorithm and conventions

Require a nonempty typed rate recipe with the appropriate Discount or Projection role. Reconstruct quotes using recipe fields, method, curve day count and optional OIS compounding. Parallel bp bumps add bp/10000 to deposit/FRA/swap rates and subtract bp/100 from futures price. Tenor bumps independently select the nearest quote; repeated targets can accumulate on the same observation. Proximity uses Act365F from curve base date with raw tenor added to base, explicit date unchanged and futures **expiry**, not the builder's spot-start/adjusted/payment-lag/reference-end pillar. Resolution failure falls back to base date/time zero; ties follow observation order.

For discount replay, use source ID/base date/interpolation/extrapolation; discount pricing role becomes source ID, projection role comes from recipe or explicit OIS override. Create a fresh default config, preserving source `allows_non_monotonic` in target final controls. Calibrate bumped quotes and enforce report acceptance; calibrate unbumped quotes too for DeltaOverlay. At every source knot set `D_new(t)=D_source(t)×D_bumped(t)/D_replayed_base(t)`. A nonpositive denominator uses ratio1. Rebuild using source builder, retaining its curve validation policy, then attach the bumped recipe quotes. The original recipe's other fields are retained.

Forward replay builds bumped observations on the target dependency market and unbumped observations on the source market. On original forward knots set `F_new(t)=F_source(t)+F_bumped(t)−F_replayed_base(t)`. Preserve base date, tenor, reset lag, day count, interpolation/extrapolation and FX policy. Keep the source projection grid if present, otherwise use unbumped replay grid. Its intermediate solves pass through runtime validation and strict report acceptance.

## Limits and alternatives

Unlike hazard replay, the rate recipe does not replay the original full calibration policy; fresh solver, weighting, scan, acceptance and validation defaults apply. Source curve metadata/forward-floor preservation is not preservation of that optimizer policy. Discount replay calls the target directly while forward replay uses the generic runtime validator; final overlays are not repriced through a full quote report. Resampling can alter off-knot behavior and residuals. Zero-shock overlay identity is intentionally stronger than a refit to the stored shape.

Alternatives are direct return of refitted curves or lossless replay on a shared expanded support grid, each changing shape/risk semantics. Existing replay-versus-direct tests share pricing/calibration kernels and are consistency evidence, not independent accuracy certification.

## Source and validation evidence

- Implementation: [recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — quote selection, fresh policy, discount ratio and forward delta overlays.
- Implementation: [recalibration/mod.rs](../../../../finstack-quant/calibration/src/recalibration/mod.rs) — strict fit gate.
- Implementation: [build/rates.rs](../../../../finstack-quant/calibration/src/build/rates.rs) — actual pillar date contrast.
- Test evidence: [src/recalibration/tests/rates_review.rs](../../../../finstack-quant/calibration/src/recalibration/tests/rates_review.rs) — direct versus replay bumped swaps, composition and zero-shock identity; inspected or located, not executed.
- Test evidence: [src/recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — source grid/floor/FX metadata and mixed quote restoration regressions; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
