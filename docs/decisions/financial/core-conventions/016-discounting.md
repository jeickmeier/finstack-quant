# CORE-CONVENTIONS-016: Continuous flat discounting and valuation-date-normalized cashflow NPV

- Decision status: **Accepted — complete discounting/NPV package, 2026-11-07**.
- Implementation status: **Reconciled 2026-11-10 (G4); independent qualification pending.** `npv(disc, as_of, flows, include_today)` and `npv_amounts` (renamed from `npv_amounts_with_curve`) take an explicit valuation date and a required `include_today`; every row is validated first (Money currency on every row, finite scalars); past rows, zero amounts and excluded same-day rows are skipped without a source query; same-day cash is added at factor 1, and later cash uses `Discounting::df_between_dates(as_of, date)` checked finite and positive. Money products use 012's exact factor admission and exact-or-error addition in input order; scalar products reject non-finite or underflowing products and accumulate through the shared checked Neumaier sum (`checked_add`/`checked_total`), returning an unsigned zero; empty scalar input is zero and empty Money input errors. `flat_discount_factor` validates both inputs before its zero identities and rejects exponent or factor overflow and underflow to zero. The valuations `schedule_pv`/`schedule_pv_raw` helpers take `include_today` explicitly, and the private `schedule_trade_pv_raw` copy is removed (deposit trade NPV now uses `schedule_pv_raw(..., true)`).
- Validation status: **Evidence inspected; independent arithmetic examples passed; production qualification pending**. Documentation links/status/counts and independent relative-DF, zero/same-day, cancellation, subnormal/overflow, continuous-exponential and admitted-Decimal precision examples passed. Linked Rust/host tests were located or inspected, not run; this does not certify the current implementation.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

This record owns scalar continuous discount factors and curve-based NPV of dated
settlement cash. [Accepted 0044](../conventions/0044-financial-perspectives-and-boundary-conversions.md)
owns cash perspective, [0045](../conventions/0045-financial-clocks-and-date-boundaries.md)
owns source clocks and relative dated factors, [012](012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity)
owns Money arithmetic/conversion, and [015](015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata)
owns cash-versus-principal events. Those choices are not reopened. These helpers
do not generate entitlements, project coupons, select the economically appropriate
curve, perform FX conversion or implement a complete instrument price.

## Accepted decision: dated NPV selection, arithmetic and continuous discounting

Accepted on **2026-11-07** after whole-record review. Runtime implementation
and independent production/host qualification remain pending;
observed implementation is retained separately below.

1. **Keep two explicit dated-cash NPV paths.** Retain `npv` for homogeneous
   Money cash and rename `npv_amounts_with_curve` to `npv_amounts` for caller-unit
   binary64 cash. Both take the discounting source, explicit valuation date
   `as_of`, dated flows and a required `include_today: bool`, without global,
   Rust or host defaults. “Today” means `as_of`, never the system date. Values
   are signed economic cash under 0044, including owner-qualified projected/
   expected cash. Tuple APIs remain cash-only: full events are filtered under
   015 before conversion to dates/amounts. No new CashFlow overload, type-erased
   provider registry or parallel NPV engine is added.

2. **Make same-day selection a caller decision.** Exclude payment dates before
   `as_of`. Include dates equal to it exactly when `include_today` is true, at
   factor 1 without querying the curve; add the original Money or scalar amount
   directly. Include future payments under point 4. A date alone does not prove
   same-day cash has settled or remains entitled. Product owners choose that
   phase and supply eligible ex-coupon/settlement/default cash; this flag does
   not resolve those rules. No past-flow reinvestment, accrued-interest adjustment,
   clean/dirty-price conversion, include-past mode or automatic settlement roll.
   Remove universal “market-standard”/“today already paid” claims from helpers.

