# ANALYTICS-009: Rolling return, volatility, Sharpe and Sortino kernels with numerical rebuilds

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice and domain

Rolling metrics use fixed **observation-count** windows within one active ticker span and label each completed window by its ending date. Public window zero is an error; too few observations gives empty vectors. A one-observation window is valid for compounded return but returns NaN for sample-risk ratios/volatility. There is no calendar-duration or expanding-window mode here.

## Algorithms and numerical controls

Volatility and Sharpe share a rolling Welford mean/M2 kernel. The first window is built from scratch. Remove the outgoing observation using `mean_keep=mean−(old−mean)/(w−1)` and the corresponding M2 correction; add the new value with Welford's update. Emit sample variance `max(M2/(w−1),0)` and annualize with A or sqrt(A). Sharpe uses geometric cash conversion and the same ratio convention as the scalar estimator.

The kernel remembers the largest M2 scale since rebuild. It fully reconstructs the current window if mean/M2 becomes nonfinite, M2 becomes negative, M2 drops to at most sqrt(machine epsilon) times the remembered scale, or 1,024 updates have elapsed. This relative trigger is intended to remove cancellation residue after a large observation exits; it is not a policy that small positive variance is zero. The final nonnegative clamp is still present after reconstruction.

Rolling Sortino maintains a Neumaier sum of returns and a running sum of squared shortfalls below per-period MAR. Insert and remove return sums separately to preserve compensation. Update downside squares on entry/exit and reconstruct when sums are nonfinite, downside sum is negative or small relative to its remembered peak, or 1,024 updates elapse. Divide downside sum by the full window count; emit `(mean−MAR)*sqrt(A)/downside`, preserving common undefined/zero-risk conventions. Nonfinite MAR yields aligned NaNs.

Rolling total return maintains a Neumaier sum of `log1p(r)`, separately adding/removing logs, and emits `expm1(sum)`. Rebuild on nonfinite sum or every 1,024 updates. It does not apply the shared wealth engine's wipeout floor; validated panel inputs already exclude r<=−1. A nonfinite log sum emits NaN; overflow in the final exponential can still produce infinity. Direct private kernels use the shorter return/date length, while public spans are aligned.

## Selection, limits and alternatives

`Performance.rolling_*` validates ticker/window and supplies frequency A. Each output `DatedSeries` has a metric label and explicit nonfinite sequence serialization. Rebuild intervals and cancellation thresholds are internal constants. Ordinary updates are constant work; frequent reconstruction can raise total cost toward window-length work per observation. They are numerical recovery paths, not data filling or volatility smoothing.

Full independent recomputation of every window is a computational alternative; exponentially weighted, calendar-length and autocorrelation-adjusted estimates are different methods not implemented in these functions. The inspected evidence does not establish an independent accuracy budget for the fixed rebuild controls over all supported sizes/scales.

## Implementation and validation evidence

- [Rolling kernels](../../../../finstack-quant/analytics/src/risk_metrics/rolling.rs), [public window dispatch](../../../../finstack-quant/analytics/src/performance/rolling.rs), and [scalar formulas](04-return-risk-ratios.md).
- Located inline pointwise comparisons, long/high-offset volatility, small retained returns after a positive outlier, and `undefined_windows_round_trip_through_json` cover numerical and wire invariants. [Audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) compare recovery after outliers with freshly recomputed scalar windows; they are partly same-kernel comparisons rather than external estimators.
- Numerical context: [Welford](../../../REFERENCES.md#welford-1962) and [accuracy/stability](../../../REFERENCES.md#higham-accuracy-and-stability). The precise 1,024 and sqrt-epsilon controls are repository choices, not established by those references alone.

No timing benchmarks or numerical tests ran in this audit.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
