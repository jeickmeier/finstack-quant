# MODELS-ANALYTIC-05: Continuous single barriers, touch probability and rebate timing

**Decision status:** Accepted — complete checked continuous single-barrier premium and first-passage cash package, 2026-11-10.

**Implementation status:** Observed. Reconciliation covers the scalar functions and the dated asset and FX consumers below.

**Validation status:** Source, primary references and existing tests inspected. Independent bounded calculations cover selected formulas and failure cases. Production and complete product qualification remain open.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for continuous barrier premiums and cash

Accepted on **2026-11-10** after review of all 23 decisions.

The scalar contract applies the accepted [inclusive touch and conditional payment rules](../core-conventions/020-barrier-classification.md#accepted-decision-barrier-classification-touch-and-conditional-payment-timing)
and uses the accepted [checked spot law](spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results)
when the barrier state reduces to vanilla.

1. **Select a single continuously monitored barrier law.** Price a fixed-strike
   European call or put under constant-coefficient risk-neutral GBM, with one
   flat barrier H and continuous monitoring on [0,T], including both endpoints.
   The caller establishes active monitoring, an eligible time-zero observation
   and no earlier recognized hit. A live seasoned contract with qualified
   no-hit history can use this remaining-window law.

2. **Use checked shared model inputs.** Keep a single immutable validated
   BarrierParams for S,H,T,r,q,sigma. Require finite S,H>0, finite signed
   continuous decimal r,q and finite nonnegative model-year T and annual
   relative decimal sigma. Supply finite K>0 and call/put only to premium pricing.
   Validate the full relevant request before all shortcuts, including zero
   amount, deterministic and immediate-hit cases. Restore state through the
   same admission if exposed, rejecting missing, duplicate and unknown fields.

3. **Supply the flat discount and carry law explicitly.** Model the drift of
   spot as r−q and discount at exp(−rt). FX uses domestic r and foreign q in
   the quoted orientation. Remove the generic with_df inference. Dated owners
   qualify the constant-coefficient model or explicitly select a flat proxy
   using their market sources and model clock. A terminal discount factor
   specifies payment scale at T. The path law needs the rates over the window.

4. **Keep three checked Rust operations on that law.** Use
   `barrier_price(&BarrierParams, strike, BarrierType, OptionType)` for the
   no-rebate premium, `barrier_touch_probability(&BarrierParams, BarrierDirection)`
   for the reported hit probability and
   `barrier_cash(&BarrierParams, BarrierDirection, BarrierCash, amount)` for
   conditional cash. Each returns Result<f64>. Share the private first-passage
   geometry and remove raw/checked siblings and the older rebate wrapper.

5. **Separate cash entitlement from premium activation.** BarrierCash has
   HitAtHit, HitAtExpiry and NoHitAtExpiry. Amount is finite and nonnegative in
   the requested cash scale. Pay at the first eligible hit immediately or at
   T according to the selected variant. Map a knockout hit rebate and a
   knock-in failure rebate at the product boundary. Reject explicit no-hit
   AtHit terms. Absent cash needs no timing. An explicit zero-amount request
   still validates its terms. The scalar prices the premium and cash separately.

6. **Apply inclusive touch at eligible time zero.** Up hits at S>=H and down
   at S<=H. On a hit now, knock-in is the corresponding vanilla premium and
   knockout is zero. Hit probability is one and the at-hit factor is one.
   The at-expiry hit factor is D(T) and the no-hit factor is zero.
   The barrier direction remains the supplied inequality.

7. **Define the zero-horizon contract.** T=0 gives a singleton monitoring
   interval at the admitted spot. If unhit, the knock-in and hit cash are
   zero, knockout pays intrinsic and no-hit expiry cash is the full amount.
   The hit branch follows point 6 with D(0)=1. Dated expiry and outstanding
   payments are resolved by the product owner.

8. **Use the exact deterministic path when sigma=0.** On the unhit side set
   d=+1 for up and −1 for down, a=d log(H/S)>0 and b=d(r−q).
   A hit occurs iff b>0 and a<=bT. Its time is a/b, including equality at T.
   Price the entitled intrinsic on S exp((r−q)T) with the appropriate
   discount, and cash on its selected event and time. Preserve original
   log-space crossing geometry. Refuse unresolved comparisons. A rounded
   terminal spot equality, tiny positive sigma or underflowed sigma² does
   not trigger this branch.

9. **Specify the regular reflection law.** For unhit T>0 and sigma>0 use
   x=d log(S_T/S), m=d(r−q−sigma²/2) and w=sigma sqrt(T).
   Let g(x) be the normal density with mean mT and variance w². For x<a the
   hit density is exp(2ma/sigma²) g(x−2a). The surviving density there is
   g(x) times [−expm1(−2a(a−x)/w²)]. For x>=a all terminal density is hit.
   The terminal spot in the payoff is S exp(d x).

10. **Evaluate the requested option through truncated Gaussian moments.**
    Take the discounted payoff expectation over its positive-payoff support
    against the hit density for knock-in or surviving density for knockout.
    Use the equivalent analytic Gaussian/image terms for all eight cases,
    intersecting the strike support with x<a or x>=a. Compute surviving
    terms with resolved differences on their own support. Remove subtraction
    of a rounded knock-in premium from a rounded vanilla premium.

11. **Preserve structural zeros and parity.** On an unhit stochastic start,
    an up-and-out call with K>=H and a down-and-out put with K<=H have zero
    premium. The remaining stochastic premiums are positive. For identical
    no-rebate terms, knock-in plus knockout equals vanilla. Use these as
    model identities and qualification checks. Apply no nonnegativity floor
    or parity repair to a failed numerical result.

12. **Evaluate hit and survival on their own scales.** With Φ the standard
    normal CDF, P(T)=Φ((mT−a)/w)+B, where
    B=exp(2ma/sigma²) Φ((−mT−a)/w). The survival formula is
    Q(T)=Φ((a−mT)/w)−B. Use stable log-CDF, tail and difference identities,
    including the zero-drift erf reduction for survival. Keep the small tail
    for cash pricing. The public hit probability may round to 0 or 1 when
    resolved. Reported endpoints carry no historical or exact support meaning.

13. **Price expiry cash and define at-hit cash by first passage.** Expiry
    cash is amount × D(T) × P(T) or amount × D(T) × Q(T). For an unhit
    stochastic start the hit-time density is
    f_m(t)=a exp(−(a−mt)²/(2 sigma²t))/(sigma sqrt(2πt³)) for t>0.
    The at-hit factor M is the integral of exp(−rt) f_m(t) over (0,T].
    At r=0 it is P(T). It lies between min(1,D(T)) P(T) and
    max(1,D(T)) P(T). This is a bound on the expectation.

14. **Keep the real-root at-hit formula when its discriminant permits it.**
    For Δ=m²+2r sigma²>=0 set λ=sqrt(Δ) and use
    M=exp((m−λ)a/sigma²) P_λ(T), with P_λ the same hit CDF at oriented
    drift λ. Include Δ=0. Resolve the sign in original scaled geometry,
    combine the root and CDF in log scale and rationalize cancelling
    exponents. Unresolved sign or arithmetic raises a computation error.

15. **Integrate the same real expectation for Δ<0.** This region requires
    r<0 and has a valid finite-horizon expectation. Set c=−rT>0. Integration
    by parts and the change of time coordinate
    u(x)=log(x+(1−x)exp(c))/c give
    M=P[T](1+expm1(c) J), with
    J=∫_0^1 [1−P(T u(x))/P(T)] dx.
    The bracket is increasing from 0 to 1. Use those exact endpoints, stable
    log/time geometry inside the interval and the resolved hit or survival
    difference for the ratio. Combine the correction and original cash scale
    in log/scaled arithmetic. A rounded public probability is never used as
    the normalizer.

16. **Scale the estimator budget and bound numerical work.** Let E=expm1(c)
    and g(x) be the bracket in point 15. Use the accepted
    [adaptive Simpson rule](../core-numerics/quadrature.md) with depth 48 and
    at most 100,000 integrand evaluations. Cache the midpoint g(1/2) and
    count its evaluation in the budget. Since g is increasing, J>=g(1/2)/2.
    Set the absolute estimator budget for J to
    1e−12 × min(1, 1/E + g(1/2)/2), evaluated in stable scale.
    This bounds amplification of the accepted estimator against a lower
    scale for 1+E J. Require a resolved positive budget and integral, and the
    complete global estimator gate. Sampling, precision or work failure
    propagates as computation/convergence error. The estimator remains
    operational. True integration and cash accuracy require independent
    qualification.

17. **Preserve final monetary range.** Compute premium or cash in its final
    requested scale using checked log ratios, total standard deviation,
    reflection products and resolved positive differences. Keep scaled tails
    through amount and discount multiplication, including when the
    intermediate probability or factor would underflow or overflow. Return
    a finite nonnegative premium or cash amount. Numerical loss of a known
    positive result, unsupported range or material bound violation is a
    computation error. Remove NaN sentinels and probability/discount clamps.

18. **Keep dated history and outstanding cash with the owner.** Retire the
    generic expired_barrier_payoff helper. Use completed eligible history
    for the expiry amount. Require the contractual terminal observation when
    the earned option intrinsic depends on it. A fully determined rebate
    uses its qualified event and payment terms. Resolve a
    prior hit and any separate at-hit entitlement before the live model.
    Retain evidence for unpaid, paid or otherwise discharged cash. A historical
    hit flag alone is insufficient. An unresolved outstanding entitlement
    errors, and a no-hit snapshot cannot block a later eligible observation.
    Resolve known claims before querying live market inputs where possible.

19. **Reconcile the asset and FX continuous barrier pricers.** Use checked
    shared premiums and cash, validate active monitoring and honor the actual
    price, curve and volatility inputs. Apply quantity/notional once to the
    per-unit option premium and value the actual rebate amount on its own
    scale. Preserve asset total Money rebates and FX quote-currency totals.
    Retain errors and dated settlement under the relevant product contracts.

20. **Reconcile FX one-touch and no-touch.** Replace the separate root,
    complement and deterministic branches with the same checked cash law.
    Preserve the explicit direction, quote-currency payout and required
    history. Reject incompatible timing. Retain owner-qualified domestic
    discounting, carry and actual claim status.

21. **Keep discrete and alternative models with their owners.** Use this
    scalar law for continuous monitoring with the selected flat coefficients.
    Discrete observations, future monitoring starts, delayed cash, moving
    barriers, PDE and Monte Carlo each need their own model qualification.
    For otherwise identical no-rebate terms, monitoring a superset of times
    increases knock-in and decreases knockout value in the weak sense.
    The scalar applies no continuity correction or automatic method switch.

22. **Preserve the existing host price adapter.** Keep required canonical
    up/down and in/out strings in the existing Python and WASM price call.
    Parse in Rust and delegate to the same checked typed price. Remove the
    dispatcher's separate discounted-spot admission. Canonical admission
    covers structural zeros and the actual requested result scale. Reconcile
    existing exports, declarations, schemas, parity and instrument
    documentation. The strike-free probability and cash operations stay
    Rust-only. Invalid terms use validation errors and numerical refusal
    uses computation errors. Hosts preserve the distinction.

23. **Qualify the separate expectations and dated paths.** Compare independent
    terminal-density and first-passage references, all payoff regimes,
    boundary and deterministic cases, both discriminant signs and zero,
    near-one and tiny probabilities and full cash scale. Check the
    quadrature estimator and work failures separately from true price
    accuracy. Verify actual history, entitlement, timing, quantity and
    host behavior. Source-labelled Haug and same-kernel fixtures retain
    their original evidence limits.

## Independent bounded reference checks

- A down barrier with S=K=100, H=90, T=1, r=0.05, q=0.02 and sigma=0.2
  gives no-rebate call knock-in 1.640051538417633 and knockout
  7.586953969736415. The vanilla is 9.227005508154048. The hit probability
  is about 0.5825331337588608 and the at-hit factor 0.5729327168095601.
- For an up barrier with S=1, H=1.25, T=1, sigma=0.25, r=−0.125 and
  q=−0.15625, the oriented log drift is zero and Δ=−0.015625. Independent
  first-passage density integration gives P(T)≈0.37208523788780426,
  M≈0.39569660547230932 and D(T)P(T)≈0.42162781172156747. With q=−0.28125,
  Δ=0 and M≈0.58138318419969416.
- A down-and-out put with S=K=1, H=1−2^-53, T=1, sigma=0.25,
  r=0.03125 and q=0 is about 1.25325083246e−63 by bounds on the positive
  killed-density expectation. Vanilla and knock-in round to the same binary64
  value, about 0.08332294292109577.
- For up S=1, H=1+2^-52, T=1, r=0, q=−8 and sigma=4, survival is about
  4.42914905198e−17 even when the reported hit probability rounds to one.
  For up S=1, H=235385266837020000, T=1, r=0, q=−0.5 and sigma=1, the hit
  probability is about 7.31178708183e−350. Amount 2^1023 gives finite cash
  of about 6.57217472029e−42.
- With S=100, up H=110, K=90, T=1, sigma=q=0 and a deterministic dated
  curve with D(0.5)=0.80 and D(1)=0.95, the path touches by time 0.5
  and knockout premium is zero. The model flattened from D(1) misses
  the touch and gives 14.50. These are two different path laws.
- For deterministic up S=1, H=2, T=1 and q=0, the binary64 value nearest
  log(2) lies strictly below the exact log distance. Its terminal spot rounds
  to 2, even though the path stays below the barrier.
- For up S=1, H=1+2^-52, T=sigma=1, r=−80 and q=−80.5, the true at-hit
  factor is about 6.25385452366e16. A fixed absolute estimator tolerance
  of 1e−12 on J accepts about 1.64660782485e19 with the stated depth limit.
  Its global estimator and pathwise bounds both pass. With the scaled
  budget in point 16, the unresolved terminal panel exhausts depth and the
  request errors. These are independent calculations of the candidate rules.

## Observed scope and input domain

Up/down knock-in/out calls/puts under constant GBM, a single continuous barrier, and optional expiry/hit rebate helpers. Checked dispatch requires positive S,K,H and nonnegative finite T,σ.

## Observed choice and rationale

Reiner–Rubinstein-style image terms price knock-ins; knock-outs use vanilla minus knock-in. Separate first-passage probability and discounted-hit calculations distinguish rebate cash-flow timing.

## Observed algorithm

Already-hit is inclusive S≥H (up) or S≤H (down). Knock-in becomes vanilla and
knockout zero. Otherwise the code computes four direct/image terms and selects
their combinations by call/put, barrier side and K versus H. It uses log
normal CDF for reflection products. Knockout is max(vanilla−knock-in,0).
Touch probability sums the direct and reflected CDFs and clamps to [0,1].
Zero volatility follows deterministic S exp((r−q)t), including deterministic
hit time. Expiry rebates use discounted hit probability for knockout or
survival probability for knock-in. At-hit cash uses the real-root formula or
adaptive Simpson on normalized survival at negative discriminant. The expiry
helper assumes earlier at-hit payment when it returns zero for that cash.

## Observed selection and controls

Barrier type and call/put are explicit; `with_df` derives a constant rate. Knock-in AtHit requests still route to expiry rebate because failure is known only at expiry. `expired_barrier_payoff` takes caller-supplied breach history. Checked convenience dispatch validates more than raw helpers, which validate mainly S,K,H or use arithmetic directly.

## Observed conventions

Rates are continuous annual decimals, σ relative annual volatility, T years. Rebate helpers return per-unit present value and do not implicitly add it to barrier option price. Historical breach state is not inferred from current spot alone.

## Observed limitations, failure and fallback

Only continuous single-level monitoring is represented. Raw helper invalid inputs can yield NaN; numerical integration failure returns NaN. Hit discount is clamped to [0,max(1,exp(−rT))]; knockout flooring can hide small parity errors. No discrete-monitoring correction is present.

## Alternatives and tradeoffs

Discrete trees/MC with bridge corrections are possible alternatives, not selected here. Expiry and hit rebate variants are implemented and materially different under nonzero rates.

## Implementation and validation evidence

[barrier.rs](../../../../finstack-quant/models/src/closed_form/barrier.rs) and [dispatch.rs](../../../../finstack-quant/models/src/closed_form/dispatch.rs). Inline Haug-labelled cases, eight-way in/out parity and deterministic/timing regressions are located; `test_haug_2007_down_in_call` is reference-labelled evidence whose original table provenance is not requalified. Reference: [Reiner–Rubinstein](../../../REFERENCES.md#reiner-rubinstein-1991).

The live callers are the
[asset barrier pricer](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/pricer.rs),
[FX barrier pricer](../../../../finstack-quant/valuations/fx/src/fx_barrier_option/pricer.rs)
and [FX touch pricer](../../../../finstack-quant/valuations/fx/src/fx_touch_option/pricer.rs).
The FX touch pricer duplicates the first-passage formula, uses a complement
for no-touch and rejects a negative at-hit discriminant. The asset and FX
barrier wrappers use the shared rebate helper. Dated handling of prior hits
and expiry currently needs reconciliation with the accepted cash ownership.
The [Python](../../../../finstack-quant-py/src/bindings/models/closed_form.rs)
and [WASM](../../../../finstack-quant-wasm/src/api/models/closed_form.rs)
adapters expose the option price.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
