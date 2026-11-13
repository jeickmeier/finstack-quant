# MODELS-NUMERICAL-016: Modified Craig–Sneyd ADI and split startup damping

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Independent affine stage, split startup, stability counterexample and Heston reference checks executed. Production PDE stability, price, risk and host qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked Modified Craig–Sneyd ADI

1. **Use one 2D terminal-value time-march profile.** Use the checked
   prepared tensor grid and canonical uniform model-time grid from
   zero to finite positive \(T\) with explicit parent interval
   count \(N\ge1\). Traverse backward over actual adjacent
   widths \(h=t_o-t_n>0\), where \(t_o\) is the departing
   forward-model time and \(t_n\) the receiving time. Use the
   stored endpoints, including exact zero and maturity. The
   generic 2D profile evolves a terminal payoff with edge
   conditions. Exercise projections and contractual event insertion
   are outside this profile.
   [Checked model-time grids](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots),
   [PDE spatial grids](pde-grids-interpolation-and-greeks.md#accepted-decision-for-checked-pde-grids-interpolation-and-coordinate-greeks)

2. **Use the accepted split operator.** At each required time let
   \(F=F_0+F_x+F_y\). The centered tensor mixed derivative
   \(F_0\) is explicit and consumes the matching
   boundary-inclusive field. For \(j=x,y\), let
   \(F_j(v)=A_jv+s_j\), with \(A_j\) the reduced line
   matrix and \(s_j\) its affine source and edge forcing.
   Use the coherent checked coefficients, represented diffusion
   admission and equal reaction and source splits of
   [012](pde-spatial-stencils.md#accepted-decision-for-checked-directional-and-mixed-pde-stencils).
   Use the boundary law of
   [013](pde-boundary-elimination.md#accepted-decision-for-checked-pde-boundary-elimination-and-reconstruction).
   Each directional affine term includes its own boundary forcing.

3. **Use the full ordinary MCS scheme at fixed \(\theta=1/3\).**
   For the admitted old interior \(u\), compute
   \(Y_0=u+hF^o(u)\). Then solve in x followed by y
   \[
   Y_j=Y_{\rm prev}+\alpha\big(F_j^n(Y_j)-F_j^o(u)\big),
   \qquad \alpha=h/3.
   \]
   The superscripts select the actual old and receiving samples.
   Use the same direction order on every ordinary interval.
   Keep the full corrector in production. A Douglas comparison
   may remain independent test evidence.
   [ADI method and stability references](../../../REFERENCES.md#checked-modified-craig-sneyd-adi)

4. **Use both corrections and retain the old directional terms.**
   Write \(Y_2\) for the first y-sweep result. Reconstruct
   its full field at the receiving time for the mixed action.
   Set
   \[
   Z_0=Y_0+\alpha\big(F_0^n(Y_2)-F_0^o(u)\big)
          +\frac{h}{6}\big(F^n(Y_2)-F^o(u)\big).
   \]
   Apply the same x and y solves starting from \(Z_0\),
   with \(F_j^o(u)\) again subtracted in each direction.
   Return the second y-sweep result as the destination interior.
   Use the actual \(h/3\) and \(h/6\) weights with checked
   complete products and sums. The corrector uses the first
   y result for its new-time terms and the original old state
   for its old-time terms.

5. **Use the shared checked tridiagonal solve on every line.**
   Each ordinary sweep solves
   \[
   (I-\alpha A_j^n)v=
     v_{\rm prev}-\alpha F_j^o(u)+\alpha s_j^n .
   \]
   The old directional value is affine, and the receiving
   forcing enters the line RHS once in this formula. The two
   sweep pairs use the same checked receiving matrices for the
   same weights. Apply the unpivoted Thomas and componentwise
   backward-error rules of
   [014](theta-time-march-and-thomas.md#accepted-decision-for-checked-theta-marching-and-tridiagonal-solves)
   to each line. A failed line aborts the interval. The mixed
   action remains outside the implicit line matrices.

6. **Define startup by split backward-Euler substeps.** The
   explicit startup count \(k\) satisfies \(0\le k\le N\).
   Replace each of the first \(k\) parent intervals by two
   backward substeps. For a substep of actual width \(\delta\),
   let \(P=u+\delta F_0^o(u)\). Solve
   \[
   (I-\delta A_x^n)X=P+\delta s_x^n,\qquad
   (I-\delta A_y^n)U=X+\delta s_y^n .
   \]
   Use the receiving directional data and departing mixed
   data. Skip the unused departing directional actions. The
   midpoint field from the first substep supplies the second
   substep's departing state. This is directional factorization
   with an explicit mixed term. The ordinary MCS corrector and
   the 1D full-interval damping law are separate stages and
   profiles, respectively.

7. **Admit actual startup geometry and report actual work.**
   Compute the midpoint from the checked parent endpoints
   as \(t_n+h/2\). Require a finite binary64 coordinate
   strictly between them. Recompute both positive widths by
   subtraction from the actual midpoint. Reject a collapsed
   midpoint without changing the step or startup scheme.
   Check the count and sizes for \(N+k\) executed subintervals.
   Report that count as the number of executed time steps.
   An ordinary interval uses four directional sweeps. A startup
   parent also uses four sweeps across its two substeps.

8. **Use the correct full field at each stage.** Initialize the
   terminal interior from the owner's finite payoff and build its
   edges at \(T\). The old mixed action uses the old field and
   the receiving mixed action uses the reconstructed first y-sweep
   field. For startup, reconstruct the intermediate full field
   at the actual midpoint. Use the accepted edge samples and
   corner ownership from 013. Reconstruct the final full field
   at zero from the accepted final interior.

9. **Keep work buffers and reusable rows bound to the solve.**
   Check full, interior and line dimensions, index arithmetic,
   stage counts and allocation sizes before use. Reuse bounded
   scratch for the matching admitted geometry. Cache coefficient
   and source data only under the accepted homogeneous
   declaration and within the same solve and grid. Reuse reduced
   rows while their boundary form matches, refresh scalar edge
   forcing, and rebuild affected rows on a form change.
   Time-varying data use each actual endpoint. Reusing scratch
   capacity supplies no authority to reuse model state.

10. **Publish a parent interval atomically.** Check required
    samples, matrix and RHS terms, each line solve, every required
    stage value and full field for finiteness and range. Use
    scaled arithmetic for required sums, differences, stencil
    effects and products. Finite signed fields and genuine zeros
    are valid. Keep provisional corrected fields and both startup
    substeps private. On failure, retain the old matched interior
    and full fields. Return the applicable interval, substep,
    stage, direction or line context. A numeric failure supplies
    no substitute price, weight, grid or numerical scheme.

11. **Qualify stability on the actual numerical problem.**
    Retain \(\theta=1/3\) as the selected Heston profile.
    The standard pure-diffusion Fourier result concerns real
    symbols of a constant-coefficient, uniform periodic problem
    satisfying the mixed-symbol bound. The \(2/5\) lower bound
    in In 't Hout and Mishra's cited general real-mixed,
    complex-directional symbol class is necessary there. Their
    result does not establish it as a sufficient bound. On the
    selected nonuniform grid and closures, strict dissipativity
    of the full reduced generator can coexist with an amplifying
    ordinary or startup step. Pointwise diffusion admission,
    directional upwinding, finite stages and successful line
    residuals supply no general positivity, stability or accuracy
    certificate. The owning application qualifies its actual
    operator, closure and time grid. Use no inferred global
    Péclet or diffusion-only CFL threshold.

12. **Keep the live European Heston route and ownership.** Use the
    existing Heston equity model key and two startup parent
    intervals on its admitted grid. The live defaults are
    200 log-spot nodes, 80 variance nodes and 100 parent
    intervals. Qualify those settings for the owner's supported
    domain. The bridge record owns coefficient generation,
    variance-edge choice, time and financial limits. Preserve
    lifecycle handling, qualified query and financial price bounds
    with the product. Existing selected-model risk reprices
    with the admitted Heston parameters and propagates failures.
    ADI stage and grid state remain Rust-owned, with existing
    Python and WASM pricing and error routes.
    [PDE bridges](black-scholes-and-heston-pde-bridges.md)

13. **Qualify stage algebra and production prices separately.**
    Verify affine endpoint terms and direction order, the
    nonlinear tensor mixed action, midpoint resolution, both
    startup substeps, full-stage publication and cache parity.
    Independently compare smooth manufactured solutions, stiff
    modes and kinked European Heston prices and risk. Use both
    correlation signs, near-zero variance and strong convection,
    and measure time, space and far-boundary errors separately.
    Use aligned independent Fourier references and product-specific
    price and risk tolerances. The factorized startup needs its own
    accuracy evidence. The published coupled backward-Euler
    startup comparisons concern a different step.
    [Heston Fourier comparator](../models-analytic/heston-fourier-integration.md#accepted-decision-for-bounded-heston-fourier-pricing)

## Observed choice and formulation

[adi.rs](../../../../finstack-quant/models/src/pde/adi.rs) splits F=F0+F1+F2 into explicit mixed derivative and implicit x/y line operators. For ordinary steps θ=1/3. Compute Y0=u+hF_old(u). For j=1,2 solve Yj=Yj−1+θh(Fj_new(Yj)−Fj_old(u)). Form Z0=Y0+θh(F0_new(Y2)−F0_old(u))+(1/2−θ)h(F_new(Y2)−F_old(u)). Repeat the two directional solves from Z0 using the same old directional terms; the final result is the new interior solution. Each line solve uses Thomas and affine boundary/source corrections.

## Startup, selection and controls

`McsStepper::new(N)` has no startup damping. `with_damping(k,N)` replaces each of the first k intervals with two halfsteps of factorized implicit Euler: predictor u+hF0_old(u), then implicit x and y solves, with no MCS correction. Thus the mixed derivative remains explicit even during damping. This differs from the full-interval 1D startup. The bare Douglas selector exists only under tests; no production caller can select it. [equity pde2d caller](../../../../finstack-quant/valuations/equity/src/equity_option/pde2d_pricer.rs) chooses two damped intervals.

[solver2d.rs](../../../../finstack-quant/models/src/pde/solver2d.rs) requires positive finite maturity and N>0, initializes terminal full/interior arrays, enforces boundaries at maturity, marches uniform intervals, and reconstructs t=0 edges. It has no obstacle/early-exercise support. Homogeneous operator caches are scoped to each solve, with scalar edge updates; mixed terms use refreshed full boundary arrays.

## Limits and alternatives

The implementation checks positive step width and Thomas pivots but not ellipticity, a cross-term CFL bound, finite solution norms or residuals. Local upwinding of directional drift is not by itself a proof of full MCS stability for arbitrary callbacks, grids, correlations and damping intervals. The cited general convection-diffusion theta bound and local upwinding do not establish stability for every exposed callback and grid. Other θ choices, other ADI schemes or a fully coupled implicit solve are unimplemented alternatives.

## Evidence

- For one interior value, take old affine operators
  \((F_0,F_x,F_y)=(1,-2u+8,-2u+17)\), receiving
  \((-4,-4u+23,-6u+21)\), \(u=3\), and \(h=1/2\).
  Exact stages are
  \((Y_0,Y_1,Y_2)=(10,81/10,293/60)\) and
  \((Z_0,Z_1,Z_2)=(523/72,155/24,65/16)\).
  A split startup substep with \(\delta=1/4\) and
  those endpoint samples gives \(39/10\) in x-then-y order
  and \(183/40\) with the directions reversed.
  With only a spatially uniform source \(f(t)\), ordinary
  MCS reduces to trapezoidal source weighting and a split
  startup substep uses the receiving source.
- On \(x=(0,9,10)\) and \(y=(0,1,10)\), use
  \(a_x=a_y=1,m=2\), zero drift, reaction and source,
  lower-x zero Neumann, upper-x zero Dirichlet, lower-y
  zero Dirichlet and upper-y zero Neumann. The selected tensor
  and edge rules reduce to
  \(F_0=-81u/50,F_x=F_y=-u/5\). The full generator is
  dissipative. An ordinary step with \(h=2\) amplifies by
  \(358841/167042\), and two split substeps of width 2
  amplify by \(64/49\). Both amplify despite valid
  diffusion and exact line solves. This example concerns the
  selected numerical rules on the stated mesh.
- With \(\eta=2^{-1074}\), a parent from \(3\eta\)
  to 0 has midpoint \(2\eta\) in binary64. The actual
  widths are \(\eta\) and \(2\eta\). Adjacent
  binary64 endpoints have no interior midpoint. In a separate
  one-interior-node startup example with source 2 and old
  value 3, the first halfstep returns 4 when receiving reaction
  is zero. The next halfstep has a zero x-line pivot at
  receiving reaction 4. Its parent cannot publish 4.
- Independent characteristic-function quadrature for In 't Hout
  and Foulon Table 1 inputs uses \(S=K=100\), \(T=1\)
  and \(v_0\) equal to long-run variance. Case 1 with
  \((r,q,\kappa,\theta_v,\sigma_v,\rho)
  =(0.025,0,1.5,0.04,0.3,-0.9)\) gives call 8.894869360054.
  Case 2 with \((0.01,0.04,3,0.12,0.04,0.6)\) gives
  12.025300279260. These are reference prices for subsequent
  production PDE qualification.
  [Heston benchmark inputs](../../../REFERENCES.md#checked-modified-craig-sneyd-adi)

`heat_2d_adi_converges` and
`implicit_start_damps_stiff_modes_as_split_euler_halfsteps`
inspect analytic diffusion and damping behavior. Heston bridge tests
compare MCS with the repository Fourier engine, with some full-resolution
tests marked slow. The independent fixtures above check the accepted
algebra and references. Production test execution remains open.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