3. **Validate all supplied amounts before selecting dates or zeros.** Money
   inputs must all share the first row's currency, including past, excluded
   same-day and zero rows; mismatches error without FX conversion. Exact Money
   admission follows 012 rather than checking a float view. Scalar amounts must
   all be finite, including discarded rows; their units/currency consistency is
   caller-owned. Empty Money input errors because it supplies no currency; do
   not infer one from a curve ID. Empty scalar input returns normalized zero.
   A wrapper with an already known currency may return its explicit zero for an
   empty stream under its own contract. After validation, skip exact-zero cash
   and excluded rows. If no selected nonzero cash remains, return typed/scalar
   zero without querying any curve clock, context or factor. This does not waive
   the required argument/amount validation or authorize hidden invalid-data repair.

4. **Use the existing dated source interface for every required future factor.**
   Call `Discounting::df_between_dates(as_of,payment_date)`, honoring source date
   admission, clock, day-count context and interpolation/extrapolation policy.
   For a curve anchored at `b`, the relative factor is
   `D(b,payment_date)/D(b,as_of)`, not `D(b,payment_date)` alone or a newly reset
   curve evaluated at `tau(as_of,payment_date)`. Do not reconstruct source time
   locally, substitute instrument accrual or add an Act/365F/weekdays fallback.
   A distinct-date interval mapping to zero curve time may validly produce
   factor 1; actual date equality, not a guessed small-time epsilon, controls the
   same-day shortcut. Pre-anchor requests succeed or fail under the source's
   declared domain; curve base is not automatically the information date and
   does not authorize negative-time clamping. Caller supplies one coherent frozen
   curve/market snapshot for a valuation; no live-source revision/retry framework
   or extra cache is introduced here.

5. **Validate required factors and preserve source failures.** Every returned
   future relative factor must be finite and strictly positive, including from
   an overridden dated source method. For distinct-date resolution, every dated
   implementation, including overrides, must also honor 0045's finite-positive
   required source-endpoint factors and quotient. The canonical source boundary
   owns those checks; NPV does not duplicate the queries. Factors greater than 1 are valid;
   do not impose monotonicity or `DF<=1`. Zero-underflow, infinity, NaN, zero/
   negative factors, required-context/coverage/domain errors and source failures
   propagate without clipping, floors, alternate curves or fallback unit factors.
   Skip unused source queries only under points 2–3; a nonzero selected future
   flow still requires its dated factor even if its computed price might cancel
   another flow. No epsilon-based date or value exclusion.

6. **Apply 012 literally to Money products and sums.** Convert each required
   binary64 factor using 012's shortest-roundtrip-text exact Decimal admission,
   then use its bounded nearest-even Decimal multiplication. Add resulting Money
   products in caller input order with exact-or-error addition, without a float
   amount round trip, currency/minor-unit quantization or final default rounding.
   Nonrepresentable factors fail even if a combined product might fit: amount
   `1e28` with factor `1e-29` must error, rather than evade admission through a
   float/log product. Nonzero product underflow, magnitude overflow and inexact
   required addition follow 012's error rules. A same-day factor-1 amount needs
   no lossy conversion and contributes exactly. If admitted Decimal factors are
   `d_i` and multiplication uses computation scales `s_i`, successful Money PV
   differs from the exact sum of stored amounts times those admitted factors by
   at most `sum(0.5*10^(-s_i))`, with zero contribution from exact products; sums
   themselves add no rounding. This bound excludes source-curve error and the
   binary64-to-selected-decimal difference. Contract rounding remains an explicit
   owner-selected scale/mode, not an NPV default or universal pricing tolerance.

7. **Define scalar NPV by checked products and ordered Neumaier accumulation.**
   Multiply each selected nonzero future amount by its positive returned factor
   in binary64, then accumulate those products and included same-day amounts
   using the shared Neumaier primitive in original input order. Reject nonfinite
   products and nonzero-amount products that round to zero; nonzero subnormal
   products of either sign remain permitted. Detect nonfinite accumulation intermediates/final
   total through the shared numeric implementation rather than a private copy
   of the algorithm. Do not silently drop failed terms or switch numeric paths.
   Normalize a computed zero result. Compensation improves summation but does
   not promise exact PV, a uniform relative error near cancellation, arbitrary
   dynamic-range rescue or a portable fixed-ULP whole-NPV guarantee. A rounded
   zero is not proof the unrounded real-valued model PV is exactly zero.

