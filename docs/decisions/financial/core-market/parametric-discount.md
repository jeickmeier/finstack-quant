# CORE-MARKET-07: Checked Nelson–Siegel and Svensson factor evaluation

- Decision status: **Accepted — complete NS/NSS source-evaluation package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths identified below; checked evaluation, stable basis and source/view reconciliation remain unimplemented.
- Validation status: **Primary/source evidence and independent model/numerical/consumer reviews inspected; standalone high-precision expression examples executed 2026-11-08; production qualification pending.** Existing Rust tests were inspected or located, not executed. No production evaluator, calibration or host suite was run; bounded examples do not establish a universal accuracy bound or market fit.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This decision owns the NS/NSS parameter domain, one analytic discount-factor
source, stable checked evaluation and integration with accepted
[CORE-MARKET-01 rate extraction](discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction),
[03 dated conditioning/shocks](rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning)
and [06 construction/replay](calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).
[ADR0045](../conventions/0045-financial-clocks-and-date-boundaries.md) keeps source,
contractual, model and volatility clocks distinct. This source's model horizon
is explicitly nonnegative; it does not change signed queries under the declared
tails of pillar-based sources.

[Parametric calibration](../calibration-targets/08-parametric-calibration.md)
owns parameter estimation, fit bounds/identification, objective, sampling,
initialization and market suitability. An analytic curve is not a stochastic
rate process or an automatically valid projection source for every index.
[Accepted02](forward-projection.md) retains explicit projection role selection.
This package does not adopt the calibration target's single-curve pricing
assumption, tau repairs or remaining Draft methodology.

## Foundational findings

The current nonpositive-time branches combine factor identity, short-rate limit
and zero-rate inversion inconsistently. Raw model construction/decoding can
precede validation, trait dispatch falls back to finite differences, and rolling
changes base dates by summed source-time offsets. These need the same explicit
boundaries already accepted for other factor owners.

Numerically, replacing1-exp(-x) with expm1 alone is insufficient: curvature and
coefficient cancellation can still erase a representable answer. A generic
minimum tau separation is also not an evaluation domain law; equal NSS decay
horizons are well-defined, although fitting them can be unidentifiable.

## Accepted decision: validated NS/NSS sources and analytic factor queries

Accepted on **2026-11-08** after independent model, numerical and consumer
review. Runtime/caller/host reconciliation and independent production
qualification remain pending; observed implementation evidence stays separate below.

1. **Own validated parameters and one financial evaluator.** Retain NS and NSS
   as immutable validated parameter variants, selected explicitly. NS order is
   (beta0,beta1,beta2,tau); NSS order is
   (beta0,beta1,beta2,beta3,tau1,tau2), with exact vector lengths4/6. Betas are
   finite signed annual decimal continuous-rate loadings; decay horizons are
   finite strictly positive source-clock years. Remove the NSS 1e-10 absolute
   separation rule. Equal/near-equal horizons are valid for evaluation; equal
   taus combine curvature loadings algebraically, without silently changing
   the supplied NSS variant or parameters. Fit identification constraints and
   bounds remain calibration-owned. Make raw variant state non-publicly
   constructible, with checked constructors/vector conversion and strict nested
   deserialization. Keep model data/accessors separate from computation; remove
   duplicate unchecked public model zero/forward paths. ParametricCurve owns
   the checked financial queries and a small shared private analytic kernel.

2. **Require the original source clock and a declared model domain.** Require
   explicit original base date B and complete supported day count/calendar at
   construction, factories, restore and existing host boundaries. Remove the
   implicit Act/365F and default-context mapping; require the resolved calendar
   for Bus/252 and reject absent coupon/termination context. This named model
   is defined for finite original source time t>=0 with analytic continuation
   to all positive maturities, and an explicit Error boundary before the
   original origin. Reject negative/nonfinite numeric inputs and dated queries
   before B; do not hold the factor at1 or return a fabricated historical rate.
   Validate clock coverage and mapped nonnegative coordinates before identities.
   There is no interpolation/tail selector: the positive-maturity functional
   form is the source, with no financial horizon cutoff or fitted knot grid.

