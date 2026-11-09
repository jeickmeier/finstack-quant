# CORE-MARKET-08: Explicit quoted-level sources and price-parameter risk

- Decision status: **Accepted — complete quoted-level/source/shock package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths identified below; anchor removal, checked source queries and parameter/view reconciliation remain unimplemented.
- Validation status: **Primary/source evidence and independent source/consumer/shock reviews inspected; standalone rational/high-precision counterexamples executed 2026-11-08; production qualification pending.** Existing Rust/host tests were inspected or located, not executed. No production pricing/calibration suite was run; expression checks do not qualify commodity/VIX models or a universal accuracy bound.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This decision owns signed price/nonnegative volatility-index level sources,
explicit interpolation/clock/state, source-parameter shocks and frozen dated
reuse. It preserves accepted [scalar interpolation](../core-numerics/value-interpolation.md),
[ADR0045 clocks](../conventions/0045-financial-clocks-and-date-boundaries.md)
and [record03 bucket geometry](rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning).
Price-parameter risk is its own financial operation, distinct from record03's
continuous-zero factor shocks and [record05 hazard parameters](hazard-quotes-and-risk.md).
[Record06](calibration-provenance.md) retains historical provenance versus
current replay qualification wherever construction records exist.

Products/convention resolution own asset/index, currency/physical unit,
futures-versus-forward quotation, delivery/fixing/settlement dates and averaging.
[Commodity scenarios](../scenarios/08-commodity-and-vol-index.md),
[commodity delivery](../valuations-fx-and-exotics/commodity-delivery-listed.md)
and [vol-index futures](../valuations-equity-and-composition/05-vol-index-and-listed-options.md)
retain their remaining Draft models/lifecycle choices. This package does not
infer cost of carry, convenience yield, a pricing expectation measure, futures/
forward equivalence, VIX replication or a convexity adjustment.

## Foundational findings

A quoted level source has no generic normalization at0. The current mandatory
spot field and automatic origin insertion can invent a front segment and mix
observed spot with a futures/forward contract quote. A future-only level source
with Error tails is valid even when it provides no level at its base date.

Current PriceCurve shocks rebuild source pillars. Even a parallel additive
shift is not generally a literal shift of the whole evaluated function: for
LogLinear levels1 and4, adding1 to both pillars changes the midpoint from2 to
sqrt(10), rather than3. This proposal names that operation as parameter risk
instead of promising a continuous shock shape or pinned rolled spot.

Finally, frozen dated reuse must retain the original source clock. Accumulating
pairwise offsets, inserting an origin and discarding expired source points can
change interpolation or fail despite admitted tails, especially under
non-additive day counts.

## Accepted decision: explicit level pillars and parameter shocks

Accepted on **2026-11-08** after independent source, consumer and shock
review. Runtime/caller/host reconciliation and independent production
qualification remain pending; observed implementation evidence stays separate below.

1. **Retain one explicitly typed native-level source.** Keep PriceCurve with
   required explicit PriceCurveKind::Price or VolIndex. Price admits finite
   signed/zero native price levels; VolIndex admits finite nonnegative index
   levels. Product/market resolution must bind the source ID to one homogeneous
   asset/index, quotation kind, currency/physical unit or index points, and
   date/settlement interpretation, and validate agreement at consumption. Do not
   infer these from an ID or mix unlike quotes because their numbers fit the
   same scalar type. Reuse existing contractual/convention ownership without
   moving product or exchange logic into core. A VolIndex level20 is20 index
   points, not decimal volatility.20, variance.04 or a volatility-surface quote.
   Remove unsupported universal expectation/convexity-size claims from docs.

