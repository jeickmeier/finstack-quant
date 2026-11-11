# MODELS-ANALYTIC-12: Checked rough Heston Riccati solve and Lewis pricing

**Decision status:** Accepted — complete checked rough Heston law, bounded fractional Riccati solve and separate Lewis acceptance package, 2026-11-10.

**Implementation status:** Observed. Runtime, equity caller, schema and generated-contract reconciliation remain open.

**Validation status:** Source, primary references and existing tests inspected. Standalone analytic and numerical checks cover selected limits, finite-H transforms and tail bounds. Complete finite-H pricing, production enclosure and product qualification remain open.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked rough Heston pricing

The complete 26-decision package was accepted on 2026-11-10.

1. **Select the existing fractional variance law.** Use the pricing-measure
   stock process with deterministic carry and variance V driven by the
   normalized kernel K_alpha(t)=t^(alpha−1)/Gamma(alpha), alpha=H+1/2.
   Its variance drift inside the convolution is kappa(theta−V), with
   diffusion coefficient sigma_v sqrt(V) and stock–variance driver
   correlation rho. Keep the strict Fourier domain of finite positive
   v0, kappa, theta and sigma_v, open rho in (−1,1) and open H in (0,1/2).
   The admitted law has a true forward martingale and the moment strip
   required by Lewis. Neither a classical Feller test nor kappa>rho sigma_v/2
   is an admission condition. The classical and zero-vol-of-vol laws are
   qualification limits. The broader classical parameter domain in
   [record 08](heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform)
   does not determine rough Fourier admission.

2. **Use the stated fractional model clock.** T is elapsed model years.
   v0 and theta are instantaneous variance rates. Kappa and sigma_v
   are coefficients of this normalized fractional kernel, each with
   dimension year^(−alpha) when variance is expressed per model year.
   Parameter estimates are specific to the kernel and model clock.
   Rates and continuous yield are finite signed decimal rates on the
   supplied scalar clock. Dated preparation and parameter sourcing
   remain with the product owner.

3. **Make parameter and control state immutable.** Keep the six flat
   `RoughHestonFourierParams` fields and validate them at construction
   and strict restoration. Preserve the original H and resolve both
   alpha and beta=1−alpha without using a rounded endpoint to change
   models. Require a validated `RoughHestonControls` with finite positive
   `price_abs_tol` in discounted premium per underlying unit and finite
   `price_rel_tol` in (0,1) relative to time value. Remove implicit
   numerical tolerances. Reject missing, duplicate and unknown wire fields.

4. **Keep one checked scalar price route.** `price_european` takes a
   market record with S, K, r, q, T and explicit call/put, plus the
   required controls, and returns `Result<f64>` in discounted premium
   per underlying unit. Use one private prepared law and solver behind
   it. Retire the public `FractionalRiccatiSolver` and arbitrary-complex
   `char_func` entry. The direct model helper stays Rust-only.

5. **Validate before the expiry payoff.** Require finite positive S and
   K, finite T>=0, finite r and q, valid model state and valid controls
   before branching. At exact T=0 return max(eta(S−K),0), eta=+1 for
   a call and −1 for a put, from original spot and strike. Negative
   time errors. Zero model time alone supplies no dated fixing or
   settlement evidence.

6. **Keep discounted payoff geometry in scale.** Let A=S exp(−qT),
   B=K exp(−rT), M=max(A,B), a=A/M, b=B/M, L=min(a,b),
   c=sqrt(ab), and m=log(A/B). Resolve m, c, signed a−b and log M
   from original data using stable scaled/log arithmetic and `expm1`
   near equal legs. A raw forward, discount, prepaid leg or product
   ab need not fit binary64 when the required expressions resolve.

7. **Use only the normalized Lewis half contour.** For X=log(S_T/F),
   z=1/2+iu and real u, use L_z=log E[exp(z X)] as the continuous
   affine exponent. With w=u−i/2, the Riccati equation is D=I^alpha R(D),
   D(0)=0, and R(d)=a_R+b_R d+c_R d² with
   a_R=−(u²+1/4)/2, b_R=rho sigma_v z−kappa and c_R=sigma_v²/2.
   The pure-real a_R avoids complex cancellation in z(z−1)/2.
   Carry is removed algebraically. Use the affine Volterra domain
   for open rho, including both signs of Re b_R.

