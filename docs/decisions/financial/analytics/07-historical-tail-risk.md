# ANALYTICS-007: Historical return VaR, exact-tail expected shortfall and tail ratio

**Decision status:** Accepted for the four scoped decisions below (2026-11-06–2026-11-07) and the remaining complete historical-tail numerical evaluation/failure package (2026-11-07).

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: historical-tail numerical evaluation and failures

**Accepted:** 2026-11-07.

The four accepted sections below remain authoritative: signed native-period
R-7 historical VaR, fractional empirical ES, nonempty-sample availability and
the empirical magnitude tail ratio. The accepted confidence contract and
[ADR 0033's fixed historical selection](0033-value-at-risk-method-selection.md#accepted-decision-fixed-historical-tail-selection)
also remain in force. This package closes the remaining numerical/failure
choices without selecting a new estimator or reopening their financial definitions.

### Active samples, confidence and consequential fractional weights

Use each ticker's valid active investment observations under accepted records
[01](01-panels-windows-and-wire.md#accepted-decision-panels-windows-wire-and-summary)
and [02](02-wealth-and-annualization.md#accepted-decision-wealth-means-cagr-relative-wealth-and-cash-series).
Validate the chronological source history/terminal state before copying or
selecting order statistics. Sorting returns for a tail calculation does not
create a new investment history to which terminal chronology is reapplied.
The public route remains `Performance`; add no raw historical API, sample
kind, frequency, cash, benchmark or other unused input.

Validate the explicit finite `0.5 < c < 1` request before empty-sample
availability. Treat supplied binary64 confidence as its actual numeric value
and observation counts as integers. Do not reinterpret `0.6`, for example,
as an exact decimal rational after receiving its binary64 value. For this
confidence domain, `p = 1 - c` stays positive; add no confidence buffer,
probability floor or minimum tail count.

Preserve consequential residuals in lower/upper R-7 ranks `(N - 1) * p`
and `(N - 1) * c` (or equivalent reflected ranks), and empirical tail mass
`m = N * p`, including values extremely close to an integer. Use
adequate same-estimator arithmetic for integer indices and residual weights;
do not snap near-integer products, discard small boundary weights, enlarge
tails at ties or renormalize to a different tail mass. Such residuals can
multiply large valid observations and materially affect the result. The
[official R definition](https://stat.ethz.ch/R-manual/R-devel/library/stats/html/quantile.html)
supplies the R-7 order-statistic weighting; this numerical contract does not
claim identical floating-point results to every R/NumPy version.

### Stable R-7 interpolation, ES and empirical support

Reuse the canonical core R-7 owner with adequately scaled, cancellation-aware
linear interpolation. Preserve exact endpoint/constant samples and resolved
small signed quantiles. A finite value inside the observed range alone does
not prove adequate evaluation: naive subtraction of large bracketing values
can erase the sign or scale of a small interpolated result. Re-evaluate the
same estimator or error when numerical zero/sign remains unresolved.

Evaluate fractional ES as a stable weighted tail mean using its accepted
whole and boundary masses. Avoid overflowing raw tail sums or underflowing
each individually divided contribution before adding them. Preserve constant
samples and observed support, including subnormal observations. No epsilon
tail weights, exceedance-only averaging, whole-observation rounding or other
estimator substitution is selected.

For finite observations, both estimators are bounded convex combinations.
Require finite requested results within their relevant observed support:
R-7 between its bracketing order statistics and ES between its selected tail
extrema. This also preserves the model's `-1` lower bound without imposing
a separate clipping rule. The pair obeys `min(sample) <= ES <= R7_VaR <=
max(sample)` at the accepted confidence; that ordering is an independent
qualification invariant despite the different finite-sample conventions.
Do not force an unrequested VaR computation merely to report standalone ES.
Recover adequate same-estimator evaluation or error on failed invariants;
do not clip, floor or project a failed estimate back into the sample range.
These empirical-support rules do not select clipping for Gaussian/CF models.

### Tail ratio and requested-result representability

Evaluate `abs(Q_R7(c)) / abs(Q_R7(1 - c))` from the same underlying R-7
estimates before forcing their independent standalone output rounding.
Use scaled/joint evaluation when needed to preserve a representable requested
ratio. Unrequested component quantiles need not individually fit a binary64
output. This is distinct from modified Sharpe's already accepted requirement
for a finite valid CF quantile under record 04.

Only a genuinely zero lower quantile with positive upper magnitude produces
the accepted `+infinity`. Genuine two-zero quantiles remain undefined; genuine
zero upper magnitude with positive lower magnitude gives zero. Do not infer
these branches from an underflowed or cancellation-damaged component. With
a genuinely positive denominator, overflow or nonzero-to-zero underflow of
the requested ratio is a calculation failure. No near-zero cutoff, absolute
value change, sign-straddling requirement or ratio clipping is selected.

For separately requested VaR/ES, a genuine zero is a valid observed estimate.
A nonzero estimate that cannot be represented without collapsing to zero
is a calculation error. Ordinary finite output rounding remains allowed;
it must not silently manufacture a meaningful zero branch. Only requested
final outputs need reconstruction, subject to each metric's explicit gates.

Let `u = 2^-1074`, the smallest positive binary64 number. At `c = 0.75`,
sample `[0, u]` has underlying R-7 lower `u/4`, upper `3*u/4` and tail ratio
`3`. The ratio is reportable even though separately requested lower VaR
cannot be represented as a nonzero binary64 number; that VaR request errors.
Its ES is genuinely zero and remains reportable. For five identical `u`
observations at supplied `c = 0.6`, ES must remain `u`, rather than vanish when individually
weighted contributions underflow. These are independently derived arithmetic
cases, independent arithmetic definitions or statistical-accuracy certification.

### Failure propagation, outputs and shared owners

Preserve the accepted per-ticker undefined output for empty histories and
tail-ratio `0/0`, aligned with every other ticker's valid result. Invalid
requests and genuine arithmetic/representability failures error the affected
metric call through existing Rust/Python/WASM mappings. Numerical failures
must not become missing history, observed zero, an intentional infinity or
a different estimator. The combined historical VaR/ES path retains the same
two estimators, with errors propagated for either requested output.

Keep existing scalar/vector/host shapes and ticker ordering. Preserve native
numeric `NaN`/`+infinity` only where permitted and the existing summary/wire
sentinels under record 01. Do not add a tail result carrier, status hierarchy
or compatibility path. Genuine errors in a requested summary statistic
abort the summary under its accepted contract. Coordinate shared core
quantile and ES arithmetic with their other callers, including CDaR under
[record 06](06-drawdown-reward-and-cdar.md#accepted-decision-drawdown-reward-and-cdar),
without changing their separately accepted populations or definitions.

Qualification cases include near-integer ranks/masses, closest-to-one confidence,
ties, singleton/constant and terminal samples, cancellation, subnormals,
extreme scales, support/order invariants, separately versus jointly requested
outputs and native/summary host errors.

## Accepted decision: signed tail outputs

**Accepted:** 2026-11-06.

Historical VaR, expected shortfall, Gaussian VaR and Cornish–Fisher VaR return
signed decimal returns consistently across Rust, Python, WASM and summaries.
Preserve the selected estimate without taking its absolute value or applying
a zero floor.

| Valid estimated output | Meaning |
| --- | --- |
| `-0.05` | A 5% loss threshold for VaR, or a mean tail loss of 5% for ES. |
| `+0.01` | The selected lower-tail estimate is still a 1% gain. |
| `0.0` | An estimated zero return, distinct from unavailable history. |

Positive lower-tail estimates are valid and must not be discarded or forced
to zero. Both return-space and loss-space reporting conventions exist, as
discussed in the
[PerformanceAnalytics VaR source](https://raw.githubusercontent.com/braverock/PerformanceAnalytics/master/R/VaR.R).
This decision selects signed returns as the fixed public contract rather
than adding an alternative sign option. Monetary risk conversion is outside
this scope.

CDaR retains signed nonpositive drawdown outputs: `-0.12` means a mean drawdown
of 12% in the selected worst tail. A valid observed path with no drawdown may
produce zero. The [accepted record-06 package](06-drawdown-reward-and-cdar.md#accepted-decision-drawdown-reward-and-cdar)
separately defines CDaR's endpoint population, fractional estimator and empty-
history undefined result.

Under the
[accepted modified-Sharpe rule](04-return-risk-ratios.md#accepted-decision-observation-period-modified-sharpe),
a valid standalone CF quantile at or above zero remains reportable, while
modified Sharpe is undefined because no positive loss denominator exists.
For `Q_CF < 0`, its denominator is `-Q_CF`; the ratio may be positive, zero
or negative according to its excess-return numerator. Do not use an absolute
value to manufacture a positive denominator from a nonnegative CF quantile.
Invalid CF estimates remain explicit errors and unavailable moments remain
undefined.

## Accepted decision: historical VaR and fractional empirical ES

**Accepted:** 2026-11-06.

For an eligible finite active return sample, historical VaR uses the R-7
sample quantile at `p = 1 - c`: sort returns and linearly interpolate at
zero-based rank `(N - 1) * p`. This gives a continuous estimate as confidence
changes. R-7 is a sample-quantile estimator, distinct from the inverse
empirical CDF, as specified in the
[official R quantile documentation](https://stat.ethz.ch/R-manual/R-devel/library/stats/html/quantile.html).

Expected shortfall integrates exactly the worst `p` probability mass of the
equal-weight empirical distribution. For sorted returns
`x_1 <= ... <= x_N`, let `m = N * p` and `k = floor(m)`:

```text
ES_c = (sum(x_i, i = 1..k) + (m - k) * x_(k + 1)) / m
```

The empty sum for `k = 0` is zero. Fractional boundary weighting prevents
rounding or ties from enlarging the selected probability mass. If `m < 1`,
the formula reduces to the worst observed return; this is not evidence of
adequate tail resolution. Splitting discrete probability mass follows the
[Rockafellar–Uryasev general-loss formulation, Definition 3 and Proposition 6](https://sites.math.washington.edu/~rtr/papers/rtr187-CVaR2.pdf),
expressed here as signed returns rather than losses.

The pair deliberately uses different finite-sample conventions. ES is not
the integral of the R-7 interpolated quantile curve and is not the unweighted
mean of observations below the reported VaR. Do not claim that exactly
`1 - c` of finite observations fall below that interpolated threshold.

For 30 observations consisting of `-20%`, `-10%` and 28 zeros, at explicit
95% confidence:

| Calculation | Result |
| --- | --- |
| R-7 VaR, interpolated at rank `29 * 0.05 = 1.45` | `-5.5%`. |
| ES, `(-20% + 0.5 * -10%) / 1.5` | Approximately `-16.6667%`. |
| Unweighted mean of observations below reported VaR | `-15%`, a different estimator. |

Inverse-empirical-CDF VaR would be `-10%` in that example, with the same
fractional empirical ES. That alternative would derive both measures from
one equal-weight scenario distribution; it is not the selected VaR estimator.

## Accepted decision: historical availability and small tails

**Accepted:** 2026-11-07.

Validate the explicitly supplied confidence before applying per-ticker
availability, including for empty histories. Historical VaR and ES use each
ticker's finite valid active returns:

| Active return count | Required outcome |
| --- | --- |
| Zero | Undefined for that ticker. |
| One | Both historical VaR and ES equal that observed return. |
| Two or more | Apply the accepted R-7 VaR and fractional empirical ES formulas. |

There is no hard minimum tail-count cutoff in this descriptive API. For any
nonempty valid sample and accepted confidence, permit positive tail mass
`m = N * (1 - c)`, including `m < 1`. In that case ES equals the worst
observation, while R-7 VaR still follows its interpolation formula. Do not
substitute another estimator or force VaR to the worst observation merely
because `m < 1`.

For ten returns consisting of `-10%`, `-5%` and eight zeros, at explicit
95% confidence, `m = 0.5`, R-7 VaR is `-7.75%` and ES is `-10%`. With a
single `+1%` return, both statistics are `+1%` at any valid confidence.
These outcomes follow the accepted empirical formulas; they do not require
estimating sample variance, unlike Gaussian VaR.

Call `m` tail mass in observation units, not statistical effective sample
size. Dependence and repeated observations can limit information without
changing that probability mass. Computability does not establish reliable
tail estimation. Application-specific statistical qualification remains
separate; do not imply it from the sample count or confidence alone.

## Accepted decision: empirical tail ratio

**Accepted:** 2026-11-07.

Tail ratio uses the upper and lower R-7 quantile magnitudes of each ticker's
valid active return history, at explicitly supplied confidence `c`:

```text
upper = abs(Q_R7(c))
lower = abs(Q_R7(1 - c))
tail_ratio = upper / lower
```

The result is dimensionless and unannualized, with no cash adjustment.
Validate confidence under the accepted `0.5 < c < 1` contract before checking
history. Apply the formula to any nonempty valid sample; preserve per-ticker
availability rather than suppressing other tickers' valid outcomes.

| Situation | Required outcome |
| --- | --- |
| Empty active history | Undefined. |
| Nonempty history, `lower > 0` | Ordinary division, including zero when `upper = 0`. |
| `lower = 0` and `upper > 0` | Positive infinity. |
| `lower = 0` and `upper = 0` | Undefined (`0/0`). |

Use exact-zero denominator rules, without a near-zero cutoff. One nonzero
observation gives one; one zero observation gives undefined. These boundary
rules do not qualify numerical overflow or underflow; their explicit
evaluation/failure contract is accepted above. The implementation evidence is limited to its stated numerical cases.

Allow all-positive and all-negative samples. The measure describes quantile
magnitude asymmetry: an upper-gain/lower-loss interpretation applies when the
selected quantiles straddle zero. Constant `-1%` returns and constant `+1%`
returns both give one; neither a ratio of one nor a ratio above one alone
establishes favorable absolute performance.

## Implementation evidence

[risk_metrics/tail_risk.rs](../../../../finstack-quant/analytics/src/risk_metrics/tail_risk.rs), [performance/scalar.rs](../../../../finstack-quant/analytics/src/performance/scalar.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