3. **Define the unshocked source by its integrated forward.** Let x=t/tau,
   e(x)=exp(-x), g(x)=(1-e(x))/x and h(x)=g(x)-e(x), with analytic limits
   g(0)=1,h(0)=0. The NS continuous model yield is
   z(t)=beta0+beta1*g(x)+beta2*h(x), and
   f(t)=beta0+beta1*e(x)+beta2*x*e(x). NSS adds beta3*h(t/tau2) to z and
   beta3*(t/tau2)*e(t/tau2) to f, using tau1 for the original terms.
   Set tau1=tau for NS. Evaluate the integrated quantity directly:
   K(t)=beta0*t+beta1*tau1*(1-e(x1))
   +beta2*tau1*[1-(1+x1)*e(x1)]
   +beta3*tau2*[1-(1+x2)*e(x2)], omitting the last term for NS.
   D(t)=exp(-K(t)) and K(0)=0. Do not first round/overflow z and multiply it
   by t, or subtract rounded basis values to form the small curvature primitive.
   These formulas define one analytic family; the numerical profile below is
   an evaluation method, not a different piecewise financial model.

4. **Select a bounded coefficient-aware small-argument profile.** For each
   0<=x<=1/2 use a fixed degree16 Taylor profile. The underlying corrections are
   g-1=sum_(n=1..16) (-x)^n/(n+1)! and
   h=sum_(n=1..16) (-1)^(n+1)*n*x^n/(n+1)!.
   Combine common coefficient contributions before rounding their products:
   NS yield correction coefficients are
   (-1)^n*(beta1-n*beta2)/(n+1)!, and instantaneous-forward correction
   coefficients have denominator n!, both around the constant beta0+beta1.
   Add NSS curvature contributions on their own decay scale, sharing equal
   scales internally when applicable. Evaluate K consistently from integrated
   coefficient-weighted terms. Use checked scaled signed accumulation so a
   coefficient combination, power x^n or partial sum need not fit separately
   when the requested contribution/result is representable. Exact t=0 alone
   selects origin identities; a rounded t/tau=0 is not t=0. The unweighted
   real-arithmetic first-omitted bounds at1/2 are about1.192e-21 for g and
   2.026e-20 for h. These bound basis truncation only, not coefficient-scaled
   cancellation, total floating error or a financial accuracy guarantee.

5. **Preserve weighted range away from the small region.** Use expm1 for
   1-exp(-x), factored shared loadings and checked exponent/product scaling.
   For example NS z=beta0+(beta1+beta2)*g-beta2*e exposes cancellation that
   separate beta1*g/beta2*h products can lose. Do not require a materialized
   t/tau, exp(-x), beta*tau, powers or unweighted basis to retain a nonzero
   result before multiplying its coefficients. In particular, large positive
   x must not produce infinity*0 or erase a representable beta*exp(-x) or
   beta*tau/t contribution. Weighted truncation or rounding can dominate a
   cancelled residual; a computed zero then cannot certify a genuine model
   identity. Avoid unrequested intermediate overflow even when
   the requested K/factor/rate is finite. Required nonzero contributions lost
   through premature underflow must be reformulated; unresolved cancellation
   or requested range failure is a checked computation error, not a zero,
   long-end replacement or high-precision runtime fallback. Reuse existing
   checked scalar/summation primitives; no public special-function framework,
   tuning tolerance or all-finite-input success claim.

6. **Apply the accepted factor and rate boundaries.** Source factor queries
   return finite strictly positive D, including positive subnormals and D>1.
   Zero underflow, infinity and NaN fail; negative rates require no floor or
   monotonicity preset. D(0)=1 follows complete input/source admission. A finite
   nominal model yield alone does not make an unrepresentable discount factor
   usable. Curve zero-rate inversion requires positive source horizon and
   explicit compounding through accepted01's checked scalar conversion; at0
   it errors. The formula z is not an unchecked public alternative that bypasses
   factor admission or recovers information lost when D rounds to1. Relative
   and interval-forward queries use the same required endpoint admission,
   orientation and stable factor-log extraction as accepted01. Ordinary equal
   rounded endpoint factors can yield zero; no minimum tenor or invented rate.

