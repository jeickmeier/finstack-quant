# ANALYTICS-001: Performance panels, observation spans, defaults and wire contracts

**Decision status:** Accepted for explicit observation frequency and tail confidence (2026-11-06), and the remaining complete panel/window/wire/summary package (2026-11-07).

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: explicit observation frequency

**Accepted:** 2026-11-06.

Annualized analytics risk calculations require caller-supplied observation
frequency, or an explicit positive finite periods-per-year factor for scalar
APIs. Omission is a validation error. The calculation does not infer frequency
from dates, row counts or elapsed time. A source adapter may pass known
frequency metadata explicitly, and a panel may supply it once for its methods.

This covers frequency-based risk annualization and associated reward/risk
metrics, including volatility, downside deviation, Sharpe, Sortino, tracking
error, information ratio and their rolling variants. Their cash conversion
uses the same declared frequency. Daily trading observations conventionally
use 252 periods per year and monthly observations use 12; weekend and holiday
gaps do not change a declared trading frequency.

## Accepted decision: explicit tail confidence

**Accepted:** 2026-11-06.

Tail-risk calculations require caller-supplied, finite decimal confidence
`c`, strictly inside `(0.5, 1)`. Omission and values outside that interval are
request validation errors, distinct from a valid request with unavailable
per-ticker estimates. Remove implicit `0.95` defaults. Explicit `0.95` and
`0.99` remain allowed; there is no whitelist of approved confidence levels.

| Supplied confidence | Interpretation or result |
| --- | --- |
| `0.95` | Lower tail probability `1 - c = 0.05`. |
| `0.99` | Lower tail probability `1 - c = 0.01`. |
| Omitted, `0.05`, `0.5`, `1`, `95`, or nonfinite | Request validation error. |

This covers historical VaR, expected shortfall, Gaussian and Cornish–Fisher
VaR, modified Sharpe, CDaR, tail ratio and summaries that calculate these
metrics. A summary may take one explicit confidence for its affected metrics.
Callers supply confidence rather than the complementary tail probability;
for tail ratio, `Q(c)` is the upper quantile and `Q(1 - c)` the lower quantile.

