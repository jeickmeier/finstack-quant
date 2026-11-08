# ANALYTICS-002: Compounded wealth, relative wealth, cash conversion and dated CAGR

**Decision status:** Accepted for terminal investment losses and explicit cash-rate inputs, representation and conversion (2026-11-06), and the remaining complete wealth/means/CAGR/relative/cash-series package (2026-11-07).

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: terminal investment losses

**Accepted:** 2026-11-06.

| Investment return or event | Required treatment |
| --- | --- |
| Finite simple return greater than -100% | Accept and compound normally. |
| Exactly -100% | Accept as a terminal wipeout: wealth becomes exactly zero, cumulative return and maximum drawdown are exactly -100%. |
| Below -100% | Reject as unsupported by this nonnegative-wealth performance model. |
| A percentage-return observation after wipeout | Reject continuation; new capital requires explicit cash-flow treatment or a separately funded history. |

For positive starting wealth, `W_next = W_previous * (1 + r)`. A total loss
must not leave a tiny positive balance that can subsequently recover. Remove
the positive growth-factor floor and represent terminal zero explicitly,
without sending negative infinity through compensated accumulation. A
representable return strictly above -100% remains a partial loss.

The rule covers price/return panels, standalone wealth/drawdown calculations
and their hosts. A final zero price following positive prices represents the
same terminal loss; no subsequent return may be calculated from zero starting
capital. A wiped-out ticker may end its observed span while other tickers
continue. Derived CAGR over a valid dated span, aggregation and rolling
calculations containing the loss must preserve its meaning, while dependent
metrics retain their own denominator requirements.

## Accepted decision: cash rates

**Accepted:** 2026-11-06.

A calculation that uses cash requires an explicit cash rate. Omission is a
missing-input validation error; an explicitly supplied `0.0` deliberately
selects zero cash. Do not substitute zero or infer a market rate when the
caller omits it. This requirement covers Sharpe and modified Sharpe, Treynor,
M-squared, Jensen/rolling alpha, cash-adjusted CAGR drawdown ratios, total-return
multifactor conversion and summaries containing cash-dependent calculations.

Beta, tracking error, information ratio, volatility, drawdown measures, Calmar
and recovery factor do not acquire an unused cash input. Inputs explicitly
identified as already excess returns require no cash rate, no dummy zero and
no second cash subtraction. Total and excess response selection is explicit under record 13.

Cash/risk-free inputs are annual effective decimal returns: `0.05` means 5%
effective annually and `-0.01` means -1%. Supplied rates must be finite and
strictly greater than -100%; other values raise explicit validation errors.
For explicitly supplied frequency `A > 0`, convert annual cash `R` to:

```text
q = expm1(log1p(R) / A) = (1 + R)^(1 / A) - 1
```

Arithmetic excess calculations subtract period cash before annualization:
for constant `R`, annual excess is `A * (mean(r) - q)`. Cash-adjusted CAGR
ratios with constant annual cash instead use `CAGR - R`. At 5% annual cash
and monthly frequency, period cash is approximately 0.407412%.

