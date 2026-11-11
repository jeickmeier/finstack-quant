# MODELS-ANALYTIC-10: COS bounded payoffs, window and series acceptance

**Decision status:** Accepted — complete checked COS pricing package with separate window and series bounds, 2026-11-10.

**Implementation status:** Observed. Runtime, caller, schema and host reconciliation remain open.

**Validation status:** Source, primary references and existing tests inspected. Independent bounded calculations cover selected exact prices, failure cases and the accepted error decomposition. Production qualification remains open.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked COS pricing

The complete 24-decision package was accepted on 2026-11-10.

1. **Keep one checked COS engine behind the actual scalar calls.**
   Retain bs_cos_price, merton_jump_cos_price and vg_cos_price across
   Rust, Python and WASM. These price European calls and puts per
   underlying unit. Use one internal typed model path. Retire public
   generic CosPricer, arbitrary CF pricing callbacks and the strip
   cache. The current production callers are scalar. Model parameter
   and transform law details remain with
   [record 11](jump-characteristic-laws.md).

2. **Make numerical choices explicit.** Require validated immutable
   CosConfig with initial_terms N0 in [2,65536], finite positive
   initial_l and finite positive price_abs_tol in native discounted
   premium units per underlying unit, applied before final native
   rounding. N0 and L are starting search controls. Remove Default
   and the optional market n_terms override. Each scalar takes a
   required final config argument, borrowed in Rust and supplied as
   the corresponding config object in each host.

3. **Validate the complete request.** Require finite positive S and K,
   finite T>=0, finite signed r and q, an explicit call/put and an
   admitted model and config. Restore strict state with required
   fields and unknown/duplicate-field rejection. The model owner
   qualifies the variance and discount clocks. Validate before
   handling exact time, deterministic or support cases.

4. **Normalize the same martingale law.** Set F=S exp((r−q)T),
   X=log(S_T/F), m=log(K/F), A=S exp(−qT) and B=K exp(−rT).
   Under the cash measure E[exp(X)]=1. Use the model's qualified
   first-moment identity. Retire unchecked extraction of the real
   part of a complex value at −i. The normalized transform and
   cumulants describe log return relative to F.

5. **Price the requested bounded payoff directly.** For a put
   use Y=X in the cash measure, kink k=m, and scale M=B.
   For a call use Y=−X in the share measure with terminal density
   exp(X), kink k=−m, and scale M=A. Both evaluate
   M E[(1−exp(Y−k))^+]. The call's real-frequency transform is the
   normalized cash transform at −u−i. The model law supplies the
   qualified selected contour and its own cumulants. This is a
   measure change of the same law. The direct price also covers ITM
   requests without recovering an opposite-side time value.

6. **Preserve the original payoff and price scale.** Compute m from
   stable original S/K log geometry and total carry. Keep M, discount
   and requested scale in checked scaled/log arithmetic. A standalone
   forward, moment, discount or prepaid leg need not fit when the
   requested price is resolved. Discount once and restore native
   units after numerical acceptance.

7. **Handle exact point-mass cases analytically.** At T=0 use
   direct requested spot intrinsic. At positive T, a qualified
   deterministic model gives discounted forward intrinsic. Zero
   Black diffusion and genuinely inactive Merton randomness are
   examples. A small positive variance, failed transform or
   underflowed moment cannot select a deterministic branch.

8. **Distinguish structural support from a finite window.** Use exact
   zero or discounted intrinsic when the qualified terminal support
   establishes a zero payoff or an always-in-the-money linear payoff.
   An active zero-diffusion Merton law can have a no-jump atom. With
   fixed jump size it can have one-sided lattice support. The model
   owner supplies the support facts. A zero coefficient or empty
   interval intersection supplies no full-law support claim.

9. **Use cumulants for initial width only.** For the selected Y
   law use center mu=c1 and half-width h=L sqrt(c2+sqrt(c4)).
   The selected stochastic built-ins have positive variance and
   nonnegative fourth cumulant. Preserve positive small widths in
   scaled geometry and remove the 1e−12 radicand floor. Unresolved
   geometry errors. Cumulants supply a seed. The independent
   omitted-mass gate decides acceptance.

