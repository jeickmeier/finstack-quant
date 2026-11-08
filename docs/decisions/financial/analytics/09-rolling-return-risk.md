# ANALYTICS-009: Rolling return, volatility, Sharpe and Sortino kernels with numerical rebuilds

**Decision status:** Accepted for the complete rolling return-risk window,
result, terminal-loss and numerical-control package below (2026-11-07).
Other panel and rolling-regression policies retain their owning scope.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice and domain

Rolling metrics use fixed **observation-count** windows within one active ticker span and label each completed window by its ending date. Public window zero is an error; too few observations gives empty vectors. A one-observation window is valid for compounded return but returns NaN for sample-risk ratios/volatility. There is no calendar-duration or expanding-window mode here.

## Accepted decision: rolling return-risk calculations

**Accepted:** 2026-11-07. **Implementation:** Partial; selected source
inspected at `5140b499cc`. Explicit host inputs, exact alignment, terminal
state, initial/emitted-state validation, variance-clamp removal, positive-risk
underflow handling, calculation errors and carrier restoration require
reconciliation. No runtime estimator checks were executed.

This package covers window, alignment, result, terminal-loss and numerical-
control choices for rolling return, volatility, Sharpe and Sortino. Existing
frequency, cash, MAR, scalar estimator and zero-risk decisions remain
constraints. No rolling drawdown or other new estimator is introduced.

### Window selection and output contract

- Require an explicit positive integer `window = W` across Rust, Python and
  WASM. Reject zero, omission and invalid host values; remove the implicit
  63-observation default from these methods. This count describes observed
  returns rather than calendar duration.
- Use only the selected ticker's valid contiguous active span. Construct
  complete trailing windows advancing one observation at a time; do not
  borrow pre-active-range history, pad missing dates or emit partial windows.
- With `N` eligible returns, output `max(N - W + 1, 0)` values and label each
  by its final return-ending date. Oversized windows and empty active histories
  give empty date/value arrays with the correct metric label. Do not prepend
  warm-up NaNs or drop unavailable values from completed windows.
- Require exact return/date alignment and strictly increasing dates. Remove
  private shorter-length truncation. Validate ticker, window and all required
  cash/MAR/frequency inputs before returning empty or unavailable outcomes.
  Other general panel/window input ownership remains with record 01.

| Completed window | Required metric interpretation |
| --- | --- |
| Rolling return, `W >= 1` | `product(1 + r_i) - 1` over that window from its own unit baseline. No annualization, dated CAGR or extra opening-zero return. |
| Rolling volatility, `W >= 2` | Accepted sample SD with `W - 1` denominator and declared-frequency square-root annualization. |
| Rolling Sharpe, `W >= 2` | Accepted arithmetic excess-return Sharpe, using explicit annual-effective cash converted at the declared frequency. |
| Rolling Sortino, `W >= 2` | Accepted all-observation downside denominator and explicit per-observation MAR, with accepted frequency scaling. |
| Volatility/Sharpe/Sortino, `W = 1` | One aligned undefined value for every completed window; do not replace it with zero risk or a zero-risk infinity. |

Each result has the mathematical meaning of applying the corresponding
accepted scalar estimator to that exact window. Numerical equivalence needs
independently justified tolerances, rather than bitwise equality or reliance
on production scalar code as its own oracle. Available constant histories and
valid zero-risk ratios retain their accepted zero/undefined/signed-infinity
rules. Preserve these distinct values in hosts and wire serialization.
Keep the canonical `return`, `volatility`, `sharpe` or `sortino` metric label
and parallel date/value arrays. Restoring a `DatedSeries` must validate equal
lengths and chronological dates rather than repair or truncate them; this does
not settle other panel or rolling-regression carrier contracts.

### Terminal returns and history domain

