# ANALYTICS-007: Historical return VaR, exact-tail expected shortfall and tail ratio

**Decision status:** Accepted for the four scoped decisions below (2026-11-06–2026-11-07) and the remaining complete historical-tail numerical evaluation/failure package (2026-11-07). Implementation reconciliation and independent qualification remain open; other metric owners retain their scopes.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice, rationale and domain

Historical tail metrics are equal-weight empirical estimators on each ticker's finite active observations, assumed to describe the requested tail. Public confidence c must be finite and strictly inside (0,1); invalid confidence produces `Error::Validation` before private kernels. Empty samples return NaN. The returned VaR/ES are signed decimal returns: −0.05 denotes a 5% loss, not a positive monetary loss. All-positive samples can produce positive outputs. There is no money conversion, zero clipping, time decay or volatility rescaling; historical tails stay in the observations' native period.

The observed confidence range above conflicts with the
[accepted explicit-confidence contract](01-panels-windows-and-wire.md#accepted-decision-explicit-tail-confidence):
require supplied finite `0.5 < c < 1`, without an implicit 95% default.
This input decision does not select an estimator. Output signs and the
historical VaR/ES formulas follow their separately accepted decisions below.

## Accepted decision: historical-tail numerical evaluation and failures

**Accepted:** 2026-11-07.
**Implementation:** Not reconciled; selected source inspected at `827ebb75b9`.
Rank/mass residual preservation, scaled/cancellation-aware interpolation and
tail averaging, requested-result evaluation and calculation-error propagation
require reconciliation. No runtime estimator checks were executed.

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
cases, not executed source tests or statistical-accuracy certification.

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

Reconcile canonical Rust, bindings, stubs/declarations, parity, schemas,
UI consumers, docs and fixtures together when implementation is authorized.
Independently qualify near-integer ranks/masses, closest-to-one confidence,
ties, singleton/constant and terminal samples, cancellation, subnormals,
extreme scales, support/order invariants, separately versus jointly requested
outputs and native/summary host errors. This review changes records only.

## Accepted decision: signed tail outputs

**Accepted:** 2026-11-06. **Implementation:** Partial; selected source
inspected at `5140b499cc`. The kernels preserve signed return estimates, but
public VaR wording incorrectly promises nonpositive outputs. No runtime or
independent numerical checks were executed.

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
history undefined result. The current empty-zero substitution requires reconciliation.

Under the
[accepted modified-Sharpe rule](04-return-risk-ratios.md#accepted-decision-observation-period-modified-sharpe),
a valid standalone CF quantile at or above zero remains reportable, while
modified Sharpe is undefined because no positive loss denominator exists.
For `Q_CF < 0`, its denominator is `-Q_CF`; the ratio may be positive, zero
or negative according to its excess-return numerator. Do not use an absolute
value to manufacture a positive denominator from a nonnegative CF quantile.
Invalid CF estimates remain explicit errors and unavailable moments remain
undefined.

The historical finite-sample formulas are accepted separately below, and
[ADR 0033](0033-value-at-risk-method-selection.md#accepted-decision-fixed-historical-tail-selection)
owns the accepted scalar/summary selection. Tail ratio follows its separately
accepted magnitude definition below; economic-support treatment remains
separately scoped. Reconciliation is owned by the
[tail-sign follow-up](../FOLLOW_UPS.md#analytics-tail-output-signs).

## Accepted decision: historical VaR and fractional empirical ES

**Accepted:** 2026-11-06. **Implementation:** Partial; selected source
inspected at `5140b499cc`. The formulas are present, but finite-sample
probability wording requires reconciliation. The illustrative arithmetic
below was checked independently; no runtime estimator tests were executed.

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

This accepts the formulas for historical return VaR/ES. Summary method
selection is accepted in ADR 0033; CDaR's drawdown sampling/estimator contract
is separately accepted in record 06. Eligible
history and small-tail behavior follow the separately accepted rule below.
Passing availability checks does not qualify statistical tail accuracy.
Reconciliation and independent
implementation qualification are owned by the
[historical-estimator follow-up](../FOLLOW_UPS.md#analytics-historical-tail-estimators).

## Accepted decision: historical availability and small tails

**Accepted:** 2026-11-07. **Implementation:** Availability behavior observed
in selected source at `5140b499cc`. Empty/nonempty handling and the absence of
a tail-count cutoff match this scope; the shared confidence contract still
requires reconciliation. No runtime estimator checks were executed.

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

Historical summary selection is accepted separately in ADR 0033; CDaR
availability is separately accepted in record 06.
Qualification and cross-host reconciliation are owned by the
[historical-availability follow-up](../FOLLOW_UPS.md#analytics-historical-tail-availability).

## Accepted decision: empirical tail ratio

**Accepted:** 2026-11-07. **Implementation:** Partial; selected source
inspected at `5140b499cc`. The formula and exact-zero/empty-history branches
are present; gain/loss interpretation and the shared confidence contract
require reconciliation. No runtime estimator checks were executed.

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
evaluation/failure contract is accepted above. Independent
implementation qualification remains open.

Allow all-positive and all-negative samples. The measure describes quantile
magnitude asymmetry: an upper-gain/lower-loss interpretation applies when the
selected quantiles straddle zero. Constant `-1%` returns and constant `+1%`
returns both give one; neither a ratio of one nor a ratio above one alone
establishes favorable absolute performance.

The magnitude convention is also used by
[Empyrical's published implementation](https://raw.githubusercontent.com/quantopian/empyrical/master/empyrical/stats.py),
which fixes the quantiles at 95% and 5%; this API requires explicit confidence
and retains its own finite-input and availability contracts. No complete
cross-library numerical parity is claimed. Computability does not establish
reliable tail estimation. Reconciliation and qualification are owned by the
[tail-ratio follow-up](../FOLLOW_UPS.md#analytics-tail-ratio).

## Algorithm and selection

The [accepted horizon convention](08-moments-gaussian-and-cornish-fisher.md#accepted-decision-observation-period-horizons)
keeps historical VaR/ES in the supplied observations' native period, without
automatic square-root scaling. Historical VaR/ES use the accepted pair above;
confidence is governed by the separately accepted input contract.

VaR copies the observations and calls the core R-7 quantile at p=1−c. For sorted x, h=(N−1)p, j=floor(h), it linearly interpolates x[j] and x[j+1]. Partial selection avoids fully sorting. A private combined VaR/ES route shares the scratch copy for summary but preserves the two distinct estimators.

Expected shortfall instead integrates exact empirical lower-tail mass m=(1−c)N. Let k=floor(m). After selecting the boundary order statistic, sum the k lowest observations with weight 1/m each and add the next observation with weight (m−k)/m. If m<1, return the single worst observation. Ties do not expand the chosen probability mass: the estimator does not average every return below an interpolated VaR threshold. This difference matters in small and tied samples.

Tail ratio is `abs(Q(c))/abs(Q(1−c))`, with positive upper-tail magnitude/zero lower-tail magnitude giving infinity; both zero gives NaN. The use of absolute values means the numerator is not necessarily a gain quantile. The current validator permits confidence below .5, conflicting with the accepted range; that behavior is not an open choice.

`Performance.value_at_risk`, `expected_shortfall` and `tail_ratio` choose these kernels directly; summary's `value_at_risk` and `expected_shortfall` always select the historical route. `parametric_var` and `cornish_fisher_var` are separate explicit methods, not fallbacks when historical data is short. CDaR calls this ES kernel on negative drawdown observations rather than returns.

## Limitations and alternatives

Resolution is limited by sample size. The accepted descriptive availability rule permits any nonempty valid sample without a hard tail-count cutoff; statistical tail-accuracy qualification remains separate. The accepted pair uses R-7 interpolation for VaR and a step empirical distribution for ES. Finite panel validation is relied upon by the ES kernel, which does not perform its own full public input validation. The related [selection ADR 0033](0033-value-at-risk-method-selection.md) owns the cross-estimator selection rationale and prior CF-error requirement; this record owns the historical algorithms.

The public VaR documentation promises non-positive outputs, but the kernel returns the signed quantile without a zero floor. Reconcile the wording with the accepted sign decision above and [ADR 0007](../conventions/0007-units-and-sign-conventions.md); do not change the estimator or discard positive estimates.

Other empirical quantiles, exceedance-only ES, weighted historical sampling, block bootstrap, filtered historical simulation and full portfolio revaluation are not selected by this API. Naming these alternatives does not approve replacement of the observed estimator.

## Implementation and validation evidence

- [Tail kernels](../../../../finstack-quant/analytics/src/risk_metrics/tail_risk.rs), [public/summary dispatch](../../../../finstack-quant/analytics/src/performance/scalar.rs), and [R-7 implementation](../../../../finstack-quant/core/src/math/stats.rs).
- Inspected [audit regression](../../../../finstack-quant/analytics/tests/audit_regressions.rs) `tail_mass_is_exact_with_ties_and_fractional_observations` specifies independent arithmetic expectations, including −.25/1.5 and worst-return tails smaller than one observation. Inline combined-versus-standalone tests are same-kernel consistency evidence. Confidence rejection tests are located in [correctness regressions](../../../../finstack-quant/analytics/tests/correctness_regressions.rs).
- [Coherent-risk reference](../../../REFERENCES.md#artzner1999CoherentRisk) is contextual. The accepted subsection above identifies R-7 and the Rockafellar–Uryasev discrete-tail convention from primary sources. Independent implementation comparisons, numerical tolerances and statistical accuracy qualification remain incomplete.

No market observations were fetched and no runtime estimators were executed.
The accepted subsection's illustrative arithmetic is independent of production
code and does not validate the implementation.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
