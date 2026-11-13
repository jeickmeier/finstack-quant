# MODELS-NUMERICAL-017: Black–Scholes and Heston coefficient bridges

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Independent coefficient, range, deterministic price, clock, boundary and stochastic Heston comparator checks executed. Production PDE pricing, risk and host qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked PDE coefficient bridges

1. **Use validated immutable bridge parameters.** Admit an explicit
   call or put, finite positive strike, and finite signed Continuous
   discount rate \(r\) and dividend yield \(q\) in annual decimal
   units on the admitted model clock. The Black–Scholes input
   \(\sigma\) is finite nonnegative annual lognormal volatility.
   Replace public mutable fields and unchecked struct construction
   with checked admission. The product supplies the actual spot,
   currency, time and grid. Validate each required value and
   coordinate before using it.
   [Checked spot model](../models-analytic/spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results)

2. **Reuse the accepted Heston parameter domain.** Require finite
   \(v_0\ge0\), \(\kappa>0\), long-run variance
   \(\theta_v>0\), vol-of-vol \(\sigma_v\ge0\), and
   \(-1<\rho<1\). Variance values are annual variance,
   \(\kappa\) is inverse model years, and \(\sigma_v\)
   scales \(\sqrt v\,dW_v\) on that model clock. Reuse the
   checked risk-neutral parameter type and inclusive Feller
   diagnostic \(2\kappa\theta_v\ge\sigma_v^2\). Feller
   violation is admitted. Every required variance coordinate is
   finite and nonnegative. Remove the positive-part variance
   substitution. Normalize a represented zero state to zero.
   [Heston parameter and model law](../models-analytic/heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform)

