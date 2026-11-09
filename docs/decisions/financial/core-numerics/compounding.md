# CORE-NUMERICS-01: Rate conversion and periodic z-spread application

- Decision status: **Accepted — complete compounding/conversion/spread package, 2026-11-07**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Evidence inspected; independent 800-digit formula/boundary references and semantic reviews passed; production qualification pending**. Known factors, zero/limiting distinctions, tiny-rate/shift and subnormal/range counterexamples, exact FMA boundary and strict text examples were checked independently. Linked Rust/host tests were located or inspected, not run; reference arithmetic does not certify current implementation or a domain-wide accuracy bound.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`core/src/math/compounding.rs`: decimal annual rates and spreads, discount factors, finite time fractions supplied by the caller, and nonzero integer periodic frequency. No schedule, day-count conversion, credit adjustment or instrument settlement is performed here.

## Choice, scope and rationale

This record owns scalar quote/discount-factor conversion, explicit limiting quote
conversion, instantaneous spread restatement and additive periodic zero-spread
application. It does not select a product's quotation basis, coupon cadence,
day-count clock, discount curve, market spread role or monetary rounding.
[007](../core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions)
already separates payment cadence from compounding;
[016](../core-conventions/016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting)
owns its narrower continuous flat-factor/NPV contract and
[018](../core-conventions/018-rate-units.md#accepted-decision-rate-and-bps-units-admission-and-precision)
owns rate/bp units. This package does not reopen those accepted decisions.

## Accepted decision: explicit compounding, conversion and periodic spreads

Accepted on **2026-11-07** after whole-record review. Runtime, caller and host
reconciliation and independent numerical qualification remain pending;
source-observed behavior is retained separately below.

1. **Keep three explicit quote conventions without a universal default.**
   Retain Continuous, Simple and Periodic(NonZeroU32), plus the existing 2/4/12
   convenience constants. Remove Compounding::Default and implicit continuous
   selection at financial constructor/restore/host boundaries requiring a quote
   convention. Periodic m is an exact whole number in 1..=u32::MAX; reject zero,
   fractional, negative, nonfinite and out-of-range host inputs without flooring,
   rounding, saturation or annual fallback. Do not infer m from coupon tenor,
   payments_per_year, currency, instrument label or a day-count denominator.
   `periods_per_year` returns m only for Periodic, None for the other two.

2. **State units and the clock boundary.** Rates/spreads are finite signed
   binary64 annual decimals: 0.01 means 1% or 100bp as an additive spread, not
   percent/bp input units. t is a finite nonnegative year fraction on one explicit
   caller-owned clock and common economic interval. Conversion changes quote
   compounding at that same t; it does not bridge different day counts or origins.
   Base DFs must be finite and strictly positive; values above 1 and positive
   subnormals are allowed. No generic currency, positive-rate or DF<=1 restriction
   is inferred. Signed past-date cash eligibility, relative dated factors and
   re-anchoring stay with their source/product owner; do not clamp negative t.

3. **Retain precise mathematical definitions and validate their domains.**
   For t>0, discount factors are exp(-r*t), (1+r/m)^(-m*t) and 1/(1+r*t).
   Periodic rates require r>-m; simple rates require 1+r*t>0; continuous rates
   have no additional economic floor. Inversion targets are -ln(D)/t,
   m*expm1((-ln(D)/t)/m), and expm1(-ln(D))/t respectively. These are real-valued
   definitions on admitted binary64 model inputs, not a promise that every valid
   mathematical result has a successful binary64 evaluation. Validate returned
   rate domains too: a periodic result rounded onto -m or simple result onto its
   zero denominator is an unavailable target quote, not a valid finite answer.

4. **Separate the zero-time DF identity from unidentified inverse rates.**
   `df_from_rate` permits t=0 and returns exactly 1 after finite-rate/time and
   periodic-domain admission; Simple's zero-time denominator is1. `rate_from_df`
   requires t>0 and errors at t=0 even for D=1, because that DF does not identify
   a rate. D!=1 at t=0 cannot represent the same-interval discount identity.
   Negative/nonfinite times and invalid inputs must not bypass validation through
   zero-rate, same-convention, zero-spread or zero-time shortcuts. Normalize signed
   zero in admitted times and returned rates/shifts; use no epsilon-zero clock.

5. **Retain zero-time conversion only as an explicit limiting definition.**
   `convert_rate` uses the supplied known rate and conventions. For t=0, define
   the t->0+ quote-conversion limit: source Continuous/Simple c=r, source Periodic
   c=m*log1p(r/m); target Continuous/Simple returns c and target Periodic returns
   m*expm1(c/m). This is not inversion of D=1 or an inferred instantaneous curve
   rate. For t>0, convert via stable log accumulation L=-ln(D), preserving the
   finite-horizon discount equivalence; Simple's finite-horizon conversion still
   depends on t. Same-convention conversion is an exact normalized identity after
   complete input/domain admission. No artificial tiny-tenor substitute is used.

6. **Evaluate the requested quantity without avoidable cancellation/range loss.**
   Reuse small internal checked conversion primitives, with log1p/expm1 near zero,
   a stable positive simple denominator (including fused multiply-add near its
   cancellation boundary), and log-accumulation conversion rather than a mandatory
   materialized DF round trip. Do not form m*t first merely to divide by it;
   stable equivalent ordering is required. This avoids known small-tenor rate
   loss and extreme unrequested DF overflow/underflow. Required nonfinite or
   nonzero-underflow intermediates must be reformulated simply or return a
   numerical error, never silently changed to 0. No new arbitrary-precision layer,
   Decimal model storage, public log-DF API or general numerical framework is
   selected. Some otherwise valid mathematical inputs can still fail the checked
   evaluation range; state that limit instead of pretending universal coverage.

7. **Make all numerical entry points fallible and outputs usable.** Change
   df_from_rate, rate_from_df, convert_rate and continuous_spread_shift to Result;
   keep the z-spread helper fallible. Invalid required input/domain errors and
   numerical calculation/range failures retain their distinct existing error
   categories. Reject NaN/infinite outputs, nonpositive DFs and DF zero-underflow;
   retain positive subnormal factors and normalize output zero rates/shifts.
   A nonzero mathematical requested rate/shift that is lost to zero errors, unless
   the exact defined result is zero by cancellation. A DF may legitimately round
   to1 or a nonzero spread to an unchanged DF at output resolution; do not invent
   an epsilon floor or demand every input change produce a changed float. Remove
   NaN/zero/unchanged-factor fallback for failed calculations, not ordinary valid
   output rounding. This is scalar model arithmetic, not 012 Money rounding.

8. **Define continuous_spread_shift as an instantaneous quote restatement.**
   For finite continuous base c and additive spread s, Continuous and local Simple
   give delta=s. Periodic gives
   delta=m*log1p((s/m)*exp(-c/m)), with a strictly positive shifted periodic base.
   This is algebraically c(r+s)-c(r); evaluate the small shift directly rather
   than subtracting two almost equal continuous rates. Validate inputs, needed
   intermediates, shifted domain, delta and resulting c+delta, including before
   zero-spread identity. Simple=s is only its local/zero-horizon meaning, not a
   finite-tenor simple zero-spread conversion. Consumers applying a finite-tenor
   Simple spread must use its actual finite-horizon rate/DF relationship. This
   helper does not approve a bond-equivalent OAS convention for every model.

9. **Keep the z-spread helper explicitly periodic and remove frequency repair.**
   `z_spread_discount_factor` takes an exact NonZeroU32 m rather than f64. For
   t>0, define r=m*(D^(-1/(m*t))-1), add z to that periodic zero rate, require
   1+(r+z)/m>0 and return (1+(r+z)/m)^(-m*t), using the shared checked/stable
   primitives. In particular, with c=-ln(D)/t and the periodic local shift delta
   from point 8, evaluate log(D_z)=ln(D)-delta*t to avoid unnecessary implied-rate
   materialization and cancellation of a small z; this is algebraically the same
   finite-horizon periodic-zero convention. z is an additive annual decimal
   increment, not a separate compound multiplier or automatically a continuous
   spread. Do not cap negative z or DF
   at1. At t=0 require D exactly 1 and return 1 after full direct-input admission;
   this does not infer a base zero rate or require z>-m. At z=0 preserve an admitted
   D directly without materializing an unused implied rate. Negative t errors.
   Check the final positive finite factor and all required shifted quantities;
   positive time must not hide range errors behind an unchanged-factor fallback.

10. **Keep equivalent real formulas distinct from accuracy guarantees.**
    Binary64 rounding means rate/DF round trips, alternative quote routes and
    a sequence of spread additions need not be bit-identical or exact inverses.
    Once an input DF has rounded to1, inversion cannot recover its original rate;
    returning0 at positive t for that admitted D is correct for the supplied
    value. Direct known-rate conversion must avoid creating that information loss
    unnecessarily. Stable transcendental functions reduce cancellation but do not
    supply a portable correct-rounding, fixed-ULP or domain-wide relative-error
    guarantee. Qualify reference error and range boundaries for supported targets;
    keep financial solver residual/bump acceptance with callers. Do not claim a
    conversion accuracy bound from self-consistency tests or library names alone.

11. **Keep strict explicit text and wire representations.** Human FromStr
    retains continuous/simple/semi_annual/quarterly/monthly and periodic(n), with
    exact lowercase spelling and no added whitespace/case/fuzzy normalization.
    n is canonical unsigned ASCII decimal in 1..=u32::MAX, without leading sign,
    zero padding, fractions or exponent. Display keeps the existing named 2/4/12
    forms and periodic(n) for other frequencies. Canonical serde remains the
    existing continuous/simple strings or single periodic object with integer m;
    human convenience names are not additional wire forms. Required missing,
    null, invalid or unknown state errors without defaults/repair. Keep these
    representations separate and validate the same frequency on every host path.

12. **Reconcile actual owners without adopting their quote or spread policy.**
    Propagate Result/type/default/parser changes through curve construction and
    zero-rate extraction, scenario shocks, z-spread/DM pricing and inversion,
    CS01, tree OAS and inflation-linked quote conversion, including hosts and
    schemas/parity/examples. A curve's zero-time zero-rate view must obtain an
    explicitly owned derivative/limit or error, not reuse the removed inverse
    sentinel; signed pre-anchor quote paths need explicit owner reconciliation.
    Remove swallowed errors, tenor/frequency inference, arbitrary tenor floors
    and solver objective sentinels rather than adding private unchecked copies.
    Source-owned dated clocks/context remain authoritative; do not recompute a
    context-free year fraction just to apply a spread. Existing explicit fixed
    model compounding is not an invitation to infer a market quote convention.

13. **Preserve distinct financial and host concepts and implementation scope.**
    Compounding conversion does not select coupon/overnight accrual, money-market
    discount-yield conventions, spread eligibility or a constant-versus-statewise
    OAS model. Product public restrictions remain authoritative until reviewed:
    a generic helper accepting Simple does not establish supported Simple OAS.
    Reuse current host curve/scenario surfaces and mapped errors rather than
    adding redundant wrapper classes. Python input/domain errors use ValueError;
    numerical computation errors use RuntimeError under the existing centralized
    mapping, with corresponding WASM error propagation. Update required arguments,
    state and documentation atomically across affected surfaces; remove obsolete
    behavior without compatibility aliases/migrations. Acceptance does not execute
    runtime changes or independently qualify any caller/pricing model.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Omitted quote convention / periodic m=0,2.5,-2 or above u32::MAX | Error; no continuous, floor, truncate or annual fallback. |
| Continuous r=0.05,t=1 | D=exp(-0.05), approximately0.951229424500714. |
| Periodic m=2,r=0.05,t=1 | D=1/(1.025^2), approximately0.951814396192742. |
| Simple r=0.05,t=0.5 | D=1/1.025, approximately0.975609756097561. |
| Valid negative continuous rate or periodic r>-m | May produce D>1; no automatic clipping. |
| Periodic r=-m / Simple exact 1+r*t<=0 | Invalid quote domain, including periodic admission before t=0 identity. |
| Simple r=-3,t=nearest binary64 to1/3 | Exact-input denominator2^-54>0; preserve with stable/FMA evaluation rather than misclassifying a rounded product as invalid input. |
| df_from_rate(NaN,0) or a negative/nonfinite t | Error, not identity/fallback. |
| df_from_rate(valid r,0) / rate_from_df(1,0) | Exactly1 / unidentified-rate error. |
| convert_rate known continuous0.05 -> Periodic(1) at t=0 | Defined limit expm1(0.05), not an inferred rate from D=1. |
| Known continuous tiny r converted at positive tiny t | Preserve via direct stable conversion or explicit range error, not a false zero caused by materialized D=1. |
| Same-convention conversion r=1000,t=1 | Exact admitted identity1000; unrequested D underflow must not cause failure. |
| Supplied D=1 at positive t | Inverted rate0; no recovery of prior lost quote information. |
| Periodic r=minimum positive subnormal,m=2,t=1e308 | Required scaling underflow must be avoided or error; no silently false unit DF. |
| Inverse result rounds onto periodic -m or simple zero denominator | Numerical/unrepresentable-target error, not an admitted quote. |
| exp underflows DF to0 / returns positive subnormal | Error / admitted if other checks pass. |
| Instantaneous periodic spread with tiny s | Evaluate the shift directly; do not lose it through subtraction of two nearly equal rates. |
| Simple finite-tenor additive z | D_z=1/(1/D+z*t); local continuous_spread_shift returning z is not that convention. |
| z=0 and valid D,t,m / t=0,D=1 and finite z | Preserve D / return 1 after direct admission. |
| t=0,D!=1 or t<0 in z helper | Error, not unchanged-factor success. |
| Nonzero z at float output resolution produces same positive D | Permitted ordinary rounding; not a failed-calculation fallback or a promise of usable risk-bump resolution. |
| Human semi_annual versus canonical periodic wire object | Same admitted m=2 via deliberately distinct representations; not wire aliases. |
| periodic(+2),periodic(02),padded/case-altered labels | Error under the selected strict human grammar. |
| Bond public OAS restriction to continuous/semiannual | Does not establish supported Simple OAS or adopt a market basis for another product. |

## Observed algorithm

1. For rate `r`, time `t`, frequency `m`, compute continuous `D=exp(-rt)`, periodic `D=exp[-mt log1p(r/m)]`, or simple `D=1/(1+rt)`.
2. Invert positive finite `D`: continuous `r=-log(D)/t`; periodic `r=m expm1[-log(D)/(mt)]`; simple `r=(1/D-1)/t`.
3. At exactly `t=0`, rate-to-DF returns 1 before validating rate; DF-to-rate validates DF then returns 0. Cross-convention conversion at zero uses instantaneous limits: periodic `c=m log1p(r/m)` and inverse `r=m expm1(c/m)`; simple and continuous share the local limit `c=r`.
4. `continuous_spread_shift(c,s)` returns `s` for continuous/simple, or `m log1p((m expm1(c/m)+s)/m)-c` for periodic. This is a zero-tenor rate conversion, not a finite-tenor simple spread conversion.
5. `z_spread_discount_factor(D,t,z,m_raw)` validates finite inputs and positive DF; for `t<=0` returns the base DF. Otherwise `m=max(m_raw,1) as u32`, guarded into `NonZeroU32`, converts the base DF to periodic yield, adds `z`, validates `1+(r+z)/m>0`, and converts back.

## Observed selection and controls

Public methods select the variant directly; named constants are semiannual/quarterly/monthly. `z_spread_discount_factor` always selects periodic discounting and normalizes its floating frequency argument by flooring through the integer cast after a minimum of one. There is no solver or market-data lookup.

## Observed conventions

`0.01` is a 1% rate or 100 bp spread. `t` must use the caller's annual day-count basis. Negative rates are permitted where the compounding denominator stays positive; the helper does not assume DF≤1.

## Observed limitations, failure and fallback

Periodic and simple invalid denominators return NaN in scalar methods; the z-spread wrapper returns validation errors. Scalar methods do not uniformly reject nonfinite time or arithmetic overflow. Negative finite `t` is mathematically evaluated by scalar converters, but z-spread returns the unmodified DF for nonpositive time. Nonintegral or negative finite z-spread frequencies are normalized rather than rejected.

## Observed alternatives and tradeoffs

All three conversions are implemented. Applying a continuously compounded additive spread directly to a periodic-quoted spread is a different economic convention; it is not an interchangeable shortcut. Rejecting nonintegral frequency changes the observed contract; the accepted complete package above selects strict integer admission.

## Evidence

- [Owning implementation](../../../../finstack-quant/core/src/math/compounding.rs),
  df_from_rate, rate_from_df, convert_rate, continuous_spread_shift and
  z_spread_discount_factor. It currently bypasses rate admission at t=0, uses an
  inverse-zero sentinel and a mandatory DF conversion round trip, and repairs
  floating z-spread frequency before returning an unchecked final factor.
- Inline known-value tests cover continuous/semiannual/simple single-cashflow
  formulas; inverse/zero-tenor cases establish source-selected identities, not
  independent numerical accuracy. Source prose claiming expm1 is 'exact' or a
  DF round trip ensures 'exact consistency' exceeds the demonstrated guarantee.
- [Curve conversion and zero-rate extraction](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/curve.rs),
  [Python curve surface](../../../../finstack-quant-py/src/bindings/core/market_data/curves/discount.rs)
  and [WASM curve surface](../../../../finstack-quant-wasm/src/api/core/market_discount.rs)
  locate zero-time bypasses, signed time views and implicit host continuous defaults.
- [Bond zero-spread discounting](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/spread_price.rs),
  [loan quote discounting](../../../../finstack-quant/valuations/fixed-income/src/loan_quotes.rs),
  [bond OAS discounting](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/discount.rs),
  [tree OAS restatement](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/bond_valuator.rs)
  and [inflation-linked quote conversion](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/types.rs)
  locate typed-frequency, context, fallback and Result propagation work. The
  finite-horizon bond helper recomputes day count with default context; ILB
  conversion uses a one-year error fallback and tiny-tenor/frequency repair.
  These observations do not adopt the product quote/clock/model convention.
- [Scenario specification](../../../../finstack-quant/scenarios/src/spec.rs)
  imports this same core Compounding enum and currently defaults RateBindingSpec's
  compounding. The [existing Python scenario wrapper](../../../../finstack-quant-py/src/bindings/scenarios/operation_spec/kinds.rs),
  [Python rate-binding constructor](../../../../finstack-quant-py/src/bindings/scenarios/operation_spec/rate_binding.rs)
  and [WASM scenario literals](../../../../finstack-quant-wasm/src/api/scenarios/kinds.rs)
  locate existing host state/default paths; a second core enum class is unnecessary.
- [CS01 zero-spread inversion](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01_z_spread.rs)
  maps every pricing error to a 1e12 objective sentinel, losing error categories
  and potentially distorting bracketing. A final repricing/residual check exists;
  this observation is not a demonstrated wrongly accepted PV.
- [Public OAS quote restrictions](../../../../finstack-quant/valuations/core/src/instruments/pricing_overrides.rs)
  admit continuous/semiannual bases only. Simple's finite-tenor distinction is a
  generic helper/caller boundary, not a demonstrated supported Simple-OAS misprice.
- [Centralized Python error mapping](../../../../finstack-quant-py/src/errors.rs)
  distinguishes validation ValueError from computation RuntimeError. Preserve
  these categories and existing host curve/scenario surfaces in reconciliation.

[QuantLib's interest-rate implementation](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/interestrate.cpp)
provides primary comparison evidence for simple/periodic/continuous definitions
and time admission, not universal product conventions or identical zero-time
policy. [Rust's log1p/expm1 documentation](https://doc.rust-lang.org/std/primitive.f64.html#method.ln_1p)
describes cancellation-aware evaluation and explicitly leaves transcendental
precision platform/version dependent. Those primary sources were consulted on
2026-11-07. Neither proves domain-wide accuracy of these accepted checked calculation contracts.
The [Hull reference](../../../REFERENCES.md#hull-options-futures) remains background,
not authority for a universal quote basis, product spread or exact binary64 inverse.

Independent reference calculations at 800 decimal digits checked the known
factors, periodic zero-spread result, finite Simple-versus-continuous spread
distinction, small direct quote/shift examples and the exact-input simple FMA
boundary. They reproduce the expression-level counterexamples: dividing the
minimum subnormal rate by 2 before multiplying a large time loses a DF effect;
an inverse Simple reciprocal can overflow although its requested rate is finite;
and a known continuous rate 1000 can convert to itself even when its DF underflows.
These checks evaluate formulas/source expressions independently, not production
Rust or an implementation of the accepted contract. Strict human grammar examples also passed.
Independent primitive/math and caller/host reviews found no material inconsistency.

Production Rust/host/model tests were inspected or located, not executed. Numerical
boundary cases and caller integration require separate qualification; acceptance
of this record establishes definitions and result/error contracts only.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