8. **Preserve row order and operation domains.** Unsorted dates and duplicate
   payment dates are valid additive input rows. Do not sort, merge, net by date
   or discount a pre-aggregated total: bounded product rounding and representable
   intermediate sums can change. Money and scalar paths may differ numerically
   and may have different admission domains; no bit-identical or permutation-
   invariant result/success guarantee is selected. An intermediate overflow or
   exact-addition failure may error even when another order or an exact whole
   mathematical sum would fit. Keep deterministic input order; do not add a
   speculative row-count cap, tolerance knob or fallback precision engine.

9. **Retain one literal continuous flat formula with strict input admission.**
   `flat_discount_factor(rate,years)` means `exp(-rate*years)` for a finite
   continuously compounded annual decimal rate and finite `years>=0` already
   calculated on the caller's selected time basis. Negative rates, including
   rates below -1, are allowed; no simple/nominal compounding or percentage/bp
   interpretation is inferred. Validate both inputs before zero identities;
   valid zero rate or horizon gives exactly 1. For nonzero inputs, perform the
   binary64 exponent multiplication and exponential, requiring a finite exponent
   and positive finite factor. Reject exponent/factor overflow and factor
   underflow to zero. Positive subnormal factors are valid; do not clip to a
   floor. Tiny nonzero rate/time may legitimately round the factor to 1 and is
   not an ineffective-risk-shock error. No automatic date/day-count/rate conversion.

10. **Choose explicit numerical limits rather than an unsupported global guarantee.**
    The flat helper's operational formula and the scalar product/Neumaier algorithm
    are the selected model contracts. Standard `f64::exp` does not establish a
    portable few-ULP or platform-independent precision guarantee; exponent-product
    rounding also affects the real-exponential reference. Do not infer accuracy
    from a test that recomputes the same production expression. Independently
    qualify supported builds against high-precision exponential and summation
    references, including subnormals, extreme magnitudes and cancellation. For
    scalar references use `S=sum(a_i*returned_df_i)` and gross scale
    `B=sum(abs(a_i*returned_df_i))` in exact/high-precision arithmetic, treating
    source-returned binary64 factors as model inputs; compare absolute error on
    that gross scale with a subnormal allowance rather than uniform relative
    error in nearly zero S. Do not overflow B to infinity to make a test vacuous.
    No unproved universal epsilon/gamma bound is adopted. Financial owners select
    adequate price/ledger accuracy budgets and source-model qualification; this
    is a deliberate limitation, not permission to bypass the selected arithmetic
    or claim settlement-grade accuracy from the scalar path.

11. **Keep source-currency and financial selection responsibility explicit.**
    `Discounting` has no universally authoritative currency accessor. Homogeneous
    Money checks do not certify the economically correct discount/funding/CSA
    curve; owners select it and validate typed metadata where their source
    contract provides it. Never parse currency from CurveId or silently convert
    mixed cash. Instrument entitlement, cash generation, source information date,
    collateral regime, compounding conversions and accrued/clean-price adjustments
    remain separately owned. Source defaults are not blanket-adopted by consuming
    its dated interface. Do not build a new currency/provenance registry here.

12. **Reconcile actual Rust callers and existing host products together.**
    Propagate required `include_today`, `as_of` terminology, `npv_amounts` rename,
    cash filtering, dated-source use, identity/empty/zero behavior and numeric
    error rules through re-exports, docs, tests/examples/benches and real pricing/
    Monte Carlo callers in a working slice. Each product explicitly chooses its
    same-day/entitlement convention; do not mechanically preserve an implicit
    cutoff or supply a global default. Preserve these low-level helpers' current
    Rust-only scope; update existing host product behavior/docs and parity only
    where their actual public surface changes, without adding redundant host NPV
    APIs. Remove retired names, old manual clock mapping, zero-underflow success
    and fallback paths without aliases or migrations. Production implementation,
    independent curve/vendor reconciliation and accuracy qualification remain
    required after methodology acceptance.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Cash before valuation / on valuation / after valuation | Exclude / explicit include_today decides at factor1 / required future dated factor. |
