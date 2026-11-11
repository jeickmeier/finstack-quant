# MODELS-ANALYTIC-09: Heston probability inversion and bounded price acceptance

**Decision status:** Accepted — complete checked Heston Fourier price and bounded numerical acceptance package, 2026-11-10.

**Implementation status:** Observed. Reconciliation covers the scalar pricer and its actual dated and host callers below.

**Validation status:** Source, primary references and existing price fixtures inspected. Independent bounded calculations cover the law, selected prices and failure cases. Production and complete product qualification remain open.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for bounded Heston Fourier pricing

Accepted on **2026-11-10** after review of all 25 decisions.

1. **Price the accepted Heston European law.** Use the
   [checked Heston parameters and normalized cash/share contours](heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform)
   for fixed-strike European calls and puts. Return a discounted premium
   per underlying unit in the supplied spot/strike units. The dated
   owner applies position scale and cash conventions.

2. **Select one price entry point and one internal profile.** Use
   `heston_price(spot,strike,time,&params,option_type) -> Result<f64>`.
   Require the call/put choice in every language. Retire the public
   call-only alias, HestonFourierSettings, defaults and omitted-settings
   heuristic. Current dated and host callers already delegate grid
   selection. Adopt the fixed integration policy below for them.

3. **Validate the whole request first.** Require finite positive S and K,
   finite T>=0 in model years, and the checked pricing state from 08.
   Preserve the existing finite signed r and q and the strict flat wire
   fields. Reject negative time. Standalone discount factors and
   prepaid legs can exceed binary64 range. The requested price
   must fit and satisfy the stochastic resolution gates below.

4. **Evaluate the requested expiry payoff directly.** At exact T=0,
   return max(eta(S−K),0), eta=+1 for call and −1 for put.
   Use complete admission and the original S and K. The scalar zero
   horizon supplies no fixing, expiry-date or discharge evidence.

5. **Use the exact deterministic-variance price at zero vol-of-vol.**
   For sigma_v=0 and T>0 use the integrated variance I from 08 and
   total standard deviation sqrt(I) in the checked discounted Black
   law. Preserve its scaled weights and final payoff range.
   For every positive sigma_v use the stochastic transform. Feller
   status, very small positive parameters or failed integration
   supply no Gaussian price branch.

6. **Keep discounted legs in stable scale.** Define A=S exp(−qT),
   B=K exp(−rT), M=max(A,B) and m=log(A/B)=log(F/K). Work with
   a=A/M and b=B/M, so max(a,b)=1, and let L=min(a,b).
   Obtain signed m and a−b from original scaled log geometry and
   expm1 near equality. Preserve log M for final restoration.
   The price requires neither raw F nor A, B or M as standalone outputs.

7. **Invert the same two normalized contours.** With checked log
   transforms L_j(u) from 08, set
   f_j(u)=exp(Re L_j(u)) sin(Im L_j(u)+u m)/u.
   The upper probability is 1/2+∫_0^∞ f_j(u)du/π and the lower
   probability is 1/2−∫_0^∞ f_j(u)du/π. Use the upper side for an
   OTM call at m<=0 and the lower side for an OTM put at m>0.
   Compute each selected probability from its signed integral.

8. **Respect the removable origin.** The limit of f_j at zero is
   m+a_j I_j, where a_1=+1/2, a_2=−1/2 and I_j is the
   contour-specific expected integrated variance from 08's
   origin identity. The Gauss–Legendre nodes remain strictly
   positive. Use stable phase and sine-over-frequency evaluation
   at the actual nodes, including near the origin.

9. **Assemble OTM time value directly.** The normalized time value is
   Q=−|a−b|/2+∫_0^∞ [a f_1(u)−b f_2(u)]du/π.
   This is a P1−b P2 for the OTM call, or b Q2−a Q1 using
   the two lower probabilities for the OTM put. Restore the
   requested normalized price by adding max(eta(a−b),0) to
   the resolved time value. Avoid subtracting a rounded ITM
   premium to price a small OTM option.

10. **Preserve the complete node contribution.** Use checked log
    and scaled complex arithmetic through phase, the paired
    difference, quadrature weight and 1/u. Resolve close paired
    terms with stable differences. Accumulate the signed terms and
    their absolute masses with checked compensated arithmetic.
    A premature rounded zero supplies no weighted omission rule.
    A hard node or phase error propagates immediately.