The above-one-half range is a chosen tail-risk API convention, not a
mathematical restriction on VaR/ES. The
[Rockafellar–Uryasev general-loss formulation](https://sites.math.washington.edu/~rtr/papers/rtr187-CVaR2.pdf)
allows confidence in `(0, 1)`. General-purpose core quantile functions retain
their own probability domain. Regression confidence intervals, including the
existing fixed 95% beta interval, are outside this decision.

The [accepted record-10 contract](10-benchmark-regression.md#accepted-decision-single-factor-benchmark-regression)
separately retains that fixed, labelled two-sided 95% Student-t interval and
defines field-specific beta/uncertainty availability.

## Accepted decision: panels, windows, wire and summary

**Accepted:** 2026-11-07.

### Actual valuation grid and panel admission

Use one explicit, strictly increasing grid of actual valuation `Date`s for
both panel kinds. With `L >= 2` valuation dates and at least one ticker:

| Explicit panel kind | Accepted column shape and meaning |
| --- | --- |
| Prices | Exactly `L` price/wealth-index observations per ticker, aligned to valuation dates. Derive interval return `i` from adjacent levels at `dates[i]` and `dates[i+1]`. |
| Returns | Exactly `L - 1` decimal simple returns per ticker. Return `i` covers the declared interval `(dates[i], dates[i+1]]`, ending at `dates[i+1]`. |

This uses the existing actual-date requirement without adding an optional
baseline field, per-ticker date map or synthetic prior-frequency date. A
Python return-frame adapter with only return-ending index dates requires an
explicit actual initial valuation date before its first endpoint, then builds
the same canonical grid. Do not insert a synthetic return or expose another
WASM convenience API merely to mirror that dataframe adapter.

Retain contiguous observed spans and leading/trailing missing values only.
Every ticker must contain at least one valid interval return; a price column
needs its actual opening level and at least one subsequent observed level.
All columns must have the exact kind-specific full-grid length before span
compaction. Reject empty/all-missing tickers, interior gaps, infinities and
invalid economic values; do not trim invalid finite values, truncate arrays,
sort dates, fill gaps or silently change the ticker set. Native NaN means an
allowed missing observation only at a permitted edge; zero is an observed
value, not missing.

Require one unique, nonblank string label per ticker. Reject empty or
whitespace-only names; preserve all other supplied text and ordering exactly,
with case-sensitive identity. Do not trim/case-fold names or use column
position as an implicit financial identifier. Host index/name conversion
must resolve to these same canonical identities.

For a late-starting return ticker, its first finite return must genuinely
cover that interval's preceding declared valuation boundary. A date observed
for another ticker does not itself prove that opening valuation. Callers
must supply aligned intervals; incompatible holding periods require explicit
preparation outside this panel rather than guessed per-ticker starts.

The panel represents total investment returns or supplied price/wealth
levels under record 02's domain. Positive wealth is ordinary; an observed
final zero level or explicitly supplied terminal `-1` is handled under the
accepted wipeout rule. No later percentage observations are admitted.
Already-excess responses/factor spreads belong in the raw statistical APIs,
not a new excess-wealth panel mode.

Price conversion computes the same adjacent fractional change with adequate
scaling. Distinct close levels must not become fabricated zero returns, and
positive ending wealth must not become a false terminal `-1`. For example,
positive levels `[1, 1e-20]` cannot be represented faithfully as a single
`f64` simple return above `-1`; fail that conversion explicitly instead of
inventing a wipeout or a floor. Only an actual terminal zero establishes
that event. Overflow/underflow or an unrepresentable derived return is a
calculation error, never a NaN later reinterpreted as an allowed edge gap.

Callers own return provenance, adjustment for splits/distributions, cash-flow
treatment, currency, fee basis and valuation cutoffs. Adjacent raw prices
produce price changes; they do not automatically become dividend-inclusive
total returns. Supply appropriately prepared returns/wealth levels when that
interpretation is intended. No automatic adjustment, resampling or currency
conversion is introduced.

### Optional context, explicit benchmark and numerical-unit selection

- Keep frequency metadata optional and genuinely absent when omitted.
  Nonannualized/date-only operations need no unused frequency. Validate any
  supplied label, and require the explicit stored frequency whenever a
  calculation uses frequency-based annualization or cash conversion, as
  already accepted above. Remove the implicit Daily fallback; do not infer
  frequency or certify regular sampling from the grid.
- Keep benchmark selection optional until needed. Omission means no benchmark
  designated, not ticker zero. Benchmark-dependent methods and the fixed
  benchmark-containing summary require an explicit loaded ticker before
  computation; missing selection is a request error. An unknown explicit name
  errors. Deliberate self-benchmarking remains allowed; column reordering must
  not silently change the financial comparison.
- Frequency and benchmark inspection getters preserve absence as an optional
  result; do not substitute Daily or index zero merely to satisfy the old
  accessor return types. Their dependent calculations still validate context
  before computation.
- Require an explicit `annualize` flag on dual-unit `mean_return` and
  `volatility` methods across hosts, removing their implicit `true` default.
  `true` requires frequency; `false` returns the observation-period measure
  without an unused frequency requirement. This selects units, not a new
  estimator. The fixed summary explicitly selects annualized mean/volatility.
- Preserve owner-specific accepted inputs: cash, MAR, Omega hurdle, tail
  confidence, rolling windows and episode counts are explicit where used.
  The omitted one-period parametric tail horizon remains accepted. Calendar
  aggregation, fiscal and lookback-reference choices follow the separately
  [accepted record-03 package](03-periods-lookbacks-and-win-loss.md#accepted-decision-calendar-grouping-lookbacks-and-win-loss-statistics).

### Active windows, getters and mutation

Initially select full validated history. Date-range resets use inclusive
return-ending bounds on the original grid, independently of any prior reset.
Require `start <= end`; reject reversed bounds without changing the previous
selection or caches. Valid ordered off-grid/outside-support ranges are
allowed and can select no observations. Do not clamp a reversed request into
an empty success or treat empty selection as a zero-return history.

Selecting a return endpoint includes its whole supplied interval, even when
its opening valuation predates the requested start. Do not prorate or invent
an intermediate valuation. Scalar ticker methods use their own active span;
benchmark methods use the exact active paired intersection; record 15 uses
its separately accepted all-ticker intersection. Preserve these distinctions.
Drawdowns rebase at the actual preceding valuation under accepted record 05;
dated CAGR uses that actual holding start under its owner.

Retain compact/ragged ticker return getters aligned with each ticker's own
active dates. `PerformancePanel.dates` is the full `L`-point valuation grid;
`Performance.get_dates()` remains the full `L - 1` return-ending grid
`panel.dates[1..]`. `get_active_dates()` selects that ending-date grid by the
active range, and `get_active_dates_for_ticker()` additionally intersects the
ticker's observed span. Global dates are not interchangeable with a ticker's
compact date vector. Empty valid selections retain owner-defined unavailable/empty
outcomes without dropping identities or manufacturing returns. Benchmark and
date-range mutations validate before committing, leaving state unchanged on
failure. No hidden state changes occur during metric calculation.

### Input wire and validated state restoration

Preserve the distinction between input missingness and result unavailability.
Input-panel JSON uses finite numbers and `null` for permitted missing values;
native edge NaNs encode as null and restore as missing NaNs. Reject both
infinities before encoding as well as after decoding. Do not accept result
sentinel strings in input series, or allow an overflowed JSON number to
become an edge gap. Full panel admission rules apply consistently across
direct construction and wire import/export. Reject unknown fields and
malformed date/kind/shape/identity metadata without repair.

Persist canonical validated source state: actual valuation grid, compact
finite return columns and exact spans, ticker identities/order, optional
frequency/benchmark and effective active selection. Restoration validates the
same financial domains and interval chronology as construction, plus nested
span shapes, selection/index bounds and known benchmark identity. Reject
invalid nested fields, duplicate names, impossible spans and nonfinite source
returns. Rebuild all derived caches; neither saved caches nor serialized
estimator outputs substitute for validation. Source state need not preserve
the original off-grid query bounds when it preserves the effective selected
observations. No old-shape fallback or migration path is retained.

### Fixed summary composition and explicit inputs

Retain the existing fixed 22-metric summary, with one ticker row in supplied
order and the existing metric keys/order:

```text
cagr, mean_return, volatility, sharpe, sortino, calmar, max_drawdown,
value_at_risk, expected_shortfall, tracking_error, information_ratio,
skewness, kurtosis, geometric_mean, downside_deviation, omega_ratio,
gain_to_pain, ulcer_index, pain_index, recovery_factor, tail_ratio, r_squared
```

Require explicit annual-effective cash, per-observation MAR, a separate
per-observation Omega threshold and tail confidence. Require stored explicit
frequency and selected benchmark because this fixed report uses them.
Validate these required inputs before empty/insufficient-history outcomes.
Cash affects Sharpe in this report; MAR affects Sortino/downside,
Omega uses its independent hurdle, and confidence routes to historical
VaR/ES and tail ratio. Do not reuse one input as another or require unrelated
Sterling/window/modified-Sharpe parameters.

Dispatch the canonical owner-defined metrics instead of maintaining separate
summary formulas or zero-denominator shortcuts. Retain accepted historical
VaR/ES selection, ACT/365F default dated CAGR and all owner-specific sample,
sign, zero-risk and terminal rules. Summary selection inherits the [accepted record-02 scalar/growth contract](02-wealth-and-annualization.md#accepted-decision-wealth-means-cagr-relative-wealth-and-cash-series)
without creating an alternative estimator. Mean return, volatility, downside deviation and Sharpe/Sortino use
their documented annualized basis; geometric mean is observation-period.
Drawdown/tail measures retain their own nonannualized units. The existing
`kurtosis` key means the accepted excess-kurtosis estimator.

Each metric retains its own population: ticker metrics use that ticker's
active observations; benchmark metrics use its active benchmark overlap.
Do not force the whole report onto record 15's common matrix sample, or imply
one universal row sample count. Include supplied cash/frequency/MAR/Omega/
confidence, selected benchmark and relevant unit/day-count conventions in
the existing table metadata. Record each row's own and paired sample counts,
return-ending bounds and actual holding start where used, with absent bounds
for empty samples. These summarize provenance; source observations remain
necessary for reproduction. Labels/exports/UI must reflect the supplied
confidence and conventions rather than hardcode 95% or annualize geometric
mean in prose.

### Summary availability, serialization and failures

Preserve every ticker row and owner-defined finite zeros, ordinary undefined
cells and intentional signed infinities. Insufficient histories, no benchmark
overlap and constant-series statistics remain unavailable only where their
metric owner says so; do not discard unaffected cells or reinterpret a
calculation failure as unavailability. A genuine request, numerical or table/
conversion failure aborts the summary call without a partially successful
report. No per-cell failure-status framework or automatic estimator fallback
is introduced.

Reuse `TableEnvelope` and the existing core nonfinite adapters/schema for
lossless numeric results: native NaN/positive/negative infinity keep those
meanings, and JSON uses the established `"nan"`, `"inf"`, `"-inf"` tokens.
The shared `Float64(Vec<f64>)` table integration preserves these sentinels;
there is no parallel analytics table or conversion of these results to null/zero. This requirement preserves
analytics outputs without adopting unrelated table/domain financial policy.
Verify native Python/WASM behavior separately from JSON; a missing adapter is
not proof of a demonstrated native-array failure.

Summary construction calls `validate_summary`. After restoring a generic
`TableEnvelope`, call the public analytics `validate_summary(&table)` before
treating it as this canonical report. Generic table restoration checks its
structural envelope; it does not establish analytics metadata semantics.
The analytics validator checks row counts, unique ordered column keys, ticker
alignment, numeric-column lengths/types and required metadata, with only
owner-permitted nonfinite results. Reject
malformed/unknown fields, inappropriate nulls or sentinel tokens and
inconsistent counts instead of rebuilding a different report. A restored
table cannot certify the underlying financial calculations without source
data. Qualification cases include host/frame/wire paths, empty/ragged windows,
distinct population metadata, nonfinite identity and atomic failures.

[RFC 8259](https://www.rfc-editor.org/rfc/rfc8259#section-6) excludes NaN and
infinity from JSON numbers; it does not authorize treating them as missing
input. [Pandas' fractional-change documentation](https://pandas.pydata.org/docs/reference/api/pandas.DataFrame.pct_change.html)
supports the adjacent fractional-change distinction, without adopting
automatic filling or certifying financial adjustments. These references do
not validate source provenance, calendar regularity or estimator accuracy.

## Implementation evidence

[performance/mod.rs](../../../../finstack-quant/analytics/src/performance/mod.rs), [performance/panel.rs](../../../../finstack-quant/analytics/src/performance/panel.rs), [performance/serialization.rs](../../../../finstack-quant/analytics/src/performance/serialization.rs), [performance/scalar.rs](../../../../finstack-quant/analytics/src/performance/scalar.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
