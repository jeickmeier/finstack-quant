# ANALYTICS-005: Log-wealth drawdowns, episode segmentation and path measures

**Decision status:** Accepted for the complete path, window, scalar-measure,
episode, output and failure package below (2026-11-07). Other panel, wealth
and summary policies retain the scope of their owning records.

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: drawdown paths and episodes

**Accepted:** 2026-11-07.

This package covers path, window, scalar-measure, episode, output and failure
choices in this record. Accepted terminal-loss/actual-date requirements and
record-06 aggregation conventions remain constraints.

### Wealth, windows and scalar measures

- Start each ticker's analysis path at wealth one, with the actual valuation
  immediately before its first included return as the opening anchor.
  Geometrically compound valid simple investment returns. At endpoint `i`,
  use `d_i = W_i / max(1, W_1, ..., W_i) - 1`, a signed decimal drawdown in
  `[-1, 0]`. Use compensated log wealth and log peaks while wealth is positive,
  avoiding reconstruction of absolute wealth when it would overflow.
- Rebuild a selected active window independently from its actual opening
  anchor. Earlier high-water marks do not carry into this analysis-window
  view. No inherited-peak mode or automatic lifetime-drawdown interpretation
  is introduced.
- Preserve record 02's terminal wipeout state explicitly. Exact terminal
  loss gives drawdown `-1` and an open episode; percentage-return continuation
  remains invalid. Do not infer terminal status from a displayed drawdown
  rounded to `-1`: an extremely small positive wealth/peak ratio may round to
  the same number.
- Return one drawdown per valid active return-ending observation. The
  opening zero is used only to anchor episode peak dates/durations; it is not
  another sample in scalar averages. Use each ticker's own active span, with
  no calendar-gap filling or inferred intraperiod highs, lows or recoveries.

For `N >= 1` observed endpoint drawdowns:

| Standalone measure | Accepted definition |
| --- | --- |
| Maximum drawdown | `min(d_i)`, retaining the negative sign. |
| Mean drawdown | `sum(d_i) / N`, retaining the negative sign. |
| Ulcer index | `sqrt(sum(d_i^2) / N)`, nonnegative. |
| Pain index | `sum(abs(d_i)) / N`, nonnegative. |

Include observed zeros with equal endpoint weight, and apply no annualization
or horizon multiplier. A singleton path is sufficient. Valid all-zero paths
give zero for all four measures. Empty scalar results are undefined; an empty
returned path remains an empty collection. Equal observation weights do not
measure exact elapsed underwater time when calendar gaps differ.

### Episodes, dates and selection

| Topic | Accepted complete convention |
| --- | --- |
| Entry and recovery | An episode comprises a contiguous run of computed `d < 0`, recovering at the first observed `d = 0`. Remove the hidden `-1e-15` classification cutoff; near recovery does not close an episode. |
| Start date | The most recent observed peak immediately before the first decline. A peak plateau therefore uses its last peak observation. An immediate first-return loss uses the actual opening anchor. |
| Valley ties | Use the first observation attaining the deepest drawdown in that episode. |
| Open episodes | Include the observed valley, retain an unavailable recovery date, and measure elapsed duration through the last observed date. Do not invent a recovery. |
| Duration clock | Actual calendar `end_date - start_date` days, without an inclusive-day increment or interpolation. For an open episode, use the last observation as the duration endpoint only. |
| Ranking | Deepest first; equal depths ordered by chronological start, then valley. |
| Display count | Require explicit nonnegative integer `n`. Zero returns `[]`; if fewer than `n` episodes exist, return all available without padding. Remove the host-only five-row default. This output limit does not alter maximum-duration scanning or all-episode Burke; Sterling's positive financial count remains separately governed by record 06. |
| Alignment | Require exact date/value alignment and strictly increasing dates, including the actual opening anchor for dated episodes. Reject malformed inputs rather than truncating to the shorter sequence. |

Maximum drawdown duration examines all episodes, including open episodes.
For valid nonempty history without an episode, it is zero. With no eligible
history it is unavailable, represented as a nullable integer duration across
Rust, Python, WASM and serialization. Empty episode collections remain `[]`.
Validate ticker/count requests and required date alignment before returning
an empty selection, including when `n = 0`.

Remove `truncated_at_start` from the supported episode output: every produced
path has a known local opening peak under the rebased-window contract, so an
immediate loss is not missing-peak censoring. Also remove the fixed
`near_recovery_threshold = 0.01 * valley_depth` field. It is a derived 99%-
recovery alert threshold rather than an observed recovery event. This package
adds no alert configuration. Binding/schema and existing UI episode-table
columns follow both removals.

### Benchmark comparison, errors and qualification

Align portfolio and benchmark on their exact common active return dates,
independently rebuild both drawdown paths from the common opening anchor,
then return portfolio drawdown minus benchmark drawdown pointwise. Thus
`-10% - (-20%) = +0.10` means ten percentage points shallower portfolio
drawdown. This is a difference of drawdown levels, not a relative-wealth
drawdown or compounded outperformance measure. No common return observations
produce an empty difference series. Two terminal wipeout drawdowns can have
difference zero without dividing their zero wealth values.

Invalid input domains, ticker/count requests, date shapes and chronology are
explicit validation errors. Nonfinite or out-of-domain computed paths or
statistics are calculation errors, rather than missing history or zero risk.
Errors fail the affected metric call; unavailable histories retain per-ticker
undefined/nullable outcomes. Do not clamp real losses, repair malformed dates
or substitute another path estimator. Record 01 retains broader panel/window
request and return-provenance ownership; record 02 owns terminal investment
events; record 06 owns the accepted reward/CDaR compositions; broader summary
failure behavior remains separately scoped.

The geometric path/initial-peak construction is also used by
[PerformanceAnalytics Drawdowns](https://raw.githubusercontent.com/braverock/PerformanceAnalytics/master/R/Drawdowns.R),
and its [episode code](https://raw.githubusercontent.com/braverock/PerformanceAnalytics/master/R/findDrawdowns.R)
uses strict negative/nonnegative classification. [Peter Martin's definition](https://www.tangotools.com/ui/ui.htm)
supports RMS percentage drawdowns and explains the limits of sparse
observations. These references do not establish identical date/ranking/
duration conventions or qualify extreme-scale and near-recovery arithmetic.

## Implementation evidence

[drawdown.rs](../../../../finstack-quant/analytics/src/drawdown.rs), [performance/aggregation.rs](../../../../finstack-quant/analytics/src/performance/aggregation.rs), [performance/scalar.rs](../../../../finstack-quant/analytics/src/performance/scalar.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
