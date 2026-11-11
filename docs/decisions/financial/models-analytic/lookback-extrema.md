# MODELS-ANALYTIC-06: Continuous fixed and floating lookbacks with observed extrema

**Decision status:** Accepted — complete checked continuous lookback package with explicit extrema, carry and payment discount, 2026-11-10.

**Implementation status:** Observed. Reconciliation covers the scalar functions, existing host price call and dated asset lookback consumers.

**Validation status:** Source, primary references and existing tests inspected. Independent bounded calculations cover selected formulas and failure cases. Production and complete product qualification remain open.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for continuous lookback premiums

Accepted on **2026-11-10** after review of all 21 decisions.

1. **Select the inclusive continuous law.** Price a single European
   lookback payoff on a positive underlying. The remaining monitoring window
   is [0,T], including the eligible current observation and terminal state.
   The underlying follows constant-carry, constant-volatility GBM in the
   pricing measure. The caller qualifies the path law and supplied history.

2. **Use one checked request.** Select
   `lookback_option_price(params: &LookbackPriceParams) -> Result<f64>`.
   Require spot, time, carry, discount, volatility, extremum, option_type
   and strike. Spot and the relevant extremum are finite and positive in the
   same quote units. Time is finite and nonnegative in model years.
   Volatility is finite nonnegative annual relative decimal volatility.
   Validate all relevant fields before any shortcut.

3. **Make the strike choice explicit.** Use required nullable strike.
   Some(K) with finite K>0 selects fixed strike, and explicit None/null
   selects floating strike. Missing strike is invalid. Reuse call/put and
   the single relevant extremum. Retire the separate strike selector,
   fixed default, unused floating strike and four raw public formula leaves.

4. **Require a qualified extremum through now.** Fixed calls and floating
   puts use an observed maximum M>=S. Fixed puts and floating calls use a
   minimum 0<m<=S. Equality is valid. The scalar rejects an inconsistent
   supplied extremum. The dated owner can advance qualified strictly prior
   extrema using an eligible current observation. A proved inception can
   seed the extremum with that observation. Missing seasoned history errors.

5. **Separate path carry from payment discount.** Require finite signed
   continuous decimal net carry b and independent finite payment discount
   D>0, including D>1. Under the selected law
   S_t=S exp((b−sigma²/2)t+sigma W_t). In a flat domestic/carry
   setting b=r−q. A dated owner may deliberately qualify a flat proxy.
   The scalar consumes b and D as supplied and returns discounted value
   per underlying unit. Quantity and currency remain with the product.

6. **Define the four terminal payoffs.** Let M_T be the maximum of the
   supplied maximum and the remaining path, and m_T the corresponding
   minimum. Fixed call pays (M_T−K)^+, fixed put (K−m_T)^+, floating call
   S_T−m_T, and floating put M_T−S_T. Inclusion of S_T makes both floating
   differences nonnegative under this law.

7. **Use the singleton path when T=0.** Evaluate point 6 with S_T=S and the
   supplied extrema after complete admission. Multiply by the supplied D.
   A zero model-time coordinate and a separately prepared payment factor
   can coexist. The dated owner determines actual fixing and payment status.

8. **Use the exact deterministic path when sigma=0.** S_T=S exp(bT).
   Include that terminal level in the applicable extremum and evaluate the
   same payoff, with D once. Preserve original log comparisons and stable
   payoff differences through scaling. An underflowed variance or tiny
   positive time/volatility does not select this branch.