11. **Use the stochastic payoff bounds.** At T>0 and sigma_v>0
    under the admitted open correlation range, integrated
    variance is positive and the conditional orthogonal Gaussian
    part gives full terminal log support. The exact selected
    probabilities lie strictly between zero and one and
    0<Q<L. The requested price is above intrinsic and below its
    appropriate discounted leg. The final rounded ITM result can
    equal displayed intrinsic after a resolved positive time value
    has been added. Never floor a failed stochastic time value.

12. **Bound the unseen tail by conditioning.** Under either contour
    condition on the variance-driver path. Its independent Gaussian
    component has variance (1−rho²)∫v. Consequently
    |exp(L_j(u))|<=E_j exp(−s(u)∫v), with s(u)=(1−rho²)u²/2.
    This integrated CIR Laplace expectation uses drift c−b_j v,
    c=kappa theta and b_j from 08, including b_1<=0.

13. **Use a simple analytic envelope.** Let
    beta_1=kappa+|rho|sigma_v, beta_2=kappa and
    G_j(U)=s[U](v0 T+c T²/2)/[1+T(beta_j+sigma_v U)].
    For u>=U>0 the modulus is at most exp(−G_j(U)u/U).
    To see this, the positive Laplace Riccati coefficient at time t
    is 2s(1−e^(−dt))/[d+b_j+(d−b_j)e^(−dt)] with
    d=sqrt(b_j²+2 sigma_v² s). Its denominator is at most 2d.
    The inequality 1−e^(−dt)>=dt/(1+dt) bounds the coefficient
    and its integral from below. Then use d<=beta_j+sigma_v u
    and monotonicity of G_j(u)/u. The probability tail after U
    is at most exp(−G_j(U))/(π G_j(U)).

14. **Make the cutoff bound conservative in binary64.** Enclose
    G_j(U) from below using outward scaled arithmetic on the
    positive factors and an upper denominator. For a resolved
    lower bound g_j>=1, use E_j=2^(−floor(g_j)) as a simple
    upper bound on the omitted probability tail. Keep the dyadic
    bounds and their upward sum E=E_1+E_2 in scaled form. Since
    a,b<=1, E also bounds the omitted normalized premium tail.
    Unresolved enclosure errors. This certifies truncation only.

15. **Select cutoff before refining mesh.** Start U=1 and double it
    until E<=2^-52, using at most 64 cutoff candidates. A candidate
    with g_j<1 is ineligible. The final-tenth sampled mass, sampled
    CF maximum, Gaussian seed and large-frequency slope no longer
    determine cutoff. An exhausted cutoff search errors.
    Once chosen, hold U fixed for the mesh comparisons.

16. **Choose an explicit initial resolution.** Let I_*=max(I_1,I_2)
    and Lambda=|m|+I_*/2+sqrt(I_*). Use N0=max(1,ceil(U Lambda)).
    The phased characteristic function is Lambda-Lipschitz because
    E_j|m+X_T|<=|m|+I_j/2+sqrt(I_j). Use this as an initial
    width guard. It supplies no quadrature error guarantee.
    Resolve the count in scaled arithmetic and reject an unrepresentable
    or over-budget grid before using it.

17. **Use one quadrature family.** Use composite order-16
    Gauss–Legendre from the [accepted core rules](../core-numerics/quadrature.md).
    Check finite, strictly increasing positive nodes and finite positive
    weights and preserve deterministic panel and node order. Double
    the panel count at fixed U between mesh attempts. No alternate
    integration family or stochastic pricing model is selected.

18. **Bound total work.** Cap the total across attempted grids at
    1,048,576 Gauss–Legendre nodes per request, counting actual
    samples with checked preflight and arithmetic. Each node uses
    two contour evaluations. Preflight the next full grid before
    allocation or sampling. The cutoff-search count is separate.
    An exhausted budget returns a convergence error.

19. **Require three consecutive eligible fixed-cutoff grids.** Each
    eligible estimate has both raw selected probabilities in [0,1]
    and finite normalized time value strictly in (0,L).
    Compute the mesh proxy as the larger of the last two consecutive
    absolute time-value differences. A finite estimate outside the
    raw domain resets the streak and can trigger finer same-cutoff
    work. Preserve the raw values for diagnostics. Apply no clamps.

