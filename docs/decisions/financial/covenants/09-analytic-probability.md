# COVENANTS-09: Lognormal overlay and analytic marginal breach probability

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted decisions retain their scope.

**Implementation status:** Observed in the exact source paths identified below, including stated discrepancies and limitations.

**Validation status:** Source and identified test assertions inspected; tests, builds, numerical experiments and external benchmarks not executed. Test existence is not adoption or independent qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Model and formula

Stochastic forecasting is explicitly selected by `stochastic=true` and requires finite annualized log-volatility `sigma >= 0`; no volatility is calibrated or inferred. `num_paths=0` selects the analytic method. Defaults otherwise select deterministic maintenance forecasting, no volatility, no seed, no reference date, antithetic false and a 0.05 batch-reporting cutoff. Cutoff must be finite in `[0,1]` even in deterministic mode. Stochastic-only settings are not validated when stochastic mode is false.

For each active finite positive base projection `b`, use `T = whole_calendar_days(test_date-reference_date)/365.25`. A generic omitted reference is the day before the first period starts; dated-row wrappers require an explicit stochastic anchor. This is a hard-coded simulation clock, not a selectable accrual day count or trading-day year. Explicit references constrain dates even for deterministic calls. Inferred negative horizons are rejected when the positive-base stochastic branch computes them.

The overlay is `V(T)=b exp(-0.5 sigma² T + sigma W(T))`, a mean-preserving multiplicative perturbation of each caller projection. For `sigma=0` or `T=0`, use the exact base breach indicator. Nonpositive finite base values, including valid negative net cash/custom metrics, remain deterministic rather than using an additive or shifted distribution. Invalid-ratio sentinels remain probability one; inactive dates remain zero. Denominator and trigger observations are not themselves shocked.

For positive threshold `h`, compute `z=(ln(h/b)+0.5 sigma²T)/(sigma sqrt(T))`. An upper-bound covenant uses `1-Phi(z)`; a lower-bound covenant uses `Phi(z)`. For `h<=0` with a positive base, upper-bound breach is one and lower-bound breach zero. `Phi` delegates to core `norm_cdf`, using the statrs standard-normal CDF. Analytic output has zero sampling standard error; this is not zero model/calibration uncertainty. No default intensity, borrower cash-flow dynamics, ratio-component correlation or first-passage boundary is implied.

## Rationale, alternatives and limitations

The source describes the GBM overlay and shared deterministic fallbacks; it gives no empirical calibration or contract-specific adoption evidence for lognormal covenant metrics or the 365.25 clock. Additive/shifted ratio models, joint numerator/denominator simulation and fitted distributions offer different negative-value and tail behavior. They are alternatives, not defaults approved by this record.

Finite inputs are not sufficient to guarantee finite intermediates: `h/b`, `sigma²T` and the denominator may overflow/underflow. The upper tail is evaluated by subtraction from one, without a survival/log-tail branch, and final analytic probabilities receive no finite/range post-check. Extreme-scale and tail behavior is not qualified by the inspected evidence.

## Evidence

Owning [configuration, horizon and analytic method](../../../../finstack-quant/covenants/src/forward.rs), [core CDF](../../../../finstack-quant/core/src/math/special_functions.rs), and [dated horizon requirement](../../../../finstack-quant/covenants/src/series.rs). Inline forward test `num_paths_zero_selects_analytic_mode` uses an approximate hand calculation near 0.47526 with absolute tolerance 0.001; it is inspected fixture evidence, not an independently run benchmark. Degenerate/nonfinite and date rejection assertions are in forward/audit tests.

The repository [Glasserman reference](../../../REFERENCES.md#glasserman-2004-monte-carlo) provides mathematical background, not calibration evidence for borrower metrics. No external numerical source was checked or computation run.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
