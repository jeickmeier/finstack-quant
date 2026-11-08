# ANALYTICS-005: Log-wealth drawdowns, episode segmentation and path measures

**Decision status:** Accepted for the complete path, window, scalar-measure,
episode, output and failure package below (2026-11-07). Other panel, wealth
and summary policies retain the scope of their owning records.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice and input domain

The [accepted record-06 package](06-drawdown-reward-and-cdar.md#accepted-decision-drawdown-reward-and-cdar)
constrains its metric aggregations: equal return-endpoint weights, observed
zero drawdowns included, no extra baseline observation in moment/tail
denominators, open episode valleys included, and explicit Sterling count.
The path, window, episode, date, duration and standalone measure contracts
are separately accepted in this record below.

Drawdowns are measured relative to an initial wealth peak of one and subsequent observed peaks, using log wealth so absolute wealth need not be representable. Each observation receives equal weight in path averages; actual dates matter for episode durations. Performance windows are independently rebased. This is observed-window drawdown, not a continuous-time maximum between observations.

## Accepted decision: drawdown paths and episodes

**Accepted:** 2026-11-07. **Implementation:** Partial; selected source
inspected at `5140b499cc`. Recovery cutoff, exact alignment, missing-history
outcomes, display-count defaults, episode metadata and error propagation
require reconciliation. No runtime estimator checks were executed.

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
columns must be reconciled with both removals when implemented.

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

For daily dates January 1–6 with baseline-inclusive drawdowns
`[0, -10%, -20%, -20%, 0, -5%]`, the first episode starts January 1, has its
first deepest valley January 3, recovers January 5 and lasts four days. The
second starts January 5, has its observed valley January 6, has no recovery
date and has elapsed duration one day. The initial zero is excluded from the
five endpoint observations used by scalar measures.
Reconciliation and independent qualification are owned by the
[record-05 follow-up](../FOLLOW_UPS.md#analytics-drawdown-paths-and-episodes).

## Algorithm and controls

1. Accumulate log wealth L with the shared engine; update running peak H=max(H,L), initialized to zero; emit `expm1(L−H)`. Drawdowns are signed fractions at or below zero for valid inputs. Empty input emits an empty series; shared invalid/wipeout behavior is in [record 02](02-wealth-and-annualization.md).
2. Episode segmentation enters when `d < −1e−15`, starts at the prior observation if available, and records the first strictly deepest valley. A drawdown at the first available element marks `truncated_at_start=true`. The next value at/above the threshold closes the episode; an unclosed episode has no recovery date.
3. Dates/values are aligned to their shorter length inside the private episode helper. Duration is calendar days from start to recovery, or to the last observed date for an open episode. Reported `near_recovery_threshold` is valley depth × .01; it is metadata, not the actual episode-closing rule.
4. `drawdown_details(n)` sorts episode valleys deepest first and truncates to n, including open episodes. `worst_episode_depths(n)` supplies the same ordered depths without dates. Mean selected-episode drawdown divides by the number retained and returns zero if none.
5. Maximum drawdown is the minimum path value with zero baseline; any nonfinite path value yields NaN. Ulcer index is root mean square drawdown; Pain index is mean absolute drawdown; ordinary mean drawdown keeps the negative sign. Empty paths return zero for these measures. Maximum duration searches all detected episodes, not only the deepest n.

`Performance` prepends a zero drawdown with the known initial price date when calculating detailed episodes/durations. This preserves the prior peak date for an immediate loss; a return-only panel still relies on its synthesized initial date. Scalar path measures omit that extra zero so their denominator is the number of returns.

The observed path must be reconciled with the
[accepted terminal-loss decision](02-wealth-and-annualization.md#accepted-decision-terminal-investment-losses).
The complete accepted package above governs episode, duration, weighting,
selection and availability behavior; conflicting observed branches remain
implementation discrepancies.

## Selection, limitations and alternatives

Date-range reset rebuilds drawdowns from the selected return stream, discarding earlier peaks. Benchmark drawdown difference independently constructs the two overlapping return paths then subtracts portfolio-minus-benchmark drawdown pointwise. This is not drawdown of relative wealth. The −1e−15 recovery tolerance is internal, unconfigurable; no trading-day duration option or interpolation between observed peaks exists.

The accepted analysis-window contract independently rebases each window. Carrying prior peaks into it, reporting durations in observation counts, relative-wealth drawdown and continuous high/low drawdown are different unselected methods. The current recovery cutoff, silent alignment truncation, metadata and empty-result substitutions conflict with the accepted package and require reconciliation.

## Implementation and validation evidence

- [Path/episode kernels](../../../../finstack-quant/analytics/src/drawdown.rs), [window/cache and leading-peak handling](../../../../finstack-quant/analytics/src/performance/mod.rs), [detail/benchmark difference dispatch](../../../../finstack-quant/analytics/src/performance/aggregation.rs), and [scalar measures](../../../../finstack-quant/analytics/src/performance/scalar.rs).
- Located inline tests cover recovered/open episodes, aligned date bounds, hand Pain calculations and shared-wealth consistency. Inspected [audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) include initial known peak dates and finite relative drawdowns despite absolute wealth overflow; [correctness regressions](../../../../finstack-quant/analytics/tests/correctness_regressions.rs) cover repeated window rebasing. These are local path/algebra tests.
- [Ulcer reference](../../../REFERENCES.md#martinUlcer1987) supplies historical context, not validation of the internal threshold or sampling choice.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
