# ANALYTICS-004: Sample volatility, downside risk and return-based reward ratios

**Decision status:** Accepted for the seven estimator/input/availability scopes below (2026-11-06) and the remaining complete standalone-input, MAR-domain and scalar numerical/failure package (2026-11-07).

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: standalone scalar domains and numerical failures

**Accepted:** 2026-11-07.

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
core/dependency capabilities in the implementation. The primary
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
These are independently derived expected cases, independent arithmetic definitions.

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
Relevant numerical boundary cases include raw-domain/terminal boundaries,
invalid inputs hidden by short samples, extreme MAR/hurdles, true constant
versus tiny variation, large offsets, underflow/overflow, final-only ratios
and matching host errors/nonfinite outputs.

## Accepted decision: sample volatility and Sharpe

**Accepted:** 2026-11-06.

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

## Accepted decision: explicit per-observation MAR

**Accepted:** 2026-11-06.

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

## Accepted decision: downside deviation and Sortino

**Accepted:** 2026-11-06.

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

## Accepted decision: empirical Omega and explicit threshold

**Accepted:** 2026-11-06.

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
The undefined degenerate result is this API's accepted convention rather than
a claim that every original-paper or library variant uses this outcome.

## Accepted decision: arithmetic gain-to-pain

**Accepted:** 2026-11-06.

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

## Accepted decision: observation-period modified Sharpe

**Accepted:** 2026-11-06.

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
numerical evidence is limited to its stated cases.
The undefined availability outcomes above must not be replaced with ordinary
Sharpe's zero-risk limit. Invalid cash/frequency requests retain their accepted
validation errors.

## Accepted decision: insufficient history and zero risk

**Accepted:** 2026-11-06.

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

## Implementation evidence

[risk_metrics/return_based.rs](../../../../finstack-quant/analytics/src/risk_metrics/return_based.rs), [risk_metrics/mod.rs](../../../../finstack-quant/analytics/src/risk_metrics/mod.rs), [performance/scalar.rs](../../../../finstack-quant/analytics/src/performance/scalar.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