| include_today omitted | API/admission error; no system-date/global/host default. |
| Same-day amount100, include_today true, unusable curve | PV100 with no unused curve query; same-day entitlement remains owner-selected. |
| Same-day amount100, include_today false | PV0 after input validation, without curve queries. |
| All past/zero cash or no selected nonzero cash | Known-currency Money0 or scalar0; unused curve/context is not queried. |
| Empty Money / empty scalar stream | Currency-unavailable input error / normalized0. |
| Excluded/zero USD row mixed with EUR; discarded scalar NaN | Error before date/zero filtering. |
| Nonzero future row; required source/coverage/DF failure | Preserve error; no fallback factor or discarded term. |
| Source DF0.75 at valuation, DF0.375 at payment, cash100 | Relative factor0.5 and PV50, not37.5. These example values are exactly representable in binary64. |
| Curve-clock zero over distinct dates, source returns factor1 | Valid; still use the dated interface rather than infer same-day status. |
| Pre-anchor valuation/payment | Honor source domain; no independent negative-time clamp or look-ahead claim. |
| Future factor1.02 / positive subnormal / zero/negative/nonfinite | Valid / model-valid subject to product admission / error. |
| Money1e28 times binary64 factor1e-29 | Error at 012 factor admission even though mathematical product0.1 fits; scalar path has a different domain. |
| Money1e28 plus0.01 at same-day factor1 | Exact-addition error, no dropped cent or implicit rounding. |
| Redemption/funding duplicates on one date | Discount and accumulate each row in input order; no implicit netting. |
| Scalar products 1e16,1,-1e16 | Shared compensation retains the small contribution in this example; no naive-sum substitution. |
| Scalar nonzero product rounds0, or partial sum overflows | Error even if another evaluation order/exact whole sum might fit. |
| Computed cancellation gives0 | Normalized0; not proof of exact unrounded model PV0. |
| Flat rate0 or years0 with otherwise valid inputs | Exactly1; NaN/infinity/negative horizon still rejected before identity. |
| Negative continuous rate / annual effective or bp input | Factor may exceed1 / owner must convert explicitly, no inferred convention. |
| Flat exponent finite but exp rounds0; positive subnormal exp result | Error / valid positive factor, without clipping. |
| Flat nonzero tiny exponent rounds factor1 | Valid ordinary binary64 approximation; no risk-shock ineffectiveness rule. |
| Floating PV wrapped into Money | Does not certify Decimal-exact preceding arithmetic or financial rounding. |

## Observed algorithm and conventions

Flat DF is `exp(-rate * years)`, with decimal continuous rate and finite nonnegative time. Rate may be negative; zero rate/time gives 1. Reject nonfinite rate/time, negative time and nonfinite output; underflow to zero is accepted by this flat helper.

For NPV, reject empty flows. Read curve `day_count`, `day_count_context` and `base_date`; compute signed curve time to valuation base, `t0`, and `DF0=disc.df(t0)`. Reject DF0 unless finite positive. For each flow with date **strictly after base**, compute curve time to payment using the same day-count/context and factor `DF(t)/DF0`, requiring finite positive. Sum discounted amounts. Flows on base and earlier are excluded without reinvestment or accrued interest.

Money NPV first checks every input currency, including excluded flows, against the first flow's currency; each DF is converted to Decimal and multiplied using Money checked arithmetic, then exact-as-representable Decimal addition. Amount-only NPV validates all `f64` inputs as finite, including excluded flows, then accumulates discounted values with Neumaier compensation; each product and final total must be finite. All-past input returns zero after validating inputs and DF0.

## Observed selection, controls and limitations

