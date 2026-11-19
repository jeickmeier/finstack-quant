# COVENANTS-09: Lognormal overlay and analytic marginal breach probability

**Decision status:** Accepted 2026-11-17 for the positive-projection lognormal overlay, explicit clock and stable analytic tails.

**Implementation status:** The mean-preserving lognormal model is implemented. Explicit clock and volatility provenance, deterministic-only handling outside the positive domain and stable tail evaluation remain pending.

**Validation status:** Both focused analytic tests passed on 2026-11-17. Independent normal-tail arithmetic reproduced cancellation at nine standard deviations. Extreme scales, tail accuracy in production and the accepted boundary behavior remain to be qualified.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Use the mean-preserving lognormal overlay on a valid positive base projection with positive time and volatility. Use the explicit reference date and divide elapsed calendar days by 365.25. The supplied annual log-volatility uses that basis. Hold trigger and denominator observations fixed. Report a marginal probability conditional on the projection and volatility. [Forecast composition](08-forecast-composition.md).
2. Use the exact base-case comparison when time or volatility is zero. A valid nonpositive projection retains its deterministic assessment and carries no modeled probability. Indeterminate, incomplete and unassessed tests carry no modeled probability. In the positive stochastic domain, a nonpositive threshold gives probability one for a maximum and zero for a minimum.
3. For positive thresholds, compute the standardized boundary from a stable log ratio and use the normal lower tail for a minimum and upper tail for a maximum. Validate the result before publishing a finite probability in `[0,1]`. Check extreme arithmetic and use stable equivalent expressions wherever the exact result remains representable. [Normal distribution reference](https://www.itl.nist.gov/div898/handbook/eda/section3/eda3661.htm).

## Observed model and formula

Stochastic forecasting is explicitly selected by `stochastic=true` and requires finite annualized log-volatility `sigma >= 0`; no volatility is calibrated or inferred. `num_paths=0` selects the analytic method. Defaults otherwise select deterministic maintenance forecasting, no volatility, no seed, no reference date, antithetic false and a 0.05 batch-reporting cutoff. Cutoff must be finite in `[0,1]` even in deterministic mode. Stochastic-only settings are not validated when stochastic mode is false.

For each active finite positive base projection `b`, use `T = whole_calendar_days(test_date-reference_date)/365.25`. A generic omitted reference is the day before the first period starts; dated-row wrappers require an explicit stochastic anchor. This is a hard-coded simulation clock, not a selectable accrual day count or trading-day year. Explicit references constrain dates even for deterministic calls. Inferred negative horizons are rejected when the positive-base stochastic branch computes them.

The overlay is `V(T)=b exp(-0.5 sigma² T + sigma W(T))`, a mean-preserving multiplicative perturbation of each caller projection. For `sigma=0` or `T=0`, use the exact base breach indicator. Nonpositive finite base values, including valid negative net cash/custom metrics, remain deterministic rather than using an additive or shifted distribution. Invalid-ratio sentinels remain probability one; inactive dates remain zero. Denominator and trigger observations are not themselves shocked.

For positive threshold `h`, compute `z=(ln(h/b)+0.5 sigma²T)/(sigma sqrt(T))`. An upper-bound covenant uses `1-Phi(z)`; a lower-bound covenant uses `Phi(z)`. For `h<=0` with a positive base, upper-bound breach is one and lower-bound breach zero. `Phi` delegates to core `norm_cdf`, using the statrs standard-normal CDF. Analytic output has zero sampling standard error; this is not zero model/calibration uncertainty. No default intensity, borrower cash-flow dynamics, ratio-component correlation or first-passage boundary is implied.

## Rationale, alternatives and limitations

The overlay represents uncertainty around a caller-supplied positive projection. Calibration and empirical fitness remain the producer's responsibility. Additive or shifted distributions and joint numerator and denominator processes require their own model contracts.

Finite inputs are not sufficient to guarantee finite intermediates: `h/b`, `sigma²T` and the denominator may overflow/underflow. The upper tail is evaluated by subtraction from one, without a survival/log-tail branch, and final analytic probabilities receive no finite/range post-check. Extreme-scale and tail behavior is not qualified by the inspected evidence.

## Evidence

Owning [configuration, horizon and analytic method](../../../../finstack-quant/covenants/src/forward.rs), [core CDF](../../../../finstack-quant/core/src/math/special_functions.rs), and [dated horizon requirement](../../../../finstack-quant/covenants/src/series.rs). The focused analytic tests passed. `num_paths_zero_selects_analytic_mode` uses the moderate fixture near 0.47526 with absolute tolerance 0.001. The Monte Carlo agreement test compares against the crate's analytic result.

An independent calculation using `0.5 * erfc(9 / sqrt(2))` gives approximately `1.12858840595384e-19` for the upper tail. Subtraction of the rounded CDF from one gives zero in binary64. This demonstrates cancellation in the formula. Production accuracy over the accepted domain remains to be qualified.

## Follow-up tracking

Implementation reconciliation and qualification are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