9. **Use one positive reflection term for the regular law.** For a
   threshold H at or above spot for a maximum, or at or below spot for a
   minimum, set d=+1 or −1 respectively. Write a=d log(H/S)>=0,
   B=bT, w=sigma sqrt(T), and mu=d(B−w²/2). With Φ the standard normal
   CDF, the discounted reflection premium is
   R_d(H)=D S ∫_a^∞ exp(2d B y/w²) Φ((−mu−y)/w) dy.
   This is the reflected portion of the first-passage probability integrated
   over levels beyond the threshold, using the
   [accepted reflection law](barriers-and-rebates.md#accepted-decision-for-continuous-barrier-premiums-and-cash).
   Threshold H=S is allowed.

10. **Assemble each requested price from nonnegative components.** Let
    C(H),P(H) be the vanilla prices under the same b, D, T and sigma.
    A fixed call has H=max(K,M) and price D(M−K)^+ + C(H) + R_+(H).
    A fixed put has H=min(K,m) and price D(K−m)^+ + P(H) + R_−(H).
    A floating call is C(m)+R_−(m), and a floating put is P(M)+R_+(M).
    This preserves the fixed locked payoff. Floating assembly uses vanilla
    parity algebraically and avoids subtraction of a rounded expected
    terminal level from a rounded extremum value.

11. **Use the exact nonzero-carry divided difference.** With
    x=(−mu−a)/w and p=2d B/w, the reflection price for b≠0 is
    D S w [exp(B)Φ(x+p)−exp(p a/w)Φ(x)]/p.
    Equivalently the bracketed quotient is exp(p a/w) times
    [exp(px+p²/2)Φ(x+p)−Φ(x)]/p. Preserve original signed carry and
    scaled geometry even when a standalone p or B would underflow.
    Use the original law or return a computation error.

12. **Use the analytic limit only for b=0.** The reflection term then is
    D S w [φ(x)+xΦ(x)], where φ is the standard normal density and x is
    computed with B=0. Evaluate the positive normal loss stably in the
    small tail. Zero carry leaves the GBM log-drift correction intact.
    Remove the absolute 1e−7 carry switch and any carry-to-zero snap.

13. **Resolve the nonzero divided difference in stable scale.** Use shared
    checked log-CDF, log-ratio and expm1 geometry. A positive formulation
    for the inner quotient in point 11 is Φ(x) A exprel(p A), with
    A=∫_0^1 G(x+u p) du and G(z)=z+φ(z)/Φ(z). Here exprel(v)
    is expm1(v)/v with its continuous value at zero. This identity supports
    stable evaluation of closely spaced arguments while retaining p.
    Preserve the positive normal loss in the derivative. Refuse unresolved
    cancellation or geometry. This record selects the analytic law and
    adds no public numerical controls or alternate pricing method.

14. **Preserve the complete requested price scale.** Combine spot, strike,
    discount and positive continuation terms in scaled/log form. Reuse
    checked private Black and normal-tail primitives under the prepared
    inputs. A raw forward, variance, reflected factor or standalone
    intermediate need not fit when the full requested price is resolvable.
    Return a finite nonnegative result or a computation error. Remove
    NaN, clipping and numerical loss of a known-positive price to zero.

15. **Use appropriate payoff checks.** A fixed price is at least the
    discounted locked payoff and the corresponding vanilla at original K.
    The fixed put is at most D K. Floating call is at least
    D(S exp(B)−m)^+ and at most D S exp(B). Floating put is at least
    D(M−S exp(B))^+. The current floating spread can shrink. All four
    stochastic prices are positive for T>0 and sigma>0. Use these as
    verification identities and bounds without clipping.

16. **Reconcile the live dated asset pricer.** Establish monitoring
    inception or qualified history and eligibility before advancing an
    extremum. Prepare explicit carry, D and volatility in the qualified
    measure and clock. Remove the implicit continuous history-to-spot
    fallback. Apply quantity once to the checked price in its native
    currency. A collapsed live model clock needs an explicit owner
    treatment and supplies no expiry or settlement fact.

17. **Resolve completed monitoring with dated evidence.** Require the
    certified final relevant extremum. A fixed payoff needs terminal spot
    only if an eligible terminal observation must complete that extremum.
    A floating payoff also needs the contractual terminal fixing. Use
    qualified observations and payment/settlement state, and resolve a
    known claim before unnecessary live market lookup. The scalar singleton
    supplies no historical fixing or outstanding payment evidence.

18. **Separate volatility sampling and risk from payoff terms.** A floating
    product can currently supply a value in its strike field that changes
    the volatility lookup. Qualify that coordinate explicitly in the dated
    owner and dependency metadata when removing the payoff strike.
    Preserve actual historical observations during risk repricing. A
    hypothetical current observation can advance the effective extremum
    only under the relevant scenario contract. Replacing a current
    observation requires a qualified pre-current extremum. Generic FD
    Greek choices remain with their owner.

19. **Keep other monitoring models separate.** Discrete, delayed or partial
    windows, jump or stochastic-volatility dynamics and Monte Carlo have
    their own contracts. For matched paths, continuous extrema over a
    superset satisfy max_cont>=max_discrete and min_cont<=min_discrete.
    Qualify any correction at its owner. The scalar uses the inclusive
    continuous law throughout.

20. **Update the existing host price route.** Use the same required request
    in Python and WASM, including explicit null for floating. Validate
    unknown, missing and duplicate fields before use or collapse. Reconcile
    exports, stubs, declarations, schemas, parity and product examples.
    Document the correct four-way extremum map. Rust owns the calculation
    and validation. Hosts map invalid terms and computation errors distinctly.

21. **Qualify the four formulas and actual owners separately.** Compare
    independent payoff or first-passage integration across seasoned
    and inception states, both carry signs, exact zero and tiny nonzero
    carry, both singular limits, tiny positive values and scaled final
    prices. Verify date eligibility, final observations, volatility
    sampling and host transport. Existing same-kernel and
    reference-labelled tests retain their evidence limits.

## Independent bounded reference checks

- With S=100, K=105, M=120, m=80, b=0.03, D=exp(−0.05), T=1 and
  sigma=0.2, independent calculations give fixed call 19.570497256164023,
  fixed put 25.503957665385951, floating call 23.644735423486510 and
  floating put 21.429719498063464. At b=0 the same order gives
  18.551493752841308, 25.942197593992827, 21.186050471489257 and
  23.307640875344878.
- With S=M=100, b=2^-24, T=2^24, sigma=2^-14 and D=1, cumulative carry is
  1 and total volatility 0.25. The nonzero-carry floating put is about
  8.49459907731. The existing cutoff expression gives about 3.12724801112.
- With S=M=1, b=0.25, D=exp(−0.25), T=1 and sigma=2^-30, the floating
  put is about 2^-59. The fixed-call-plus-prepaid-difference assembly
  loses this positive value in binary64. The nonnegative assembly
  preserves its reflection contribution.
- At S=K=M=m=1, T=1, b=0, D=1 and sigma=2^-550, all four leading prices
  are about 2.16492295972e−166 even though sigma² rounds to zero. For the
  deterministic fixed call S=K=M=2^1000, b=1, T=32 and D=exp(−32), the
  undiscounted terminal level overflows binary64 while the price fits at
  about 1.07150860719e301.
- Under deterministic zero yield, S=K=100 and a dated discount curve with
  D(0.5)=0.80 and D(1)=0.95 joined log-linearly, the full maximum is 125.
  Fixed call PV is 23.75 and floating put PV 18.75. Flattening the carry
  from D(1) produces 5 and 0 for the two payoffs. Terminal discount alone
  leaves the path extrema unspecified.

## Observed scope and input domain

Continuous-lookback call/put formulas with supplied historical maximum/minimum, constant carry/volatility. Dispatch validates positive spot/extremum; fixed strikes must be positive, floating strikes only finite and are unused.

## Observed choice and rationale

Closed-form reflected distributions price extrema and historical intrinsic. An analytic carry-zero branch avoids division by r−q using a 1e−7 switching threshold.

## Observed algorithm

Repair max to max(history,S) and min to min(history,S). Fixed call first discounts (max−K)+, then replaces strike with max(K,max) for the future maximum premium; fixed put analogously uses min(K,min). Add vanilla-like terms and reflected CDF terms scaled by σ²/[2(r−q)]. Use log-CDF products for image terms. For |r−q|<1e−7 substitute explicit normal-density/log-ratio limit formulas. Floating call uses the analogous minimum expression; floating put is fixed-call maximum premium plus max exp(−rT)−S exp(−qT). At expiry return observed-extremum intrinsic; zero vol updates extremum with deterministic terminal forward. Floor prices at zero.

## Observed selection and controls

Explicit fixed/floating selector; dispatch default constant is fixed. There is no numerical solver or user-selectable monitoring scheme. Raw functions check finiteness but not the full positive-domain constraints enforced in dispatch.

## Observed conventions

Prices are per underlying unit; history extrema share spot units. Continuous monitoring over the remaining interval is assumed; annual decimal carry/volatility and supplied year time.

## Observed limitations, failure and fallback

Silently repairing inconsistent historical extrema changes supplied state. Negative raw times/volatility take deterministic branches, while checked dispatch rejects them. No discrete fixing schedule, stochastic volatility or jump-extrema law is included.

## Alternatives and tradeoffs

Discrete-path MC can represent actual monitoring but introduces discretization and sampling error; it is not the formula selected here. The analytic carry-zero limit avoids numerical cancellation at a chosen threshold.

## Implementation and validation evidence

[lookback.rs](../../../../finstack-quant/models/src/closed_form/lookback.rs) and [dispatch.rs](../../../../finstack-quant/models/src/closed_form/dispatch.rs). `test_r_eq_q_limiting_branch_matches_general_formula`, boundary-continuity tests, seasoned-intrinsic cases and `fixed_strike_lookback_otm_matches_monte_carlo` provide limit/invariant/numerical-comparison evidence. [lookback_commodity_regressions.rs](../../../../finstack-quant/models/tests/lookback_commodity_regressions.rs) is additional regression evidence. [Haug](../../../REFERENCES.md#haug-2007-option-formulas) is the formula catalogue; table-level provenance for every branch is not established.

The [asset lookback pricer](../../../../finstack-quant/valuations/exotics/src/exotics/lookback_option/pricer.rs)
derives effective rates from the dated terminal discount, fills missing
continuous history with spot and sends the four formula calls separately.
Its terminal handling currently resolves spot even when a fixed payoff
can use a complete extremum. The [product type](../../../../finstack-quant/valuations/exotics/src/exotics/lookback_option/types.rs)
also uses the optional strike in volatility dependency metadata. The
[Python](../../../../finstack-quant-py/src/bindings/models/closed_form.rs)
and [WASM](../../../../finstack-quant-wasm/src/api/models/closed_form.rs)
adapters currently expose the dispatcher with a fixed default.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
