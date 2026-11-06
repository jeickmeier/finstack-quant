# FACTOR-011: Credit calibration panel, clock, and DTS preparation

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

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

Daily validation allows holiday-like gaps up to seven days, so unequal elapsed intervals still count as equal observations. A single observation can enter some strategies and produce zero estimated variance; LedoitWolf subsequently requires two complete working observations. As-of spreads need not numerically equal the last historical values. The spread range is a domain heuristic, not definitive detection of bp mistakes. The crate README opening still advertises sparse history, whereas its later calibration section and the implementation require complete observations. Duration history is a single static value, not a dated series.

## Alternatives and tradeoffs

Calendar-aware daily grids, elapsed-time volatility scaling, ragged panels, percentage spread changes and dated DTS durations each change estimation semantics. Sparse private helpers do not establish public support for ragged calibration.

## Implementation and evidence

- [src/credit/calibration/config.rs](../../../../finstack-quant/factor/src/credit/calibration/config.rs) — inspected implementation.
- [src/credit/calibration/inputs.rs](../../../../finstack-quant/factor/src/credit/calibration/inputs.rs) — inspected implementation.
- [src/credit/calibration/validation.rs](../../../../finstack-quant/factor/src/credit/calibration/validation.rs) — inspected implementation.
- [src/credit/calibration/panel.rs](../../../../finstack-quant/factor/src/credit/calibration/panel.rs) — inspected implementation.
- [src/credit/calibration/calibrator.rs](../../../../finstack-quant/factor/src/credit/calibration/calibrator.rs) — inspected implementation.
- [src/credit/units.rs](../../../../finstack-quant/factor/src/credit/units.rs) — inspected implementation.

[src/credit/calibration/tests.rs](../../../../finstack-quant/factor/src/credit/calibration/tests.rs) contains inspected or located date/None rejection, bp conversion, levels-to-changes volatility, DTS hand arithmetic and duration requirements. The contemporaneous DTS fixture computes (100+2*200)/3 bp at the first date and 250 bp later. [tests/credit_calibration.rs](../../../../finstack-quant/factor/tests/credit_calibration.rs) locates as-of/universe and config validation cases.

[Ben Dor DTS](../../../REFERENCES.md#ben-dor-2007-dts) is the catalogued duration-times-spread convention reference; it does not independently validate this weighted sequential factor-construction algorithm.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