Validate the complete supplied investment history under record 02 before
window selection. A valid terminal return `-1` ends its observed span; a
selected range cannot legitimize subsequent percentage returns. Every full
rolling-return window containing that terminal observation returns exactly
`-1`. Keep terminal state explicit instead of sending `log1p(-1)` through a
compensated finite-log accumulator. The risk estimators include the valid
`-1` return normally, subject to their own window minima. Do not add future
zero returns, restore positive terminal wealth or infer a terminal event from
a rounded output.

For `[+1%, +2%, -100%]` and `W = 2`, rolling compounded returns are
`[+3.02%, -100%]`, labelled by the second and third observation dates.

### Numerical controls and failures

- Retain incremental Welford mean/M2 for volatility/Sharpe, compensated return
  sums and downside-square tracking for Sortino, and compensated finite
  log-growth accumulation for ordinary compounded-return windows. Keep
  separate insert/remove updates so compensation is not lost.
- Retain a scheduled full-window rebuild after 1,024 slide updates and an
  earlier rebuild when incremental state becomes nonfinite, a risk sum is
  negative, or a nonnegative risk sum collapses to at most `sqrt(epsilon)` of
  its largest scale since the last rebuild. These are internal numerical
  controls, not financial thresholds, smoothing, or public configuration.
  They may be revised through independent accuracy/performance qualification
  while preserving the accepted window semantics.
- Validate initial states before their first output and every state before
  emission, including reconstructed states. On an incremental warning, reconstruct from the
  current raw window; if the rebuilt moments/sums remain invalid or nonfinite,
  fail the affected rolling metric call with an explicit calculation error.
  Do not clamp residual negative or NaN variance to zero, or apply a floor to
  small positive risk. A small-risk rebuild is not permission to erase risk.
- A zero computed risk also needs a genuine zero-risk window. For volatility
  and Sharpe, zero sample dispersion requires identical return observations.
  For Sortino, zero downside requires no observation below MAR; varying
  returns entirely at or above MAR can legitimately have zero downside.
  Otherwise do not erase positive risk through squared-intermediate underflow
  or residual cancellation. Recompute the same estimator with numerically
  adequate scaling, or return a calculation error if the requested result
  cannot be represented.
- Require valid adequately scaled moment/risk state and representable requested
  outputs under the accepted scalar contract in record 04. Unrequested
  standalone mean, SD, variance or annualized numerator/risk reconstruction
  must not spoil a representable rolling ratio. Nonfinite ratios with positive
  positive risk are calculation failures. Intentional signed infinity from
  valid finite nonzero reward and exactly zero measured risk remains valid;
  unavailable estimator minima and valid `0/0` remain undefined at their
  completed-window dates. Request and calculation errors fail the metric
  call rather than returning a partially repaired series.

These controls do not establish an accuracy budget by themselves. Independent
qualification must cover first windows, rebuild boundaries, large observations
leaving a window, tiny positive retained risk, high offsets, extreme finite
inputs, terminal losses and nonfinite wire round-trips. Calendar-duration,
expanding, weighted and serial-correlation-adjusted estimators remain outside
this package, as does the separate rolling-regression contract in record 12.

