# ANALYTICS-011: Tracking error, capture, batting average, Treynor and M-squared

**Decision status:** Accepted for the complete benchmark-relative metric package below. Implementation reconciliation and independent qualification remain open; other records retain their own scopes.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice and domain

The [accepted record-10 contract](10-benchmark-regression.md#accepted-decision-single-factor-benchmark-regression)
owns OLS beta, including an available point estimate with two varying-
benchmark pairs and field-specific uncertainty minima. This record separately
owns relative-metric formulas, capture subsets and denominator conventions.

Benchmark-relative statistics operate on the intersection of portfolio/benchmark active date spans. Equal observation weighting and the declared frequency A are used; no currency/benchmark hedging, missing-date interpolation or transaction cost adjustment is performed. Signed decimal simple returns are supplied by the caller. Capture ratios are ratios, not multiplied by 100.

## Accepted decision: benchmark-relative metrics

**Accepted:** 2026-11-07. **Implementation:** Partial; selected source
inspected at `827ebb75b9`. Alignment/availability, strict up selection,
cutoff removal, M-squared boundary outcomes and numerical/error behavior
require reconciliation. No runtime estimator checks were executed.

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

## Algorithms and selection

Tracking error is the sample standard deviation of active returns `p−b`, annualized by sqrt(A). Information ratio is `mean(p−b)*sqrt(A)/sample_sd(p−b)` using the common signed-infinity/0/0 helper. The private kernels return zero for empty input, but the public Information Ratio wrapper requires at least two pairs and returns NaN otherwise; public Tracking Error can remain zero for empty/singleton samples.

Zero substitutions above require reconciliation with the
[accepted history/zero-risk outcomes](04-return-risk-ratios.md#accepted-decision-insufficient-history-and-zero-risk).
Capture conventions and removal of near-zero cutoffs are accepted above;
the observed behavior below requires reconciliation with that contract.

Capture selects observation pairs by benchmark return, compounds each selected subset using compensated log growth, annualizes each to `exp(A*sum(log(1+r))/selected_count)−1`, and divides portfolio by benchmark annualized subset return. **Up capture currently selects `benchmark >= 0`; the prior agreed direction requires strictly positive benchmark returns.** Down capture selects benchmark<0. Empty input, no selected observations, or absolute compounded benchmark return below 1e−18 yields zero. Capture ratio divides up capture by down capture, but returns zero when down capture is exactly zero; NaNs otherwise propagate. Neutral benchmark observations can therefore alter the current up-capture numerator/count even though the intended selection excludes them.

Batting average is the fraction of aligned observations for which portfolio return is strictly above benchmark return; ties are not wins, empty input gives zero.

Treynor uses annual arithmetic excess return after geometric cash conversion, divided by OLS beta. Absolute beta below 1e−10 is treated as zero, then the common helper produces signed infinity or zero; invalid excess/beta gives NaN. Negative beta remains signed. The public wrapper requires two observations and uses the slope-only estimator, not the three-observation `BetaResult` uncertainty contract.

M-squared is the **absolute** risk-adjusted annual return `cash_A+(ann_mean−cash_A)*(benchmark_ann_vol/portfolio_ann_vol)`, where cash_A=A*q. It does not subtract the benchmark return. Either annual volatility below 1e−10 or any nonfinite input yields NaN. Both volatilities are computed over the same aligned span; no independent benchmark date window is substituted.

## Rationale, alternatives and limitations

Geometric subset returns distinguish capture from the ratio of arithmetic means, but annualizing a small selected sample can amplify scale dramatically. The accepted package retains no clipping or extra subset-count minimum and requires stable evaluation of representable requested results. The observed 1e−18 capture and 1e−10 beta/volatility cutoffs must be removed. Arithmetic capture, full-period conditional attribution and relative rather than absolute M² remain unimplemented alternatives, outside this accepted estimator. The observed inclusive up filter conflicts with the accepted strictly positive selection.

## Implementation and validation evidence

- [Owning formulas](../../../../finstack-quant/analytics/src/benchmark.rs) and [public alignment/guards](../../../../finstack-quant/analytics/src/performance/benchmark.rs).
- Located inline capture tests compare geometric subset formulas and no-up/no-down conditions; they are implementation/formula checks, not validation of neutral-observation inclusion. Treynor/M² hand cases and [audit regression](../../../../finstack-quant/analytics/tests/audit_regressions.rs) `m_squared_self_comparison_has_no_cash_basis_residual` cover identity/basis behavior. No tests ran.
- Canonical context: [Treynor](../../../REFERENCES.md#treynor1965), [M²](../../../REFERENCES.md#modigliani1997), and [tracking/active management](../../../REFERENCES.md#grinoldKahn1999ActivePortfolio). The capture convention and cutoff removal are accepted above; independent numerical/statistical qualification remains open.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[benchmark-relative metric follow-up](../FOLLOW_UPS.md#analytics-benchmark-relative-metrics).
