# ANALYTICS-011: Tracking error, capture, batting average, Treynor and M-squared

**Decision status:** Accepted for the complete benchmark-relative metric package below.

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: benchmark-relative metrics

**Accepted:** 2026-11-07.

This package
covers all remaining alignment, formula, subset, scaling, denominator,
availability, interpretation and failure choices for this record's eight
metrics. Existing explicit frequency/cash, sample-risk, zero-risk and record-10
OLS contracts remain constraints. Strictly positive up-capture selection is
already agreed and is not reopened.

### Shared observations, cash and units

Use exactly the common ticker/benchmark active return observations, with equal
weights. Every component of an individual metric uses the same paired span,
or the same explicitly defined capture subset. Require matched finite arrays
and ordered aligned dates; no shorter-prefix truncation, filling, interpolation
or independent benchmark history. Original investment histories retain
record 02's domain; derived active differences `a_i = p_i - b_i` are arithmetic
statistics and may be below `-1`.

Let `A` be explicit periods per year, `R` explicit annual-effective cash where
used, `q = expm1(log1p(R) / A)`, `C = A*q`, and
`E = A * (mean(p) - q)`. Cash is required only for Treynor and M-squared.
Validate relevant cash/frequency inputs before insufficient-history outcomes.
Currency/benchmark hedging, fee adjustments and return provenance remain
caller/panel responsibilities; no automatic adjustment is introduced.

### Tracking error, information ratio and batting average

For `N >= 2` paired observations, use centered sample SD with denominator
`N - 1` of active differences:

```text
tracking_error = sqrt(A) * sample_sd(p - b)
information_ratio = sqrt(A) * mean(p - b) / sample_sd(p - b)
```

This is variation about mean active return and arithmetic active reward,
not RMS difference from zero or regression-residual risk. Preserve the accepted
unadjusted annualization limitation; no serial-dependence correction is
introduced. TE is decimal annualized volatility; IR is dimensionless.
Adequately observed constant active differences have zero TE.
Then finite nonzero mean active reward gives signed infinite IR, while zero
reward/zero risk is undefined. Fewer than two pairs makes both unavailable.

Batting average is `count(p_i > b_i) / N` for `N >= 1`, including benchmark-
neutral observations. Ties stay in the denominator and are not wins; no
half-credit or tolerance is applied. Empty history is undefined. Return a
fraction in `[0, 1]`, without annualization or cash adjustment.

### Capture estimator, populations and result domains

Up capture selects only `b_i > 0`; down capture selects only `b_i < 0`.
Benchmark-neutral observations enter neither capture subset, while remaining
eligible for all other metrics. For either selected subset `S` of size `m`:

```text
G_x = expm1((A / m) * sum(log1p(x_i), i in S))
capture = G_portfolio / G_benchmark
```

Use selected count `m`, not full-history count or actual elapsed dates, with
the declared full observation frequency `A`. This annualizes conditional
subset growth; it is not calendar-year contribution or dated CAGR. Geometric
annualization does not cancel in the ratio. One selected monthly pair with
portfolio `+2%` and benchmark `+1%` gives approximately `2.115054`, rather
than the cumulative-return ratio of two.

| Capture condition | Accepted outcome |
| --- | --- |
| No aligned observations or no observations in the requested regime | Undefined, preserving absent evidence rather than zero. |
| At least one selected pair | Compute the signed geometric annualized subset ratio; no extra sample-count gate. |
| Zero portfolio subset growth | Individual capture zero when its strict-regime benchmark denominator is valid. |
| Negative down capture | Preserve it: the portfolio gained while the selected benchmark declined. |
| Combined `capture_ratio` | Signed up capture divided by down capture, requiring both regimes/components to be mathematically defined and down capture nonzero. |
| Combined capture with down capture zero | Undefined, including when up capture is nonzero. A flat aggregate conditional return does not establish zero measured risk. |

Remove the `1e-18` benchmark-growth cutoff. With a nonempty strict-sign
benchmark subset, benchmark growth is mathematically nonzero; a computed zero
requires adequate same-estimator evaluation or a calculation error.
Combined capture's zero-denominator rule is a specific accepted product
choice, separate from the accepted nonnegative-risk division conventions.
Preserve negative combined ratios without claiming that a higher value
universally establishes superior performance.

All capture outputs are quotients: one means 100% capture, without multiplying
results by 100. The combined metric corresponds to up/down capture, not the
ratio of full-history portfolio and benchmark growth.