7. **Use analytic instantaneous-forward dispatch.** The canonical named query
   is instantaneous_forward, replacing ambiguous forward_rate labels without
   aliases. At the original unshocked origin its analytic value is beta0+beta1,
   subject to requested finite-result range. This is a model derivative limit,
   not zero-rate inversion of D(0). For t>0 use the analytic f above with the
   same complete selected-factor admission and checked range; do not exponentiate
   an underflowed standalone basis before applying its coefficient. Discounting
   trait dispatch calls this actual owner, without finite-difference epsilon,
   clamping or different origin behavior. Retained overlays differentiate their
   complete evaluator under accepted01/03's derivative-side rules; an unshocked
   NS/NSS source has no economic interpolation knots.

8. **Retain the analytic source through dated conditioning and shocks.** Use
   accepted03's dated view over original B/model/clock and explicit reference
   v>=B, mapping both reference and maturity with the original clock. Preserve
   its selected-view, transition, absolute-factor and query range rules; do not
   accumulate pairwise clock offsets, change the model base, insert/refit knots
   or rewrite parameters to claim the same future source. Backward reference
   changes are supported only within the original modeled horizon; positive
   maturities have the model's analytic coverage subject to numerical range.
   Even with an additive clock, NSS is not generally closed under re-origining:
   its shifted second curvature term introduces an independent exp(-t/tau2)
   loading. Apply accepted03 continuous-zero overlays to the retained analytic
   evaluator when direct factor risk is selected, with the same explicit units,
   coordinates, merged adjustment and checked derivative/state behavior.
   Reconcile the current generic parametric-bump rejection through that complete
   path; do not reinterpret a generic bp request as a coefficient perturbation
   or manufacture a sampled DiscountCurve. Coefficient-estimation risk and
   future-parameter dynamics remain separately selected model/calibration work.

9. **Persist one strict model/source/view state.** Store original base, complete
   source clock/calendar, validated explicit variant/parameters, and any selected
   dated-reference/accepted03 adjustment state at its actual owner. Remove the
   old time_offset field/default and new-base numeric roll schema without
   aliases or migration. Restore validates nested fields, finite parameters,
   strict horizons and retained-view consistency; unknown/missing/retired state
   errors rather than selecting defaults or repairing taus. Rebuild only private
   caches. Source/replacement/transform mutations remain atomic and preserve
   accepted06 historical-record versus replay-qualification boundaries. The
   original model remains identifiable after an overlay; do not label adjusted
   factors as an untouched fitted six-parameter source.

10. **Reconcile real calibration, context and host consumers.** Parametric
    calibration must pass the explicit source clock and price the same selected
    analytic evaluator it delivers. Its temporary daily sampled LogLinear curve
    is not automatically equivalent for off-date queries, derivatives or future
    pricer changes; remove implicit evaluator substitution, or separately select
    and qualify any sampling adapter at the calibration owner. This does not
    approve the target's single discount/projection curve, seeds/bounds/tau
    repair or fit objective. Restore/attribution classify the source by its
    declared discount role, not the current FORWARD restore flag or type-name
    inference. Reconcile the existing calibration payload and context/prior-market
    serialization surfaces in Python/WASM, generated schema/types/docs and
    parity; no new standalone host class is required by this methodology.
    Runtime/source/trait/caller reconciliation and independent numerical/market
    qualification remain pending after adoption.

## Rationale and alternatives

One checked factor owner removes disagreement between raw model, concrete curve
and trait queries. Equal NSS horizons are an evaluation identity rather than a
fit-identification rule. Nonnegative source time is an explicit model domain;
analytic negative-time extension would add a separate modeling choice and is
not selected. Stable weighted kernels retain information the current pointwise
basis arithmetic discards, while checked failure keeps unsupported numerical
range visible.

Retaining the original model through accepted03 views avoids both day-count
non-additivity and loss of functional form during re-origining. Model evaluation,
curve fitting, single-curve projection and future-rate dynamics remain separate
financial decisions. The NS/NSS family is parsimonious; its name alone does not
establish accurate calibration to any particular benchmark set.