8. **Keep the half-contour origin regular.** At u=0 and T>0 the
   transform is the real half moment E exp(X/2), which is positive
   and strictly below one under the admitted stochastic law.
   The Lewis denominator there is 1/4. Use the exact normalization
   and first exponential moment identities at z=0 and z=1 for
   qualification. They do not replace the half moment.

9. **Keep fully implicit product integration.** On N uniform time
   steps h=T/N use the piecewise-linear interpolant of R(D), including
   the unknown endpoint. For a fractional order gamma in (0,1), set
   w_gamma=h^gamma/Gamma(gamma+2). The endpoint weight is w_gamma.
   Interior lag l>=0 has weight w_gamma[(l+2)^(gamma+1)+l^(gamma+1)
   −2(l+1)^(gamma+1)]. At time t_(j+1), the weight of t_0 is
   w_gamma[j^(gamma+1)−(j−gamma)(j+1)^gamma]. For the solve take
   gamma=alpha. History H_j is the checked sum over R(D_0) through
   R(D_j), with interior lag j−k for D_k, 1<=k<=j.

10. **Evaluate the positive weights stably.** Use scaled h^gamma
    and the checked positive log-Gamma kernel. For interior lag zero,
    use 2 expm1(gamma log 2) inside the weight. At l>=1 use the
    positive even binomial series about l+1 with delta=1/(l+1)
    and exponent 1+gamma, preserving the leading factor gamma.
    Consecutive positive terms have ratio at most delta². Use at
    most 64 terms and require the geometric remainder within
    2^-54 of the positive accumulated sum. At j=0 the initial
    weight is gamma w_gamma. For j>=1 factor (j+1)^gamma and
    evaluate the remainder as
    j[−gamma log1pmx(1/j)+expm1mx(−gamma log1p(1/j))], where
    log1pmx(x)=log1p(x)−x and expm1mx(x)=expm1(x)−x.
    Reject unresolved weight or grid arithmetic.

11. **Choose the checked local implicit root.** For w=w_alpha,
    let ell=1−w b_R, q_R=w c_R, C_j=H_j+w a_R and
    chi=4 q_R C_j/ell². Require Re ell>0 and chi outside the real
    ray [1,infinity). Then D_(j+1)=(2 C_j/ell)/(1+sqrt(1−chi))
    uses the principal square root. As the quadratic coefficient
    varies from zero to q_R with ell and C_j fixed, 1−s chi
    avoids the nonpositive real cut for every s in [0,1]. This
    selects the root continuous from the linear endpoint. It
    yields exact zero for exact C_j=0. Resolve the ray test using
    scaled signs and use checked complex division and square root.
    A proven failed guard makes that time level ineligible and
    can trigger refinement.
    Unresolved arithmetic errors. A positive q_R cannot select
    a zero-vol-of-vol branch through underflow.

12. **Check the actual trajectory.** Require D and checked R values at
    all N+1 ordered time nodes. Use compensated real and imaginary
    history accumulation. For each endpoint, check the residual of
    D=C_j+w(b_R D+c_R D²) against 64×2^-52 times the sum of the
    absolute term magnitudes. A finite residual failure makes that
    time level ineligible. Hard arithmetic failures propagate with
    node and step context. Never fill a trajectory with NaN or replace
    an invalid node by a zero contribution.

13. **Assemble the affine exponent with both time integrals.** Use
    L_z=kappa theta I^1 D(T)+v0 I^beta D(T), beta=1−alpha.
    Use trapezoids for I^1 D and the same piecewise-linear weights
    of order beta for I^beta D. The D_0 contribution is zero.
    Carry beta explicitly near H=1/2. Preserve nonzero integrals
    and their coefficients in scale. The classical v0 D(T) coefficient
    is a limit identity.

14. **Evaluate weighted nodes in checked scale.** A computed positive
    Re L_z contradicts the half-contour modulus and makes that time
    level ineligible. Use log attenuation and a qualified phase for
    u m+Im L_z through the Lewis denominator, factor c and weight.
    Preserve usable small contributions through accumulation. Remove
    the real-exponent cutoff 700. Unresolved phase, range or scale
    errors propagate with their cause.

15. **Price the common OTM time value directly.** Let
    J(u)=Re[exp(L_z+i u m)]/(u²+1/4). The normalized time value is
    Q=L−(c/pi) integral_0^infinity J(u) du. The requested price is
    M[Q+max(eta(a−b),0)]. The inversion follows from
    min(A exp(X),B)=sqrt(A B) exp(X/2) exp(−|m+X|/2) and the
    Fourier transform of exp(−|x|/2). Use compensated signed and
    absolute weighted sums. Resolve Q before adding intrinsic.
    Positive-time stochastic log support is full under the open
    correlation domain, so exact
    Q lies strictly in (0,L). Preserve the raw signs and bounds.