2. **Use explicit pillars without a spot anchor.** Require at least two finite
   strictly ordered distinct nonnegative source times and finite admitted
   levels. Do not sort, merge, snap, impose epsilon spacing or insert a point.
   Exact0 is an optional caller-supplied source coordinate, not a required
   observation or spot identity. Remove spot_price/spot_level fields, builders,
   accessors and implicit first-knot inference, including 1e-14 near-zero repair.
   Future-only sources are valid. Source price(0) follows the same explicit
   interpolation/tail coverage as any other query and is never automatically
   an observed spot or final settlement. Observed spot/fixings stay at their
   actual market/product owner. An explicitly supplied origin pillar remains
   an ordinary parameter, including under shocks.

3. **Require source clock, interpolation and literal tails.** Require original
   base B, complete supported source day count/calendar, style and Flat/Extend/
   Error tails in generic construction/restore/host inputs. Remove Price-kind,
   Act/365F, Linear and tail defaults. Require a resolved Bus/252 calendar and
   reject clocks needing absent coupon/termination context. Keep Linear and
   accepted CubicHermite/PCHIP for signed source levels; LogLinear requires all
   retained source ordinates strictly positive. Reject both MonotoneConvex and
   PiecewiseQuadraticForward, which reconstruct factors rather than native
   scalar levels. Flat holds a boundary level; Extend uses the selected scalar
   working-space continuation; Error rejects outside the supplied grid. No
   volatility-time, carry, positivity-floor or financial tail model is inferred.

4. **Make the whole query boundary checked and consistent.** Numeric queries
   require finite t>=0. Dated queries require date>=B and map once with the
   original stored context, without negative-time/zero clamps or replacement
   by coupon/model time. Validate coverage/domain before exact-knot shortcuts;
   return exact stored untransformed levels at exact knots, with no approximate
   matching. Distinct dates can map to one admitted source coordinate. All
   requested results must be finite and kind-admitted, including after Extend;
   VolIndex extrapolation below0 errors without clipping. Preserve genuine zero
   levels and signed prices, but do not turn an unresolved computation or
   required nonzero result lost to0 into a fabricated zero level. No NaN,
   endpoint fallback, hidden style change or apparent successful wrapper.

5. **Keep factories and quotation conversions explicit.** A named constant
   factory may use the declared two-pillar recipe (0,p),(1,p), Linear+Flat,
   with explicit kind/base/clock and admitted p. This is a constant source
   assumption, not a recovered spot observation. An owner wishing to anchor
   another curve explicitly supplies its own (0,level) pillar. A delivery-month
   future, daily fixing forward, forward contract, spot observation and official
   settlement are different data. Their mapping/averaging/conversion belongs to
   the contract/calibration owner, not point interpolation. In particular do not
   label a square root of expected variance as a quoted volatility-index future
   or silently apply a universal convexity correction.

6. **Name direct shocks as canonical level-parameter changes.** Reuse the
   existing bump carrier with explicit supported meanings: Additive/Fraction
   carries an unscaled native-unit absolute change b; Multiplicative/Percent
   carries relative percent p; Multiplicative/Factor carries target multiplier k.
   These apply to parallel or exact-width triangular parameter shapes. Reject
   RateBp, Additive/Percent and other unsupported mode/unit combinations rather
   than overloading additive percent or a scenario bp field. At original source
   pillar t_i with weight w_i, absolute changes give q_i'=q_i+b*w_i; percent
   changes give q_i'=q_i*[1+(p/100)*w_i]; factors give
   q_i'=q_i*[1+(k-1)*w_i]. Parallel has w_i=1. State native units explicitly:
   +2 means2 USD/barrel on such a Price source or2 index points on VolIndex.
   Relative +10% at price-10 gives-11; it is algebraic scaling, not an absolute
   price rise or a volatility percentage-point change.