10. **Keep the basis in centered coordinates.** Set W=2h and
    a=mu−h mathematically. For n>=0 use u_n=nπ/W and
    C_n=Re[phi_(Y−mu)(u_n) i^n]. Use the exact four-cycle for i^n.
    Keep W and the kink relative to the center without subtracting
    rounded shifted endpoints or cancelling a large location phase.
    Require resolved positive width and distinct usable frequencies.

11. **Clip the payoff integral in its own coordinate.** Let
    ell=min(W,max(k−a,0)) and t=a+ell−k. Define
    V_n=(2/W)∫_0^ell [1−exp(a+x−k)] cos(u_n x) dx.
    This is the integration intersection. A full intersection uses
    exact endpoint phase nπ. An empty intersection has zero
    coefficients. The nonstructural window search requires positive
    payoff intersection before evaluation.

12. **Use stable analytic coefficients.** For ell>0, the unnormalized
    zero-mode integral is
    ell[−expm1(t)]+exp[t](ell+expm1(−ell)).
    For n>0 set z=u_n ell. The unnormalized integral is
    exp[t](ell sinc(z)+expm1(−ell)+2 sin²(z/2))/(1+u_n²).
    The boundary term vanishes at the payoff kink or at full width.
    Evaluate the numerator through stable small remainders
    [ell+expm1(−ell)] + ell[sinc(z)−1] + 2 sin²(z/2), and
    evaluate the denominator in checked scale.
    Handle ell=0 before exponentials. Apply 2/W and the half weight
    at n=0 once. The price estimate is the compensated sum of C_n V_n.

13. **Bound the window error as reflected payoff error.** Extend
    the bounded payoff by even reflection about the interval endpoints
    and period 2W. Its cosine values agree with the full-law basis
    everywhere. The absolutely convergent infinite payoff series
    therefore prices the reflected payoff. Its difference from the
    true bounded payoff is zero inside the interval and at most
    one in magnitude outside. The absolute normalized error is
    at most P(|Y−mu|>h). This reasoning also covers atoms.

14. **Select an admissible two-sided moment probe.** Let
    K_c(p)=log E[exp(p(Y−mu))] for the selected measure and center.
    Start p=1/sqrt(c2) and halve, using at most 64 candidates, until
    both +p and −p lie in the model's qualified finite moment domain.
    Evaluate the admitted centered log moments with outward bounds.
    Unknown or exhausted domain, or unresolved admitted moment
    arithmetic, returns an explicit error.

15. **Choose the window with a separate tail budget.** Let
    tau=price_abs_tol/M in scaled units. Bound the omitted
    probability by E_W=exp(K_c(p)−p h)+exp(K_c(−p)−p h).
    Starting at initial_l, double L over at most 64 candidate
    windows until the conservative E_W<=tau/4 and the payoff
    intersection is positive. Use the chosen actual width for
    every later spectral calculation. Exhaustion is a convergence
    failure.

16. **Bound the omitted cosine terms separately.** Integration by
    parts twice gives |V_n|<=4W/(π² n²) for n>=1 because the
    endpoint derivative magnitudes and the variation of the
    payoff derivative sum to at most two. With a decreasing
    CF modulus envelope H, terms n>=N contribute at most
    4W H(u_N)/(π²(N−1)). Use the conservative arithmetic form
    E_N=4W H(3N/W)/(9(N−1)), since π>3.

17. **Use envelopes tied to the admitted law.** For Black–Scholes
    and Merton use H(u)=exp(−sigma² T u²/2) from the Brownian part,
    including H=1 at zero diffusion. For VG let s=0 for cash
    or 1 for share, d_s=1−nu(theta s+sigma² s²/2)>0, and use
    H(u)=(1+nu sigma² u²/(2d_s))^(−T/nu).
    This bounds the modulus using the real part of the tilted
    denominator. Reflection preserves modulus. The model owner
    admits the underlying parameter and tilted moment law.

