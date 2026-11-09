# CASHFLOWS-006 — Term projection, fixing selection and rate constraints

- Decision status: **Accepted — complete term-observation and rate-control package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, scoped below.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded exact/numerical/source-selection examples and read-only projection/spec/host/consumer reviews are reported below; they do not qualify production execution, benchmark fit or product pricing models.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and domain

The observed implementation uses the forward curve's embedded-tenor quote at reset, with annualization converted to the coupon basis. That source formula is already superseded by accepted CORE-MARKET-02, below. The remaining package selects publication-qualified term observation, explicit supported controls and checked shared rate arithmetic. Projection and discounting remain separate financial roles; the observed implementation is retained separately from the accepted target.

## Accepted decision: resolved term observations, exact rate controls and strict failures

Accepted on **2026-11-08** after complete whole-record review. Runtime/caller/host/schema reconciliation and independent qualification remain pending.
Inherit accepted [market02's index-specific factor projection](../core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates),
[market12's exact observation identity and qualification boundary](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings),
[ADR0045's source/accrual/publication clocks](../conventions/0045-financial-clocks-and-date-boundaries.md),
[001's actual reset/context rules](01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context),
[002's explicit owner cutoff and canonical build path](02-program-compilation.md#accepted-decision-coupon-and-payment-program-compilation)
and [005's parent amount and exact allocation authority](05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization).
Core [012 monetary precision](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity)
and [018 rate units](../core-conventions/018-rate-units.md#accepted-decision-rate-and-bps-units-admission-and-precision) retain their scopes.
The ordinary positive-gearing term profile, projection permission for qualified
unpublished observations and bounded Decimal-rate stage below are selected
consumer decisions, not universal benchmark or product conventions. No runtime
or generated artifact changes here.

1. **Use accepted factor projection rather than a coupon proxy.** Delegate to
   market02's checked observation query. For actual index effective/maturity
   dates Sx,Ex and index fraction ax>0, the annual decimal raw index is
   I=(P_x(Sx)/P_x(Ex)-1)/ax, evaluated through the accepted stable factor/log/
   expm1 arithmetic and source clock. Remove ForwardCurve::rate(reset), the
   curve-accrual/coupon-accrual multiplier and average/embedded-tenor authority.
   Do not reproduce the factor kernel or its interpolation/tail controls here.
   A projection source is not automatically a discount source, benchmark fit or
   stochastic model; its financial role and calibration retain their owners.

2. **Require one resolved contractual term observation.** Carry configured
   IndexId/currency, actual fixing date, Sx,Ex, finite positive ax and original
   parent association through the existing compiled observation data. Require
   Ex>Sx and the selected ordinary simple-index domain: finite signed raw quote
   with positive simple gross1+I*ax, checked without avoidable intermediate
   overflow or zero-boundary rounding. Apply the same quote-domain checks to
   observed and projected values. The product/index owner resolves actual
   conventions; core/cashflows must not depend on valuations or infer them from
   a CurveId string. Pass the resolved data/information context down through
   the existing canonical compile/build request, not a new registry, callback
   framework, convention preset or second compiler. Replace the unconditional
   FloatingRateSpec.forward_curve_id input with required observed index_id
   (IndexId); resolve its selected market source association only when projection
   is needed. A fixing-only request requires no dummy CurveId or source factory.
   Fixing identity stays independent of the selected projection source.

3. **Select one term observation per original parent coupon.** Resolve its
   actual contractual benchmark tenor/period exactly. Coupon length, stub,
   payment frequency and005 balance/prefix slices do not replace the index
   period or create new resets. A contract may explicitly hold one full-tenor
   benchmark quote over a different-length parent; that is not automatic tenor
   interpolation. Multiple in-coupon resets/interpolated stubs need their actual
   owner-resolved model or error as unsupported here. Remove reset_frequency
   from the common projection spec and move genuine reset-grid cadence to its
   owning product (including the revolver). Remove optional index_tenor hints,
   numeric year-ratio/10% warnings and nearest-tenor selection. The actual tenor
   remains part of the resolved IndexId/conventions/source association.

4. **Require an explicit term versus overnight method.** Keep the existing
   FloatingLegCompounding::Simple label for this single-term-observation path;
   require it explicitly, not compounding=None or index-name inference. Explicit
   Simple works in both builder and RateSpec. Overnight variants route to007's
   actual observation/compounding owner or error in a term-only operation; this
   package does not own007's separately accepted formulas. Reject irrelevant overnight
   basis/constraint controls on a term request rather than silently ignore them.
   Do not add a second term/overnight selector or reinterpret Simple as an
   arithmetic mean of source rates across a coupon.

5. **Resolve fixing dates once from explicit reset/calendar authority.** When
   the owner uses001's actual-parent-start reset rule, require an explicit
   nonnegative business-day lag; positive lag requires its explicit fixing
   calendar. Zero lag is identity and does not need an unused calendar search;
   any provided ID is still validated strictly. No implicit T-2, payment-calendar
   or weekends-only fallback. An already resolved fixing date is not shifted
   again downstream. Actual index Sx/Ex follow the resolved index contract and
   need not equal fixing date or coupon bounds. Unknown IDs, missing required
   calendar coverage and inconsistent supplied/resolved dates error. Keep the
   actual single-reset/payment supported-domain boundary under001/core015.

6. **Require publication-qualified information, not a date-only cutoff rule.**
   Keep002's required owner as_of, and additionally use the actual operation's
   declared information phase, publication convention and selected source/version
   under0045/market12. The owning caller qualifies whether the requested fixing
   is already eligible/published or explicitly not yet eligible. Unknown or
   inconsistent qualification errors before treating absence as projection.
   No universal fixing_date<as_of, <=as_of, same-day presence or wall-clock rule.
   A past value date can remain unpublished, and a future/revised stored sample
   is not automatically eligible. Reuse existing owner request/snapshot metadata;
   add no global timestamp engine, vintage database or generic eligibility enum.

7. **Consume required published fixings exactly, without a curve dependency.**
   For a qualified published observation, resolve the configured
   FIXING:<observed IndexId> association, verify exact ID/currency/annual-decimal
   units and retrieve that exact fixing date through market12. No interpolation,
   tail carry, tolerance match or renamed history when projection CurveId changes.
   Missing series/date, wrong association, unavailable selected version or invalid
   sample errors; neither total absence nor an empty history permits a substitute.
   The annual index quote is already in its own convention and is not
   reannualized by coupon/source fractions. Do not resolve or validate an unused
   projection curve: a fully fixed coupon works without one. Still validate the
   actual supplied contract/observation/control inputs and publication authority.

8. **Project explicitly unpublished term observations through the same path.**
   This selected builder/RateSpec term operation permits projection only after
   the owner has explicitly qualified the observation as not yet eligible. Ignore
   ineligible stored future/revision data, then resolve the exact IndexId-to-
   projection-source association and query1's actual Sx/Ex/ax. Missing/inconsistent
   mapping or required source errors. Do not require fixing_date>=curve base:
   market02's signed pre-origin endpoint queries use explicit source tails and
   their own checked domain, independently of observation publication. Parent
   coupon alpha/prefix weights may be zero and do not replace positive index ax.
   A fixed-only product can reject projection at its own operation boundary;
   it must not change this shared selector's meaning or invent missing history.

9. **Remove generic failure-triggered rate substitutes.** Delete
   FloatingRateFallback, its field/constructors and Error/SpreadOnly/FixedRate
   dispatch, including whole-coupon recovery of missing curves, histories or
   calculation errors. Preserve actual source/identity/publication/calendar/
   domain/range/lock/computation causes; do not erase them with .ok(). A legal
   benchmark replacement, fixed-index election or degraded projection model
   requires the actual owner-selected contract/source and provenance, not a
   substitute chosen because any part of evaluation failed. RateSpec::Fixed
   already expresses a fixed all-in quote; it is not an automatic floating-error
   fallback. Do not retain old policies as aliases or an alternate resolver.

10. **Keep two explicit gearing laws and an ordinary-floater domain.** Retain
    exact Decimal spread_bp, gearing and optional index/all-in floor_bp/cap_bp.
    Require explicit gearing>0 and explicit gearing_includes_spread: true means
    g*(J+s), false means g*J+s, where J is the index after index bounds and
    s=spread_bp/10000. Zero/inverse gearing is outside this selected profile;
    do not silently broaden other product/overnight models. Index rates, spreads
    and bounds may be signed. None means intentionally absent bounds; equal
    bounds are valid; reject either floor>cap before evaluation. No implicit
    zero floor, g=1, spread-placement true or parser/instrument convention default.
    Validate active index/source requirements even when equal bounds make the
    final rate constant; an actual fixed contract should choose Fixed explicitly.

11. **Use one precise checked contractual-rate arithmetic authority.** Admit
    a genuine finite observed/projected binary64 raw index once through012's
    shortest-text model-to-Decimal boundary. Recheck positive simple gross using
    that admitted index and the original resolved index alpha before any index
    bound/gearing; a conversion crossing onto/past the zero-gross boundary errors.
    Preserve all contractual Decimal
    controls exactly; remove the duplicate public f64 FloatingRateParams authority
    and independent control conversions. Evaluate BP/10000, index clipping, the
    complete selected gearing/spread expression and all-in clipping exactly in
    checked transient arithmetic, then restore the final contractual Decimal
    annual rate R ONCE under012's nearest-even greatest-representable-scale
    profile. No successive J+s or gJ rounding. An uncapped intermediate need
    not fit Decimal when a mathematical final bound makes R admissible, but
    a cap cannot rescue an invalid/unadmitted raw index or malformed control.
    Nonzero final R lost to zero, final overflow or required admission failure
    errors. Reuse existing numerical dependencies/one authority, not a new
    public precision mode. Keep calculate_floating_rate as the single fallible
    contractual-rate evaluator and remove public apply_bp_bounds; reuse a private
    exact clamp stage without restoring an intermediate J to Decimal. For raw
    I=0, index floor1e-28bp and g=1e28, J=1e-32 is valid transiently and final
    R=1e-4 is representable. No unchecked NaN-masking .min/.max helpers.

12. **Unify RateSpec and builder resolution without unused coupon proxies.**
    RateSpec::Fixed stores an exact annual Decimal quote like005's fixed coupon,
    with no Money→model→Decimal detour. Replace try_rate_for_period by get_rate
    through the same canonical resolved request/observation/control authority;
    remove its independent reset/calendar/cutoff/projector implementation and
    coupon-accrual rescaling argument. Floating requests carry the needed resolved
    term/information inputs; fixed requests require no invented fixing period,
    market curve or dummy publication context. Validate the actual supplied
    request/variant and reject inappropriate mode-specific inputs. Use existing
    parameter-object/plain-value boundaries rather than new host classes or
    host-side rate logic. Return the admitted Decimal contractual quote; any
    requested binary64/Rate model view is a separate explicit qualified conversion
    and cannot replace exact controls in monetary calculations. No old helper alias.

13. **Make the two numerical stages and their limits explicit.** The raw-model
    index admission and complete bounded Decimal R restoration are006 stages;
    accepted005 then performs its separate whole-parent Money restoration using
    that R and original-parent weights. Do not convert R to f64 and re-admit it
    before the monetary kernel. Thus an exact contractual gearing increment can
    survive even when a model view would round it away. This is not exact
    arithmetic relative to an unrounded mathematical market rate. A finite raw
    index outside Decimal's admitted domain can error despite a later rescue cap;
    a nonzero R below Decimal precision can error despite a representable final
    coupon after multiplying a large B. Do not bypass those selected boundaries,
    add a high-precision fallback, defer the failure or claim a global PV accuracy
    bound. Zero all-in R from exact cancellation is valid after full validation.

14. **Keep observed/projected provenance literal and atomic.** Preserve IndexId,
    fixing/Sx/Ex/ax, selected information phase/source version, observed-versus-
    projected role, actually used source CurveId and raw index before controls,
    along with the final R/controls for005. Only an actual model projection
    populates projected raw-index metadata; an observed fixing is not renamed
    projected. Remove pre-appended unresolved placeholders and leaked partial
    values from a failed coupon. Stage metadata until the full observation/rate
    succeeds. Record016 retains deduplication/conflicts/materialization/scenario
    knowledge policy; inserting projected values never proves publication.

15. **Require explicit guarded inputs and remove convention bundles.** Remove
    generic SOFR/SONIA/EURIBOR preset constructors and hidden method/gearing/
    lag/calendar/source/fallback defaults from shared specs and bindings. The
    existing index/product owner supplies its frozen resolved convention values;
    identifiers are not executable conventions. Keep optional bounds literal,
    term inputs complete and unused overnight controls rejected in this operation.
    Apply005's canonical strict nested coupon wire and exact Decimal admission,
    plus validated construction/restoration before a complete plan can escape.
    Remove obsolete fields/default readers and duplicate parsers without aliases,
    migrations or compatibility classes.007 retains the actual overnight-specific
    observation/basis/constraint decisions and validates its own active inputs.

16. **Reconcile actual products and hosts against the same consumer contract.**
    Coordinate factor-source/source-association/exact-fixing changes with canonical
    Rust/spec/JSON, Python/WASM, schema/stub/declaration/example and parity paths.
    Move real reset-grid controls to their product owners, including revolving
    credit; reconcile term-loan, facility, structured-credit tranche, bond/LSMC,
    basis/other rate callers and calibration observation construction. Remove
    silent calendar/fixing/fallback differences and adjust consumers of the new
    get_rate/Decimal quote. Existing stochastic/product models must explicitly
    reject unsupported contexts rather than claim this deterministic projector
    supplies convexity, multi-reset or benchmark-replacement economics.005 owns
    amount/split/PIK;007/008 own overnight/replay, with no duplicated rate controls.

17. **Bound work and preserve complete results or actual errors.** Validate the
    supplied local spec, resolved observation and information/source association
    before shortcuts; query only the actually selected observation/projection
    source. Check dates, alpha, transformed result, capacities and transient exact
    arithmetic work under the accepted bounded contracts. Follow deterministic
    input-validation/work order; incidental traversal/callback completion cannot
    change valid amounts or hide failures. Use immutable request snapshots and
    stage parent results/provenance before003/005 publication. Failed build/rate
    calculation does not mutate inputs or return partial output; external market/
    cache effects are not transactionally rolled back. Preserve real causes,
    never report a wrong source or arithmetic failure as missing market data.

18. **Qualify the complete resolved observation/rate path independently.** Cover
    fixing versus effective/maturity dates, index versus source/coupon clocks,
    published/unpublished/unknown/revision cases, absent required observations,
    fixing-only/no-curve success, identity/tenor/currency errors and permitted
    signed pre-origin queries. Qualify explicit methods/calendars, both gearing
    laws, signed rates/controls, ordered/equal bounds, exact-control cancellation,
    capped wide intermediates, both numerical admission stages and truthful
    provenance. Require builder/RateSpec/host/restoration/product agreement and
    actual benchmark/contract fixtures. Bounded examples below support accepted
    definitions, not production execution, all-index suitability, calibrated
    market fit or stochastic product-price equivalence.

## Observed formula and ordered selection — not the accepted target

For curve clock `t(reset)`, curve quote `F(t)`, curve-basis accrual `αc(start,end)` and contractual accrual `αk`, project `I = F(t(reset)) × αc/αk`. Require positive-length accrual and finite positive `αk` and `αc`; reset before curve base is an error rather than short-end extrapolation. Exact historical fixings are already in the index convention and are not re-annualized.

Given decimal raw index `I`, spread `s=spread_bp×10^-4` and gearing `g`:

1. `J = min(max(I,index_floor),index_cap)` for present bounds.
2. Default `gearing_includes_spread=true`: `R0=g(J+s)`. Explicit affine mode: `R0=gJ+s`.
3. `R=min(max(R0,all_in_floor),all_in_cap)` for present bounds.

All bounds are input in basis points and converted by `10^-4`. Validation requires finite spread/bounds, finite **positive** gearing, floor no greater than cap, and nonnegative reset lag. Negative rates/spreads are allowed. `calculate_floating_rate` and `apply_bp_bounds` themselves are infallible arithmetic helpers and do not call validation; checked callers must establish their domain.

## Observed historical fixings, fallback and alternative entry point

Builder emission resolves `FIXING:{forward_curve_id}` and the curve per leg. `build_as_of` supplies the observation cutoff; ordinary `build` uses curve base date. Observation before cutoff requires an exact-date fixing; observation on cutoff prefers an exact fixing but projects if absent; future supplied observations are ignored. Fixing calendar overrides payment calendar; reset lag counts business days, with zero lag retaining accrual start.

Projection currently resolves a forward curve **before** choosing historical fixings, so even a fully fixed builder coupon requires a curve unless an explicit fallback is used. Default fallback is Error. SpreadOnly substitutes raw index zero; FixedRate substitutes the supplied decimal index. Normal constraints/spread/gearing still apply. A projection failure falls back for the whole coupon, not just the missing segment. If both curve and fixing series exist, a second check requires all historical fixings; a supplied but incomplete series cannot be silently repaired by fallback. Missing entire series/curve can reach the configured fallback. Earlier raw observations appended before a later failure can remain in metadata; unresolved observations retain `value=None` for [CASHFLOWS-016](16-fixing-materialization.md).

`RateSpec::try_rate_for_period`, used for facility/tranche-style term coupons, is a materially different path:

- Fixed rate only requires finiteness; the supplied period inputs are otherwise unused.
- Floating rejects **any** explicit compounding value, including `Some(Simple)`; unset compounding is required.
- Fixing calendar defaults to weekends-only, not a supplied payment calendar.
- Reset **on or before** `as_of` requires an exact fixing; same-day projection is disallowed.
- Future projection calls the same rate kernel. Configured FloatingRateFallback is not consumed by this method.

## Observed controls and limitations

Floating spec defaults: gearing one, spread-inclusive gearing, reset lag two, daily overnight index bounds, no bounds, no explicit compounding and Error fallback. The SOFR/SONIA presets choose compounded-in-arrears with zero lookback, zero reset lag and ACT/360/ACT/365F index basis respectively. EURIBOR 3M is term, 3M tenor, two-business-day lag and TARGET2 calendar.

`reset_frequency` does not create a separate fixing grid in this builder: term coupons reset once per generated coupon period. `index_tenor` is checked only for a warning when it differs from resolved curve tenor by more than 10%; projection uses the curve's embedded tenor. No interpolation between index tenors or multiple-reset coupon aggregation is implemented here. The separate overnight engine is [CASHFLOWS-007](07-overnight-compounding.md).

## Alternatives and ownership limits

The accepted package retains two explicit gearing laws but removes generic failure-driven
substitutes, calendar/method inference and an alternate fixing policy. It inherits
factor-based index projection rather than the observed reset-point quote/basis
conversion. One term observation held over an explicitly selected parent remains
distinct from interpolated-stub, multi-reset, overnight, convexity and legal
benchmark-replacement contracts. No new selectable source or fallback modes.

## Independent reference examples

Read-only source/spec/host/consumer reviews and bounded exact-rational/numerical/
source-selection examples on2026-11-08 checked these accepted targets without
production helpers:

- P(Sx)/P(Ex)=1.0125 and ax=90/360 imply annual index5%, observed or projected.
  Coupon alpha90/365 does not change that quote: principal100 has mathematical
  coupon100×.05×90/365≈1.2328767123, not1.25. Actual005 prefix/model/Decimal
  stages retain their stated accuracy limits; source/coupon alpha are separate.
- Index3%, spread100bp, gearing2 gives8% with spread-inclusive gearing and7%
  in affine mode. Index floor/cap acts first and all-in bounds last; equal
  bounds are valid. Zero/negative gearing, crossed bounds and nonfinite inputs
  error in the selected ordinary-floater profile, even if a final cap is finite.
- Exact gearing1+1e-28 with admitted I=1, spread0 and B=1e28/parent weight1
  yields rate1+1e-28 and coupon1e28+1; converting gearing to f64 first loses1.
  Exact spread_bp=10000+1e-24 with I=-1 and g=1 yields R=1e-28, not0.
  Raw binary64 I=1.00000000000001 admits that same shortest Decimal text;
  exact g=0.99999999999999 and affine spread=-10000bp give R=-1e-28,
  rather than the false0 from separately rounded binary64 multiplication.
- I=g=1e20 with all-in cap10000bp yields R=1 despite uncapped1e40. Raw index
  admission precedes bounds and cannot be repaired by that cap. An exact
  R=1e-32 underflows at the selected rate stage even if B=1e28 could produce
  representable Money1e-4; no hidden bypass of that chosen precision boundary.
  Conversely, raw I=0,index floor1e-28bp,g=1e28 gives valid R=1e-4: only the
  internal J=1e-32 is below Decimal precision, and no intermediate restoration
  is requested or permitted to cause a false failure.
  With binary64 raw I=-0.7 and binary64 ax=10/7, exact binary-input gross is
  positive, but shortest-Decimal I=-0.7 crosses the gross boundary against that
  same ax. The post-admission domain check errors before any rescuing bound.
- An eligible exact5% fixing works without any projection curve and cannot be
  replaced by a4% substitute. Required missing published data errors; explicitly
  unpublished data projects under its owner-qualified request, ignoring an
  ineligible stored sample. Unknown eligibility and wrong series/source never
  become permission to project. Projection CurveId changes do not rename history.

These are small admitted-number and policy examples, not executions of production
sources, calendars, schemas, snapshots or product valuations. The
[CME Term SOFR methodology v1.5.3](https://www.cmegroup.com/content/dam/cmegroup/market-data/files/cme-term-sofr-reference-rates-benchmark-methodology.pdf),
§2, separates publication from the tenor's effective date. The
[New York Fed's publication/revision policy](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates)
separates initial publication and eligible same-day revisions. These primary
examples support explicit index dates/information phases, not a universal reset
lag, hour, source version, projection policy or this consumer's Decimal/gearing
profile. No vendor tolerance or benchmark/model conformity claim is made.

## Evidence

- [rate_helpers.rs](../../../../finstack-quant/cashflows/src/builder/rate_helpers.rs), [rate specs/defaults](../../../../finstack-quant/cashflows/src/builder/specs/coupon.rs), [RateSpec](../../../../finstack-quant/cashflows/src/builder/specs/rate.rs), [emission selection/fallback](../../../../finstack-quant/cashflows/src/builder/emission/coupons.rs), [curve-tenor warning](../../../../finstack-quant/cashflows/src/builder/pipeline.rs).
- Located formula/regression tests: `term_projection_uses_the_reset_date_fixing_not_the_period_average`, `term_projection_converts_curve_basis_before_coupon_constraints` in rate_helpers; [floating_rate.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/floating_rate.rs) covers fallback modes, reset-date fixings and same-day projection. Emission unit test `overnight_fixing_policy_uses_valuation_cutoff_not_curve_base` was inspected; it explicitly expects future observations to be ignored and missing same-day fixing to project.
- [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions) is the repository convention pointer. No independent vendor tolerance study or contractual adoption rationale for every fallback/entry-point difference was located.
- Actual API/consumer inspection includes [revolver reset-grid use](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/utils.rs), [structured-credit rate calls](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/waterfall.rs), [Python floating specs](../../../../finstack-quant-py/src/bindings/cashflows/builder/specs/coupon.rs) and [WASM spec operations](../../../../finstack-quant-wasm/src/api/cashflows/specs.rs).
- Production tests/builds/generators were not run. The complete package is accepted; factor-source, exact rate arithmetic, resolution/host/schema/caller reconciliation and independent qualification are implementation work, not established by the independent bounded checks.

## Relationship to the accepted exact fixing boundary

[CORE-MARKET-12's complete accepted package](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings) resolves observed IndexId
independently of projection CurveId and verifies exact configured series
association before lookup. Exact retrieval supplies no publication authority;
qualify the actual information phase and selected source/version before using
a fixing, without presence-only same-day or swallowed-error projection. Reconcile
both builder and RateSpec paths and raw fixing metadata together. Accepted02's
projection contract is unchanged. Record12 was accepted on **2026-11-08**,
with reconciliation/qualification pending; this record's remaining cutoff/
fallback/coupon choices were accepted in this record's complete package on
2026-11-08. The inherited market contracts are not reopened.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