7. **Preserve the chosen interpolation under parameter risk.** Rebuild a new
   canonical source from candidate pillars with the same kind/base/clock/style/
   tails and association. Preserve the source ID unless a caller explicitly
   requests a replacement ID; remove implicit bp-labelled price-shock IDs.
   Weights use original source-clock coordinates, even
   after a horizon reference changes; dated bucket requests map actual dates
   once on that clock. Exact-node targets name existing source parameters;
   reject off-grid, missing, duplicate or nonfinite targets without nearest-knot
   or containing-segment fallback. A separate scenario solve for an off-pillar
   level is another financial operation owned by its existing record. Do not
   insert a rolled-origin pillar, re-anchor a weight at the new valuation date,
   pin a source-origin/reference level or shock observed spot implicitly. The
   reinterpolated function need not equal F(t)+b*w(t) or F(t)*(1+r*w(t)) between
   pillars, even for a parallel absolute shift under LogLinear. No second
   continuous-overlay shock mode is introduced here.

8. **Validate relative domains, range and identities atomically.** Price permits
   finite signed multipliers, including explicit sign-reversing stresses.
   VolIndex requires p>=-100 or k>=0 for relative requests; zero scaling is
   eligible but still must satisfy the selected interpolation's source domain.
   Thus LogLinear rejects zero/negative candidate pillars rather than switching
   style. Validate all units, shapes/targets, normalized magnitudes and candidate
   values before publishing; nonzero normalization lost to0 and required
   nonzero products lost to0 are computation errors, distinct from explicitly
   requested zero scaling, genuine zero prices or a mathematically zero effective
   node multiplier (for example Price k=-1,w=.5). Use stable requested-output
   arithmetic without demanding unneeded overflowing intermediates. Factor
   weights preserve w=0 -> multiplier1 and w=1 -> multiplier k exactly; use
   endpoint branches and stable affine/scaled composition rather than first
   rounding k-1. For example k=1e-300,q=1e300,w=1 must retain a candidate
   near1, not0 from 1+(rounded(k-1)). Percent updates likewise preserve the
   requested level rather than unnecessarily materializing a rounded multiplier. Exact
   validated direct no-ops preserve source/metadata; failed changes leave the
   source/context unchanged. Finite checks at pillars do not prove admissibility
   of all extended queries. Where construction provenance exists, actual
   parameter changes retain history without automatic current-fit authority.

9. **Qualify bucket composition and finite sensitivities.** Reuse accepted03's
   one exact-width geometry without the generic 1e-10 floor. A coherent ordered
   full bucket set partitions unity at each source parameter. Absolute bucket
   additions can compose to a parallel parameter addition in real arithmetic.
   Relative bucket shocks have multiplicative cross terms: independently
   composing them does not generally equal one parallel relative shock.
   First-order parameter directions add; separately repriced finite PV changes
   need not. Publish the parameter/risk basis and qualify effective steps and
   residuals at the sensitivity/scenario owner. No exact finite bucket-sum,
   no-arbitrage, carry or universal continuous-shape claim is selected.

10. **Freeze levels on the original dated evaluator through horizon changes.**
    For a new valuation reference v within the original modeled date domain,
    reuse the original source and F_frozen(T)=I(tau_B(T)). This is an absolute
    level, without factor normalization or a new spot. Preserve B/clock/context,
    all original pillars, interpolation and tails. A context/product/scenario
    owns the new valuation reference; the level source need not change merely
    because that reference advances. Any dated-view adapter keeps source_base
    and reference_date distinct and does not expose fresh DayCount(v,T) scalar
    coordinates. Validate reference mapping and clock/calendar coverage, but do not require
    I(tau_B(v)) unless its level is actually requested: a future-only Error
    source can lack a reference-date level while valid future queries exist.
    Reference is not itself a fixing cutoff. Queries retain original date
    domain/tail admission; observed historical use stays product-owned. Permit
    forward and backward reference changes within the original domain and
    references beyond reporting knots without discarding source points or
    refitting. Error tails still reject unsupported level queries. Remove the
    price-specific numeric-offset/expired-grid/backward-rebuild roll path.
    Fixed-source parameter changes and reference relabeling commute in real
    arithmetic because the original parameter coordinates do not move. Future
    price dynamics, carry evolution, settlement and full context aging remain
    separate selected operations, not consequences of this frozen calculation.