20. **Gate resolution on time value.** For the finest grid let
    mass=(a+b)/2+Σ_i w_i[a|f_1(u_i)|+b|f_2(u_i)|]/π and set
    R=64×2^-52×mass. This is a fixed arithmetic-resolution guard.
    Require E+mesh_proxy+R<=1e−9 Q and E+mesh_proxy+R<L−Q.
    Apply checked scale arithmetic to these tests and restore the
    actual requested native premium only after acceptance.
    A lost positive requested value or unrepresentable result errors.
    Tail E is an analytic enclosure. The mesh and arithmetic
    terms are operational checks and give no certified price error.
    The baseline alone makes R>=32×2^-52. The profile therefore
    refuses Q below about 7.11e−6 of M, even with exact mesh
    agreement. At ATM the baseline raises that threshold to about
    1.42e−5 of M. This also affects ITM requests whose opposite
    time value is too small to qualify.

21. **Preserve actual failure causes.** Invalid terms use validation
    errors. Unresolved log-CF, geometry, arithmetic or output range
    uses computation errors with context. A finite unresolved
    mesh/price diagnostic can cause same-cutoff refinement.
    Exhaustion uses the existing convergence error category with
    the actual cutoff, work or price gate identified. Remove
    corrupt-node zeros, all-underflow status and silent skipped samples.

22. **Keep one production scalar path.** Stream the paired node
    evaluation in the scalar driver. Current production callers
    use a singleton. Retire the separate production strip constructor
    and cache. Keep independent references and test-only diagnostics
    for validation. A future genuine
    batch consumer can qualify its own reuse against this law.

23. **Reconcile the dated equity owner.** Keep qualified market
    parameters, deterministic carry/discount and model clock under
    the actual instrument and sources. Remove the unused Black-vol
    lookup from the Heston route. Resolve actual expiry/fixing and
    outstanding claims separately from collapsed live model time.
    Apply quantity once and preserve actual valuation errors.

24. **Reconcile the existing host price surfaces.** Use the same
    checked scalar price and required option type in Python and
    WASM, retaining the accepted parameter fields. Update exports,
    stubs, declarations, schemas, parity and examples. Bindings
    convert and map Rust errors. Host-default call selection and
    hidden alternate grid settings are retired.

25. **Qualify the separate numerical and dated gates.** Compare
    independent payoff and price references, including zero time,
    exact deterministic variance, short and long horizons, zero
    initial variance, both share-drift signs and wing resolution.
    Verify the conservative tail enclosure, phase/mesh failures,
    actual budget and raw-price gates separately. Existing
    same-kernel, same-method and external fixtures retain their
    distinct evidence scope.

## Independent bounded reference checks

- At T=0, S=2^54 and K=1, the exact put is zero. Rounded call intrinsic
  equals S, so the observed parity path computes 1 instead.
- For S=K=100, r=0.03, q=0.01 and T=1, independent inversion gives
  call/put 8.252848982578 / 6.292418962512 at
  (v0,kappa,theta,sigma_v,rho)=(0.04,2,0.04,0.5,−0.7).
  With (0,1.5,0.04,0.4,−0.5) it gives
  6.122205138776 / 4.161775118710. With (0.04,0.25,0.04,0.8,0.75)
  it gives 6.120320340709 / 4.159890320643, with a negative
  share-contour coefficient b_1. A standalone profile calculation
  using exact rational envelope inputs and independently generated
  Gauss–Legendre nodes agreed with these prices at U=2048, 2048
  and 8192, using 55,104, 38,752 and 255,248 cumulative nodes.
- At S=K=1, r=q=0, T=v0=theta=kappa=1, sigma_v=2^-40 and rho=0,
  the observed explicit grids (U,N)=(4000,1) and (8000,4) at
  order 16 start at about 21.20 and 10.60. They miss the origin
  lobe and both produce rounded zero with quiet sampled tails.
  Conditional Black and a variance-moment bound place the
  actual call within 2.57e−13 of 0.382924922548. The standalone
  profile resolves that price at U=16 with 2,688 cumulative nodes.