The [pandas rolling documentation](https://pandas.pydata.org/docs/reference/api/pandas.DataFrame.rolling.html)
distinguishes integer observation windows from time-offset windows and
documents right-edge labels; this API's compact output count remains its own
contract. [Rust's documented `f64::max` behavior](https://doc.rust-lang.org/std/primitive.f64.html#method.max)
ignores one NaN operand, so a final `.max(0.0)` is not a validity check and can
turn an invalid variance into a finite zero. These references do not qualify
the accuracy of the rolling numerical controls.
Reconciliation and qualification are owned by the
[record-09 follow-up](../FOLLOW_UPS.md#analytics-rolling-return-risk).

## Algorithms and numerical controls

Volatility and Sharpe share a rolling Welford mean/M2 kernel. The first window is built from scratch. Remove the outgoing observation using `mean_keep=mean−(old−mean)/(w−1)` and the corresponding M2 correction; add the new value with Welford's update. Emit sample variance `max(M2/(w−1),0)` and annualize with A or sqrt(A). Sharpe uses geometric cash conversion and the same ratio convention as the scalar estimator.

The kernel remembers the largest M2 scale since rebuild. It fully reconstructs the current window if mean/M2 becomes nonfinite, M2 becomes negative, M2 drops to at most sqrt(machine epsilon) times the remembered scale, or 1,024 updates have elapsed. This relative trigger is intended to remove cancellation residue after a large observation exits; it is not a policy that small positive variance is zero. The final nonnegative clamp is still present after reconstruction.

Rolling Sortino maintains a Neumaier sum of returns and a running sum of squared shortfalls below per-period MAR. Insert and remove return sums separately to preserve compensation. Update downside squares on entry/exit and reconstruct when sums are nonfinite, downside sum is negative or small relative to its remembered peak, or 1,024 updates elapse. Divide downside sum by the full window count; emit `(mean−MAR)*sqrt(A)/downside`, preserving common undefined/zero-risk conventions. Nonfinite MAR yields aligned NaNs.

Rolling total return maintains a Neumaier sum of `log1p(r)`, separately adding/removing logs, and emits `expm1(sum)`. Rebuild on nonfinite sum or every 1,024 updates. It does not apply the shared wealth engine's wipeout floor; validated panel inputs already exclude r<=−1. A nonfinite log sum emits NaN; overflow in the final exponential can still produce infinity. Direct private kernels use the shorter return/date length, while public spans are aligned.

The observed branches above require reconciliation with the accepted package:
invalid numerical state must not become zero risk through `.max(0.0)` or an
unavailable-window NaN. Valid terminal returns require explicit handling;
nonfinite MAR is a request error and final overflow a calculation error.
Exact alignment and initial/every-output validity gates are required.

## Selection, limits and alternatives

`Performance.rolling_*` validates ticker/window and supplies frequency A. Each output `DatedSeries` has a metric label and explicit nonfinite sequence serialization. Rebuild intervals and cancellation thresholds are internal constants. Ordinary updates are constant work; frequent reconstruction can raise total cost toward window-length work per observation. They are numerical recovery paths, not data filling or volatility smoothing.

Full independent recomputation of every window is a computational alternative; exponentially weighted, calendar-length and autocorrelation-adjusted estimates are different methods not implemented in these functions. The inspected evidence does not establish an independent accuracy budget for the fixed rebuild controls over all supported sizes/scales.

## Implementation and validation evidence

The [accepted volatility/Sharpe decision](04-return-risk-ratios.md#accepted-decision-sample-volatility-and-sharpe)
owns the sample estimator, arithmetic excess numerator and unadjusted frequency
scaling within rolling windows.

The [accepted downside/Sortino decision](04-return-risk-ratios.md#accepted-decision-downside-deviation-and-sortino)
owns the full-window denominator, frequency scaling, two-observation reporting
minimum and zero-downside outcomes for available rolling Sortino windows.
This record separately accepts the complete window, output, terminal-loss,
numerical-control and failure contract above. The accepted scalar definitions
remain authoritative within each completed window.

- [Rolling kernels](../../../../finstack-quant/analytics/src/risk_metrics/rolling.rs), [public window dispatch](../../../../finstack-quant/analytics/src/performance/rolling.rs), and [scalar formulas](04-return-risk-ratios.md).
- Located inline pointwise comparisons, long/high-offset volatility, small retained returns after a positive outlier, and `undefined_windows_round_trip_through_json` cover numerical and wire invariants. [Audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) compare recovery after outliers with freshly recomputed scalar windows; they are partly same-kernel comparisons rather than external estimators.
- Numerical context: [Welford](../../../REFERENCES.md#welford-1962) and [accuracy/stability](../../../REFERENCES.md#higham-accuracy-and-stability). The precise 1,024 and sqrt-epsilon controls are repository choices, not established by those references alone.

No timing benchmarks or numerical tests ran in this audit.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
