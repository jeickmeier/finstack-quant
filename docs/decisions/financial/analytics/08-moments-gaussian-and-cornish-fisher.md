# ANALYTICS-008: Fourth moments, Gaussian and Cornish–Fisher horizon quantiles

**Decision status:** Accepted for the five scoped decisions below (2026-11-06) and the complete model-support, scaled-evaluation and exact-horizon package (2026-11-07), including its explicit refinement of the earlier finite-moment reconstruction gate.

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: model support, numerical evaluation and exact horizons

**Accepted:** 2026-11-07.

Keep the five accepted sections below: Type 2 reported moments, classical
plug-in CF, the analytic global ordering gate, additive whole-period horizons
and estimator-specific availability. This package completes the remaining
contract. **Its requested-output rule explicitly refines the earlier Gaussian
wording that requires a finite reconstructed variance.** This refinement was
accepted on 2026-11-07; the Gaussian availability section below incorporates it.

### Model quantiles and financial return support

Return finite Gaussian and globally valid CF quantiles unchanged, including
values below `-1`. Do not clip to `-1`, `-h` or empirical sample extrema,
reject solely for crossing those bounds, or switch models automatically.
These are unbounded/additive approximation outputs in return units, not
admitted observations of a nonnegative-wealth investment history. At `h > 1`
the selected quantity is an additive sum approximation, not compounded holding
performance. Even at `h = 1`, a nondegenerate
[normal approximation](https://search.r-project.org/R/refmans/stats/html/Normal.html)
has unbounded lower support; global ordering of a nonconstant CF cubic does
not supply a financial lower bound.

Keep source investment admission/terminal chronology under accepted records
[01](01-panels-windows-and-wire.md#accepted-decision-panels-windows-wire-and-summary)
and [02](02-wealth-and-annualization.md#accepted-decision-wealth-means-cagr-relative-wealth-and-cash-series).
A modeled `Q = -1` is not a terminal event, and a modeled `Q < -1` must not
be fed back as a supported investment observation or advertised as compounded
wealth performance. Document that limitation on the existing methods without
adding a support-repair mode, alternate distribution or output flag.

For constant `-75%` observations and `h = 2`, the accepted Gaussian point-mass
formula gives `-150%` additive return units. Compounded holding performance
would be `-93.75%`. This difference illustrates the selected approximation;
it is not a reason to clip or replace its computed result.

### One exact horizon-count domain across interfaces

Use canonical `Option<u64>` for `horizon_periods`, with a common supported
range `1 <= h <= 2^53 - 1` in Rust, Python and WASM. Omission remains one.
This is an exact-transport bound, not an economic accuracy limit. It matches
the [ECMAScript safe-integer bound](https://tc39.es/ecma262/multipage/numbers-and-dates.html#sec-number.max_safe_integer)
and the existing WASM checked `js_opt_uint::<u64>` capability, avoiding a
platform-dependent `usize` range or another BigInt/numeric mode.

Python supplies an integer and WASM an integral primitive number; reject
booleans, strings, fractional/nonfinite values, zero, negatives and out-of-range
counts without rounding, truncation or lossy intermediate conversion. Enforce
the same upper bound in Rust/Python, not only WASM. Validate supplied confidence
and horizon before per-ticker availability, even when every sample is empty
or too short. Convert the validated count exactly for calculation as needed.
Dates/frequency do not infer it and CF admissibility never adjusts it.
Modified Sharpe stays fixed at `h = 1`; historical tails stay native-period.

### Scaled moments and requested-output representability

Evaluate centered second/third/fourth moments, Type 2 corrections and horizon
quantiles with adequately scaled/compensated arithmetic, reusing core and
existing dependency capabilities. Request only the dependencies of the chosen
statistic: reported skewness does not acquire kurtosis's fourth-observation
minimum or require its evaluation; Gaussian does not require shape moments.
Public skewness/kurtosis remain reportable when their own conditions hold,
even if the same history cannot supply an admissible CF curve. Negative
excess kurtosis or valid zero skewness is not an availability failure.

**Accepted refinement:** require representability of requested final outputs
and explicitly required owner quantities, rather than unrequested standalone
reconstructions of mean, SD, variance or raw higher powers. Internally scaled
moment state may retain the information needed for a representable shape or
horizon quantile even when individual dimensional reconstructions overflow
or underflow. This replaces the earlier finite-variance-intermediate gate;
it does not change any estimator or sample minimum. If a
variance is itself requested elsewhere, its own owner still governs it.

A genuinely constant observed sample gives the accepted Gaussian point mass
and unavailable shape/CF estimates. Distinct observations whose differences
or squares collapse numerically must not become a point mass or missing
shape. Recover the same estimator or error. Requested nonzero statistics/
quantiles that collapse to zero are calculation failures; genuine zeros are
valid. Preserve sufficiently evaluated signs for CF validity and modified
Sharpe availability. No variance floor, data trimming, coefficient clipping
or Gaussian shape substitution is selected.

For `[0, 1, 2, 3] * scale`, at `scale = 2^600` or `2^-600`, Type 2 skewness
is `0`, excess kurtosis is `-1.2`, and sample SD is `sqrt(5/3) * scale`.
Raw centered powers need not fit to report those requested quantities.
These are independent arithmetic cases, independent arithmetic definitions.

### Stable horizon polynomial and analytic global gate

Apply the accepted analytic gate to the same polynomial actually used at
the caller's selected horizon. Preserve consequential coefficients, products
and cancellation residuals in the scaling `S_h = G1/sqrt(h)`, `K_h = G2/h`,
quadratic derivative coefficients and discriminant/minimum checks. Equivalent
positive rescaling or adequately evaluated algebra is allowed; it must not
erase a nonzero coefficient and manufacture the Gaussian identity. In
particular, identity requires genuine `S_h = K_h = 0`, not merely derivative
coefficients rounded to zero.

Keep the accepted exact nonnegative-boundary rule, including isolated flat
points. An adequately determined invalid curve is an explicit domain error;
unresolved numerical signs/boundary arithmetic are calculation errors. Do
not accept uncertain cases using an epsilon, clamp a negative discriminant,
sample a grid, rearrange the curve or switch estimators. There is no new
public tolerance or admissibility control.

Evaluate the selected cubic and final `h*mu + sqrt(h)*s*g(z)` adequately,
without forcing avoidable overflowing intermediates or cancellation through
an expanded expression. Use the canonical core normal inverse CDF at the
accepted confidence. Genuine Gaussian identity and a valid CF flat boundary
remain supported; they are not fallback branches. For `S_h = 0, K_h = 8`,
the standardized polynomial is `z^3/3`: evaluation near `z = 0` must preserve
its resolved nonzero value rather than lose it in cancellation of linear
terms. Retain the accepted finite, valid **final CF quantile** requirement
when modified Sharpe consumes it; no ratio may bypass an unrepresentable
required CF denominator.

### Availability, errors and existing result surfaces

Retain all accepted minima and genuine-zero/constant outcomes. Validate source
state and actually used request inputs first. Preserve ordinary per-ticker
undefined estimates for insufficient/constant samples without suppressing
other valid estimates. Invalid curves, unrepresentable requested outputs or
unresolved numerical failures error the affected metric call through existing
Rust/Python/WASM mappings; they do not become undefined history, an infinity,
Gaussian fallback or repaired CF output. Modeled out-of-wealth-support finite
results remain valid under the explicit support choice above.

Make affected public shape/quantile methods propagate `Result` errors while
retaining ordered output shapes. Use native numeric `NaN` only for allowed
unavailability and record 01's existing JSON/schema nonfinite conventions.
No new raw method, input-kind enum, result carrier, status hierarchy or
compatibility alias is selected. Genuine errors in requested summary moments
abort the summary under its accepted contract; historical summary selection
remains fixed under ADR 0033.

Qualification cases include short/constant versus tiny variation, separate moment
dependencies, large offsets/scales, subnormals, cancellation, flat/near-global
boundaries, selected-horizon scaling, exact horizon limits, finite out-of-support
estimates and host errors/wire outcomes. Model accuracy and i.i.d. horizon
validity remain separately unqualified.

## Accepted decision: reported skewness and excess kurtosis

**Accepted:** 2026-11-06.

Public skewness and excess kurtosis use Fisher-adjusted sample estimators,
also called Type 2. With `m_j = mean((r - mean(r))^j)`, define:

```text
g1 = m3 / m2^(3/2)
g2 = m4 / m2^2 - 3
G1 = sqrt(N * (N - 1)) * g1 / (N - 2)
G2 = (N - 1) * ((N + 1) * g2 + 6) / ((N - 2) * (N - 3))
```

| Public statistic | Required valid history | Otherwise |
| --- | --- | --- |
| Skewness `G1` | At least three observations and positive variance | Undefined. |
| Excess kurtosis `G2` | At least four observations and positive variance | Undefined. |

Both outputs are dimensionless; excess kurtosis uses zero as the normal-
distribution reference. A valid computed zero stays zero and does not prove
normality. Describe the estimators as finite-sample adjusted, not unbiased
for every distribution. Their definitions match the published e1071 Type 2
contracts for [skewness](https://search.r-project.org/CRAN/refmans/e1071/html/skewness.html)
and [excess kurtosis](https://search.r-project.org/CRAN/refmans/e1071/html/kurtosis.html),
which cite Joanes and Gill.

## Accepted decision: classical plug-in CF

**Accepted:** 2026-11-06.

Define this API as the classical CF quantile approximation. For observation-
period returns, use arithmetic sample mean `mu`, sample standard deviation `s`
with denominator `N - 1`, and the accepted Type 2 estimates as expansion
coefficients `S = G1` and `K = G2` (excess kurtosis). For probability `p`:

```text
z = Phi^-1(p)
g(z; S, K) = z + (z^2 - 1)*S/6 + (z^3 - 3*z)*K/24
             - (2*z^3 - 5*z)*S^2/36
Q_CF(p) = mu + s * g(z; S, K)
```

The [Fisher–Cornish primary expansion paper](../../../REFERENCES.md#fisherCornish1960)
provides the quantile-expansion context. The explicit cubic above and its
derivative below define this implementation, without a claim of calibrated
moment matching or a Maillard-specific algorithm.

All selected estimates must be available: require at least four valid
observations and positive variance, otherwise return an undefined estimate.
Do not invent zero shape coefficients for missing or constant histories.
Apply the accepted global-ordering and finite-estimate/error rules. Do not
calibrate, renormalize, clip coefficients, rearrange quantiles or substitute
another estimator automatically.

This directly approximates quantiles; it does not fit a distribution with
exactly the supplied variance, skewness and kurtosis. Expansion coefficients
are not generally the resulting polynomial distribution's actual moments.
Passing global ordering does not remove that approximation or establish tail
accuracy. A calibrated, moment-matching CF distribution is a different
methodology, not the meaning of this API.

## Accepted decision: global CF quantile ordering

**Accepted:** 2026-11-06.

Require the CF quantile curve to be nondecreasing over the entire probability
range `(0, 1)`, with a finite computed estimate at the requested confidence.
Check the full curve analytically rather than sampling a grid or testing one
confidence. Isolated flat points are allowed when quantile order never
reverses. Invalid curves and nonfinite computed estimates raise explicit
errors; do not clip coefficients, rearrange the curve or substitute another
estimator automatically.

For the cubic documented below, `S` is the skew coefficient and `K` is the
excess-kurtosis coefficient. Differentiating its standardized quantile map
gives the quadratic `g'(z) = a*z^2 + b*z + d`, where:

```text
a = K / 8 - S^2 / 6
b = S / 3
d = 1 - K / 8 + 5*S^2 / 36
```

Its derivative is nonnegative for every real `z` exactly when `a > 0` and
`4*a*d - b^2 >= 0`, or when `a = 0`, `b = 0` and `d > 0`. In this CF family
the latter case is the Gaussian identity `S = K = 0`, not a fallback. Use the
actual polynomial coefficients for the selected calculation. Their base-period
plug-in estimates are accepted above; horizon construction follows its accepted
rule below.

For `a > 0`, the minimum derivative is `d - b^2/(4*a)`. Equality is admissible:
`S = 0, K = 8` gives `g(z) = z^3/3`, whose isolated zero slope does not reverse
ordering. Conversely, `S = 0, K = 10` passes the 5% lower-tail local check but
has a negative derivative near the median and must be rejected. Numerical
uncertainty at the boundary must not be hidden by clamping.

## Accepted decision: observation-period horizons

**Accepted:** 2026-11-06.

Gaussian and CF VaR horizons count whole observation periods: `h = 1, 2, ...`.
Omission means one observation period. Dates and annualization frequency do not
automatically select `h`. Zero, negative, fractional and nonfinite supplied
horizons are request validation errors, distinct from unavailable ticker
estimates.

The accepted 2026-11-07 complete package fixes the common exact transport
domain to canonical `Option<u64>` with `1 <= h <= 2^53 - 1` across interfaces;
out-of-range counts and lossy conversions are request errors.

For chosen confidence `c`, base-period mean `mu`, sample SD `s` and CF plug-in
coefficients `G1, G2`, use:

```text
z = Phi^-1(1 - c)
Gaussian_Q_h = h*mu + sqrt(h)*s*z
CF_Q_h = h*mu + sqrt(h)*s*g(z; G1/sqrt(h), G2/h)
```

These are additive i.i.d. horizon approximations. Mean scales by `h`, SD by
`sqrt(h)`, the skew coefficient by `1/sqrt(h)` and excess-kurtosis coefficient
by `1/h`, based on independent-sum [cumulant scaling](https://www.stat.uchicago.edu/~pmcc/courses/stat306/2013/cumulants.pdf).
They do not calculate compounded holding-period return
`product(1 + r_t) - 1`, correct serial dependence, or convolve an exactly fitted
CF distribution. Whole-period horizons are this API's chosen aggregation
contract, not a claim that every financial model forbids fractional time.

Apply the accepted global CF gate to the coefficients at the explicitly
requested horizon. Do not change `h` automatically to make a curve admissible.
The selected horizon can legitimately change admissibility; it must be the
caller's choice. Existing CF availability and error rules still apply.

## Accepted decision: Gaussian VaR availability and failures

**Accepted:** 2026-11-06.

Validate the explicit confidence and observation-period horizon before
applying per-ticker sample-availability rules. An invalid request remains an
error even if the active sample is empty or too short. Count valid active
returns for each ticker, not price observations or another ticker's history.

| Eligible return history | Required outcome |
| --- | --- |
| Fewer than two returns | Undefined for that ticker. |
| At least two returns and positive sample variance | Use the accepted Gaussian horizon formula. |
| At least two returns and exactly zero sample variance | Use the point-mass quantile `Q_h = h * mean(r)`. |
| Invalid numerical state or unrepresentable requested quantile after adequate evaluation | Explicit calculation error, not an unavailable-history result. |

Use sample variance with denominator `N - 1`. Do not apply a cutoff or floor
that treats small positive variance as zero. The sample-SD convention and
unavailability for zero/one observation match the
[official R standard-deviation contract](https://search.r-project.org/R/refmans/stats/html/sd.html).
The zero-dispersion Gaussian is the
[point-mass limit at its mean](https://search.r-project.org/R/refmans/stats/html/Normal.html).

At `h = 1`, `[+1%]` is unavailable, `[+1%, +1%]` gives `+1%`, `[0%, 0%]`
gives zero and `[-1%, -1%]` gives `-1%`, at any valid confidence. Preserve
these signs under the accepted signed-tail contract. For larger `h`, the
constant-return result remains the accepted additive approximation, not a
compounded return.

Under the accepted 2026-11-07 refinement above, scaled internal moment state
may preserve a representable requested quantile without reconstructing an
unrequested standalone mean, variance or SD. A failed intermediate is not a
valid operand: recover adequate same-estimator evaluation or error. Unresolved
invalid/negative variance state, unrepresentable requested outputs and genuine
calculation failures must not become zero risk, undefined history or another
estimator. Two observations establish computability, not reliable tail
estimation, normality or future risklessness.

## Implementation evidence

[risk_metrics/tail_risk.rs](../../../../finstack-quant/analytics/src/risk_metrics/tail_risk.rs), [performance/scalar.rs](../../../../finstack-quant/analytics/src/performance/scalar.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