- With S=100, K=5, r=q=0, T=1, v0=theta=0.04, kappa=2,
  sigma_v=0.05 and rho=0, conditioning and moment bounds put
  the true stochastic put between 2.49e−101 and 2.641e−22.
  The exact rounded call is 95 and even perfect rounded
  call parity loses the put. The selected profile can refuse
  an unresolved wing.
- At sigma_v=0, S=K=1, r=q=−800, T=1 and v0=theta=2^-1074,
  the exact ATM premium is about 2.4176193159e185. The
  standalone discount overflows while the requested payoff fits.
- For the existing short-expiry case at S=K=100, r=0.05, q=0,
  T=0.0001 and (v0,kappa,theta,sigma_v,rho)=(0.04,2,0.04,0.3,−0.7),
  the standalone profile gives call 0.0800381498677 at U=8192
  with 1,904 cumulative nodes. The source comparator is a
  wider-grid calculation of the same inversion, a separate
  evidence class from external prices.

## Observed scope and input domain

European calls/puts, positive S and K, finite time (nonpositive time returns intrinsic after validation), constant Heston/rate inputs.

## Observed choice and rationale

Composite Gauss–Legendre integrates P1/P2 and caches CF values across strikes. Tail and raw-bound diagnostics precede price acceptance; repeated-grid stability is mandatory even for explicit settings.

## Observed algorithm

Compute Pj=1/2+π⁻¹∫Re[exp(−iφ ln K)ψj/(iφ)]dφ. Cache both ψj/(iφ); call=S exp(−qT)P1−K exp(−rT)P2; put via parity. Record absolute integrand mass and max CF magnitude in the last 10% of the integration interval. Reject any overflow node or all-underflow grid, tail CF>1e−8, price-scaled tail mass>1e−9 discounted S, or probability-clamp excursion beyond that tolerance. Check intrinsic/upper price bounds before clamping accepted roundoff. Repeat up to six grids, doubling upper limit and quadrupling panels until prices differ by≤1e−9 discounted S.

## Observed selection and controls

Explicit settings support GL orders 2,4,8,16, positive panels/upper limit and ≤1,048,576 nodes. Plain settings default 100/100/16. Omitted settings instead choose upper=max(80,√[2×27.631/(min(v0,θ)T)],27.631/c∞), c∞=√(1−ρ²)(v0+κθT)/σv, panels=ceil(upper), order16. Work-budget/overflow failure errors; no BS substitution.

## Observed conventions

CF integrates log spot, price uses continuous domestic discount and dividend carry. Settings are initial grids, not a promise that exactly those nodes will be used. Strip comparison requires all requested strikes to converge.

## Observed limitations, failure and fallback

Tail tests are finite diagnostics, not a mathematical tail bound. A difficult strike can make an entire strip fail; explicit coarse settings can be expanded. Probabilities and final prices are clamped only within tested tolerances, whereas parity puts additionally floor at zero.

## Alternatives and tradeoffs

Adaptive integrators/COS are possible alternative numerical inversions; no alternate Heston kernel is automatically selected. Cached strips trade memory for fewer CF evaluations.

## Implementation and validation evidence

[fourier_prices.rs](../../../../finstack-quant/models/src/closed_form/heston/fourier_prices.rs), [strip_pricer.rs](../../../../finstack-quant/models/src/closed_form/heston/strip_pricer.rs) and [params.rs](../../../../finstack-quant/models/src/closed_form/heston/params.rs). [tests.rs](../../../../finstack-quant/models/src/closed_form/heston/tests.rs) contains short-expiry reference numbers, unsupported-order/corruption/unresolved-grid rejection and scalar/strip consistency. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) uses separately generated QuantLib Heston values. Reference: [Heston](../../../REFERENCES.md#heston-1993).

The [equity Heston adapter](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/mod.rs)
currently calls the scalar model with omitted grid settings. Its
[input collector](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/inputs.rs)
shares an unused Black volatility lookup. The Python and WASM
price adapters also omit grid settings and currently default to call.
The direct-node diagnostic is test-only. For primary inversion
and conditional variance provenance, see the
[Fourier price references](../../../REFERENCES.md#heston-fourier-price-and-tail).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