16. **Bound continuous tail attenuation by comparison.** Put
    g=−Re D>=0, y=Im D, b_h=kappa−rho sigma_v/2 and
    s(u)=(1−rho²)u²/2. Completing the square gives
    D^alpha g=1/8+s(u)−b_h g−c_R g²+(sigma_v y+rho u)²/2.
    The positive linear Volterra comparison for differences gives
    g>=g_0, where D^alpha g_0=s−b_h g_0−c_R g_0² and g_0(0)=0.
    For u>0 the scalar solution stays between zero and its positive
    equilibrium r_*. With d=b_h+c_R r_*>0, the scalar right-hand
    side is at least s−d g_0. Linear comparison and the
    Mittag–Leffler bound E_alpha(−x)<=1/(1+x/Gamma(1+alpha)) yield
    g(t)>=s t^alpha/[Gamma(1+alpha)+d t^alpha]. This comparison
    applies for either sign of b_h.

17. **Compute a conservative cutoff enclosure.** For U>0 use
    B_T=max(T,sqrt(T)), C_T=T min(T,sqrt(T)),
    B_U=kappa+|rho| sigma_v/2+sigma_v U and
    G(U)=s[U](v0 T+kappa theta C_T/2)/[1+2 B_U B_T].
    Here d=(b_h+sqrt(b_h²+2 sigma_v² s))/2<=B_U,
    Gamma(1+alpha)>=1/2 and Gamma(2+alpha)<=2. Applying I^1 and
    I^beta to the preceding lower bound gives −Re L_z>=G(u).
    Since G(u)/u is increasing, the modulus for u>=U is at most
    exp(−G(U)u/U). Enclose 1−rho² from (1−rho)(1+rho). With
    outward scaled arithmetic obtain a positive
    lower bound g_U on G(U) and an upper bound c_up on c. Use
    E_U=c_up 2^(−floor(10 g_U/7))/(3 U g_U) as an upper bound on
    the omitted normalized premium. Round the exponent argument
    down before taking its floor. This uses pi>3, u²+1/4>=U²
    and log 2<7/10. Refuse unresolved enclosures.

18. **Select and, if needed, extend the cutoff.** Set tau_abs=
    price_abs_tol/M in scale. Start U=8 and double until E_U<=tau_abs/8,
    with at most 64 cutoff candidates across the request. Compare
    time and mesh only at a common cutoff. When the refinement
    rule in decision 23 extends the tail, double U and evaluate
    the new cutoff independently. Charge all earlier
    work to the same request.

19. **Integrate the entire finite contour with one rule.** Use composite
    order-16 Gauss–Legendre from the
    [accepted core rule](../core-numerics/quadrature.md). Begin with
    one panel in each of [0,1], [1,5] and [5,U]. Double the panel
    count in each segment on mesh refinement. Require ordered usable
    positive interior nodes and positive weights. Include the interval
    from zero, with no frequency hole or fixed upper limit 200.

20. **Check both discretization axes at fixed cutoff.** Begin with
    128 time steps and the initial panels above. Double time steps
    and panel counts independently. Form the last three consecutive
    levels on each axis, evaluating their nine common-cutoff pairs.
    Reuse overlapping scalar results while the cutoff is fixed.
    E_time is the largest of the adjacent time differences across
    those three meshes. E_mesh is the largest of the adjacent mesh
    differences across those three time levels. Finite raw values
    outside (0,L) may drive refinement. All nine must be within
    (0,L) at acceptance.

21. **Accept on the actual raw price and both requested tolerances.**
    At the finest pair let mass=L plus the sum of absolute normalized
    weighted Lewis terms and R=64×2^-52×mass. Require
    E_U+E_time+E_mesh+R<=min(tau_abs, price_rel_tol Q) and
    E_U+E_time+E_mesh+R<L−Q. The analytic enclosure covers the
    continuous omitted tail. The finite-grid differences and R are
    operational checks, with no certified total pricing error.
    Add intrinsic and restore the native price once. A positive
    premium that rounds to zero or exceeds the output range is a
    computation error. A resolved ITM premium can round to displayed
    intrinsic. The tolerances apply before final rounding.