11. **Reconcile actual products and scenario units.** Commodity forward delivery
    must resolve actual observed/settlement data; removed spot_price cannot be
    replaced by price(0). A missing/type/computation error must not silently
    choose a spot/discount carry model. Commodity future/Asian/swap observation
    aggregation remains contractual, including known/projected splits and
    delivery windows. VolIndexFuture must use the checked source dated query
    for settlement maturity, keeping model forward levels, live marks and SOQ
    final settlement separate. Positive lognormal-forward/model restrictions
    belong to the option/product, not the signed core Price domain. Reconcile
    scenario VolIndex zero rejection with the source's nonnegative domain;
    stricter contract requirements remain product-owned. Replace Commodity
    CurveParallelBp-as-percent with explicit native/relative-price operations.
    Preserve typed Price/VolIndex lookup and explicit unit/quotation associations.

12. **Publish one strict source/state/host contract.** Persist explicit kind as
    a discriminator, original base/complete clock, ordered level pillars, style/
    tails and authoritative association at its owner. Remove spot-field kind
    inference and old price roll_transform state without aliases or migration.
    Reporting/reference metadata does not replace the retained source. Restore
    rebuilds private caches and validates raw source data without repair or
    supplied-cache truth. Reconcile Rust/query/error/transform/context dispatch,
    source constructors, product/scenario adapters, Python/WASM options/getters,
    generated schema/declarations/docs and parity in coherent slices. Numeric
    wrappers must propagate checked errors. Runtime/caller/host reconciliation
    and independent numerical/financial qualification remain pending after adoption.

## Rationale and alternatives

Removing automatic spot anchoring gives a level source only the data/assumptions
its owner actually supplied. It also removes the need to repair an interpolated
rolled origin just to preserve a duplicate spot field. Explicit parameter risk
uses the same accepted scalar interpolation owner; its off-pillar effects are
part of that selected representation rather than an invented continuous shock.

A carry-derived curve, delivery-period calibration, literal continuous function
shock or variance-native volatility-index model is another financial operation.
None is silently substituted when lookup, interpolation or quote coverage fails.
Frozen dated reuse needs neither a financial normalization nor a new generic
roll framework; retaining the original evaluator is sufficient.

## Observed choice and rationale

One `PriceCurve` representation supports signed native-unit prices and, via `PriceCurveKind::VolIndex`, nonnegative volatility-index levels. It interpolates supplied levels without deriving cost-of-carry, convenience yield, or a volatility-index replication model. For VolIndex, the documented input is the quoted futures/forward level; a square root of expected variance is not automatically the same quantity, and this curve applies no convexity adjustment. Type-specific validation and unit-specific shocks are financially material choices.

## Observed algorithm, selection, and conventions

Builder defaults are Price, Act/365F, Linear and FlatZero. A base date and at least two supplied knots are required before origin insertion. Knots must be ordered and values finite. Spot must be supplied unless the first knot is within 1e-14 of zero. The builder inserts an exact `(0,spot)` anchor when absent and rejects an exact-zero knot disagreeing with spot. Negative price knots and spot are allowed for Price; VolIndex rejects negative values. MonotoneConvex is rejected; positive-domain styles reject zero/negative source levels despite the broader Price domain.

For `t<=0`, `price(t)` returns spot; positive time uses the source interpolator, or the original interpolator evaluated at a retained roll offset. Date queries before base explicitly error. FlatZero holds boundary levels; FlatForward extends the selected kernel's endpoint slope and may leave the economic domain. A VolIndex constructor validates knots, not all extrapolated/interpolated queries.

Parallel additive Fraction bumps add native price/level units. Additive Percent and multiplicative Percent both scale by `1+value/100`; multiplicative Factor scales by its value. RateBp is rejected. Triangular shocks accept additive Fraction (absolute local price shift) or additive Percent (local relative scaling) and leave `t<=0` spot unchanged. Other key-rate combinations are rejected. These percentage semantics differ from unitless scalar percentage-point shocks.