18. **Keep the two numerical enclosures conservative.** Use
    outward scaled arithmetic and qualified bounds on any
    elementary log or exponential in the estimates. A Chernoff
    exponent whose upper bound is nonnegative can use the
    probability upper bound one. For negative exponents a dyadic
    upper bound exp(−x)<=2^(−floor(x)) for x>=0 is sufficient.
    For the VG modulus, if the base is bounded below by 2^j for
    integer j>=0, use H<=2^(−floor(j T/nu)) with a lower bound
    on j T/nu. Keep bounds positive in scaled form. Return an
    error if the required enclosure is unresolved.
    These bounds address the exact-real window and series errors.

19. **Choose terms at fixed width before evaluating the series.**
    Start at N0 and double N at fixed W, including a final
    candidate of 65,536 if doubling would cross it. Select the
    first N with conservative E_N<=tau/4. Fail on exhaustion.
    The window search and term search require no intermediate
    price or spectrum evaluations. Evaluate one final spectrum
    of at most 65,536 checked CF coefficients and payoff terms.
    Preflight sizes, counts and numerical node geometry.

20. **Gate the raw price on its own scale.** Let p_hat be the
    normalized sum and mass the sum of absolute weighted terms.
    Use the arithmetic-resolution guard R=64×2^-52×mass.
    Require E_W+E_N+R<=tau and E_W+E_N+R<p_hat for
    a nonstructural stochastic result. The raw estimate must lie
    between normalized discounted intrinsic and the normalized
    upper bound one. Restore the finite native price with no
    clipping. A positive premium, including an analytic branch,
    that rounds to zero or exceeds the output range returns a
    computation error. Resolved rounding may reach a positive
    payoff bound. The tolerance gates the estimate before final
    native rounding. The arithmetic guard is operational and
    supplies no certified total floating-point error.

21. **Preserve explicit failure and bounded scope.** Invalid model,
    market or control inputs use validation errors. Hard transform,
    coefficient, range or enclosure failures use computation errors
    with context. Exhausted window/term search or unresolved raw
    price uses the existing convergence category. Remove negative
    floors and warning-only recovery. Slow-decaying VG and
    atomic Merton can exhaust the selected window or series work.
    No alternate pricing algorithm or default tolerance is selected.

22. **Keep parity and model limits as qualification identities.**
    Check C−P=A−B and the matching Black/deterministic reductions
    on independently calculated prices. The scalar directly
    prices its selected side. Near-boundary or deep-wing failure
    is a numerical outcome under this profile.

23. **Reconcile the actual hosts and examples.** Use identical
    Rust, Python and WASM market, model and required config
    semantics. Propagate typed errors and update schema, exports,
    parity, declarations and notebook calls that set n_terms.
    Hosts supply explicit controls and conversion only.
    Valuations, calibration and Heston Fourier pricing retain
    their separate owners.

24. **Qualify both bounds and price behavior separately.** Compare
    independent analytic Black, Poisson-mixture and VG payoff
    references, including exact singular/support cases, the
    reflected call, narrow geometry and tiny prices. Check
    omitted-tail and series estimates, different widths and
    resolutions, work-cap refusal and host/error parity.
    Existing finite-N comparisons and scalar/strip consistency
    retain their limited evidence meaning.

## Independent bounded reference checks

- With S=K=1, r=q=0, T=1, sigma=0.2 and L=10, the old N=1
  ATM estimate is about 0.288163866270, within ordinary price
  bounds. The analytic Black price is 0.079655674554.
- With S=1, K=exp(−2.1), r=q=0, T=1 and sigma=0.2, the old
  cash window is [−2.02,1.98] and excludes the put region at
  every N. The actual put is about 2.81495535694e−28.
  With S=K=1 and sigma=1e−7, the actual ATM price is about
  3.98942280401e−8 despite the old tiny-radicand rejection.
- For Merton S=1e6, K=1e5, r=q=0, T=1, sigma=sigma_jump=0,
  lambda=1e−6 and mu_jump=−4, the old window excludes the put.
  The independent Poisson sum gives about 0.0816843112790.
  With initial L=10, the proposed moment probe gives a bound
  too loose for window acceptance at tolerance 1e−8. An atomic
  Merton example with S=K=1, the same rates, time and zero
  volatilities, lambda=0.04 and mu_jump=−1 passes the window
  gate at L=20 for tolerance 1e−8. Its series bound at
  N=65,536 is about 1.329e−4.
