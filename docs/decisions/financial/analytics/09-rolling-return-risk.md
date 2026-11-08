# ANALYTICS-009: Rolling return, volatility, Sharpe and Sortino kernels with numerical rebuilds

**Decision status:** Accepted for the complete rolling return-risk window,
result, terminal-loss and numerical-control package below (2026-11-07).
Other panel and rolling-regression policies retain their owning scope.

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: rolling return-risk calculations

**Accepted:** 2026-11-07.

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

## Implementation evidence

[risk_metrics/rolling.rs](../../../../finstack-quant/analytics/src/risk_metrics/rolling.rs), [performance/rolling.rs](../../../../finstack-quant/analytics/src/performance/rolling.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