Rolling by signed days changes the clock, keeps strictly future shifted knots, sets new spot to the old curve at the offset, and inserts a zero anchor. It errors if all future knots expire. Forward rolls retain source interpolation. Shared `InterpRollState` drops the retained source when cumulative offset becomes negative and rebuilds from available rolled pillars; this is an explicit backward-roll approximation. Shocking a retained source maps original knots in current-relative time, then checks the rolled-origin level. If interpolation moved that origin by more than 1e-12 scaled tolerance, an origin source knot is inserted and the shock rebuilt. Generic replacement knots deliberately remove retained roll state.

## Observed limitations and alternatives

A price shock is applied to stored/source knots and reinterpolated; its continuous shape need not equal the requested triangular weight between knots for nonlinear interpolation. Signed percentage shocks can reverse sign with factors below zero for Price, while VolIndex revalidation rejects resulting negative knots. There is no universal query-level positivity check or no-arbitrage carry constraint.

Supported interpolation styles trade shape and positivity; explicit no-extrapolation can yield NaN. Cost-of-carry construction or variance-native volatility-index modeling belongs to other layers and is not silently inferred from these quotes.

## Observed evidence

Owning code and inline tests: [price_curve.rs](../../../../finstack-quant/core/src/market_data/term_structures/price_curve.rs), [shared roll machinery](../../../../finstack-quant/core/src/market_data/term_structures/common/roll.rs). Located tests cover signed prices, local percentage shocks, spot preservation, VolIndex rejection, retained-source serde/bumps, and backward rebuilding. [Scalar-curve invariants](../../../../finstack-quant/core/tests/market_data/curves/scalar_curves_invariants.rs) and [lifecycle tests](../../../../finstack-quant/core/tests/market_data/lifecycle.rs) provide algebraic/round-trip checks, not independent commodity or VIX price validation. Reference context: [Hull](../../../REFERENCES.md#hull-options-futures), [CBOE VIX](../../../REFERENCES.md#cboe-vix-white-paper); storing a VolIndex level does not implement the latter replication formula.

## Caller and primary-reference evidence

| Inspected owner | Required reconciliation |
| --- | --- |
| [Price source](../../../../finstack-quant/core/src/market_data/term_structures/price_curve.rs), [shared roll](../../../../finstack-quant/core/src/market_data/term_structures/common/roll.rs) | Remove duplicate spot/origin repair and price offset roll; strict explicit scalar source/query/state and parameter-risk semantics. Other curve-family roll choices remain separately owned. |
| [Typed getters](../../../../finstack-quant/core/src/market_data/context/getters.rs), [prior-market construction](../../../../finstack-quant/calibration/src/api/context_builder.rs) | Preserve kind checks and enforce explicit compatible unit/quotation association rather than ID inference. |
| [Commodity forward](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_forward/types.rs), [future](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_future/types.rs), [averaging](../../../../finstack-quant/valuations/exotics/src/commodity/averaging.rs) | Separate delivered observations, forward source selection, explicit carry and contractual aggregation; no catch-all lookup fallback or price(0)-as-fixing. |
| [Commodity Asian](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/pricer.rs), [spot option](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/types.rs) | Preserve dated source versus product/vol clocks and separately selected lognormal positivity/model domain. |
| [VolIndex future](../../../../finstack-quant/valuations/equity/src/vol_index_future/types.rs), [scenario adapters](../../../../finstack-quant/scenarios/src/adapters/curves.rs), [scenario spec](../../../../finstack-quant/scenarios/src/spec.rs) | Canonical dated queries, model/mark/SOQ distinction, source zero domain and explicit native/relative risk units without bp overloading. |
| [Python](../../../../finstack-quant-py/src/bindings/core/market_data/curves/price.rs), [WASM](../../../../finstack-quant-wasm/src/api/core/market_curves.rs) | Mandatory choices, retired spot/roll state and kind encoding, checked numeric/date errors, matching schema/docs/parity. |

[Cboe VX specifications](https://www.cboe.com/tradable_products/vix/vix_futures/specifications)
identify index-point quotation and official SOQ settlement. Its
[FAQ](https://www.cboe.com/tradable-products/vix/faqs/) distinguishes spot VIX and
settlement questions. [EIA's account of negative WTI futures](https://www.eia.gov/TodayinEnergy/detail.php?id=43495)
supports admitting signed commodity quotes. These establish concrete quote/unit
examples, not universal futures/forward/expectation equivalence or this source's
interpolation, shock, carry and risk qualification.

## Independent examples and required verification

On 2026-11-08 standalone Fraction arithmetic and Decimal at precision200 checked
these bounded relationships. No production source, pricing or host module was
imported and no production suite was run.

| Example | Reference result / implication |
| --- | --- |
| Future-only Linear pillars (1,100),(2,120) | price(0) is100 under Flat,80 under Extend and unavailable under Error; none is automatically observed spot. Supplying (0,90) explicitly gives95 at.5 instead of Flat's100. |
| Linear signed pillars (0,-10),(1,10) | Exact midpoint is0. Signed prices and genuine zero need no clipping or positive-factor assumption. |
| LogLinear pillars (0,1),(1,4), then absolute+1 at both parameters | Base midpoint2; shocked midpoint sqrt(10)≈3.16227766017, different from literal whole-function midpoint3. |
| Same LogLinear source, relative bucket w(0)=0,w(1)=1,p=100% | Rebuilt midpoint sqrt(8)≈2.82842712475, different from literal continuous-weight result3. |
| Level100, two relative bucket weights1/2, each+10% | Sequential composition110.25 differs from parallel110, despite a partition of unity. First-order directions still add. |
| Price-10 with relative+10% or target factor-1 | Candidate levels are-11 and+10. Signed scaling is explicit parameter algebra; negative multipliers are refused for VolIndex. |
| VolIndex level20, absolute+2 points or relative+10% | Both give22points; this does not make the input decimal sigma.20 or variance.04. Zero scaling is admissible with Linear/PCHIP but fails positive-source LogLinear admission. |
| Factor k=1e-300 at weight1 on level1e300 | Candidate level is approximately1. Forming k-1 first rounds to-1 in binary64 and fabricates multiplier0; exact endpoint semantics and stable scaling preserve the requested result. |
| US30/360 B=2025-01-29,v=Jan30,T=Jan31 | tau_B(v)=1/360,tau_B(T)=2/360 while tau_v(T)=0. For I(t)=100+360t, frozen dated level at T is102; new-base translation fromv would give101. |

Independent qualification must cover:

- Strict kind/clock/calendar/style/tail and ordered source/state admission,
  future-only coverage, exact optional0 and removal of all spot inference/repair.
- Signed/zero Price, nonnegative VolIndex and strictly positive LogLinear source
  domains, DF-style rejection, queried tail/domain/range failures and no NaN/
  clamp/fallback; native quotation/unit compatibility at actual resolution.
- Parameter formulas versus literal shocks under each scalar strategy, exact
  original-coordinate geometry/targets, zero/negative multipliers, normalization/
  product range, genuine zeros, no-op preservation and atomic failed changes.
- Absolute versus relative bucket composition, finite sensitivity convergence,
  source-domain refusal after shocks and independently qualified risk labels.
- Original-clock frozen dated reuse, non-additive/colliding clocks, absent
  reference levels, backward/beyond-grid references, preserved full source and
  product-owned fixing/settlement/observation aggregation.
- Actual commodity/VolIndex/carry/option/scenario consumers and host schema/
  state/error/parity, plus separately selected financial model benchmarks.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