## Observed choice and rationale

`ParametricCurve` evaluates a user-supplied Nelson–Siegel (NS) or Nelson–Siegel–Svensson (NSS) parameterization, with analytic zero/instantaneous-forward formulas and realized-forward rolling. It does not fit the coefficients here. A parsimonious smooth term structure is the model rationale supported by the canonical references, but adoption and fit suitability remain open.

## Observed algorithm and domain

Write `g(x)=(1-exp(-x))/x`. NS continuous zero rate is `beta0 + beta1*g(t/tau) + beta2*(g(t/tau)-exp(-t/tau))`. NSS adds `beta3*(g(t/tau2)-exp(-t/tau2))`, using tau1 for the first decay. At `t<=0`, model zero returns `beta0+beta1`. NS instantaneous forward is `beta0+beta1*exp(-x)+beta2*x*exp(-x)`; NSS adds `beta3*x2*exp(-x2)`. Its nonpositive-time model branch returns the short zero rate.

Coefficients must be finite, decay horizons positive, and NSS horizons differ by at least 1e-10 in absolute value. Direct enum construction and plain model deserialization can precede validation; `from_params_vec` and curve building call it. Parameter vectors require exactly four (NS) or six (NSS) values. Negative rates/forwards are permitted; no monotonic-DF or forward-floor constraint is imposed.

The source discount function is one for nonpositive source time, otherwise `exp(-z(t)*t)`. With stored roll offset `o`, the curve returns `D(o+t)/D(o)` for `t>0`, and one for `t<=0`. Curve `zero_rate(t>0)=-ln(Dcurve(t))/t`; at nonpositive time it returns curve `forward_rate(0)`. The curve's analytic forward evaluates the model at `o+t`, but returns zero when that source time is negative. Rolling adds the configured day-count year fraction to offset and checks positive finite normalization. Act/365F is default; the `Discounting` trait has the generic finite-difference instantaneous-forward method described in [discount evaluation](discount-evaluation.md).

## Observed limitations and alternatives

The `g` calculation uses direct subtraction `1-exp(-x)` with no `expm1` or small-x series. Tiny positive `t/tau` can therefore lose its limiting value even though exactly zero has a separate branch. Existing tests cover zero and long time but do not establish tiny-positive-time accuracy. Queries also have no general finite/domain guard; extreme coefficients/times can overflow or underflow. Negative-source-time truncation is a chosen boundary convention, not a calibrated economic history.

A stable `-expm1(-x)/x`/series branch, analytic trait dispatch and explicit query-domain validation are possible changes. Supported NS versus NSS trades fewer parameters for an additional curvature component; nonparametric curves are separately supported. Context generic bumping rejects parametric curves rather than manufacturing a knot approximation.

## Observed evidence

