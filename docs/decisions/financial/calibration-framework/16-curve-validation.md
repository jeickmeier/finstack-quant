# CALIBRATION-FRAMEWORK-016: Sampled curve shape, plausibility and bounds

- Decision status: **Accepted 2026-11-15 — complete structural, coverage, economic-check and publication package**.
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent curve examples verified.** Production evaluator, calibration, replay and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for curve validation and publication

1. **Require structural validity and pricing support.** Validate the
   final artifact with its Rust source owner and require checked support
   for all prepared-instrument queries. Elect structural checks alone,
   a typed list of economic checks or a named versioned preset. Each elected
   check declares scope, quantity, units, inclusive bounds, any
   check-specific comparison tolerance and required or advisory severity.
   Optional tolerances apply only to the named economic bounds. Only a
   measured advisory breach can warn. Invalid policy, invalid values,
   failed evaluations and missing required support fail at their actual
   stage. Retire the blanket warning downgrade and shared thresholds.
   [Accepted calibration controls](02-controls-and-market-context.md#accepted-decision-for-calibration-controls-freshness-and-initial-market).

2. **Report the actual coverage.** Include applicable anchors, stored
   knots, interpolation breakpoints, one-sided boundaries and required
   pricing or observation dates. Elect additional interior probes and
   any extra tail horizon explicitly. Use the source's actual clock and
   tail policy. Preserve original date identities and require distinct
   represented coordinates for curve-time annualization. Report the
   evaluated scope, work and unrun checks. Sampled results cover the
   selected points and intervals. Continuous claims require evidence for
   the specific evaluator. A required check with incomplete coverage
   fails. Retire fixed grids and silent omission of invalid probes.
   [Accepted log-discount interpolation](../core-numerics/forward-interpolation.md#accepted-decision-natural-log-discount-and-hagan-west-interpolation) ·
   [Hagan and West on curve interpolation](https://bank.uni-hohenheim.de/uploads/media/Hagan_and_West__2006__-_Interpolation_Methods_for_Curve_Construction.pdf).

3. **Use source-native discount and projection quantities.** Require
   positive finite factors and exact normalization at the origin.
   Admit rising factors and negative rates. Elect discount bands
   separately for continuous interval forwards
   `(log D(a) − log D(b))/(b − a)`, positive-time continuous zeros
   `−log D(t)/t` and supported one-sided instantaneous forwards. Resolve
   the actual index observation and use the simple projection forward
   `expm1(log P(S) − log P(E))/α` with contractual accrual `α`.
   Retire the generic negative-rate switch, fixed factor caps,
   multiplied rate floors and fixed forward-jump warning.
   [Discount source](../core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction) ·
   [Index projection source](../core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates).

4. **Validate active hazard segments.** Require finite nonnegative
   intensities, including zero, and recovery in `[0,1]` where present.
   The piecewise-constant nonnegative intensity law guarantees
   nonincreasing survival. Apply any elected annual intensity cap to
   every segment in scope and to the constant tail when covered.
   Retire the universal 0.50/year cap and sampled survival monotonicity
   check.
   [Hazard source](../core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation).

5. **Use absolute CPI and explicit intervals.** Require finite positive
   CPI levels and the source's exact base anchor. Admit deflation.
   Evaluate any elected growth band on explicit unlagged intervals via
   `expm1((log I(b) − log I(a))/(b − a))` with positive source-clock
   width, and specify the dates of any one-year forward check.
   Retire fixed growth grids and universal CPI bands.
   [Inflation source](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks).

6. **Preserve base-correlation source and tranche boundaries.** Require
   aligned finite values, strictly increasing detachment percentages
   in `[0,100]`, correlation fractions in `[0,1]`, and the source's
   linear interpolation and flat extrapolation. Allow elected advisory
   reporting of drops and an explicitly elected nondecreasing fit
   constraint. Tranche expected-loss qualification belongs to the pool
   and copula owners.
   [Base-correlation source](../core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks).

7. **Use one final gate across execution and replay.** Apply the
   selected policy before caching, installing or publishing final
   results from direct calibration, serial and parallel plans and
   replay. Check NS/NSS with their analytic discount evaluator and
   cross-currency outputs on the actual foreign discount source.
   Publish coupled base-correlation and rebound-index updates
   atomically. Keep separate outcomes for search, quote fit and curve
   validation. Preserve check and source identities, values, units,
   bounds and coverage through existing host reports. Record 017 owns
   surface validation.
   [Parametric discount source](../core-market/parametric-discount.md#accepted-decision-validated-nsnss-sources-and-analytic-factor-queries) ·
   [Accepted NS/NSS calibration](../calibration-targets/08-parametric-calibration.md#accepted-decision-for-calibration-on-the-analytic-nsnss-discount-source) ·
   [Accepted fit reporting](14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted surface validation](17-surface-validation.md#accepted-decision-for-surface-validation-and-publication).

## Independent calculation checks

- A natural spline of `−log D` at years `0,1,2,3` with values
  `0,.0600,.0701,.1501` has strictly positive forwards at every knot
  and midpoint. Its forward at `1+539/1198` is
  `−6607/35940000`, about −1.84 bp.
- A projection factor ratio of `199/200` over 90 days gives −2% with
  contractual ACT/360 and about −2.028% with ACT/365F curve time.
  Only the contractual result passes a −2.01% floor.
- Hazard ending knots at `.1,.2,1` with intensities `.1,1.2,.1`
  pass a 0.50/year cap at the old half-year probes and breach it on
  the actual `(.1,.2]` segment. CPI 300 to 294 in one source-clock
  year gives −2%.
- With two independent equal-size zero-recovery names and 10% PD,
  30%-detachment capped equity loss is 5.7% of pool. At 40% detachment
  under comonotonic defaults it is 4.0% of pool. The difference is
  negative despite the base-correlation increase from 0 to 1.

## Observed choice and rationale at the audit revision

`CurveValidator` applies financial shape and plausibility checks after construction. The method name `validate_no_arbitrage` includes heuristic economic limits, not a complete mathematical arbitrage certificate. Full validation first validates config, then no-arbitrage, monotonicity and bounds, stopping at the first error.

## Checks at the audit revision

- Discount: when arbitrage checking is enabled, combine zero, all actual knots and standard sample tenors below the last knot, sort/deduplicate, and check segment continuous forward `f=(ln D1−ln D2)/(t2−t1)` against min/max rates. Only finite positive DF pairs enter this check. Monotonicity uses standard times 0 to 50 years only within last knot+0.01; DF rises are allowed only when `allow_negative_rates` and the implied segment rate is below negative tolerance. Bounds at a separate fixed grid require positive DF, DF≤1 (or≤1.5 with negative-rate opt-in), and continuous zero rate within `[5×min_forward_rate,max_forward_rate]`.
- Forward: a fixed grid 0.25–10y checks lower bound when either arbitrage or forward-positivity is enabled. Adjacent grid jumps above 0.10 only log warnings; this monotonicity method never rejects. Bounds on another 0.25–10y grid enforce `[5×min,max]` even when arbitrage checks are disabled.
- Hazard: fixed 0.5–10y grid checks nonnegative hazard and configured maximum; survival on 0–10y must not increase beyond tolerance and bounds lie in [0,1]. Recovery lies in [0,1].
- Inflation: positive CPI on 0–20y grid; annualized growth `(CPI(t)/CPI(0))^(1/t)−1` on 1–10y grid; one-year forward ratio `CPI(t+1)/CPI(t)−1` on 1–30y grid. These are growth/plausibility checks, not literal CPI monotonicity.
- Base correlation: enforce matching vector lengths, nondecreasing correlation across stored detachments within tolerance when arbitrage enabled, and values in [0,1]. The separate monotonicity method is a no-op. Nondecreasing base correlation alone does not prove nonnegative tranche losses or spreads.

Defaults: forward range [-0.01,0.50], tolerance1e-10, max hazard0.50, annual CPI growth[-0.10,0.50], forward inflation[-0.20,0.50], max vol5, negative rates disabled. Recovery consistency tolerance1e-12 and minimum LGD for hazard guess1e-6 are separate controls. See `points.rs` for exact grids.

## Observed selection, limitations and alternatives

Discount/forward/hazard/inflation/base-correlation and XCCY discount outputs use the runtime validator; target-local checks may add constraints. Parametric outputs are not passed through this generic trait by runtime. Warn adds one warning without changing validation status; Error marks failure for the engine gate. Direct trait submethods do not themselves all validate config.

Most grids omit arbitrary interior extrema, and forward/hazard/inflation probes can extrapolate beyond supplied support. Several scalar comparisons do not explicitly reject NaN, relying on the curve builder/evaluator invariant; discount no-arbitrage skips invalid DF pairs. Search hazard cap10 versus validator cap0.5 and currency-specific rate bounds versus default negative-rate prohibition are intentionally separate implemented policies, not one consistent permitted domain.

Analytic segment checks or adaptive dense extrema searches would provide stronger coverage but require interpolation-specific contracts.

## Source and validation evidence

- Implementation: [validation/curves.rs](../../../../finstack-quant/calibration/src/validation/curves.rs) — five curve implementations and check ordering.
- Implementation: [validation/points.rs](../../../../finstack-quant/calibration/src/validation/points.rs) — exact fixed probe grids.
- Implementation: [validation/config.rs](../../../../finstack-quant/calibration/src/validation/config.rs) — economic thresholds and flag validation.
- Implementation: [validation/mod.rs](../../../../finstack-quant/calibration/src/validation/mod.rs) — Warn/Error result handling.
- Implementation: [step_runtime.rs](../../../../finstack-quant/calibration/src/step_runtime.rs) — which targets receive generic validation.
- Test evidence: [tests/calibration/validation.rs](../../../../finstack-quant/calibration/tests/calibration/validation.rs) — negative-rate opt-in, hidden actual-knot forward floor, CPI/hazard/forward/base-correlation bounds; inspected or located, not executed.
- The independent examples above cover scope, rate basis, segment caps
  and the limit of a correlation shape screen. Production admission,
  evaluator coverage, lifecycle and host behavior require qualification.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
