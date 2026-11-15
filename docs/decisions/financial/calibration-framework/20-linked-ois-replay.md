# CALIBRATION-FRAMEWORK-020: Linked single-curve OIS discount/projection replay

- Decision status: **Accepted 2026-11-15 — complete explicit shared OIS binding, dated projection and atomic replay package**.
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent accrual, off-knot and published-fixing checks verified.** Production replay, publication, restoration and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for linked single-curve OIS replay and publication

1. **Elect one shared OIS evaluator explicitly.** Bind the exact typed
   discount source and overnight index on one qualified currency,
   collateral basis, dated convention and information cut. Preserve one
   authoritative construction record with the ordered quotes, effective
   compounding and fit policy. Any named projection role resolves to
   that same evaluator. Retire inference from equal IDs, an absent
   forward curve, reciprocal recipe flags or matching quote vectors.
   Fail an invalid selected link. Independently calibrated roles use
   the general rate replay contract.
   [Explicit index projection](../core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates) ·
   [Accepted rate replay](19-rate-quote-replay.md#accepted-decision-for-exact-rate-quote-replay-and-publication).

2. **Qualify the source and solve the shared target.** Reconstruct
   the unbumped fit on the immutable original market. Require
   reproduction of the complete evaluator and index binding. Apply
   the exact quote action once on the explicit target market. Every
   candidate supplies both pricing roles during that fit. Count the
   source reconstruction and shared target fit separately. A cold
   nonzero request performs both. A fully identical zero request may
   reuse the accepted reconstruction under record 019.
   [Accepted provenance and replay qualification](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

3. **Project actual overnight observations through the fitted source.**
   Use resolved value dates `S,E`, the discount evaluator's clock and
   tails, and the contractual index accrual `α`. Use checked positive
   endpoint factors and compute the annual decimal rate
   `expm1(log D(S) − log D(E))/α` with stable log-ratio arithmetic.
   Retire inverse date rounding, sampled forward grids and fabricated
   terminal intervals.
   [Strata's discount-backed overnight projection](https://github.com/OpenGamma/Strata/blob/main/modules/pricer/src/main/java/com/opengamma/strata/pricer/rate/DiscountOvernightIndexRates.java#L166-L175).

4. **Preserve fixing authority and dated cashflows.** Use eligible
   published fixings and project only eligible unresolved observations.
   Reuse prepared lookback, observation shift, cutoff and weights.
   Compound actual blocks, apply contractual spread and coupon
   accrual, and discount on the actual payment date. A whole-coupon
   discount ratio is an algebraic identity for contiguous fully
   projected blocks with matching weights. Use the accepted observation
   evaluator for pricing and fit.
   [Accepted overnight projection](../valuations-framework/overnight-projection.md#accepted-decision-for-checked-overnight-projection-and-quote-risk) ·
   [Accepted OIS materialization](06-swap-materialization.md#accepted-decision-for-term-and-overnight-swap-calibration).

5. **Publish the delivered evaluator and role binding together.** Stage
   both roles on the explicit target snapshot. Require complete
   quote-fit and curve checks on that delivered market and validate
   affected dependencies before publication. Preserve the fitted
   knots, interpolation and tails. Publish the actual evaluator,
   its projection binding, construction record and fit report
   atomically.
   [Accepted plan execution](03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution) ·
   [Accepted fit reports](14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted final curve checks](16-curve-validation.md#accepted-decision-for-curve-validation-and-publication).

6. **Use one Rust contract across existing callers.** Require an
   explicit quote-replay election in valuation and risk paths and
   propagate qualification errors. Direct curve risk has its own
   election. Preserve typed bindings, reports and errors in existing
   state, restoration and host surfaces. Record 023 owns cache
   identity and lifetime.
   [Accepted rate risk bases](../valuations-framework/finite-difference-rates.md) ·
   [Replay cache](23-recalibration-cache.md#accepted-decision-for-exact-request-local-recalibration-caching).

## Independent calculation checks

- From Friday to Monday, a factor ratio of `10000/9997` gives an
  annual rate of `360/9997` on ACT/360 index accrual. ACT/365F
  source-clock width gives `365/9997`, about 5.0015 bp higher.
- A linear fit on `(0,.5,1)` with factors `(1,.985,.96)` gives
  `.985` at the midpoint. Keeping only its endpoint factors
  changes the midpoint to `.98`.
- Forward factors `1,.9999,.9998` give two contiguous fully
  projected daily gross factors that multiply to `5000/4999`.
  Replacing day one's projection with a published 3% ACT/360
  fixing gives `39999333/39992000`.

## Observed choice and rationale at the audit revision

When two stored representations declare the same OIS quote source, replay calibrates the discount curve once and derives projection from it. It does not independently bump and calibrate two representations of one economic curve.

## Observed selection and algorithm

Linked mode requires OIS compounding metadata and reciprocal Discount(projection ID)/Projection(discount ID) roles. If neither side declares a link, use independent discount/forward replay. If one does, require the other, equal OIS compounding, currency, curve day count and exact quote vector equality; malformed links fail. Base dates must also agree when constructing projection. Method equality is not one of the link checks.

Recalibrate discount with projection role overridden to its own ID, returning newly calibrated values resampled at the **source discount knots** rather than the general DF-ratio overlay. The SingleOis request takes this path directly. For linked projection, if the source has a projection grid use each adjacent grid interval and append a terminal interval of source tenor; otherwise use [source knot, knot+tenor]. Compute simple forward `L=(D(date_start)/D(date_end)−1)/(end_time−start_time)`.

Map forward times to dates using source clock: round 360t days for Act360, 365t for Act365F; for other day counts binary-search integer days from zero to ceil(500t)+366, selecting the nearest year fraction, with lower date on a tie. Require finite nonnegative times, valid end>start, representable dates and finite positive DFs. Note the numerator uses rounded calendar dates while denominator remains the supplied forward-time interval.

Rebuild forward preserving its grid, tenor, reset lag, day count, interpolation/extrapolation and FX policy; replace its quote recipe with the bumped discount quote vector. Install both into a cloned source market. Source fixings and all unrelated market data remain as in the source snapshot.

## Observed limitations and alternatives

This construction enforces sampled discount/projection relationships but interpolation between retained nodes, date rounding, terminal extrapolation and long-tenor day-count inversion can break exact equality away from those intervals. The source-grid resampled discount is not itself run through a new quote-fit acceptance report. Alternatives include retaining the calibrated support or using a direct discount-backed projection object. Existing one-calibration/clock tests support the intended data flow, not all interpolation choices.

## Source and validation evidence

- Implementation: [recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — link checks, one-curve replay, integer-date inversion and simple-forward construction.
- Test evidence: [src/recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — single-calibration counter, reciprocal recipe failures, source fixings and cross-clock projection checks; inspected or located, not executed.
- Test evidence: [src/recalibration/tests/rates_review.rs](../../../../finstack-quant/calibration/src/recalibration/tests/rates_review.rs) — linked and single-OIS composition/direct-repricing fixtures; inspected or located, not executed.
- Independent examples above cover accrual, off-knot behavior and
  authority of eligible published fixings. Complete-path production
  replay and host qualification remain pending.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
