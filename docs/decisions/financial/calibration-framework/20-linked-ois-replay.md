# CALIBRATION-FRAMEWORK-020: Linked single-curve OIS discount/projection replay

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

When two stored representations declare the same OIS quote source, replay calibrates the discount curve once and derives projection from it. It does not independently bump and calibrate two representations of one economic curve.

## Selection and algorithm

Linked mode requires OIS compounding metadata and reciprocal Discount(projection ID)/Projection(discount ID) roles. If neither side declares a link, use independent discount/forward replay. If one does, require the other, equal OIS compounding, currency, curve day count and exact quote vector equality; malformed links fail. Base dates must also agree when constructing projection. Method equality is not one of the link checks.

Recalibrate discount with projection role overridden to its own ID, returning newly calibrated values resampled at the **source discount knots** rather than the general DF-ratio overlay. The SingleOis request takes this path directly. For linked projection, if the source has a projection grid use each adjacent grid interval and append a terminal interval of source tenor; otherwise use [source knot, knot+tenor]. Compute simple forward `L=(D(date_start)/D(date_end)−1)/(end_time−start_time)`.

Map forward times to dates using source clock: round 360t days for Act360, 365t for Act365F; for other day counts binary-search integer days from zero to ceil(500t)+366, selecting the nearest year fraction, with lower date on a tie. Require finite nonnegative times, valid end>start, representable dates and finite positive DFs. Note the numerator uses rounded calendar dates while denominator remains the supplied forward-time interval.

Rebuild forward preserving its grid, tenor, reset lag, day count, interpolation/extrapolation and FX policy; replace its quote recipe with the bumped discount quote vector. Install both into a cloned source market. Source fixings and all unrelated market data remain as in the source snapshot.

## Alternatives

This construction enforces sampled discount/projection relationships but interpolation between retained nodes, date rounding, terminal extrapolation and long-tenor day-count inversion can break exact equality away from those intervals. The source-grid resampled discount is not itself run through a new quote-fit acceptance report. Alternatives include retaining the calibrated support or using a direct discount-backed projection object. Existing one-calibration/clock tests support the intended data flow, not all interpolation choices.

## Source and validation evidence

- Implementation: [recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — link checks, one-curve replay, integer-date inversion and simple-forward construction.
- Test evidence: [src/recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — single-calibration counter, reciprocal recipe failures, source fixings and cross-clock projection checks; inspected or located, not executed.
- Test evidence: [src/recalibration/rates_review_tests.rs](../../../../finstack-quant/calibration/src/recalibration/rates_review_tests.rs) — linked and single-OIS composition/direct-repricing fixtures; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
