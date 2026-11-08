# ANALYTICS-003: Calendar grouping, lookback returns and win/loss statistics

**Decision status:** Accepted for the complete calendar/fiscal grouping, lookback and win/loss package (2026-11-07). Implementation reconciliation and independent qualification remain open; other metric owners retain their scopes.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Accepted decision: calendar grouping, lookbacks and win-loss statistics

**Accepted:** 2026-11-07.
**Implementation:** Partial; selected source inspected at `827ebb75b9`.
Explicit aggregation frequency, missing-sample outcomes, result provenance,
conditional-statistic availability/names and numerical/wire reconciliation
remain unimplemented. No runtime or binding checks were executed.

This package settles the remaining choices in this record. The accepted
[actual valuation grid and active-window contract](01-panels-windows-and-wire.md#accepted-decision-panels-windows-wire-and-summary)
and [wealth/terminal/representability rules](02-wealth-and-annualization.md#accepted-decision-wealth-means-cagr-relative-wealth-and-cash-series)
remain constraints. These calculations do not acquire unused cash, MAR,
benchmark, confidence or annualization inputs.

### Calendar/fiscal buckets and successive observed streaks

Require an explicit aggregation frequency for both `periodic_returns` and
`period_stats`, including Python/WASM; remove their implicit monthly defaults.
This bucket frequency is separate from the observation frequency used by
annualized metrics. Support the existing daily, weekly, monthly, quarterly,
semiannual and annual `PeriodKind` values without adding another calendar.

Use core `PeriodId::from_date` boundaries. `periodic_returns` remains calendar
only. For `period_stats`, omitted fiscal configuration means Gregorian
periods, including ISO week-years; explicit `FiscalConfig` applies core fiscal
boundaries at every frequency. Fiscal weeks are seven-day blocks from the
fiscal-year opening, with a possible partial final week. Even an explicit
January-1 fiscal start therefore need not match Gregorian ISO weeks. Keep
core month-end/leap-day clamping and fiscal-year labels; boundary failures
are request errors, never a reason to substitute another period.

Assign each whole supplied return interval to the bucket containing its
**ending date**. Include occupied partial first/last buckets and omit empty
buckets. Do not split or prorate an interval that crosses a bucket boundary,
manufacture zero returns, resample, or apply a full-period filter. A grouped
value is observed endpoint-assigned growth, not proof of complete calendar
coverage. Compound the selected original observations using record 02's
wealth kernel. Require exact date/return lengths, strictly increasing dates
and validated finite investment histories; private grouping must not silently
truncate mismatched inputs with `zip`.

Retain each `PeriodicReturn`'s `date`/`value` shape and existing host conversion.
`date` is the bucket's last **actual observed return endpoint**, not a
synthetic period-end date; the caller retains the explicit frequency. Empty
ticker histories produce empty bucket vectors. Reported bucket values are
finite and at least `-1`. A true terminal loss remains
terminal in the original history. A positive-wealth bucket value may round
to `-1` under record 02's reporting rule; this does not terminate the source
history or invalidate its later buckets. Compute from original validated
observations/state, not by refeeding rounded exported bucket returns into
the wealth engine. Exported values are reports, not lossless wealth-state
interchange; exact chaining identities hold before output rounding.

`consecutive_wins`/`consecutive_losses` count the longest run of strictly
positive/negative **successive occupied buckets**. An observed zero breaks
both runs; an absent bucket contributes no outcome and does not break a run.
For example, occupied winning January and March buckets form a run of two,
without a claim of uninterrupted monthly performance. The same rule avoids
inventing weekend losses/zeros for a daily trading history. Use strict signs,
with no epsilon band that reclassifies small returns as neutral.

### Lookback selection, empty samples and actual coverage

Retain the four nonannualized compounded-return horizons. MTD begins on the
first day of the reference month, QTD on the first day of its Gregorian
quarter, YTD on January 1 and FYTD on the configured fiscal-year opening.
Fiscal configuration affects FYTD only; omission resolves to January 1,
making FYTD equal YTD. Use existing core fiscal clamping and boundary rules.

Omitted `ref_date` resolves to the last **global active return endpoint**,
not the current date or each ticker's latest observation. Omission over an
empty active grid errors. An explicit representable reference date remains
the cutoff even when off-grid, before the active history or after its final
observation; do not clamp it or read outside the active window. Return the
resolved reference date and resolved FYTD fiscal configuration.

For each ticker/horizon, include only its active returns whose endpoints
lie in the inclusive interval `[horizon_start, ref_date]`. Include the whole
first selected interval even if its actual opening precedes the horizon
start; do not prorate or claim full horizon coverage. At least one selected
return suffices. An empty selected sample gives an undefined return, not
observed zero performance; preserve other available ticker/horizon values.
Empty grouping remains `[]`, and the private empty-product identity remains
algebraic rather than a reported lookback result. Validate arguments and
construct all requested boundaries before empty/terminal shortcuts. Remove
the current boundary-to-`ref_date` fallbacks; genuine boundary failures error
the call.

Keep `LookbackReturns`' ordered ticker names, reference date and four value
vectors. Add the resolved `fiscal_config` and matching `mtd_samples`,
`qtd_samples`, `ytd_samples`, `fytd_samples` vectors. Each result-only
`LookbackSample` carries `n_obs`, `holding_start` and `holding_end`: the raw
selected return count,
actual valuation immediately preceding the first selected return, and last
selected return endpoint. Count zero requires both bounds absent; positive
count requires ordered actual holding bounds, with closing endpoint inside
the requested horizon/cutoff. This exposes stale, partial and empty coverage
without duplicating complete date grids or adding a reporting framework.

### Period statistics, conditional means and availability

Compute statistics across the occupied bucket returns, giving every bucket
equal weight, including partial/unequal buckets. They are not statistics of
the original observations, duration-weighted estimates or dollar trade P&L.
Add `n_obs`, `wins` and `losses` to `PeriodStats`; here `n_obs = N` counts
occupied **aggregation buckets**, unlike lookback sample counts. Neutral
count is derived as `N - W - L` rather than stored redundantly. Let:

```text
W = count(r_j > 0)                 L = count(r_j < 0)
G = sum(positive r_j)              D = sum(abs(negative r_j))
avg_win = G / W                    avg_loss = -D / L
b = (G / W) / (D / L)             PF = G / D
win_rate = W / N
CPC = PF * win_rate * b = PF^2 * (L / N)   # when W > 0 and L > 0
p = W / (W + L)                   q = L / (W + L)
K = p - q / b                            # when W > 0 and L > 0
```

`best`/`worst` are extrema and `avg_return` is the arithmetic mean of all
bucket returns. `avg_loss` stays negative. Do not use an unavailable
conditional mean as a measured zero. There is no arbitrary sample minimum
beyond those needed to define each statistic:

| Observed sample | Required outcomes |
| --- | --- |
| `N = 0` | Counts/streak lengths are zero; every floating statistic is undefined. |
| `N >= 1` | Extrema, arithmetic mean and `win_rate = W/N` are available, including a one-bucket sample. |
| `W > 0` / `L > 0` | Corresponding conditional mean is available; an absent group mean is undefined. |
| Both wins and losses | Payoff `b`, profit factor `PF`, CPC and signed Kelly statistic `K` use the formulas above. |
| Gains, no losses | Profit factor is `+infinity`; average loss, payoff, CPC and Kelly are undefined. |
| Losses, no gains | Profit factor is `0`; average win, payoff, CPC and Kelly are undefined. |
| Observed all-neutral sample | Extrema/mean/win rate are measured zero and streaks are zero; conditional means, payoff, profit factor, CPC and Kelly are undefined. |

Profit factor's one-sided outcomes use **defined empirical gain/loss totals**,
not fabricated conditional means or a claim about future loss risk. Neither
empty nor all-neutral `0/0` becomes zero. Intentional infinity is confined
here to positive observed gains with exactly no observed losses. Overflow
of a mathematically finite two-sided estimate is a calculation failure.

### CPC and loss-normalized Kelly meaning

Retain the descriptive CPC product and rename `cpc_ratio` to `cpc_index`.
It is this API's equal-weight period-return adaptation of Harris's Cardinal
Profitability Construct, not the Common Sense Ratio. Harris's
[Indicator Manual, chapter 8, printed page 119](https://www.moneymentor.com/downloads/IndicatorManual.pdf)
supplies the trade-statistics product. The review located its indexed primary
excerpt; the complete large PDF was not retrieved. That provenance supports
the algebra, not vendor parity, period-data predictive validity or adoption
of trading thresholds. Calendar return buckets are not closed trades.

Rename `kelly_criterion` to `kelly_risk_fraction`. `K = p - q/b` is a signed
**loss-normalized binary risk fraction** under hypothetical outcomes `+b/-1`.
Probabilities are conditional on decisive buckets; compute `p` and `q`
directly from their counts. Neutrals remain in ordinary win rate and CPC's
`N`, but do not change `p`, `q`, payoff or Kelly. Preserve negative values
without clipping to zero or `[0,1]`. Require both observed gain/loss groups;
remove current one-sided `1`/`0` and empty/all-neutral `0` shortcuts.

[Kelly's original 1956 paper](https://www.princeton.edu/~wbialek/rome/refs/kelly_56.pdf)
provides the log-wealth objective. The following unit distinction follows
independent binary-model algebra: for actual binary returns
`+a/-ell`, with `a = avg_win` and `ell = abs(avg_loss)`, hypothetical asset
exposure is `K/ell`, not `K`. Equally likely `+2%/-1%` gives `K = 0.25`,
whereas that binary model's exposure is `25` times capital. Do not label
`K` a 25% portfolio allocation, optimize the observed continuous return
distribution, or add automatic position/leverage limits or sizing advice.
The renamed fields are canonical across Rust/Python/WASM, with no old aliases.

For bucket returns `[+2%, -1%, 0]`, payoff and profit factor are both `2`,
CPC is `4/3`, win rate is `1/3` and Kelly is `0.25`. Removing the neutral
bucket raises win rate to `1/2` and CPC to `2`, while payoff, profit factor
and Kelly remain unchanged. These are local expected cases, not independent
certification of an implementation or trading signal.

### Numerical, wire and integration contract

Use compensated/scaled arithmetic that preserves representable requested
means and ratios even if a naive gross sum or intermediate product overflows.
Evaluate final outputs without treating overflown intermediate components as
valid operands: for example, calculate Kelly without first materializing an
unrepresentable payoff. The CPC
identity above can avoid avoidable intermediate overflow, but does not permit
turning an unrepresentable requested final CPC into infinity. Any
unrepresentable requested final field still fails the result call. No denominator
floors, sign/neutral bands, automatic clipping, invented missing groups or
silent numerical-failure-to-zero/undefined conversions are selected. Actual
numeric/representability failures error the call; ordinary sample-driven
undefined fields preserve the other defined estimates.

Extend existing nonfinite field/sequence adapters and schemas to **every**
floating `PeriodStats` field and every lookback value vector. Use canonical
JSON `"nan"`/`"inf"`/`"-inf"` and native numeric `NaN`/infinite values in
Python/WASM under record 01. In particular, restoring sentinel elements in
WASM lookback arrays requires sequence-aware conversion; scalar-only field
restoration is insufficient. `PeriodicReturn` values remain finite reports.

Keep counts/streaks integer-valued in typed APIs and tabular integer columns;
an existing homogeneous floating-series presentation may retain its documented
count conversion. Validate count relationships, streak limits, finite domains
and statistic availability on result construction/wire restoration; all
lookback value/sample vectors must match ordered ticker names. Check raw
lookback count/bound feasibility under the actual-date contract: positive
`n_obs` cannot exceed elapsed calendar days from holding start to holding end.
`PeriodStats` counts refer to buckets. A nonempty lookback
has a finite return at least `-1`; count zero requires undefined return and
absent bounds. Do not infer full-period completeness
or rerun an unavailable source history from compact summaries. Invalid
requests/restored results error explicitly; no partial repair or new status
framework is introduced. Reconcile canonical owners, both bindings,
stubs/declarations, parity, schemas, UI consumers, docs and fixtures together
when implementation is authorized.

Independent qualification remains open for calendar/fiscal boundary and
clamping cases, cross-boundary intervals, sparse/partial/stale/empty samples,
one-sided/neutral/negative-Kelly cases, terminal versus rounded bucket losses,
high-scale/subnormal sums and ratios, restored metadata and matching host
outcomes. This review changes records only.

## Observed choice and input domain

Calendar/fiscal aggregation compounds the observed simple returns in each occupied period. It does not create missing periods or prorate partial buckets. `Performance` provides chronological finite spans; grouping is private and assumes those checked inputs. The canonical period-boundary library supplies `PeriodId` and fiscal rules. This reuse gives consistent boundaries, but does not establish that each financial reporting use should include a partial first or last bucket.

## Observed algorithm and selection

`group_period_buckets` walks aligned dates/returns, detects changes in `PeriodId::from_date`, compounds the bucket using the log-wealth kernel, and labels it with its last observed date. The public `periodic_returns(frequency)` uses calendar periods; `period_stats(ticker_idx,aggregation_frequency,fiscal_config)` can use fiscal grouping before computing statistics. Boundary-construction errors propagate.

Lookbacks select returns whose ending dates lie in `[period_start,ref_date]`, further bounded by each active ticker span. MTD starts on day one of the reference month, QTD on the calendar quarter's first month, YTD on calendar-year start, FYTD on the configured fiscal-year start. Omitted reference date is the last active date; if none exists it errors. Omitted fiscal configuration is calendar year. No observations in a selected interval gives total return zero. If a fiscal start precedes the representable date range, the implementation falls back to `ref_date`; month construction also uses that fallback if it fails.

For N bucket returns: best/worst are extrema; arithmetic average is a Neumaier sum/N; wins are strictly positive, losses strictly negative, zero breaks both streaks. Win rate is wins/N, while average win/loss divides only by its group's count and returns zero for an empty group. Payoff ratio is average gain/absolute average loss; profit factor is total gains/absolute total losses; CPC is profit factor × win rate × payoff ratio. Their division helper returns signed infinity for nonzero/zero and zero for 0/0, and propagates nonfinite operands as NaN.

Kelly uses only decisive periods: `p=wins/(wins+losses)` and `p−(1−p)/payoff`. Infinite payoff returns one; zero payoff returns zero. Neutral buckets therefore affect ordinary win rate but not the binary Kelly fraction. Empty input returns a struct of zeros. Ratio fields explicitly preserve nonfinite wire values.

## Observed conventions, limitations and alternatives

Bucket returns and lookbacks are dimensionless compounded returns, not annualized rates. Streaks count adjacent occupied buckets; an unobserved calendar bucket is not inserted as a neutral outcome. Kelly here is a descriptive binary gain/loss estimate without position limits or a growth-optimal portfolio solve. CPC can overflow as a product of its component ratios. Full-period-only reporting, interpolated missing periods and weighted win rates are unimplemented alternatives requiring separate policies.

## Implementation and validation evidence

- [Grouping/statistics](../../../../finstack-quant/analytics/src/aggregation.rs) and [public aggregation/lookbacks](../../../../finstack-quant/analytics/src/performance/aggregation.rs) own the algorithms.
- Located inline tests cover month/fiscal windows, default fiscal year, empty reference-date errors and monthly buckets. Inspected [audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) include neutral-period Kelly invariance and shared fiscal-boundary fixtures; [fiscal data](../../../../finstack-quant/analytics/tests/data/fiscal_buckets.json) is a local fixture, not an independently certified reporting calendar.
- [Gain-to-pain reference](../../../REFERENCES.md#schwager2012) is adjacent context, not provenance for every CPC/Kelly variant. Specific Kelly/CPC sources and independent expected cases were not established in the original audit; the accepted package above distinguishes newly located primary algebra from this API's period-data adaptation and unqualified numerical behavior.

No tests or fixtures were changed.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