Cash-series entries retain this annual-effective meaning and are converted
individually. `A = 1` means annual observations, not an already-periodic
shortcut for daily/monthly data. Other market quote formats require prior
conversion. Cash-series alignment and missing-data policy are accepted in
the complete package below.
A separate realized-period-cash mode is not selected; threshold conventions
retain their metric owners. The [accepted MAR input rule](04-return-risk-ratios.md#accepted-decision-explicit-per-observation-mar)
owns Sortino/downside targets. No cash instrument, currency, curve or
irregular-time estimator is selected by this cash rule.

## Accepted decision: wealth, means, CAGR, relative wealth and cash series

**Accepted:** 2026-11-07.

This package covers the remaining compounding, means, dated CAGR,
relative-wealth and aligned cash-series choices. Accepted terminal loss/cash
rules above, [record 01's actual valuation grid and report contract](01-panels-windows-and-wire.md#accepted-decision-panels-windows-wire-and-summary),
ACT/365F default and other metric owners remain constraints.

### Wealth, arithmetic mean and geometric mean

Use the validated total-investment-return domain above. Each selected own-
ticker history begins from unit wealth at its actual preceding valuation:

```text
L_k = sum(log1p(r_i), i = 1..k)
cumulative_return_k = expm1(L_k)
total_return = expm1(L_N)
arithmetic_mean = sum(r_i) / N
annual_arithmetic_mean = A * arithmetic_mean   # only when requested
geometric_mean = expm1(L_N / N)                # observation-period
```

Retain compensated log accumulation and stable `log1p`/`expm1`. Geometric
compounding, arithmetic averaging and dated annualization are distinct
operations. Do not sum simple returns to obtain total growth or compound
arithmetic excess observations as investment wealth. Arithmetic averaging
uses equal observation weights, not elapsed-day weights. Explicit
`annualize = true` uses supplied finite positive `A`; false and geometric
mean require no unused frequency. Annual arithmetic mean is a scaled
statistic and may legitimately be below `-1`, without becoming a new
investment return eligible for compounding.

| Selected history | Accepted outcomes |
| --- | --- |
| `N >= 1`, no terminal event | Total/cumulative growth and both means use the formulas above; one observation suffices. |
| Valid final terminal `-1` | Total return and the terminal cumulative value are exactly `-1`; geometric mean is `-1`. Arithmetic mean includes the `-1` normally. |
| `N = 0` | Public scalar total/mean/geometric/CAGR estimates are undefined; cumulative collections are `[]`. No observed zero performance is inferred. |

Keep an empty product's internal unit-wealth/zero-return identity only where
needed algebraically. It is not a reported observed return. Record 03 owns
the separate lookback/empty-bucket reporting choices. Emit one cumulative
value per included return endpoint, without inserting the opening baseline
as an extra observation. Own-ticker series remain ragged and align with
`get_active_dates_for_ticker`.

For `[+10%, -10%]`, total return is `-1%`, arithmetic mean is `0%`, and
geometric mean is approximately `-0.501256%` per observation. These are
different answers to different questions, rather than alternative names for
one performance statistic.

### Dated CAGR, explicit clocks and availability

Use:

```text
T = selected_year_fraction(actual_opening, actual_closing, day_count, context)
CAGR = expm1(L_N / T)
```

The accepted default is Actual/365 Fixed, including leap years. Keep explicit
alternative clocks, including the existing named actual-days/365.25 option.
For core day-count alternatives, reuse the existing `DayCountContext` and its
validated host/state forms.
An empty context is sufficient for context-free clocks. Business-day clocks
require their calendar; coupon/reference or termination-dependent clocks use
explicit caller-supplied context under the core convention's rules. Do not
infer coupon frequency from observation frequency, invent reference periods
or silently substitute another day count. This reuses the existing context
type, not a separate coupon-CAGR framework or new convention implementation.

Use each ticker's actual selected holding start/finish, as fixed by record 01.
No observation count/frequency substitutes for elapsed dates. One valid
return and positive finite selected year fraction suffice. There is no
one-year minimum, annualization cap, clipping, automatic resampling or
switch from short-span CAGR to total return. For positive selected time, a
terminal loss gives CAGR exactly `-1`.

Validate convention/context requests before empty or terminal shortcuts.
An actual holding opening at or after closing (`opening >= closing`), unknown convention or
missing/incompatible required context is a request error. A valid actual
positive span can have selected `T = 0`, such as a business-day clock over
nonbusiness days: report unavailable CAGR for that ticker, not a substituted
clock/time floor or zero return. Preserve unaffected ticker estimates.
Negative/nonfinite computed `T` is a calculation failure. Empty active
history gives undefined CAGR under the public panel contract.

For a single 10% holding gain, default CAGR is 10% over 365 days and
approximately 9.971359% over 366 days. Short-span CAGR is an annualized
equivalent, not additional observed performance or a forecast. This generic
calculation is distinct from performance-report presentation standards:
[GIPS provision 2.A.12](https://www.gipsstandards.org/standards/gips-standards-for-firms/gips-standards-handbook-for-firms/)
does not permit annualizing sub-year returns in its applicable reports. No
automatic compliance/reporting mode is introduced by this numerical package.

### Relative cumulative wealth and paired dates

Retain relative wealth, with both paths rebased to one immediately before
their first exact common active return interval:

```text
relative_cumulative_k = W_portfolio,k / W_benchmark,k - 1
```

Use exactly the selected ticker/benchmark active overlap and identical
interval boundaries, not independent full histories or the difference of
their cumulative simple returns. Require the explicit benchmark under record
01. Empty overlap gives `[]`; one paired return suffices. Emit only paired
return-ending points, without baseline padding or synthetic post-terminal
returns.

For positive paths, accumulate paired `log1p(portfolio_i) -
log1p(benchmark_i)` with compensation, reconstructing only the requested
relative output. Evaluate each paired log-ratio adequately before summation:
subtracting two separately rounded logarithms must not erase a representable
relative move between adjacent large growth factors. Use stable scaled
growth-ratio/`log1p` evaluation as needed; compensation alone does not repair
within-pair cancellation. Identical positive paths give relative return zero even
when their conceptual absolute wealth cannot be represented.

A portfolio gain of 20% against a benchmark gain of 10% gives relative
return `1.2/1.1 - 1`, approximately 9.090909%, rather than the 10-percentage-
point difference of the two returns.

| Actual paired wealth state | Accepted result |
| --- | --- |
| Benchmark positive, portfolio positive | Stable wealth quotient minus one. |
| Portfolio terminal zero, benchmark positive | `-1`. |
| Benchmark terminal zero | Undefined at that point, including simultaneous `0/0` and a self-comparison at wipeout. Preserve preceding valid points and other tickers. |

This defines the quotient on a positive benchmark numeraire. Do not introduce
a one-sided-limit infinity for a wiped-out benchmark or borrow the separate
zero-risk ratio rules. A numerical overflow with positive benchmark wealth
is instead a calculation failure, not a terminal-domain undefined point.

Retain the existing nested-vector surface and add the minimal canonical
`get_paired_dates(ticker_idx)` accessor for that ticker against the selected
benchmark, using the identical active paired intersection. Validate the
ticker/benchmark request before returning an empty date slice. Relative
values and paired dates must have identical lengths/order; own-ticker dates
can be longer. Reuse the same accessor in hosts and paired displays rather
than adding another dated carrier or having bindings infer alignment.

### Cash-series alignment and arithmetic excess observations

Keep `excess_returns` as arithmetic differential observations:

```text
q_i = expm1(log1p(R_i) / A)
excess_i = investment_return_i - q_i
```

Every `R_i` is an explicitly supplied annual-effective decimal cash assumption
assigned to the interval ending at that active return date. Require the
stored explicit positive finite `A` and remove the `nperiods` override and
its already-periodic shortcut. Exactly `A = 1` means annual observations;
only that exact mathematical identity can return `R_i` directly. A nearby
frequency is not rounded to one. No separate realized-period-cash mode is
added; callers can prepare differential data externally for appropriate
statistical APIs without reinterpreting an annual rate.

Require one cash entry for every position in the full active **return-ending**
grid, not the valuation grid or an individual ticker's shorter history.
Validate exact length and every supplied `R_i` as finite and greater than
`-1` before ticker slicing, including positions unused by a particular ticker.
Do not permit missing cash values, shorter-prefix pairing, zero filling,
automatic joins or interpolated rates. Host numeric inputs are positional;
labels on a supplied Series-like object's values do not create date alignment.
Callers must align those values explicitly before the call.

Slice the shared cash vector by each ticker's active offsets. Preserve outer
ticker order and own-ticker endpoint alignment, with empty ticker slices
returning `[]`. An empty global active grid requires `[]` but still validates
required frequency. For a January–March grid and a ticker observed only in
February–March, use the second and third cash entries, not the first two.

The output is unannualized arithmetic excess, may be below `-1`, and does not
construct investment wealth. A valid terminal investment `-1` minus positive
cash can therefore produce excess below `-1` without violating the original
investment domain. This is not geometric relative wealth versus cash.
When annual arithmetic excess is requested elsewhere, use `A * mean(r - q)`
on the same observations; constant cash recovers accepted `A*(mean(r)-q)`.
Cash-adjusted CAGR ratios retain their accepted `CAGR - R` convention.

No curve selection, currency conversion, realized cash return, quote
observability/look-ahead assumption or actual-day cash accrual is inferred
from `R_i` or its position. Irregular observation dates remain permitted;
means and cash conversion use the declared nominal observation convention,
while CAGR uses its actual dated clock. This is not an irregular-time risk
estimator or resampling policy and introduces no calendar-validation scheme.

### Numerical state, requested-result representability and errors

- Keep positive log wealth and explicit terminal state separate. Remove the
  `1e-18` growth floor. Do not pass terminal `log(0)` through a compensated
  finite-log accumulator, restore positive wealth after wipeout, or infer a
  terminal event from a rounded reported statistic.
- A cumulative/CAGR or positive-numeraire relative statistic can
  round to `-1` when its true residual positive growth is below output
  precision. That reporting value must not change the internal positive
  state. This differs from record 01's rejected price-derived per-observation
  return that would itself enter history and falsely create a terminal event.
  A nonterminal geometric mean remains at least the smallest supplied return
  and therefore above `-1` for representable nonterminal inputs; a numerical
  collapse of that mean to `-1` is a calculation failure.
- Use scaled/compensated averaging so overflowing raw sums do not discard a
  representable mean. Evaluate requested arithmetic excess/scaled results
  without separately overflowing `A*mean` and `A*q`, or erasing a nonzero
  differential through avoidable cancellation/underflow. Reuse the canonical
  numerical infrastructure rather than create alternative estimators.
- Representability applies to requested outputs, not every conceptual wealth
  level or unrequested intermediate. A valid final CAGR/geometric/relative
  result can survive absolute wealth overflow/underflow. A cumulative-series
  request needs every emitted value; overflow in one requested prefix errors
  that series call, even if a later prefix would recover to a finite value.
  A final-only statistic need not reconstruct those unrequested prefixes.
- Validate financial requests/history before availability outcomes. Invalid
  history/cash/alignment/context is a request error; unresolved adequate
  same-estimator evaluation or an unrepresentable requested result is a
  calculation error. Expected empty/zero-clock/benchmark-zero outcomes remain
  aligned undefined values where specified, preserving other available
  fields/tickers. Do not substitute numerical failures with NaN, zero or
  artificial infinity, or return a partially repaired failed calculation.

Preserve finite values, permitted undefined results and collection shapes
across Rust/Python/WASM and the already accepted wire conventions. No new
wealth-output class, caller tuning or compatibility layer is needed.
Independent qualification must cover tiny/large growth, genuine versus
reported terminal states, paired-path cancellation, requested-prefix versus
final-only overflow, cash offsets, day-count/context boundaries and host/
serialization behavior. The [accepted record-03 package](03-periods-lookbacks-and-win-loss.md#accepted-decision-calendar-grouping-lookbacks-and-win-loss-statistics)
owns calendar/lookback/win-loss choices,
and records 04–15 keep their separate estimator and report scopes.

[Sharpe's differential-return definition](https://stanford.edu/~wfsharpe/art/sr/sr.htm)
supports same-period arithmetic subtraction, not compounding those spreads
as a funded wealth history. Core day-count algorithms retain their own
methodology ownership. Formula identities and locally derived examples do
not certify empirical return provenance or numerical accuracy.

## Implementation evidence

[returns.rs](../../../../finstack-quant/analytics/src/returns.rs), [risk_metrics/return_based.rs](../../../../finstack-quant/analytics/src/risk_metrics/return_based.rs), [performance/aggregation.rs](../../../../finstack-quant/analytics/src/performance/aggregation.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
