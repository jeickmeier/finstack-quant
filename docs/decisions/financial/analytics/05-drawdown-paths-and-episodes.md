# ANALYTICS-005: Log-wealth drawdowns, episode segmentation and path measures

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice and input domain

Drawdowns are measured relative to an initial wealth peak of one and subsequent observed peaks, using log wealth so absolute wealth need not be representable. Each observation receives equal weight in path averages; actual dates matter for episode durations. Performance windows are independently rebased. This is observed-window drawdown, not a continuous-time maximum between observations.

## Algorithm and controls

1. Accumulate log wealth L with the shared engine; update running peak H=max(H,L), initialized to zero; emit `expm1(L−H)`. Drawdowns are signed fractions at or below zero for valid inputs. Empty input emits an empty series; shared invalid/wipeout behavior is in [record 02](02-wealth-and-annualization.md).
2. Episode segmentation enters when `d < −1e−15`, starts at the prior observation if available, and records the first strictly deepest valley. A drawdown at the first available element marks `truncated_at_start=true`. The next value at/above the threshold closes the episode; an unclosed episode has no recovery date.
3. Dates/values are aligned to their shorter length inside the private episode helper. Duration is calendar days from start to recovery, or to the last observed date for an open episode. Reported `near_recovery_threshold` is valley depth × .01; it is metadata, not the actual episode-closing rule.
4. `drawdown_details(n)` sorts episode valleys deepest first and truncates to n, including open episodes. `worst_episode_depths(n)` supplies the same ordered depths without dates. Mean selected-episode drawdown divides by the number retained and returns zero if none.
5. Maximum drawdown is the minimum path value with zero baseline; any nonfinite path value yields NaN. Ulcer index is root mean square drawdown; Pain index is mean absolute drawdown; ordinary mean drawdown keeps the negative sign. Empty paths return zero for these measures. Maximum duration searches all detected episodes, not only the deepest n.

`Performance` prepends a zero drawdown with the known initial price date when calculating detailed episodes/durations. This preserves the prior peak date for an immediate loss; a return-only panel still relies on its synthesized initial date. Scalar path measures omit that extra zero so their denominator is the number of returns.

## Selection, limitations and alternatives

Date-range reset rebuilds drawdowns from the selected return stream, discarding earlier peaks. Benchmark drawdown difference independently constructs the two overlapping return paths then subtracts portfolio-minus-benchmark drawdown pointwise. This is not drawdown of relative wealth. The −1e−15 recovery tolerance is internal, unconfigurable; no trading-day duration option or interpolation between observed peaks exists.

Carrying prior peaks into an active window, reporting underwater durations in observation counts, measuring relative-wealth drawdown and continuous high/low drawdown are distinct unimplemented alternatives. The partial-window convention and tolerance are not adopted for each reporting use by this audit.

## Implementation and validation evidence

- [Path/episode kernels](../../../../finstack-quant/analytics/src/drawdown.rs), [window/cache and leading-peak handling](../../../../finstack-quant/analytics/src/performance/mod.rs), [detail/benchmark difference dispatch](../../../../finstack-quant/analytics/src/performance/aggregation.rs), and [scalar measures](../../../../finstack-quant/analytics/src/performance/scalar.rs).
- Located inline tests cover recovered/open episodes, aligned date bounds, hand Pain calculations and shared-wealth consistency. Inspected [audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) include initial known peak dates and finite relative drawdowns despite absolute wealth overflow; [correctness regressions](../../../../finstack-quant/analytics/tests/correctness_regressions.rs) cover repeated window rebasing. These are local path/algebra tests.
- [Ulcer reference](../../../REFERENCES.md#martinUlcer1987) supplies historical context, not validation of the internal threshold or sampling choice.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