Validate original investment histories before regime filtering. A selected
valid terminal `-1` return gives subset growth exactly `-1`, without passing
negative infinity through finite compensated sums. No percentage continuation
after wipeout or inserted future zero returns are permitted. A terminal event
on a benchmark-neutral observation remains outside both conditional subsets;
capture is not a complete performance summary.

### Treynor and absolute M-squared

Treynor is `E / beta`, using the accepted record-10 intercept OLS point beta
on the same paired observations. Require two pairs and an identified beta.
Remove the `1e-10` beta cutoff; preserve every finite nonzero beta and its
sign. For true beta zero, apply the chosen reward/risk convention: finite
nonzero excess reward gives infinity with that reward's sign; zero reward
gives undefined. Unidentified beta remains unavailable. Units are annual
arithmetic excess-return fraction per unit beta; negative-beta Treynor is
descriptive and does not support a universal higher-is-better ranking.

M-squared remains the absolute arithmetic annual risk-adjusted return:

```text
M2_absolute = C + E * (sample_sd(b) / sample_sd(p))
```

Use sample SDs from the identical paired span and require `N >= 2`. The cash
basis is arithmetic annual cash `C = A*q`, consistent with the numerator;
do not add annual-effective `R` directly or substitute CAGR. The function does
not subtract benchmark mean return. A caller comparing it with the benchmark
must use the same arithmetic annual basis `A * mean(b)`.

| M-squared risk domain | Accepted outcome |
| --- | --- |
| Portfolio SD positive, benchmark SD positive | Apply the absolute risk-matching formula. |
| Portfolio SD positive, benchmark SD zero | Return `C`: zero target risky allocation. |
| Portfolio SD zero | Undefined, including when both SDs are zero, because the risk-matching scale is unidentified/unavailable. |
| Fewer than two pairs | Undefined sample risk and M-squared. |

Remove both `1e-10` volatility cutoffs. M-squared does not require beta, so a
constant benchmark can be a valid zero-volatility target even though its
regression slope is unidentified. With positive portfolio SD, self-comparison
must recover its arithmetic annual mean regardless of cash. For annual
arithmetic mean 12%, portfolio SD 20%, benchmark SD 15%, and explicit 2% cash
with `A = 1`, absolute M-squared is 9.5%.

### Numerical/output rules and reference limits

Use adequately scaled centered moments and stable `log1p`/`expm1` or
log-ratio evaluation. Preserve true constant active differences, zero beta
and volatility roles, without manufacturing zeros through underflow,
cancellation or fixed cutoffs. Genuine zero TE requires constant active
differences; genuine zero portfolio/benchmark SD requires that corresponding
return series to be constant. Avoidable intermediate overflow must not
discard a representable requested answer: identical explosive histories can
have capture exactly one even when each conceptual annualized return would
overflow. The same applies to a requested combined capture whose unrequested
component magnitudes need not individually fit the output type.

Invalid requests are validation errors. If adequate same-estimator evaluation
cannot produce a valid representable requested result, fail the affected
metric call with a calculation error. Do not report numerical overflow as an
intentional zero-risk infinity, repair inputs, substitute an estimator or emit
a partly repaired result. Ordinary absent/insufficient/domain-unavailable
statistics remain undefined per ticker, preserving finite zeros and accepted
intentional infinities across Rust/Python/WASM and serialization. Independent
numerical/statistical qualification and broader summary failure behavior
remain separately scoped.

The chosen capture family follows [Empyrical's strict-sign selectors](https://raw.githubusercontent.com/quantopian/empyrical/master/empyrical/utils.py)
and [annualized capture/up-down definitions](https://raw.githubusercontent.com/quantopian/empyrical/master/empyrical/stats.py).
[PerformanceAnalytics uses cumulative subset capture conventions](https://raw.githubusercontent.com/braverock/PerformanceAnalytics/master/R/UpDownRatios.R),
so no universal vendor equivalence is claimed. The absolute risk-matched-return
interpretation also appears in its [Modigliani implementation](https://raw.githubusercontent.com/braverock/PerformanceAnalytics/master/R/Modigliani.R),
with different cash/scaling inputs, and the beta-normalized excess numerator
in its [Treynor implementation](https://raw.githubusercontent.com/braverock/PerformanceAnalytics/master/R/TreynorRatio.R).
These references do not validate the numerical domains or qualify the small-
subset annualization used here.

## Implementation evidence

[benchmark.rs](../../../../finstack-quant/analytics/src/benchmark.rs), [performance/benchmark.rs](../../../../finstack-quant/analytics/src/performance/benchmark.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