Owning implementation and unit tests: [parametric_curve.rs](../../../../finstack-quant/core/src/market_data/term_structures/parametric_curve.rs). Tests located include `ns_zero_rate_at_zero`, `ns_long_term_rate`, `forward_rate_ns`, parameter-domain rejection, and `ns_and_nss_rolls_realize_discount_factors_and_survive_serde`; those are formula/invariant and round-trip evidence, not independent fitted-market benchmarks. Canonical references: [Nelson–Siegel](../../../REFERENCES.md#nelson-siegel-1987), [Svensson](../../../REFERENCES.md#svensson-1994).

## Caller and primary-reference evidence

| Inspected owner | Required reconciliation |
| --- | --- |
| [Parametric model/curve/state/trait](../../../../finstack-quant/core/src/market_data/term_structures/parametric_curve.rs) | One validated owner; remove raw unchecked queries, tau separation, nonpositive clamps and offset rolling; stable checked K/f evaluation and analytic trait dispatch. |
| [Curve context dispatch](../../../../finstack-quant/core/src/market_data/context/curve_storage.rs) | Dated retained-model conditioning and complete accepted03 overlay path, without sampled knot or coefficient-shock substitution. |
| [Calibration target](../../../../finstack-quant/calibration/src/targets/parametric.rs), [calibration schema](../../../../finstack-quant/calibration/src/api/schema.rs) | Explicit source context and canonical delivered evaluator; keep fit/sampling, tau repair and single-curve assumptions separately owned. |
| [Attribution restoration](../../../../finstack-quant/attribution/src/factors.rs), [context state](../../../../finstack-quant/core/src/market_data/context/state_serde.rs) | Correct declared source role and strict original-model/view state; no FORWARD classification or obsolete offset/default semantics. |

The original [Nelson–Siegel reference](../../../REFERENCES.md#nelson-siegel-1987)
and [Svensson reference](../../../REFERENCES.md#svensson-1994) identify the
functional family. The [author-hosted Svensson paper](https://larseosvensson.se/files/papers/estimating-and-interpreting-forward-rates-sweden-1992-1994-IMFwp94-114.pdf),
equations9–10 on printed pages6–7, gives the NS forward form and second NSS
curvature term. These support the formulas, not this repository's nonnegative
domain, coefficient-aware numerical profile, fit suitability or range claims.

## Independent examples and required verification

On 2026-11-08 standalone Fraction/Decimal reference calculations checked the
following bounded examples, with precision800 where severe small-x cancellation
requires it. No production source or calibration module was imported. Fixed
Taylor bounds are exact-real truncation statements, not production ULP bounds.

| Example | Reference result / implication |
| --- | --- |
| Any valid model at0 | K=0,D=1; instantaneous f=beta0+beta1. Zero-rate inversion at0 remains unidentified. |
| NSS tau1=tau2 and representable beta2+beta3 | Same real function as NS with combined curvature; the supplied NSS state remains unchanged. |
| x=1e-18, beta2=1, other betas0 | h≈5e-19, while separately rounded g and exp(-x) can both be1 and their subtraction0. |
| x=750, beta1=-1e308,beta2=1e308,beta0=0 | z=-1e308*exp(-750)≈-1.90168496348e-18. Underflowing the standalone exponential or separately combining g/h loses a representable contribution. |
| x=1e-200,beta0=-1e308,beta1=beta2=1e308 | Constant and linear terms cancel; z≈-1.66666666667e-93 and f≈-5e-93. Computing x² before coefficient scaling loses the surviving terms. |
| t=1e308,tau=1e-308,beta1=1e308,other betas0 | t/tau overflows binary64, but K≈1,D≈exp(-1) and the factor-derived continuous zero is approximately1e-308. An infinite ratio is not authority to discard the loading. |
| Degree16 at x=.5 | First-omitted basis bounds are .5^17/18!≈1.1916509223e-21 for g and17 times that≈2.0258065679e-20 for h. They do not bound weighted-rate cancellation. |
| A shifted NSS second term with shift a>0,beta3!=0 | beta3*((t+a)/tau2)*exp(-(t+a)/tau2) contains both t*exp(-t/tau2) and exp(-t/tau2). A generic six-parameter re-origin cannot discard the second component. |

Qualification must cover:

- Strict model/variant/vector/serde admission, equal/near-equal horizons, signed
  rates, explicit clocks/calendars, pre-origin refusal and origin-limit versus
  quote-inversion behavior across concrete and trait calls.
- Independent NS/NSS formulas, zero coefficients and equal-scale identities,
  small-basis/weighted cancellation, degree16 transition, subnormal ratios/powers,
  coefficient-scaled exponential tails, overflowing ratios and final-result range.
- Direct integrated K versus rounded nominal yields, factor positivity/subnormals,
  rounding to1, stable interval/zero extraction and analytic derivative coherence;
  unresolved cases error rather than fabricate zero or a tail replacement.
- Original-clock dated views on additive/non-additive clocks, reference transitions,
  complete-factor admission and overlays/derivative sides without coefficient
  refits; strict removal of old offset state and atomic failure.
- Actual calibration/pricer evaluator equality and declared roles, attribution
  restoration and existing host/context/schema/parity/error surfaces; separate
  independent financial benchmarks for the chosen fit/model assumptions.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