22. **Bound cumulative time and integration work.** Cap a time level
    at 8,192 steps and the request at 65,536 evaluated frequency nodes
    and 2^32 combined history products and endpoint updates across
    all attempts.
    A full N-step node uses N(N+1)/2 history products including the
    initial contribution and N endpoint updates. Check counts, grid
    geometry and allocations before starting each grid. Stream one
    trajectory per node. Count actual work on incomplete attempts.
    Reuse checked weight tables only for the corresponding time grid.

23. **Make refinement and failure explicit.** A finite time-level
    diagnostic in decisions 11–14 advances the time axis until the
    stencil contains three eligible time levels. With finite stencil
    results, refine the axis of the larger discretization difference
    when a raw or numerical gate fails. Extend the cutoff instead
    when the tail is strictly larger than both discretization
    differences and every estimate is within E_U of [0,L].
    Use time before mesh on ties and retain the current time/panel
    levels as the starting stencil at a new cutoff. When the budget or
    a needed refinement is exhausted, return the existing convergence
    error with the failed gate and actual work. Invalid requests use
    validation errors. Hard node, enclosure, geometry or output
    failures propagate as computation errors. Use no price clamp,
    NaN sentinel or alternate pricing model.

24. **Keep implied volatility with the selected inverse owner.** Retire
    the unused rough-model `implied_vol` helper and its silent `None`.
    A caller that requests implied volatility uses the checked premium,
    explicit price units and
    [record 03 controls](implied-volatility-solvers.md#accepted-decision-explicit-bounded-model-inversion-and-paired-premium-acceptance).

25. **Reconcile the actual equity and schema consumers.** Pass the two
    required tolerances through `model_config.rough_heston_fourier`
    on the existing instrument override carrier. Keep them separate from
    the six rough market scalars. Reconcile the dated equity pricer,
    schemas, generated types, parity ledger and actual host product route.
    Propagate Rust failures. The equity owner retains the dated lifecycle,
    qualified model clock, carry, settlement, dividend admission and one
    quantity/currency conversion. Keep direct Fourier helpers Rust-only.
    Rough Monte Carlo retains its own discretization and parameter domain.

26. **Qualify the transform, price and work gates.** Use independent
    fractional-series or other finite-H transform references and a
    separate finite-H price reference. Check both signs of Re b_R,
    small time, near-endpoint H and rho, the normalized half moment,
    analytic tail enclosures, root and raw payoff bounds, axis-specific
    refinement, caps and range failures. Use fractional deterministic
    variance and the classical limit as distinct checks. Reconcile the
    existing equity, classical-limit and MC/Fourier comparison cases.
    Same-kernel comparisons establish limited consistency evidence.

## Independent bounded reference checks

- At unit ATM with a point-mass transform, the exact premium is zero.
  Truncating the raw Lewis integral at U=200 gives
  2 atan(1/400)/pi = 0.00159154611520 even with exact time integration.
  For Gaussian total variance 1e−12, the true Black premium is about
  3.98942280401e−7. Elementary bounds put the same finite-cutoff
  premium between 0.00159154611520 and 0.00159154614704. This is a
  limiting reference for the cutoff mechanism.
- For H=0.25, v0=0.04, kappa=0.8, theta=0.05, sigma_v=0.2,
  rho=−0.6 and T=0.3, an independent fractional power series gives
  L_z at u=2.5 about −0.039898590639+0.002546774559i. The standalone
  guarded product-integration error declines from about 1.35e−6 at
  64 steps to 3.47e−8 at 512. At u=0 the log half moment is
  about −0.001544900479.
- With H=0.25, v0=0.04, kappa=0.1, theta=0.05, sigma_v=0.8,
  rho=0.8 and T=0.2, Re b_R=0.22. At u=2.5 the fractional
  series gives about −0.024924161263−0.007290870093i. The same
  standalone solver's error declines from about 1.23e−6 to
  3.21e−8 over 64 to 512 steps.
- At zero vol-of-vol the limiting total variance is
  theta T+(v0−theta)T E_(alpha,2)(−kappa T^alpha). An independent
  series gives 0.0775758933652 for H=0.25, v0=0.09, theta=0.04,
  kappa=0.5 and T=1. The constant v0=theta case reduces to v0 T.
- For v0=theta=0.04, kappa=2, sigma_v=0.3, rho=−0.7, H=0.1
  and T=1, the selected coarse G(200) is about 6.51705. The
  corresponding dyadic unit-ATM tail bound is about 4.995e−7
  in ordinary floating-point arithmetic. The sharper mathematical
  comparison with the exact Gamma factors gives G about 13.88214.
  These standalone calculations do not qualify production enclosures.
- At unit ATM for the same H=0.1 parameters, a standalone
  implementation of the refinement profile passed its operational
  gates at absolute tolerance 1e−6 and relative tolerance 1e−4.
  It selected U=256 and reached 1,024 time steps and 16 panels in
  each of the three segments. The normalized estimate was about
  0.075750688498, with 5,808 cumulative nodes and 964,690,944
  history products and endpoint updates. This checks that this
  profile can finish within its caps in ordinary arithmetic.
- A 125-digit check of 32 weight calculations matched the literal
  formulas, including fractional order 2^-54 and lag 8,191.

## Observed scope and input domain

European calls/puts with positive Heston parameters and H in (0,.5), α=H+.5. At positive expiry the price function requires finite positive spot/strike and finite rate, yield and expiry. Before those checks, every t≤0—including negative infinity—returns undiscounted intrinsic provided spot and strike are finite; that branch permits negative spot/strike and does not inspect rate/yield. The public characteristic function instead rejects negative/nonfinite expiry and nonfinite frequency/rate/yield before returning one at t=0. Standalone fractional solver constructor does not validate step count/H/maturity.

## Observed choice and rationale

An implicit product-integration fractional Riccati solve feeds a fixed Lewis contour quadrature. Time refinement is tested; frequency truncation is fixed. This is distinct from classical Heston integration.

## Observed algorithm

Solve fractional D with forcing a=−(u²+iu)/2, b=iuρσv−κ,c=σv²/2. Precompute convolution weights ∝h^α/Γ(α+2) from differences of powers. Each step combines all history then solves the implicit quadratic with the rationalized root 2constant/[1−bweight+√((1−bweight)²−4cweight constant)]. Nonfinite trajectory fills remaining values with NaN. C=κθ∫D ds uses trapezoids; v0 multiplies a piecewise-linear fractional integral I^(1−α)D. CF adds iu(r−q)T. Lewis call integrates Re[exp((.5+iu)ln(F/K)) φdemeaned(u−i/2)]/(u²+.25), then C=discounted S−discounted K×integral/π.

## Observed selection and controls

Riccati steps start200, double to1600 for price and3200 for public CF. Acceptance requires successive difference≤1e−6×max(S exp(−qT), K exp(−rT)) for prices or absolute1e−6 for CF, and call bounds within tolerance. Both spot and strike are discounted in the price stopping scale. GL order16 with one panel on [1e−8,1], one on [1,5],16 on [5,200]. Exponent real>700 rejects. Put via parity; IV uses shared Black inversion and maps failure to None.

## Observed conventions

H is dimensionless roughness, σv and variance inputs follow Heston convention; time in years. Rates contribute deterministic drift/discount only. Fourier upper limit200 is not chosen from maturity/variance.

## Observed limitations, failure and fallback

Failed convergence returns NaN, not Result; accepted prices may retain tiny bound excursions (no clamp). Refinement compares the same quadrature/tail cutoff, so it cannot measure omitted frequency-tail error. O(N²) history work has fixed caps. Classical H=.5 is excluded rather than dispatched to classical Heston.

## Alternatives and tradeoffs

Classical Heston is an implemented separate alternative. Adaptive contour/tail integration, fast convolution or a Markovian approximation are unimplemented here and require independent tradeoff decisions.

## Implementation and validation evidence

[rough_heston.rs](../../../../finstack-quant/models/src/volatility/rough_heston.rs). `stiff_rough_heston_price_matches_refined_native_solver` compares against the same numerical kernel; deterministic-variance limit is an analytic Gaussian/Black check. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) has classical-limit comparisons at H=.499, not independent finite-H rough-Heston references. Reference: [El Euch–Rosenbaum](../../../REFERENCES.md#el-euch-rosenbaum-2019).

The [equity rough Fourier pricer](../../../../finstack-quant/valuations/equity/src/equity_option/rough_heston_fourier_pricer.rs)
consumes the [six market parameters](../../../../finstack-quant/valuations/equity/src/equity_option/rough_heston_market.rs).
The [instrument override carrier](../../../../finstack-quant/valuations/core/src/instruments/pricing_overrides.rs)
owns numerical configuration at that boundary. The
[parity ledger](../../../../finstack-quant-py/parity_contract.toml)
excludes standalone rough model helpers. See the
[rough Heston pricing references](../../../REFERENCES.md#rough-heston-lewis-and-tail-comparison)
for the selected transform, payoff inversion and tail-comparison derivation.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