Callers select curve and valuation date; the curve supplies context, including Bus252 calendar. Currency is inferred from the first Money flow rather than a curve-currency check. There is no `include_today` option, cashflow-kind filter, accrued value, settlement adjustment or past-flow discounting mode. A flat DF underflow accepted by the low-level helper is rejected if supplied as a curve factor in NPV. Compensated float and Decimal paths have different precision/error behavior and need not be bit-identical.

## Observed alternatives and tradeoffs

The Money and float-amount paths are supported alternatives. A base-date-inclusive convention or discount-to-curve-origin convention would alter observed behavior. Different compounding conventions belong in supplied curves rather than this flat formula; no nominal/simple flat override is provided.

## Evidence

[Canonical dated discounting interface](../../../../finstack-quant/core/src/market_data/traits.rs),
[shared Neumaier implementation](../../../../finstack-quant/core/src/math/summation.rs)
and [core exports](../../../../finstack-quant/core/src/cashflow/mod.rs)
locate the existing implementation owners to reuse. Current NPV rebuilds curve
coordinates and calls numeric df directly; it always resolves a valuation-date
factor even when no nonzero eligible cash exists. It has a fixed strictly-future
cutoff, rejects both empty shapes and checks scalar product/final finiteness without
an explicit nonzero-product-underflow or checked-intermediate contract. Those source
choices do not establish adoption of the stronger/different package above.

[Term-loan discounting](../../../../finstack-quant/valuations/fixed-income/src/term_loan/pricing/discounting.rs),
[PE fund valuation](../../../../finstack-quant/valuations/equity/src/pe_fund/types.rs),
[structured-credit valuation](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/instrument.rs),
[asset-backed-facility valuation](../../../../finstack-quant/valuations/structured-credit/src/asset_backed_facility/pricing.rs)
and [Monte Carlo discounting](../../../../finstack-quant/models/src/monte_carlo/pricer/path_dependent.rs)
show actual product/empty-stream/flat-helper reconciliation boundaries. No direct
host exposure of these low-level functions was located; product bindings retain
their own public surfaces.

[QuantLib's dated cash-flow NPV source](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/cashflows/cashflows.cpp)
separates settlement-date flow selection and normalizes by a discount factor at
the NPV date. It is background for an explicit choice and relative normalization,
not adoption of its global settings, default dates, entitlement or ex-coupon rules.
[Rust's exponential documentation](https://doc.rust-lang.org/stable/std/primitive.f64.html#method.exp)
leaves transcendental precision unspecified across supported environments.
[Higham's floating-point arithmetic discussion](https://nhigham.com/wp-content/uploads/2021/04/high21m.pdf)
provides rounding/range background, not a proved bound for this exact NPV loop.
These sources do not qualify current Rust code or supply owner-specific financial
accuracy tolerances. The 012 Money targets remain independently binding.

[Implementation](../../../../finstack-quant/core/src/cashflow/discounting.rs). Inspected inline tests include `npv_discounts_to_valuation_date_when_base_differs_from_curve_base`, `npv_rejects_invalid_valuation_date_discount_factor`, `npv_counts_business_days_on_the_curve_calendar`, `flat_discount_factor_matches_the_closed_form`, and `npv_precision_many_cashflows`. The flat formula and valuation normalization tests provide direct expected-value evidence; curve tests also rely on the production curve builder. [Hull](../../../REFERENCES.md#hull-options-futures) and [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) support financial/numerical background.

No independent vendor valuation reconciliation was executed or inferred.

The 2026-11-07 whole-record review passed documentation links/anchors,
status/count and whitespace checks. Independent exact/rational and high-precision
examples checked the stated normalization, cash identities, cancellation/gross
scale, signed subnormal/overflow boundaries, continuous formula and conditional
Money precision domains. They qualify those examples, not production source
dispatch, unused-query behavior, accumulator checks, platform exponential accuracy
or instrument prices. No runtime source/tests/fixtures were changed and no
production Rust/host test or build pass was run.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
