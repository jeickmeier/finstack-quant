# FACTOR-011: Credit calibration panel, clock, and DTS preparation

**Decision status:** Accepted 2026-11-17 for complete credit panels, explicit observation clocks and DTS preparation.

**Implementation status:** Complete-panel admission, basis-point conversion and time-local DTS weights are implemented. Exact calendar admission, explicit method choices, minimum history and anchor consistency remain pending.

**Validation status:** All 82 tests selected by the calibration filter passed on 2026-11-17 against current behavior. Independent exact arithmetic checked a lagged DTS move. The accepted calendar, anchor and minimum-history rules remain to be qualified.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Require complete issuer and generic series on one qualified observation grid and cut. Daily uses an explicit business calendar. Monthly and quarterly preserve the declared origin and end-of-month convention. Annualize per-observation variance by `252`, `12` and `4`, respectively. Require at least two usable spread changes for variance calibration and apply any stricter elected estimator requirements.
2. Accept finite decimal spread levels in the supported range `(-0.5, 2)` and convert once to basis points. Explicitly elect levels or first differences in bp, weighting, issuer-fit policy, volatility estimator and covariance strategy. Set the calibration anchor from the final qualified panel observation over the same issuer universe.
3. Use a sourced fixed positive duration per issuer for DTS. For spread changes use duration times opening spread; for levels use duration times same-date spread. Require finite strictly positive weights. With opening spreads `100` and `200 bp`, durations `1` and `2` years and changes `+10` and `−20 bp`, DTS weights `100` and `400` give a weighted change of `−14 bp`.

## Observed choice and rationale

Calibrate a deterministic credit hierarchy on a fully aligned issuer spread panel, using either spread levels or first differences and an explicitly chosen observation frequency. Public validation rejects missing observations even though internal statistical kernels retain sparse representations.

## Input domain and conventions

All supplied issuer/generic/as-of spreads are decimal spreads strictly in (-0.5,2), finite; 0.01 means 100 bp. Idiosyncratic overrides are finite nonnegative annualized decimal-spread volatility. Durations are in years. Inputs are converted once to bp (including overrides); artifacts use bp, bp volatility and bp² covariance. As-of must equal the final panel date and as-of/history issuer sets must agree exactly.

## Algorithm stages

1. Validate shrink alpha in [0,1], ridge alpha >=0, EWMA lambda in (0,1), all finite, and reject dotted hierarchy dimension keys.
2. Require increasing dates. Monthly/quarterly grids are stepped from the first date, preserve its EOM status, and require exact scheduled dates. Daily dates must be weekdays with no gap above seven calendar days; they are not checked against a holiday calendar.
3. Reject every None issuer observation, unequal series lengths, empty dates, or generic length mismatch. Check decimal ranges, overrides and required DTS durations.
4. Convert spreads/overrides by 10,000. Returns mode uses differences s_t-s_(t-1) in bp, not percentage/log returns; Levels mode retains bp levels.
5. Equal weights are 1. DTS weights are fixed issuer duration times spread_bp: current observation for Levels, starting observation for each Returns interval. Require positive finite products. Anchor weights instead use the supplied as-of cross-section.
6. Classify issuer mode: GloballyOff gives BucketOnly; Dynamic overrides win, otherwise compare available levels or adjacent return count with min_history.

## Selection and controls

Defaults are Monthly, Returns, DTS weighting, Sample volatility, FullSampleRepaired covariance, no beta shrinkage, empty hierarchy, and GloballyOff issuer betas. Annualization is 252/12/4 for Daily/Monthly/Quarterly regardless of observed elapsed days. `from_panel` chooses the last date and available final spreads but does not fully validate the panel. Extra durations are rejected under DTS; Equal does not validate them and later rows default an absent duration to 1.

## Limitations, failures, and fallbacks

Daily validation allows gaps up to seven days, so an omitted trading observation can pass as a closure. A single observation can enter some strategies and produce zero estimated variance; LedoitWolf subsequently requires two complete working observations. Supplied as-of spreads can differ from the last historical values. The spread range defines supported inputs. The crate README now describes the current complete-panel requirement. Duration history is one fixed value per issuer.

## Alternatives and tradeoffs

Calendar-aware daily grids, elapsed-time volatility scaling, ragged panels, percentage spread changes and dated DTS durations each change estimation semantics. Sparse private helpers do not establish public support for ragged calibration.

## Implementation and evidence

- [src/credit/calibration/config.rs](../../../../finstack-quant/factor/src/credit/calibration/config.rs) — inspected implementation.
- [src/credit/calibration/inputs.rs](../../../../finstack-quant/factor/src/credit/calibration/inputs.rs) — inspected implementation.
- [src/credit/calibration/validation.rs](../../../../finstack-quant/factor/src/credit/calibration/validation.rs) — inspected implementation.
- [src/credit/calibration/panel.rs](../../../../finstack-quant/factor/src/credit/calibration/panel.rs) — inspected implementation.
- [src/credit/calibration/calibrator.rs](../../../../finstack-quant/factor/src/credit/calibration/calibrator.rs) — inspected implementation.
- [src/credit/units.rs](../../../../finstack-quant/factor/src/credit/units.rs) — inspected implementation.

[Calibration tests](../../../../finstack-quant/factor/src/credit/calibration/tests.rs) and [integration tests](../../../../finstack-quant/factor/tests/credit_calibration.rs) passed against current behavior, including complete observations, basis-point conversions, DTS and duration requirements. Qualify business-calendar gaps, exact anchor equality and the selected minimum working sample.

[Ben Dor DTS](../../../REFERENCES.md#ben-dor-2007-dts) is the catalogued duration-times-spread convention reference; it does not independently validate this weighted sequential factor-construction algorithm.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
