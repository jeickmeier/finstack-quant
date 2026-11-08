# ANALYTICS-004: Sample volatility, downside risk and return-based reward ratios

**Decision status:** Accepted for the seven estimator/input/availability scopes below (2026-11-06) and the remaining complete standalone-input, MAR-domain and scalar numerical/failure package (2026-11-07). Implementation reconciliation and independent qualification remain open; other metric owners retain their scopes.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice, rationale and domain

The scalar ratios use equal observation weights and the panel's declared observation frequency. Sample volatility uses n−1; downside risk uses all N observations in its lower-partial-moment denominator. These are materially different estimators. Returns, thresholds and MAR are per-observation decimal fractions; Sharpe cash is an annual effective decimal rate converted as in [record 02](02-wealth-and-annualization.md).

The [accepted cash-rate decision](02-wealth-and-annualization.md#accepted-decision-cash-rates)
owns explicit inputs, representation, conversion and validation. Implicit
zero cash in host calls requires reconciliation. MAR and Omega have their own
accepted input rules below.

## Accepted decision: standalone scalar domains and numerical failures

**Accepted:** 2026-11-07.
**Implementation:** Not reconciled; selected source inspected at `827ebb75b9`.
Standalone history validation, explicit finite MAR range and scaled scalar
evaluation/error propagation remain open. No runtime checks were executed.

All seven accepted sections below retain their formulas, scaling, minimum
samples and zero-denominator outcomes. This package closes the remaining
input/numerical boundaries, without adding another estimator, a cash-series
Sharpe mode, serial-correlation corrections or new tuning parameters.
[Record 02](02-wealth-and-annualization.md#accepted-decision-wealth-means-cagr-relative-wealth-and-cash-series)
owns arithmetic means/cash conversion and
[record 09](09-rolling-return-risk.md#accepted-decision-rolling-return-risk-calculations)
owns rolling selection/rebuilds. Their accepted requirements remain constraints.

### Standalone total-investment histories and request validation

Make public standalone `volatility`, `sharpe` and `sortino` consume the same
**total-investment histories** as `Performance`: chronological finite decimal
simple returns greater than `-1`, with one exact `-1` allowed only as the
final observation. Reject values below `-1`, any continuation after a
terminal loss, and every raw `NaN`/infinity. Standalone slices do not acquire
the panels' leading/trailing missing-value padding convention. Empty slices
remain valid samples with unavailable estimates under each accepted minimum.
Validate the entire supplied slice before insufficient-sample shortcuts.
A valid terminal loss is included normally in the arithmetic estimator;
it does not force volatility or a ratio itself to be `-1`.

This is an explicit financial API boundary, **not** a mathematical requirement
of variance or lower partial moments. Generic core statistics remain available
for arbitrary finite statistical values. Do not introduce an input-kind enum,
unchecked financial path or separate excess-return overload. Derived excess
differences are arithmetic statistics and do not inherit the investment
return bound or terminal chronology.

Require explicit finite MAR, with **any finite real value** accepted in the
same observation-period decimal units as returns. A hurdle can be below
`-1`; it is not itself an observed funded investment return. Do not clamp it,
impose the return-history lower bound, annualize it or substitute cash/zero.
For example, `MAR = -2` is a deliberately supplied -200% period hurdle.
Nonfinite MAR is an invalid request rather than an unavailable estimate.
Omega retains its already accepted any-finite threshold domain and separate
explicit input; it does not automatically share MAR.

Validate all actually used arguments before sample shortcuts: standalone
positive finite `periods_per_year`, panel frequency when required, annual
effective cash under record 02, finite MAR/Omega threshold, and confidence
for modified Sharpe. Thus an empty/single-observation sample does not hide
an invalid request. Preserve existing unit choices: standalone volatility
is annualized; `Performance.volatility(annualize = false)` needs no unused
frequency, while public downside deviation remains annualized and requires
frequency. No extra native-downside API is selected.

### Requested scalar estimates and genuine zero risk

Evaluate the accepted arithmetic mean, centered sample SD and full-sample
shortfall RMS with adequately scaled/compensated arithmetic. Preserve small
resolved variation and representable large results; compensation alone does
not prevent overflow/underflow when squaring differences. Prefer existing
core/dependency capabilities when implementation is authorized. The primary
[LAPACK scaled sum-of-squares contract](https://netlib.org/lapack/explore-html/d8/d76/group__lassq_gae8f40b0a34771b4f2d9c863de3af7be5.html)
is numerical context, not a new mandatory dependency or financial estimator.

Only requested final statistics and explicitly required owner quantities
must be representable. A sample SD need not first materialize its potentially
overflowing/underflowing variance square. A finite Sharpe/Sortino need not
first materialize annualized numerator, annualized risk or raw square sums.
Use equivalent adequately evaluated same-estimator expressions, including
the accepted `sqrt(A) * period_excess / period_risk`, without forcing an
overflow-prone multiplication/division order. Apply the same principle to
Omega/gain-to-pain gross/net sums and ratios. This does not relax cash
conversion validation or modified Sharpe's independently required **finite,
valid CF quantile**, its ordering/domain gate and `Q_CF < 0` availability.
An unrepresentable required CF quantile still errors even if a formal final
division could be finite. Other metric owners retain their own contracts.

Zero dispersion requires a genuinely constant observed sample; zero downside
requires no actual return below MAR. Do not infer either from collapsed
squares, cancellation, negative rounded variance or an epsilon threshold.
Use direct observed equality/shortfall comparisons to check those conditions.
If a positive risk is lost numerically, recover the same estimator with
adequate scaling or fail the calculation. Do not manufacture zero risk,
undefined history or an intentional zero-risk infinity from arithmetic
failure. Determine the zero/nonzero reward and its sign adequately before
applying the accepted denominator-zero rule. No floors, clipping, winsorizing,
epsilon-neutral bands, distribution substitution or public tolerances are
selected.

For `[+1e-200, -1e-200]`, sample SD is `sqrt(2) * 1e-200`, not zero even
though directly squared differences underflow. For `[0, 1e308]`, `A = 12`
and explicit zero cash, Sharpe is `sqrt(6)`, although separately requested
annualized mean and volatility are unrepresentable. Those separate requests
must error; their unnecessary intermediate evaluation must not spoil Sharpe.
These are independently derived expected cases, not executed source tests.

### Invalid requests, unavailable estimates and computation failures

Keep three distinct outcomes under the already accepted estimator minima:

| Situation | Required outcome |
| --- | --- |
| Invalid history/context/input | Explicit request error before sample shortcuts. |
| Valid but insufficient sample, or an owner-defined unavailable denominator | Undefined estimate; retain other available ticker estimates. |
| Genuine zero risk | Accepted measured-zero/undefined/signed-infinity result, as appropriate to that metric. |
| Positive denominator with unrepresentable finite ratio, unrepresentable requested risk, or unresolved invalid numerical state | Explicit calculation error, not an infinity/zero/undefined substitution. |

Preserve the modified-Sharpe exception to ordinary zero-risk limits: the
accepted CF moment/variance availability, invalid-curve errors and nonpositive
loss-denominator rules remain authoritative. Empty/all-neutral Omega and
gain-to-pain keep their accepted undefined outcomes. Standalone raw functions
and affected panel scalar methods must propagate `Result`-based errors through
the existing Python/WASM mappings; remove old silent-NaN failure paths rather
than adding wrappers or compatibility aliases. A genuine calculation failure
fails that metric call; permitted per-ticker unavailability is not an error.
Record 01 continues to own fixed-summary atomic failure and wire behavior.

Preserve ordered scalar/vector shapes and canonical undefined/signed-infinity
wire values. Add no new result carrier, status hierarchy or metadata framework.
Reconcile canonical Rust, both bindings, declarations/stubs, parity, schema,
UI consumers, docs and fixtures together when implementation is authorized.
Independent qualification remains open for raw-domain/terminal boundaries,
invalid inputs hidden by short samples, extreme MAR/hurdles, true constant
versus tiny variation, large offsets, underflow/overflow, final-only ratios
and matching host errors/nonfinite outputs. This review changes records only.

## Accepted decision: sample volatility and Sharpe

**Accepted:** 2026-11-06. **Implementation:** Partial; selected source
inspected at `5140b499cc`. Scalar and rolling formulas match, while accepted
input and boundary outcomes still require reconciliation. Implementation and
independent numerical checks have not run.

For `N` valid observations, arithmetic mean `mean(r)`, explicitly supplied
frequency `A`, and constant annual effective cash `R`, use:

```text
s = sqrt(sum((r - mean(r))^2) / (N - 1))
q = expm1(log1p(R) / A)
annualized_volatility = sqrt(A) * s
Sharpe = sqrt(A) * (mean(r) - q) / s
```

Volatility uses sample standard deviation, with denominator `N - 1`. Sharpe's
numerator uses arithmetic mean excess return. Constant period cash does not
change sample standard deviation when subtracted from every observation.
Use the same estimator within each rolling window. Nonannualized volatility
is `s` and does not require frequency solely under this rule.

Square-root scaling is the conventional unadjusted reporting method. Its
interpretation as annual risk requires suitable stable moments and no serial
covariance under additive aggregation; it is not exact volatility of compounded
annual returns. No automatic serial-correlation correction or estimator switch
is selected. The construction follows [Sharpe's historical ratio](https://stanford.edu/~wfsharpe/art/sr/sr.htm),
with the aggregation limitations described by [Lo](https://rpc.cfainstitute.org/research/financial-analysts-journal/2002/the-statistics-of-sharpe-ratios).

For monthly mean 1%, monthly sample standard deviation 2% and annual effective
cash 5%, annualized volatility is approximately 6.9282%, annualized arithmetic
excess return is 7.1111%, and Sharpe is 1.0264. The accepted explicit-frequency,
cash, minimum-sample and zero-risk rules apply. Time-varying cash inputs for
Sharpe remain outside this constant-cash scope. Qualification is owned by the
[volatility/Sharpe follow-up](../FOLLOW_UPS.md#analytics-volatility-and-sharpe).

## Accepted decision: explicit per-observation MAR

**Accepted:** 2026-11-06. **Implementation:** Partial; selected source
inspected at `5140b499cc`. Rust methods take per-observation MAR, while host
defaults and hardcoded summary targets require reconciliation. Implementation
and cross-interface checks have not run.

Sortino, rolling Sortino, downside deviation and summaries containing these
metrics require an explicitly supplied minimum acceptable return (MAR).
Omission is a missing-input validation error. An explicit `0.0` selects a zero
target; neither zero nor the cash rate is supplied automatically.

MAR is a decimal simple-return target per observation, matching the input
return periodicity. For monthly returns, `0.005` means a 0.5% monthly target.
A caller choosing a 6% annual effective target converts it first using the
declared frequency: `(1.06)^(1/12) - 1`, approximately `0.00486755` monthly.
The calculation does not reinterpret MAR as an annual rate. A summary uses
the caller's MAR for its affected metrics instead of hardcoding zero.

MAR is an investor-selected hurdle and need not equal cash. Its units follow
the [published PerformanceAnalytics Sortino contract](https://search.r-project.org/CRAN/refmans/PerformanceAnalytics/html/SortinoRatio.html);
requiring an explicit input is this API's chosen policy. Omega has its own
accepted rule below; other estimator formulas are accepted in their own
sections. The complete domain/numerical package above is accepted.
Reconciliation is owned by the
[MAR follow-up](../FOLLOW_UPS.md#analytics-explicit-mar).

## Accepted decision: downside deviation and Sortino

**Accepted:** 2026-11-06. **Implementation:** Partial; selected source
inspected at `5140b499cc`. Scalar/rolling formulas and Sortino's minimum
sample size match; empty downside history and zero-over-zero outcomes require
reconciliation. Implementation and cross-interface checks have not run.

Use the explicit per-observation MAR and all `N` valid observations:

```text
DD = sqrt(sum(min(r - MAR, 0)^2) / N)
annualized_downside_deviation = sqrt(A) * DD
Sortino = sqrt(A) * (mean(r) - MAR) / DD
```

`A` is the explicitly supplied observations-per-year convention. The full
observation count measures shortfall frequency as well as size; dividing only
by observations below MAR would measure severity conditional on a shortfall.
The chosen denominator matches the full-series method in
[PerformanceAnalytics' downside-deviation contract](https://search.r-project.org/CRAN/refmans/PerformanceAnalytics/html/DownsideDeviation.html).
Square-root scaling is frequency-based reporting, not a calculation of the
downside deviation of compounded annual outcomes.

| Valid history | Required outcome |
| --- | --- |
| At least one observation | Downside deviation is calculable. |
| Empty history | Undefined downside deviation and scalar Sortino. |
| Fewer than two observations for scalar Sortino or an available rolling Sortino window | Undefined Sortino. |
| No observations below MAR | Zero measured downside deviation. |
| Enough observations, none below MAR and some above MAR | Positive-infinite Sortino. |
| Every observation equals MAR | Undefined Sortino (`0/0`). |

Sortino's two-observation minimum is a reporting policy, not a claim that a
one-observation lower partial moment is mathematically undefined. Rolling
window selection and output shape remain separately owned by
[record 09](09-rolling-return-risk.md).

For `[+2%, 0%, -1%]` and explicit MAR zero, `DD` is approximately 0.57735%.
With monthly observations (`A = 12`), annualized downside deviation is 2%
and Sortino is 2. Reconciliation and verification are owned by the
[downside/Sortino follow-up](../FOLLOW_UPS.md#analytics-downside-and-sortino).

## Accepted decision: empirical Omega and explicit threshold

**Accepted:** 2026-11-06. **Implementation:** Partial; selected source
inspected at `5140b499cc`. The empirical sum formula is implemented, but host
defaults, summary inputs, threshold validation and the all-at-threshold result
require reconciliation. Implementation and cross-interface checks have not run.

Omega and summaries containing it require an explicit finite decimal threshold
per observation, matching the return periodicity. An explicit zero is valid;
omission or a nonfinite threshold is an input validation error. The calculation
does not automatically reuse cash or Sortino MAR or reinterpret the threshold
as annual.

For equally weighted observations and supplied threshold `h`, use:

```text
gains = sum(max(r - h, 0))
shortfalls = sum(max(h - r, 0))
Omega = gains / shortfalls
```

| Observed outcomes | Required result |
| --- | --- |
| Positive gains and shortfalls | Their ratio; +2% and -1% at threshold zero gives 2. |
| Positive gains, no shortfalls | Positive infinity. |
| Shortfalls, no gains | Zero. |
| Every return equals the threshold | Undefined (`0/0`). |
| Empty history | Undefined. |

One valid observation suffices for this descriptive calculation; it does not
establish a reliable distribution estimate. Omega is dimensionless and is not
annualized. It uses observed empirical sums without distribution fitting or
interpolation. The formula matches the empirical method in the
[PerformanceAnalytics implementation](https://raw.githubusercontent.com/braverock/PerformanceAnalytics/master/R/Omega.R).
The undefined degenerate result is this API's accepted convention; the current
source attribution does not establish a contrary original-paper requirement.

Gain-to-pain has its own accepted rule below; other gain/loss metrics retain
their separate adoption status.
Reconciliation is owned by the [Omega follow-up](../FOLLOW_UPS.md#analytics-omega).

## Accepted decision: arithmetic gain-to-pain

**Accepted:** 2026-11-06. **Implementation:** Partial; selected source
inspected at `5140b499cc`. The arithmetic formula and empty-history outcome
are implemented; the all-zero result requires reconciliation. Implementation
and cross-interface checks have not run.

For valid period returns, calculate:

```text
gain_to_pain = sum(r) / sum(abs(r) for r < 0)
```

This is the arithmetic net sum of period returns per unit of negative period
returns, matching [Schwager's explanation](https://www.linkedin.com/pulse/what-best-way-measure-risk-part-2-jack-schwager).
It does not compound returns, subtract cash, accept a configurable hurdle or
annualize. Zero identifies losses in the definition; it is not an omitted
threshold input. Results are dimensionless and should be compared at matching
observation frequencies.

| Valid return history | Required result |
| --- | --- |
| +2%, -1% | 1. |
| +1%, -2% | -0.5. |
| Only losses, possibly with zero returns | -1. |
| Positive gains and no losses | Positive infinity. |
| All zero, or empty history | Undefined. |

One valid observation suffices for this descriptive calculation. For a
positive loss sum, divide the net sum by that loss sum; the finite result is
at least -1. This does not adopt profit-factor, Kelly or other gain/loss
conventions. Reconciliation is owned by the
[gain-to-pain follow-up](../FOLLOW_UPS.md#analytics-gain-to-pain).

## Accepted decision: observation-period modified Sharpe

**Accepted:** 2026-11-06. **Implementation:** Not reconciled; selected source
inspected at `5140b499cc`. The current annual horizon, two-observation minimum
and inherited Gaussian substitution conflict with the accepted scope.
Implementation and independent numerical checks have not run.

Use a corresponding-observation-period excess numerator and a Cornish–Fisher
loss denominator calculated from total returns:

```text
q = expm1(log1p(R) / A)
Q_CF = one-observation-period CF lower-tail quantile of total returns
modified_sharpe = (mean(r) - q) / (-Q_CF), provided Q_CF < 0
```

`R` is the explicitly supplied annual effective cash rate and `A` is the
declared observations per year used for cash conversion. Both ratio components
refer to the input observation period. Do not project them to an annual
horizon or multiply the ratio by `sqrt(A)`. This measures excess reward per
unit of absolute portfolio loss risk, following the corresponding-period
definition in [Ardia and Boudt, section 2](https://libra.unine.ch/bitstreams/f86ec672-39c1-43ca-ac33-025486604e6b/download).
It does not select the alternative denominator based on excess-return VaR.

| Situation | Required result |
| --- | --- |
| Fewer than four observations, or zero variance preventing shape estimation | Undefined. |
| Supported, finite, valid CF return quantile below zero | Calculate the signed ratio. |
| Supported, finite, valid CF return quantile at or above zero | Undefined: no positive loss denominator. |
| Invalid CF approximation/domain or nonfinite computed CF estimate | Explicit error; no Gaussian or historical substitution. |

A valid standalone CF quantile at or above zero remains reportable under the
[accepted signed-tail contract](07-historical-tail-risk.md#accepted-decision-signed-tail-outputs).
Its modified-Sharpe result above is undefined; do not take its absolute value
to manufacture a positive loss denominator.

Four observations establish moment availability, not reliability.
[Record 08](08-moments-gaussian-and-cornish-fisher.md) owns the accepted
classical plug-in estimator and global ordering. Confidence follows the
[accepted explicit-input contract](01-panels-windows-and-wire.md#accepted-decision-explicit-tail-confidence);
accuracy qualification remains open.
The undefined availability outcomes above must not be replaced with ordinary
Sharpe's zero-risk limit. Invalid cash/frequency requests retain their accepted
validation errors.

For monthly mean return 1%, monthly cash 0.4% and a valid monthly CF loss VaR
of 4%, modified Sharpe is 0.15. Reconciliation is owned by the
[modified-Sharpe follow-up](../FOLLOW_UPS.md#analytics-modified-sharpe).

## Accepted decision: insufficient history and zero risk

**Accepted:** 2026-11-06. **Implementation:** Not reconciled; selected source
inspected at `5140b499cc`, with no implementation checks executed.

| Situation | Required outcome |
| --- | --- |
| Fewer than two valid observations for sample volatility, or aligned pairs for sample tracking error | Undefined. |
| Adequate valid history with zero measured dispersion | Preserve zero measured risk. |
| Valid reward/risk inputs with zero reward and zero risk | Undefined (`0/0`). |
| Valid inputs with finite nonzero reward and zero measured risk | Signed infinity according to the reward's sign. |

One observation cannot establish sample dispersion; several identical returns
can establish zero dispersion in the observed sample. Infinity describes the
observed calculation, not future risklessness.

The ratio rules cover Sharpe, Sortino, information ratio and reward/risk
ratios, including reward/drawdown ratios, after each metric satisfies its
sample and domain requirements. Sample-volatility and tracking-error minima
also apply to rolling variants. Other minima remain estimator-specific; the
two-observation rule is not imposed on every lower partial moment or statistic.

Gaussian VaR now follows its separately
[accepted availability and failure rules](08-moments-gaussian-and-cornish-fisher.md#accepted-decision-gaussian-var-availability-and-failures),
including its two-return minimum and valid constant-history quantile. CF
retains its own four-observation and positive-variance requirements.

Rust, Python, WASM and serialization must distinguish finite zero, undefined
and signed infinity, preserving unaffected tickers where supported. Invalid
requests retain validation errors. Invalid Cornish–Fisher estimates retain
their explicit-error requirement. Omega and gain-to-pain are governed by their
accepted rules above. Capture follows the [accepted record-11 package](11-benchmark-relative-metrics.md#accepted-decision-benchmark-relative-metrics),
and Kelly/CPC follow the [accepted record-03 package](03-periods-lookbacks-and-win-loss.md#accepted-decision-calendar-grouping-lookbacks-and-win-loss-statistics).
The complete scalar-domain/numerical controls are accepted above. Reconciliation is
owned by the [zero-risk follow-up](../FOLLOW_UPS.md#analytics-insufficient-history-and-zero-risk).

## Algorithm and selection

Scalar and rolling volatility use the accepted sample estimator, with Welford
moments supplied by core. Core variance is zero for fewer than two observations,
so scalar volatility still returns zero there; that boundary requires
reconciliation. Nonpositive/nonfinite A currently returns NaN.

Scalar and rolling Sharpe use the accepted arithmetic formula and require at
least two observations. Nonfinite annual moments currently return NaN. Scalar
and rolling Sortino use their accepted formula and sample minimum above.
Public downside deviation is annualized, but empty input still returns zero
instead of the accepted undefined result. Nonfinite input or MAR returns NaN.

The common ratio helper first returns NaN for any nonfinite operand; finite nonzero numerator/zero denominator yields signed infinity; finite 0/0 yields zero. This differs from insufficient-sample NaN and from Omega's all-neutral result. Sortino/Sharpe use the helper, preserving finite constant positive/negative excess signs when risk is zero.

Zero substitutions for insufficient sample volatility and reward/risk `0/0`
conflict with the accepted outcomes above and require reconciliation.

The observed Omega kernel matches the accepted empirical sum formula, using
ordinary addition, but its all-at-threshold output of one requires
reconciliation. Gain-to-pain uses the accepted arithmetic formula with Kahan
sums and empty NaN, but inherits the common helper's all-zero result of zero;
that result requires reconciliation with the accepted undefined outcome.

The observed modified-Sharpe kernel uses annual arithmetic excess return and
an annual-horizon CF quantile, with a two-observation minimum. These choices
require reconciliation with the accepted observation-period formulation and
four-observation availability minimum. Its inherited Gaussian substitution is
the separately tracked CF discrepancy in [record 08](08-moments-gaussian-and-cornish-fisher.md).

## Controls, failures and alternatives

Sharpe/Sortino free functions accept A explicitly; `Performance` obtains A from frequency. Scalar threshold/MAR/cash inputs do not share a single `Result`-based validator: invalid numerical inputs usually propagate NaN, while tail confidence is checked at the public boundary. The summary uses zero MAR and zero Omega threshold. It does not estimate cash from a curve or apply serial-correlation corrections.

Host zero-MAR and zero-Omega-threshold defaults, and the summary's hardcoded
targets, conflict with the corresponding accepted input decisions above.

Population risk, downside-only denominators, geometric excess-growth numerators, HAC-adjusted risk and time-weighted irregular observations are unimplemented alternatives here. Choice must match use; current tests alone are not adoption.

## Implementation and validation evidence

- [Return-risk implementations](../../../../finstack-quant/analytics/src/risk_metrics/return_based.rs), [ratio helper](../../../../finstack-quant/analytics/src/risk_metrics/mod.rs), [public dispatch](../../../../finstack-quant/analytics/src/performance/scalar.rs), and [sample moments](../../../../finstack-quant/core/src/math/stats.rs).
- Located inline hand calculations cover downside denominators, threshold Omega, gain-to-pain and MAR consistency. Modified-Sharpe formula comparisons reuse production CF/moment kernels. [Audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) cover singleton/invalid scalar behavior and are boundary/invariant evidence, not distribution calibration.
- Canonical context: [Sharpe](../../../REFERENCES.md#sharpe1966), [Sortino](../../../REFERENCES.md#sortinoVanDerMeer1991), [Omega](../../../REFERENCES.md#keatingShadwick2002), [gain-to-pain](../../../REFERENCES.md#schwager2012), [modified Sharpe](../../../REFERENCES.md#gregoriou2003). Exact source-to-formula reconciliation and tolerances remain open.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
