# ANALYTICS-003: Calendar grouping, lookback returns and win/loss statistics

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice and input domain

Calendar/fiscal aggregation compounds the observed simple returns in each occupied period. It does not create missing periods or prorate partial buckets. `Performance` provides chronological finite spans; grouping is private and assumes those checked inputs. The canonical period-boundary library supplies `PeriodId` and fiscal rules. This reuse gives consistent boundaries, but does not establish that each financial reporting use should include a partial first or last bucket.

## Algorithm and selection

`group_period_buckets` walks aligned dates/returns, detects changes in `PeriodId::from_date`, compounds the bucket using the log-wealth kernel, and labels it with its last observed date. The public `periodic_returns(frequency)` uses calendar periods; `period_stats(ticker_idx,aggregation_frequency,fiscal_config)` can use fiscal grouping before computing statistics. Boundary-construction errors propagate.

Lookbacks select returns whose ending dates lie in `[period_start,ref_date]`, further bounded by each active ticker span. MTD starts on day one of the reference month, QTD on the calendar quarter's first month, YTD on calendar-year start, FYTD on the configured fiscal-year start. Omitted reference date is the last active date; if none exists it errors. Omitted fiscal configuration is calendar year. No observations in a selected interval gives total return zero. If a fiscal start precedes the representable date range, the implementation falls back to `ref_date`; month construction also uses that fallback if it fails.

For N bucket returns: best/worst are extrema; arithmetic average is a Neumaier sum/N; wins are strictly positive, losses strictly negative, zero breaks both streaks. Win rate is wins/N, while average win/loss divides only by its group's count and returns zero for an empty group. Payoff ratio is average gain/absolute average loss; profit factor is total gains/absolute total losses; CPC is profit factor × win rate × payoff ratio. Their division helper returns signed infinity for nonzero/zero and zero for 0/0, and propagates nonfinite operands as NaN.

Kelly uses only decisive periods: `p=wins/(wins+losses)` and `p−(1−p)/payoff`. Infinite payoff returns one; zero payoff returns zero. Neutral buckets therefore affect ordinary win rate but not the binary Kelly fraction. Empty input returns a struct of zeros. Ratio fields explicitly preserve nonfinite wire values.

## Conventions, limitations and alternatives

Bucket returns and lookbacks are dimensionless compounded returns, not annualized rates. Streaks count adjacent occupied buckets; an unobserved calendar bucket is not inserted as a neutral outcome. Kelly here is a descriptive binary gain/loss estimate without position limits or a growth-optimal portfolio solve. CPC can overflow as a product of its component ratios. Full-period-only reporting, interpolated missing periods and weighted win rates are unimplemented alternatives requiring separate policies.

## Implementation and validation evidence

- [Grouping/statistics](../../../../finstack-quant/analytics/src/aggregation.rs) and [public aggregation/lookbacks](../../../../finstack-quant/analytics/src/performance/aggregation.rs) own the algorithms.
- Located inline tests cover month/fiscal windows, default fiscal year, empty reference-date errors and monthly buckets. Inspected [audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) include neutral-period Kelly invariance and shared fiscal-boundary fixtures; [fiscal data](../../../../finstack-quant/analytics/tests/data/fiscal_buckets.json) is a local fixture, not an independently certified reporting calendar.
- [Gain-to-pain reference](../../../REFERENCES.md#schwager2012) is adjacent context, not provenance for every CPC/Kelly variant. Specific Kelly/CPC source editions and independent expected cases were not established in this audit.

No tests or fixtures were changed.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