- With Merton S=100, K=110, r=0.03, q=0.01, T=1, sigma=0.2,
  lambda=0.4, mu_jump=−0.1 and sigma_jump=0.25, independent
  conditional Gaussian integration gives call/put
  6.612743286008 / 14.356768601427.
- With VG S=1, r=q=0, sigma=0.5, theta=−0.125 and nu=T=4,
  the normalized cash return is the difference of independent
  exponentials of rates two and one. Direct payoff integration
  gives call/put (1/3,1/3) at K=1, (1/6,7/6) at K=2 and
  (7/12,1/12) at K=0.5. These are independent of COS inversion.
- Standalone binary64 calculations of the proposed series matched
  the Black ATM and narrow Black ATM references at tolerances
  1e−10 and 1e−12, respectively. Smooth Merton and all six VG
  premiums matched at tolerance 1e−8. The VG cases required
  32,768 or 65,536 terms. These checks
  used ordinary floating-point estimates of the bounds. Production
  outward enclosures and complete price accuracy remain open.
- At S=K=2^-1074 with r=q=0, T=1 and sigma=0.2, the positive
  normalized Black ATM price is about 0.079655674554. Restoring
  its native price to binary64 rounds to zero, so the checked
  output contract requires a range error.

## Observed scope and input domain

European options with a user-supplied log-return characteristic function/cumulants; positive S,K,T and finite discount rate. Wrappers expose BS, Merton jump and variance-gamma models.

## Observed choice and rationale

Fang–Oosterlee cosine expansion prices puts directly and calls by the CF forward moment. This avoids exponentially growing call-payoff coefficients; the selected finite expansion is not automatically convergence-certified.

## Observed algorithm

Window [a,b]=c1±L√(c2+√|c4|); reject radicand≤1e−12 or invalid interval. Cache Ak=Re[φ(kπ/(b−a))exp(−ikπa/(b−a))]. Shift window by ln(S/K), integrate put payoff over [aX,clamp(0,aX,bX)] using analytic χ (exp×cos) and ψ (cos) integrals. Sum with half-weight k=0 and factor 2/(b−a), Neumaier accumulation, then multiply K exp(−rT). Call=put+df[S Reφ(−i)−K]. Validate finite CF evaluations/forward moment/discount/output.

## Observed selection and controls

Default N=128,L=10; N in 1..65536, L finite positive. Wrapper n_terms overrides N; generic `CosPricer` exposes both. Scalar delegates to strip. There is no N/L refinement loop. BS wrapper requires strictly positive σ even when deterministic pricing could be calculated separately.

## Observed conventions

CF is for log return, not log price; its drift must match the separately supplied discount rate and carry. Cumulants are at the requested maturity. Unit premiums are discounted once.

## Observed limitations, failure and fallback

All finite negative prices are clamped to zero; materially negative put raw values trigger warning above K×1e−10+1e−12, but are still accepted as zero. Generic CF contract does not enforce φ(0)=1, martingale matching or positive-definite law. Near-deterministic distributions reject instead of falling back.

## Alternatives and tradeoffs

Numerical Fourier quadrature is implemented for Heston in a separate API. Increasing N or L trades accuracy/cost but can require joint tuning; adaptive bounds would be an unimplemented policy.

## Implementation and validation evidence

[cos.rs](../../../../finstack-quant/models/src/fourier/cos.rs) and [mod.rs](../../../../finstack-quant/models/src/fourier/characteristic_function/mod.rs). Inline `cos_matches_bs_call_itm_otm`, `cos_put_matches_bs` and strip/single tests compare against library BS; invalid-input/degenerate-window checks are invariants. Reference: [Fang–Oosterlee](../../../REFERENCES.md#fang-oosterlee-2008).

The live [Python](../../../../finstack-quant-py/src/bindings/models/fourier.rs)
and [WASM](../../../../finstack-quant-wasm/src/api/models/fourier.rs) wrappers
call the three scalar variants. The
[notebook](../../../../finstack-quant-py/examples/notebooks/02_pricing/instruments/equity_and_options.ipynb)
uses the existing explicit n_terms field. The
[COS references](../../../REFERENCES.md#cos-payoff-window-and-series)
distinguish finite series, window and coefficient errors.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
