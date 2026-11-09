# CORE-MARKET-04: Piecewise-constant intensity and checked survival/default probabilities

- Decision status: **Accepted — complete intensity/probability/dated-conditioning package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths identified below; direct-integration and boundary reconciliation remain unimplemented.
- Validation status: **Source/reference evidence and independent financial/numerical/consumer reviews inspected; standalone rational/high-precision expression checks executed 2026-11-08; production qualification pending.** Existing Rust/host tests were inspected or located, not executed. Expression checks do not certify production behavior, market calibration or a universal accuracy bound.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This decision owns one nonnegative piecewise-constant intensity source, checked
hazard/integral/survival/default-probability queries, explicit source clocks,
survival-input conversion and frozen dated conditioning. It preserves accepted
[ADR0045 clocks](../conventions/0045-financial-clocks-and-date-boundaries.md),
[rate units](../core-conventions/018-rate-units.md) and the
[numerical probability-range distinction](../core-numerics/distribution-functions.md#accepted-decision-checked-special-functions-and-distribution-domains).
[CORE-MARKET-01/02](discount-evaluation.md) own positive discount/projection
factors; [CORE-MARKET-03](rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning)
owns their shocks and dated conditioning. This probability source has its own
range: rounded zero survival does not destroy the underlying local intensity.

[Record05](hazard-quotes-and-risk.md) retains indicative CDS quotes, spread versus
hazard risk, recovery overrides and quote consistency; [record06](calibration-provenance.md)
owns replay/fit provenance. CDS cashflow integration, contractual recovery,
calibration constraints and market fit remain with
[ADR0030](../models-and-valuations-credit/0030-cds-survival-and-integration.md),
[hazard calibration](../calibration-targets/05-hazard-calibration.md) and product
owners. This package does not adopt their Draft choices, credit-event eligibility,
physical-versus-pricing measure selection, stochastic default dynamics or a
complete CDS standard-model implementation.

## Foundational findings

Canonical nonnegative intensities directly define a monotone survival function
and local conditional probabilities. Requiring positive interpolated survival
pillars adds a needless construction failure when accumulated intensity is
large. Conversely, subtracting rounded survival endpoints can erase a still
representable unconditional default probability.

The current 1e-9 origin skip also makes the integrated survival disagree with
hazard lookup on a valid short first interval. The default 10/year cap is a
plausibility/fit policy, not the mathematical domain of finite intensity.
Finally, rebuilding on a new-base clock repeats the non-additive-clock problem
identified in accepted03. These require explicit financial and numerical choices.

## Accepted decision: explicit intensity sources and local probability evaluation

Accepted on **2026-11-08** after independent financial, numerical and consumer
review. Runtime/caller/host reconciliation and independent production
qualification remain pending; observed implementation evidence stays separate below.

1. **Retain one canonical intensity representation.** Keep an immutable owned
   HazardCurve with at least one finite strictly positive ending time,
   0<t1<...<tn, and a finite nonnegative intensity lambda_i at each end. Lambda
   is an annual decimal intensity per source-clock year, not a one-year PD,
   quoted CDS spread or recovery-adjusted spread. Reject unsorted/duplicate
   times, negative/nonfinite intensities and zero-time hazard samples. Do not
   sort, merge, snap or impose a minimum gap. S(0)=1 is implicit normalization,
   not an extra fitted hazard. Store these intensities as canonical truth;
   remove survival-pillar interpolation and any required positive-survival or
   finite whole-history cache. Reuse existing summation and checked scalar
   arithmetic; no generic positive-factor adapter or interpolation selector.

2. **Use exact ending intervals and one declared tail.** Lambda_i applies on
   (t_(i-1),t_i], with t0=0. At an exact interior ending knot return lambda_i,
   the ending/left interval's intensity; at0 use lambda1 as the right/inward
   value. Correct the current right-continuous description. Beyond tn, retain
   lambda_n constantly as this named model's fixed tail; no inferred tenor
   steps, tolerance-origin skip or new tail/interpolation mode. Zero intensity
   is valid, including a final zero tail with positive limiting survival; this
   model does not assume every issuer eventually defaults. The mathematical
   derivative S'=-lambda*S uses the same one-sided convention, without claiming
   that differentiating rounded-zero probabilities recovers the intensity.

3. **Require explicit source clocks and a nonnegative modeled horizon.**
   Construction, factories, canonical restore and host inputs require the base
   date and complete supported source day count/context. Require a resolved
   calendar for Bus/252 and reject conventions requiring absent coupon or
   termination context. Remove implicit Act/365F and default-context date
   mapping. Numeric queries require finite nonnegative source coordinates;
   intervals require 0<=a<=b. Dated queries require dates on/after the original
   source base, chronological endpoints, complete mapping and finite ordered
   nonnegative mapped coordinates. Do not clamp historical dates or reinterpret
   the CDS premium/protection/model clock as the source clock. Validate all
   inputs and calendar coverage before zero/equality identities. Distinct
   admitted dates can map to one coordinate and then have zero interval hazard.
   Historical default status and product coverage partitioning stay external.

4. **Integrate the requested intensity interval directly.** Define
   J(a,b)=integral_a^b lambda(u)du by exact ending-cell overlaps and the declared
   tail. Use checked nonnegative products and compensated/scaled accumulation
   in deterministic interval order; skip only exact zero overlaps/intensities.
   Do not subtract two origin-to-endpoint cumulative hazards. Avoid losing
   individually small products before their aggregate becomes representable.
   Expose checked numeric and dated integrated-hazard operations for actual
   consumers. A requested finite J must be nonnegative and representable;
   overflow or a required positive value lost to zero is a computation/range
   error, distinct from the probability-output rules below. Private caches may
   accelerate queries only if they preserve local information and the admitted
   domain; cache representability must not become a new construction bound.

5. **Compute survival and conditional probability from local intensity.** For
   the original source, S(t)=exp(-J(0,t)); conditional survival on [a,b] is
   exp(-J(a,b)) and conditional PD is -expm1(-J(a,b)). Provide checked numeric
   and dated operations through this same evaluator. Neither conditional query
   requires positive representable S(a), a ratio S(b)/S(a), or a log of rounded
   survival. Equal admitted endpoints yield survival1 and PD0. This is the
   model conditional on survival at a, not proof that the issuer is alive or
   that a product may project past a known credit event. Keep survival and PD
   direct rather than computing a tiny requested quantity by subtracting1.

6. **Preserve unconditional interval mass.** Default probability on (a,b],
   conditional only on survival at source origin, is
   exp(-J(0,a))*[-expm1(-J(a,b))]. Compute this complete result with suitable
   scaling rather than S(a)-S(b) or premature rounding of independently cached
   endpoints. Direct local accumulation is still required when the preceding
   history is large. The source has no modeled point mass at the origin or
   deterministic jump dates. For a dated view referenced at v, the analogous
   unconditional interval probability uses J(tau_B(v),tau_B(start)) as its
   preceding history; conditional interval PD remains the local formula.
   Product default-time/LGD/payment integration is separately owned.

7. **Use probability range, without floors or observed-default inference.**
   Return finite probabilities in[0,1], including positive subnormals. Ordinary
   final rounding/underflow to0 or1 is allowed; it does not establish exact zero
   or full probability mass. Do not clamp, floor or impose an arbitrary tail
   cutoff. For probability requests, overflow of a guaranteed nonnegative
   product/sum can establish the representational tail S=0 or conditional PD=1;
   use an explicit private overflow classification instead of propagating NaN
   through compensated sums. This is not successful infinity for a public
   finite integrated-hazard request. Similarly preserve small accumulated
   intensity before deciding a rounded probability endpoint. Reject invalid
   inputs, negative/NaN arithmetic and unresolved computation failures. Do not
   demand representable unrequested absolute survival or cumulative hazard,
   require arbitrary precision, or promise correct rounding/global error bounds.

8. **Remove the generic hazard cap and retain recovery's stated role.** Delete
   DEFAULT_MAX_HAZARD_RATE, max_hazard_rate fields/accessors/builders, wire/host
   defaults and cap checks. All finite nonnegative intensities are eligible;
   their required query computations still have explicit range limits.
   Calibration/product owners retain explicit qualified fit/plausibility bounds.
   Retain explicitly supplied finite recovery R in[0,1] as existing calibration/
   reporting metadata. R does not enter lambda/J/S/PD formulas; R=1 is valid
   for this source. It can still affect CDS prices, spread-to-hazard mappings
   and calibration assumptions at their financial owners. This package does
   not authorize division by1-R at R=1, stale quote reuse or recovery overrides
   as recalibration; those remain record05/product decisions.

9. **Make survival-input conversion an explicit factory.** Require already
   ordered distinct finite times and positive-time finite probabilities
   0<S_i<=1, nonincreasing from the exact origin S0=1. An optional exact (0,1)
   input is admitted as normalization and is not retained as a hazard sample;
   at least one positive-time pillar is required. Reject negative times,
   duplicate/near-origin repair, approximate origin probabilities, zero survival
   and increasing survival. Derive each lambda from the stable positive-factor
   log difference divided by its actual positive source width; reuse the
   accepted private log-difference primitive without materializing a tiny ratio.
   Equal input survival values give zero intensity; otherwise require a finite
   nonnegative represented rate without losing a required positive rate to0.
   Zero survival input does not identify a finite intensity, although finite
   intensities may later produce rounded-zero survival. Rounded input levels
   cannot reveal intensity information already lost. No exact inverse claim.
   The named flat factory uses one endpoint (1,lambda), the same constant tail,
   explicit clock/base/recovery and the same admission; no implicit clock/cap.

10. **Condition a frozen source on its retained dated clock.** Keep original
    source base B, complete clock, intensity ends/values and a distinct reference
    date v>=B. For T>=v, map both dates on tau_B and evaluate
    S_v(T)=exp(-J(tau_B(v),tau_B(T))) directly. Do not require S(v)>0 or divide
    rounded absolute survival. Preserve chronological and mapped-coordinate
    admission. This is a dated view, not a fresh scalar curve on tau_v(T);
    it inherits no new-base numeric-clock API. Repeated conditioning replaces
    the reference over the same retained source, without nested curves or
    pairwise clock-offset addition. Permit references beyond the final pillar
    through the constant tail and backward changes within the original modeled
    domain, even after absolute survival has underflowed. A view's probability
    queries begin on/after its reference; earlier modeled queries use the
    retained source explicitly. Do not drop expired source pillars, insert
    zero-time hazards, shift stored quotes or infer an observed credit event.
    Underflow-safe intensity conditioning has a different numerical boundary
    from accepted03's positive rate-factor conditioning. Future dynamics and
    quote recalibration remain separately selected.

11. **Reconcile actual callers and strict state/host boundaries together.**
    Make source queries fallible and preserve existing input/domain versus
    computation/range error categories. Reconcile the source, builders,
    transforms, canonical wire/schema, calibration, valuation, margin and
    Python/WASM surfaces in coherent slices. Propagate errors rather than wrap
    unchecked numeric results in success. Persist ordered intensity state,
    explicit original clock/calendar, recovery and authoritative metadata;
    dated views additionally retain their reference. Rebuild private caches;
    do not trust supplied survival/interpolation caches or repair restored data.
    Reject retired cap/zero-time/implicit-clock state without aliases or
    migration. Validate a whole replacement before committing; probability
    batches return complete validated results rather than partial/clamped data.
    Transforms owned by05 must preserve this representation/admission, but their
    risk/quote semantics are not adopted here. Construction provenance remains
    distinct from proof of fit after conditioning or another transformation.

## Rationale and alternatives

Nonnegative piecewise intensities encode the model directly, guarantee monotone
survival in real arithmetic and retain local information after an absolute
probability has rounded to zero. An interpolated positive-survival source would
impose the wrong range restriction; a separately configurable smooth hazard or
cumulative-hazard interpolation model is unnecessary for this contract.

A pre-base survival extension would add a historical modeling assumption. This
package instead makes the modeled origin explicit. The fixed constant last
intensity is part of the named representation, not an undisclosed generic tail
default. Financial calibration bounds and recovery-sensitive credit pricing
remain at their actual owners rather than becoming numerical source-domain laws.

## Observed choice and rationale

`HazardCurve` represents nonnegative piecewise-constant hazard rates attached to **interval-ending knots**, with log-linear survival and constant last-hazard extrapolation. Conditional default probability integrates interval hazards directly, avoiding loss of information when unconditional survival underflows. This does not implement CDS cashflow integration; that resides in the valuations/calibration scopes and [ADR 0030](../models-and-valuations-credit/0030-cds-survival-and-integration.md).

## Observed algorithm, domain, and conventions

The builder sorts finite nonnegative times, rejects duplicates and nonfinite/negative hazards, requires explicit recovery in [0,1], and caps hazard at 10 per year by default (override available). At least one supplied hazard point is required. It starts survival at `(0,1)`, skips knots with `t<=1e-9`, and accumulates `H_i += lambda_i*(t_i-t_previous)`, storing `S_i=exp(-H_i)`. Strict log-linear interpolation of these survival pillars reproduces constant interval hazard. Tail survival is `S(last)*exp(-lambda_last*(t-last))`; `sp(t<=0)=1`.

`hazard_rate(t)` chooses the first knot at or after `t`, or last beyond the tail; for nonpositive time it returns the first stored hazard. `default_prob(t1,t2)` is unconditional `S(t1)-S(t2)` and rejects reversed endpoints. Date-based `conditional_default_prob` maps both dates by stored day count (Act/365F default), floors times to zero, integrates overlap with each interval and tail, and returns `-expm1(-integral)`. It returns zero for equal dates and rejects reversed dates. `survival_at_dates` additionally clamps results to [0,1].

`from_survival_probs` sorts finite times and probabilities in (0,1], treats times <=1e-9 as origin requiring survival within 1e-12 of one, requires nonincreasing survival, and derives `lambda=-ln(S_i/S_previous)/dt`. Zero survival cannot be ingested. A curve whose stored survival pillars underflow to zero can fail strict interpolation construction even though later tail evaluation may legitimately underflow.

Rolling shifts surviving ending knots, inserts a new zero-time hazard sample, and rebuilds; it fails when no positive-time knot survives. This realizes conditional survival for retained piecewise segments. Stored par quotes are shifted and expired quotes removed; calibration recipe is dropped.

## Observed limitations and alternatives

The max-hazard override itself is not checked for a finite, nonnegative value; NaN bypasses `lambda > cap`. `default_prob` does not validate finite numeric endpoints, unlike the date-based conditional method. The `<=1e-9` origin tolerance is applied by survival integration but `hazard_rate` uses exact stored knots: a positive near-zero knot can report a hazard different from the derivative of survival over that tiny interval. Exact origin handling or a consistently applied tolerance needs agreement.

Time-varying smooth hazard, interpolated cumulative hazard, or directly stored log survival are alternative representations, not selectable here. A log-survival representation could extend extreme-survival construction range; the current direct conditional integral already protects short conditional periods after large accumulated hazards.

## Observed evidence

Owning code: [builder](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/builder.rs), [survival/conditional kernels](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/curve.rs), [defaults](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/mod.rs), [roll/transform](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/transform.rs), [wire](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/wire.rs). Inspected [tests](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/tests.rs) cover interval boundaries, derivative consistency away from knots, conditional survival under roll, direct overlap integrals, 1e-18 hazard after a large integral, and unconditional underflow. Evidence is analytic formula/invariant, not an independent CDS vendor reconciliation. References: [ISDA CDS model](../../../REFERENCES.md#isda-cds-standard-model), [Hull–White CDS](../../../REFERENCES.md#hull-white-2000-cds).

## Caller and primary-reference evidence

| Inspected owner | Required reconciliation |
| --- | --- |
| [Builder](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/builder.rs), [curve](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/curve.rs), [state](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/wire.rs) | Strict positive ending times, explicit clock/calendar, no cap/origin tolerance; replace survival interpolation and rounded-survival subtraction with direct overlap integration. |
| [Hazard roll](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/transform.rs) | Retain original clock/intensity state and dated reference; remove numeric-offset/expired-grid rebuilding and leave record05 risk semantics distinct. |
| [CDS helpers](../../../../finstack-quant/valuations/credit/src/cds/pricing/helpers.rs), [CDS engine](../../../../finstack-quant/valuations/credit/src/cds/pricing/engine.rs) | Remove absolute-survival floors/divisions and low-survival observed-default inference; reconcile conditional factors/density scaling without adopting CDS leg integration policy. |
| [Revolver path generator](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_generator.rs), [tranche timing engine](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/engine.rs) | Obtain local intensity integrals rather than floored/rounded survival ratios; qualify model-average horizon and default-timing approximations separately. |
| [CVA](../../../../finstack-quant/margin/src/xva/cva.rs), [MVA](../../../../finstack-quant/margin/src/xva/mva.rs) | Validate exposure/hazard common-clock assumptions or map actual dates with source context; a fallible scalar API does not repair a clock mismatch. |
| [Hazard calibration](../../../../finstack-quant/calibration/src/targets/hazard.rs) | Make source-clock selection explicit and retain calibration bounds at their owner; update source parameter/state construction without treating cap removal as market-fit acceptance. |
| [Python](../../../../finstack-quant-py/src/bindings/core/market_data/curves/hazard.rs), [WASM](../../../../finstack-quant-wasm/src/api/core/market_hazard.rs) | Expose the same checked numeric/dated integral and probability operations, context and error behavior; remove implicit day count/cap and contradictory pre-base docs; update schema/stubs/generated declarations/parity. |

[QuantLib's hazard-rate adapter](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructures/credit/hazardratestructure.hpp)
defines survival from integrated annual continuous intensity. Its
[interpolated hazard curve](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructures/credit/interpolatedhazardratecurve.hpp)
uses nonnegative hazards and constant last-hazard continuation. Its
[default-term-structure contract](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructures/defaulttermstructure.hpp)
requires numeric times on the source day count. These support the representation
and clock separation, not our exact endpoint/underflow/error policies.
The [ISDA CDS Standard Model](https://www.cdsmodel.com/) and repository
[ISDA reference](../../../REFERENCES.md#isda-cds-standard-model) concern a fuller
contract/pricing/calibration calculation; this source alone does not establish
CDS model conformance or independent vendor reconciliation.

## Independent examples and required verification

On 2026-11-08 standalone Fraction arithmetic and Decimal at precision200 checked
these relationships; Python math was used only to illustrate binary64 rounding.
No library module was imported and no production suite was run.

| Example | Independent result / implication |
| --- | --- |
| Ends1,3 with lambda1=.02,lambda2=.05; query [.5,4] | J=.02*.5+.05*2+.05*1=.16, including constant tail. Exact hazard at1 is.02, just after1 is.05. |
| First end1e-10 with lambda.02; query [0,5e-11] | J=1e-12. Skipping the first interval by a 1e-9 origin tolerance changes the model. |
| Hazard7 through100, then1e-18; query [125,126] | Local conditional PD≈1e-18; both rounded S endpoints are equal. Unconditional interval PD≈9.85967654376e-323 rounds to20 positive minimum subnormals, despite rounded-survival subtraction giving0. |
| Hazard10 through100, then.02; query [125,126] | Absolute S has underflowed, while conditional survival=exp(-.02) and PD≈.0198013266932447 remain usable. |
| Flat lambda20, one source year | S=exp(-20)≈2.06115362244e-9 is a valid probability; a generic cap10 is not a mathematical domain requirement. |
| Survival observations S(1)=exp(-.02), S(3)=exp(-.12) | Real-arithmetic implied intensities are.02 and.05; inserting exact (0,1) adds no hazard sample. |
| US30/360 B=2025-01-29,v=Jan30,T=Jan31; flat lambda=.02 | Old-coordinate interval length is1/360; new-base tau_v(T)=0. Original-clock conditioned PD is -expm1(-.02/360), not0. |
| Finite lambda=1e308 over a finite length10 | Public finite J is unrepresentable; binary64 survival/conditional PD can legitimately return0/1 from the proven nonnegative tail. |
| Zero lambda with recovery.4 or1 | S=1 and all interval PDs=0. Recovery metadata does not alter these intensity-source probabilities. |

Expression examples establish definitions and counterexamples, not production
correctness or uniform accuracy. Independent qualification must cover:

- Strict ending-knot/zero/origin/ordering state, singleton and zero-tail cases,
  exact knot-side attribution, close positive ends and removed caps/defaults.
- All source clock/context/calendar boundaries, pre-base/reversed/nonfinite
  refusal, mapped-date collisions and identity validation before shortcuts.
- Independent local integrals, tiny intervals after huge histories, stable
  unconditional subnormal mass, zero/one rounding versus genuine identities,
  nonnegative-overflow classification and finite-J range failures.
- Stable survival-to-intensity conversion across nearby/widely separated positive
  inputs, exact origin requirements, positive inferred-rate range and loss already
  present in rounded survival inputs.
- Original-clock conditioning, non-additive clocks, underflowed origin survival,
  forward/backward references in domain, constant tails and exhausted reporting
  grids; view-origin versus source-origin unconditional interval probabilities.
- Actual CDS/revolver/tranche/margin callers' clock and conditional meanings,
  recovery/fit boundaries, error propagation, atomic changes, strict state/cache
  restoration and complete Rust/Python/WASM behavioral/documentation parity.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