3. **Map the Black–Scholes generator on log spot.** In the
   backward-time convention of 012, let \(x=\log S\) and
   use
   \[
   a=\sigma^2/2,\qquad b=r-q-\sigma^2/2,\qquad c=-r,\qquad f=0 .
   \]
   Evaluate complete coefficients with checked scale. Preserve
   signed carry and reaction, exact zero diffusion and small
   positive diffusion when representable. A required positive
   coefficient that cannot be resolved is a range error.
   [Model equations](../../../REFERENCES.md#checked-pde-coefficient-bridges)

4. **Map the Heston generator on log spot and variance.** For
   \(x=\log S,y=v\) and the full mixed multiplier \(m\),
   use
   \[
   a_x=v/2,\qquad a_y=v\sigma_v^2/2,\qquad m=\rho\sigma_v v,
   \]
   \[
   b_x=r-q-v/2,\qquad b_y=\kappa(\theta_v-v),\qquad c=-r,\qquad f=0 .
   \]
   Evaluate drift from the model expressions. Form one coherent
   coefficient sample at each required node. Exact zero variance
   gives zero diffusion and mixed coefficient. Exact zero
   vol-of-vol gives zero variance diffusion and mixed coefficient.
   A zero correlation gives zero mixed coefficient. Preserve
   nonzero effects through complete scaled products.
   [Heston generator](../../../REFERENCES.md#checked-pde-coefficient-bridges)

5. **Construct a positive semidefinite represented diffusion.**
   From the exact model products of the admitted binary64
   inputs, round the nonnegative diagonals upward and the
   signed mixed coefficient toward zero. Preserve exact zeros.
   Each required nonzero magnitude must fit between the least
   positive subnormal and largest finite binary64 values. Use
   checked scale and rounding to produce the chosen coefficients
   without an intermediate overflow, underflow or rounded
   detached square. This representation preserves
   \(\hat m^2\le4\hat a_x\hat a_y\) and the supplied
   correlation input and mixed-term sign. Enforce 012's exact
   represented gate after construction. The bridge defines
   the coefficient rounding once. Generic admission continues
   to reject indefinite supplied matrices.
   [Exact represented diffusion admission](pde-spatial-stencils.md#accepted-decision-for-checked-directional-and-mixed-pde-stencils)

6. **Use one checked vanilla payoff expression.** On admitted
   finite log-spot nodes return \((e^x-K)^+\) for calls
   and \((K-e^x)^+\) for puts. Use stable log, exponential
   and difference arithmetic for the complete requested payoff.
   Preserve an exact out-of-the-money zero and a representable
   difference when an intermediate spot is out of range.
   A match with rounded \(\log K\) alone cannot establish a
   zero payoff. Reuse the checked terminal payoff expression
   for the supported nodal American exercise profile. At actual
   spot use the original \(S,K\) comparison from 015.

7. **Keep explicit vanilla spot-edge laws.** Calls use zero
   Dirichlet at the lower spot edge and LinearInExp at the
   upper edge. Puts use LinearInExp at the lower edge and zero
   Dirichlet at the upper edge. The exponential law is affine
   in spot on the boundary triple. These laws approximate the
   far fields at finite truncation. Admit both query spot and
   strike strictly within the prepared live vanilla spot domain
   so the selected edges straddle the payoff kink. A caller
   prepares the required domain or returns a domain error.
   Keep barrier-specific closure, rebate and observation laws
   with the barrier. Its Black–Scholes vanilla parity leg
   qualifies the vanilla edge roles while preserving required
   shared knots.
   [Boundary laws](pde-boundary-elimination.md#accepted-decision-for-checked-pde-boundary-elimination-and-reconstruction)

8. **Keep Heston variance-edge closure explicit.** Use an axis
   starting at exact zero, a finite upper bound greater than
   the actual \(v_0\), and enough interior nodes for Linear
   extrapolation at both variance edges. Use the 013 x-edge
   corner rule. Admit the query at \(v_0=0\). At that edge
   the limiting generator is
   \((r-q)U_x+\kappa\theta_v U_v-rU\). The chosen
   Linear reconstruction is a local numerical closure. It
   does not make zero variance absorbing or integrate the
   limiting boundary PDE. Qualify zero-start and reachable-zero
   prices independently, including Feller violation. The upper
   edge also needs separate truncation evidence.
   [Variance boundary reference](../../../REFERENCES.md#checked-pde-coefficient-bridges)

9. **Keep prepared geometry and homogeneous use with their
   owners.** Apply the accepted spatial and time-grid rules,
   checked extraction and solver profiles. Use the actual
   admitted \((\log S,v_0)\) or \(\log S\) for the
   query. The owner selects axis extents and concentration
   and qualifies truncation and coverage for both edge laws.
   Any Heston moment-based domain sizing uses the checked
   integrated mean and scaled variance moments. Treat its
   margins as grid heuristics. Declare time homogeneity for
   these constant-input bridges only within their bound
   model and grid. Preserve 013's edge-state reuse rules.
   [Closed-domain extraction](pde-grids-interpolation-and-greeks.md#accepted-decision-for-checked-pde-grids-interpolation-and-coordinate-greeks),
   [1D time march](theta-time-march-and-thomas.md#accepted-decision-for-checked-theta-marching-and-tridiagonal-solves),
   [2D time march](mcs-adi-time-march.md#accepted-decision-for-checked-modified-craigsneyd-adi)

10. **Resolve the actual dated lifecycle and positive model
    horizon.** The equity owner resolves observed exercise,
    expiry and outstanding settlement before live PDE
    preparation. A still-live contractual horizon that
    collapses to zero or negative on the selected model clock
    fails this profile. A direct numeric zero-time payoff is
    a scalar identity after admission, without dated
    settlement meaning. The PDE march requires positive
    finite time. Preserve the curve-native date-to-date
    discount \(D>0\) through the checked effective rate
    \(r=-\log D/T\) on the model clock. This flat equivalent
    matches the terminal discount. Intermediate carry and
    American exercise remain qualifications of the flat
    approximation. Use the model clock for declared \(q\).
    [Financial clocks](../conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility)

11. **Resolve volatility from the selected model's inputs.**
    For Black–Scholes use the qualified active quote and
    source clock. Where the owner admits the variance bridge
    over the same physical horizon, preserve total variance
    through \(\sigma=\sigma_{\rm src}
    \sqrt{T_{\rm src}/T}\). Heston PDE reads
    `HESTON_KAPPA`, `HESTON_THETA`, `HESTON_SIGMA_V`,
    `HESTON_RHO` and `HESTON_V0` as required unitless
    scalars through its canonical pricing-parameter path.
    Remove the unused
    Black implied volatility lookup and require Heston
    dynamics on the declared PDE model clock. There is no
    inferred time rescaling of Heston process parameters.
    Dividend sources, any qualified European escrowed spot
    preparation and exact dated conventions stay with the
    equity adapter.
    [Equity PDE adapter](../valuations-equity-and-composition/09-equity-pde.md)

12. **Use the exact Black–Scholes zero-volatility branches.**
    At \(\sigma=0\) use the checked deterministic
    discounted payoff for European exercise. For the live
    American constant-carry profile, take the maximum of
    that payoff over time zero, expiry and any interior
    stationary time. For nonzero \(r,q\) of the same sign
    with \(r\ne q\), the candidate is
    \[
    t_* = \frac{\log(|r|K)-\log(|q|S)}{r-q}.
    \]
    Use it only when it lies strictly inside the admitted
    exercise horizon. Otherwise use the endpoints. Evaluate
    the signed log ratio and discounted leg differences at
    the scale of the full answer. Treat identical rates and
    true zero cases by their literal limiting expressions.
    Positive-volatility American pricing uses the grid
    exercise and actual-spot rules of
    [015](pde-exercise-projection.md#accepted-decision-for-checked-grid-based-pde-exercise).

13. **Use the exact Heston deterministic-variance branch.**
    At \(\sigma_v=0\) the European product uses the
    accepted integrated deterministic variance and the
    checked discounted Black price with total standard
    deviation \(\sqrt I\). Reuse the nonnegative weights
    \(I=T[v_0 A(\kappa T)+\theta_v(1-A(\kappa T))]\)
    with \(A(z)=(1-e^{-z})/z\), evaluating the small
    complementary weight directly. This also supplies the
    mean integrated variance for a stochastic domain
    heuristic. Positive vol-of-vol selects the stochastic
    PDE, including when \(v_0=0\). Zero is an explicit
    parameter case. Failed geometry or stochastic solves
    propagate their errors.
    [Deterministic Heston pricing](../models-analytic/heston-fourier-integration.md#accepted-decision-for-bounded-heston-fourier-pricing)

14. **Check the unit price against its financial bounds.**
    For the effective European model, let
    \(A=S e^{-qT}\) and \(B=K e^{-rT}\). Require a
    finite call in \([(A-B)^+,A]\) or put in
    \([(B-A)^+,B]\). For the supported American model use
    the greater of the eligible immediate payoff and the
    European lower bound, and upper bound
    \(S e^{\max(-qT,0)}\) for calls or
    \(K e^{\max(-rT,0)}\) for puts. Use checked complete
    comparisons and stable leg differences. Apply 015's
    immediate exercise choice before this gate. Report a
    resolved violation as a pricing error. Retire the Heston
    clamp and its absolute tolerance floor. Apply signed
    quantity and convert to Money after the per-unit checks.
    Financial bounds alone supply no accuracy or stability
    certificate.

15. **Reconcile existing model and risk capabilities.** Keep
    the existing equity PDE pricing keys, 1D European and
    qualified American support, and 2D European Heston
    support without future cash dividends. The 1D product
    keeps its existing live Bermudan and American future
    cash-dividend exclusions. Preserve existing selected-model
    bump-and-reprice conventions with checked prices and
    actual lifecycle and eligibility. For valid live Heston
    requests, sensitivities defined only by bumps to Black
    volatility are zero when Heston inputs are held fixed.
    They require no unrelated surface. Changes to Heston
    parameters or calibration are distinct risk measures.
    Keep bridge state in Rust and propagate errors through
    existing Python and WASM pricing routes.

16. **Qualify generator, edge, product price and risk separately.**
    Verify coefficient scale, exact represented PSD, true
    zeros, both correlation signs, Feller cases, actual
    payoff, price bounds, clock identity and both exact
    deterministic limits. Compare independently expected
    Black–Scholes and Heston prices and Greeks with aligned
    carry and clocks. Include signed carry, starting at and
    reaching zero variance, strong variance drift, wings,
    and a checked American exercise benchmark. Refine time,
    space and far boundaries separately. Check both bumped
    price legs and host errors. Use product-specific tolerances
    for the supported domain. Put-call parity alone cannot
    qualify variance dynamics, correlation or stability.

## Observed choice and equations

[bridge.rs](../../../../finstack-quant/models/src/pde/bridge.rs) maps Black–Scholes with x=ln S to a=σ²/2, b=r−q−σ²/2, c=−r and zero source. Terminal payoff is max(exp(x)−K,0) for calls or max(K−exp(x),0) for puts. The in-the-money edge uses `LinearInExp`; the out-of-the-money edge is Dirichlet zero. This preserves affine-in-spot behavior at truncated log-spot edges.

[bridge2d.rs](../../../../finstack-quant/models/src/pde/bridge2d.rs) uses y=v, v+=max(v,0): a_xx=v+/2, a_yy=σ_v²v+/2, a_xy=ρσ_v v+, b_x=r−q−v+/2, b_y=κ(θ_v−v), c=−r. The drift uses raw v while diffusion uses v+. Terminal payoff and x boundaries match the BS bridge. Both lower and upper variance boundaries use `Linear`, including the degenerate v=0 edge; the bridge does not solve the limiting boundary PDE explicitly.

## Input conventions and selection

Rates and yield are continuous annual decimals; σ is annual lognormal volatility, v and θ_v are annual variance, κ is inverse years, σ_v is variance-process volatility and ρ is correlation. Both structs expose public fields and implement no validation. The generic solver does not enforce positive strike, valid correlation, positive parameters or a nonnegative variance grid. Model homogeneity permits operator reuse. The 1D solver may attach an obstacle; the 2D solver remains European. Grid/domain, maturity and damping are caller choices, not fields of the bridge.

## Validation

- With \(\rho=\operatorname{nextDown}(1)\),
  \(\sigma_v=0.9441473299836161\) and
  \(v=0.036718169527261216\), the existing coefficient
  arithmetic yields exact represented
  \(4a_xa_y-m^2\approx-8.94319\times10^{-21}\).
  Directed construction yields approximately
  \(+7.26947\times10^{-19}\) with the same \(\rho\).
  At \(v=2^{-700},\sigma_v=2^{600},\rho=1/2\),
  the complete coefficients are
  \((2^{-701},2^{499},2^{-101})\) although the
  detached square overflows.
- A call with log spot 710 and strike equal to the
  largest finite binary64 value has finite payoff
  approximately \(4.363016313\times10^{307}\).
  The detached exponential overflows. A linear variance
  closure on \((0,1/8,1/2)\) for \(U=v^2\)
  reconstructs \(-1/16\) at zero. The exact edge
  value is 0.
- A curve-native discount of 0.997 over 30 days yields
  an effective Act/360 model rate of approximately
  0.03605410824. An Act/365F volatility of 0.20
  maps to approximately 0.19862541326 with unchanged
  total variance. An expiry-equivalent flat rate from
  \(D(0,1)=e^{-0.1}\) yields \(e^{-0.05}\) at
  half a year. A native curve with half-year factor 1
  has different intermediate exercise economics.
- For \(S=K=100,r=2,q=1,T=2,\sigma=0\),
  the American call maximum is 25 at \(t=\log2\).
  At expiry alone it is approximately 11.70196443.
  For Heston \(\kappa=1,\theta_v=0.04,v_0=0.09,
  \sigma_v=0,T=2\), integrated variance is
  0.123233235838. With \(S=100,K=110,r=0.03,q=0.01\)
  the call is approximately 11.430592872415.
- With \(S=120,K=100,r=0.05,q=0.02,T=1\), the
  European call lower bound is approximately
  22.5008983467. A premium of 20 passes the current
  Heston check and violates the proposed lower bound.
- An independent Fourier calculation uses \(S=K=100\),
  \(T=1,r=0.03,q=0.01,\kappa=1.5,\theta_v=0.04,
  \sigma_v=0.5,v_0=0\). The call is approximately
  5.946991261045 for \(\rho=-0.7\) and
  5.693517412518 for \(\rho=0.7\). These Feller-violating
  cases are targets for production zero-edge qualification.

BS call/put tests compare with the repository analytical formula (for example 0.1% call tolerance). Heston `heston_pde_vs_fourier_coarse_anchor` checks K=100 and 120 at 2.5% relative tolerance against the repository Fourier engine; it is numerical-method cross-validation, not an independent vendor source. The full-resolution ATM test is ignored as slow and allows 2%. Put-call parity alone cannot identify correlation or variance-dynamics errors. Feller-violation/refinement and high-κ cases exercise selected parameter regimes, not the whole exposed domain.

A degenerate-PDE boundary closure or validated model-parameter constructor would be alternative implementations. [Black–Scholes](../../../REFERENCES.md#black-scholes-1973), [Heston](../../../REFERENCES.md#heston-1993), and [MCS](../../../REFERENCES.md#in-t-hout-welfert-2009) identify model/method provenance; none approves the selected grid and boundary policy.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
