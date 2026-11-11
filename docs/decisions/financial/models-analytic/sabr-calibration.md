# MODELS-ANALYTIC-14: Deterministic multistart SABR fit, weights, shifts and ATM pinning

**Decision status:** Accepted — complete explicit deterministic SABR fit, native quote acceptance and observed ATM pin package, 2026-11-10.

**Implementation status:** Observed. Runtime, target, report, schema and host reconciliation remain open.

**Validation status:** Source, primary references and existing tests inspected. Standalone arithmetic, ATM branch and illustrative fit checks cover selected decisions. Production solver, independent market and publication qualification remain open.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for explicit checked SABR calibration

The complete 27-decision package was accepted on 2026-11-10.

1. **Keep one checked slice fit entry.** Construct immutable
   `SabrCalibrator` with required checked fit controls. Use one
   named calibration request with the original coordinates and
   paired observations, fixed beta, quote convention, model
   displacement and fit mode. Return `Result<SabrCalibrationOutcome>`.
   Use the same route in Rust and the existing hosts. Remove
   default construction, `high_precision` and mutation-based
   controls. No legacy string or tolerance aliases.

2. **Fit one expiry under the accepted expansion.** Fit alpha, nu
   and rho at literal supplied beta in [0,1]. Use the
   [checked record 13 quote](sabr-expansions.md#accepted-decision-for-explicit-checked-sabr-quotes)
   for every model value. T is the owner's finite positive model
   expiry. F and K share the underlying units, alpha depends on
   beta and nu is per square root model year. Keep date and quote
   preparation with the actual target owner.

3. **Require the actual quote convention and model displacement.**
   Use explicit Normal, Black or shifted Black and an optional
   numeric model displacement. Black requires
   absent model shift. Shifted Black requires the literal finite
   signed source shift, matching the convention exactly, with
   explicit zero distinct from absence. Normal admits absent or
   explicitly supplied signed model displacement. Preserve raw F
   and K through weighting, fitting and final evaluation. Retire
   `SabrShift::Auto`, the standardized ladder and headroom rule.
   Selection of a model displacement for Normal belongs upstream.

4. **Admit a complete single smile before solving.** Require
   at least three aligned finite strikes and finite strictly
   positive market vols. Validate finite F, T>0, beta, shift,
   convention and all record 13 effective-coordinate domains.
   Exact beta-zero Normal can cross zero. Zero effective rates in
   positive-domain cases error before the optimizer. The caller
   supplies already qualified quotes in one convention and clock.

5. **Preserve paired quote identity and deterministic order.**
   Validate and sort paired quotes by ascending original strike
   for preparation and solver evaluation. Reject duplicate
   numeric strikes, including signed-zero equality. Restore all
   signed quote errors in caller order and retain the identity
   of the exact observed ATM row if present. Use no grouping,
   deduplication, tolerance-based equality or row dropping.

6. **Keep vega-weighted relative volatility as the objective.**
   Use market Normal vega for Normal input and market Black vega
   for Black or shifted Black with its actual coordinate shift.
   The weights are fixed during fitting. Minimize the norm of
   r_i=sqrt(w_i)(sigma_i−q_i)/q_i, with model quote sigma_i and
   observed quote q_i. This is a project weighting policy in
   volatility space. Its score supplies no premium error bound.

7. **Normalize vegas without materializing vanishing raw values.**
   For Normal use d_i=(F−K_i)/(q_i sqrt(T)). For relative quotes
   use d_i=log(x/y_i)/(q_i sqrt(T))+q_i sqrt(T)/2 with the record 13
   geometry. The other market-vega factors are common within a
   slice. Let D=min_i |d_i| and use
   v_i=max(1e−8, exp(−(|d_i|−D)(|d_i|+D)/2)), then w_i=v_i/sum v.
   Keep the 1e−8 relative floor as explicit objective policy.
   Resolve ordering and positive differences in scale, and skip
   the exponential when the ratio is proven below the floor.
   Use a checked compensated sum. An unresolved required weight
   or standardized coordinate errors.

8. **Form the complete weighted residual.** Compute the signed
   native difference first and apply sqrt(w_i)/q_i in checked
   scaled arithmetic. Avoid an unneeded overflowing standalone
   factor or ratio. Fill every slot with a resolved finite result.
   Keep raw signed differences separate for the financial gate.
   Never substitute a large penalty for an invalid model value.

9. **Require an absolute native quote-error cap.** Use a required
   finite positive `max_abs_quote_error` in the input quote units.
   Apply |sigma_i−q_i|<=cap to every final original observation,
   including wings and the ATM row. Remove the implicit relative
   cap and the product `absolute_cap/min_quote` translation.
   The solver residual and step controls govern numerical work.
   The per-quote cap governs fit acceptance.

10. **Use the accepted checked local solver.** Reuse
    [CORE-NUMERICS-06](../core-numerics/levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes)
    with required positive finite residual-norm and step-norm
    tolerances and a nonzero attempted-step budget per start.
    Use its fallible fixed-shape residual and supplied Jacobian
    callbacks and augmented pivoted QR. The step tolerance is
    in the selected transformed coordinates. Use no implicit
    tightening or solver-method fallback.

11. **Make the nu and rho fit intervals explicit.** Require
    finite 0<=nu_lo<nu_hi and −1<=rho_lo<rho_hi<=1. Fit in
    their open interiors. Use stable logistic interpolation
    from each unconstrained coordinate to its interval.
    Check the actual restored interior, including rounding,
    without clamps. Free alpha uses alpha_seed exp(p_alpha),
    combined in checked scale. This selected search has nu>0.
    Canonical parameter storage still supports nu=0.

12. **Choose a deterministic free-alpha seed.** Use the observed
    quote nearest F in checked scaled original strike distance.
    The lower strike wins an exact tie. From that quote q_ref and
    effective forward x use q_ref x^(1−beta) for Black outputs,
    q_ref/x^beta for positive-backbone Normal, and q_ref for
    exact beta-zero Normal. Check the actual seed's range and
    positivity. This quote is an initialization input only.

13. **Run nine ordered independent starts.** Use the physical
    one-quarter, midpoint and three-quarter fractions of the
    supplied nu and rho intervals through their logistic
    coordinates, in nu-major and rho-minor ascending order.
    Require distinct representable interior starts on each
    axis. Free fits start at the common alpha seed. Pinned fits
    derive alpha from the actual pin at each initial pair.
    The nine-start schedule is a bounded project profile.

14. **Select Free or observed-ATM Pinned explicitly.** Free
    fits use all three coordinates and need no ATM quote.
    Pinned fits require the unique original K=F observation
    and solve only for nu and rho externally. Require a finite
    positive closed alpha bracket with distinct endpoints,
    a positive finite alpha-coordinate tolerance, a nonzero
    pin update budget and a positive finite native ATM quote
    tolerance no larger than the overall cap. Missing exact
    ATM errors. Remove interpolation, extrapolation and the
    free-fit fallback from the pin.

15. **Use the selected ATM equation to check the pin branch.**
    Let u=alpha sqrt(T)/x^(1−beta), n=nu sqrt(T), and let
    h=1 for Black outputs or h=x for positive-backbone Normal.
    For exact beta-zero Normal use u=alpha sqrt(T), h=1.
    The ATM quote is h u(A+B u+C u²)/sqrt(T), with
    A=1+(2−3rho²)n²/24, B=rho beta n/4, and C=P/24 using
    record 13's Black or Normal coefficient P. This polynomial
    supplies a branch test for the same checked ATM evaluator.

16. **Require a strictly increasing positive bracket.** For
    each nu/rho evaluation require valid finite endpoint
    quotes with the observed target between them inclusively.
    Check A+2B u+3C u² is strictly positive throughout the
    normalized bracket. Check both endpoints and the vertex
    −B/(3C) if C>0 and the vertex lies inside. Use scaled
    sign evaluation and propagate unresolved signs. This
    selects a unique root on the supplied increasing branch.

17. **Use checked Brent and reprice the returned alpha.** Use
    the [accepted supplied-bracket Brent kernel](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search)
    in literal alpha coordinates on the validated closed
    bracket. The callback is the checked ATM quote minus the
    observed target. Reuse any matching endpoint quote already
    evaluated for admission. Check the returned alpha and independently
    require its native ATM residual within the pin tolerance.
    Apply the same native gate to computed endpoint matches.
    Remove Newton steps, clipped alpha updates and automatic
    search for another root.

18. **Differentiate the same prepared objective.** Use the
    accepted quote partials and weight factors with the actual
    transformed-coordinate chain. For Pinned use
    d alpha/d p=−(partial_p sigma_ATM)/(partial_alpha sigma_ATM)
    for each physical nu or rho coordinate before its logistic
    chain. Evaluate the derivative at the checked pin and
    qualify the effect of the inner solve tolerance. Reuse an
    already checked matching pin at the same coordinates.
    At a bracket endpoint qualify the derivative on the admitted
    branch. A failed derivative or implicit division propagates.

19. **Keep local failures visible.** A hard callback, pin,
    derivative, range or QR failure ends that start under
    the checked solver contract. Proceed only to the other
    scheduled independent starts and retain the actual stage
    and cause. Return a validation error for invalid requests
    before attempting any starts. If no start passes, return
    a calibration error with the actual failures and finite
    rejected diagnostics where available.

20. **Reprice every returned candidate and select among passes.**
    Rebuild the literal physical parameters and shift and use
    the same checked quote on every original observation.
    Recheck the pin when active. Apply the per-quote cap, then
    recompute the finite objective from those same final values.
    Select the lowest computed norm among passes, keeping the
    earlier scheduled start on exact ties. A finite outcome
    with step, damping or attempt exhaustion can pass the
    financial checks. Preserve its literal solver status.

21. **Preflight and count bounded work.** Check dimensions,
    allocation sizes and aggregate count arithmetic for nine starts,
    all quotes, the LM attempts and the nested pin budgets.
    With A attempted steps per start, budget at most 1+A
    residual calls, A supplied Jacobian calls and one candidate
    check per start. Allow at most 2A+3 pin uses per start
    including the initial physical tuple, callbacks and final
    candidate. Each use can add two ATM endpoint evaluations,
    the allowed Brent updates and the returned-alpha check.
    Count actual calls, including failed work.
    Matching cached values do not add evaluations.

22. **Report literal outcomes and work.** Record the winning
    start index, actual physical starting values, solver
    termination and matching step counts. Report final
    signed errors in input order, maximum absolute error
    and weighted norm. Count actual residual/Jacobian calls,
    pin solves and ATM evaluations, and candidate reprices
    across starts. Report unavailable attempt counts explicitly
    if an abort lacks core stats, including the aggregate
    attempt total in that case. Preserve rejected causes.
    Replace the relative-error summary and `at_bounds` proximity
    labels with the native gate and the actual search intervals.

23. **Keep fit provenance truthful.** Preserve the selected
    quote convention, actual model displacement, fixed beta,
    forward, modeled expiry, controls and mode in the fit
    outcome/report. Validate state and diagnostic shape on
    restoration. A replayable fit retains its input observations
    with the owning request/report. Keep dated clock and source
    identity with the target owner. A published parameter cube
    does not acquire a persisted output quote type from the fit.

24. **Reconcile the actual equity target.** Its unshifted Black
    input and publication contract uses explicit Black and
    absent displacement. Remove the beta-as-quote-tag cutoff
    and auto shift. Pass its native quote cap and explicit fit
    controls. Keep source preparation, materialization and
    published-surface repricing with its target owner.

25. **Reconcile the actual swaption target.** Pass the qualified
    quote convention, actual displacement, literal beta and
    explicitly selected fit mode. Admit signed or zero shifted
    Black when effective levels qualify. Do not force beta-zero
    or pinning from a Normal tag. A selected pin uses an exact
    observed strike at the prepared forward. Bucket failures and
    publication belong to the target owner. Preserve separate
    accepted model, source and off-grid contracts.

26. **Reconcile existing hosts and configuration.** Use the
    immutable request and controls on the direct Python/WASM
    calibrator and the actual equity and swaption plan carriers.
    Reconcile outcome schema, typed errors, reports, generated
    host/UI contracts, parity, examples and benchmark. Require
    explicit controls and strict state in those requests. Check
    integer budgets before host casts. Hosts convert and
    propagate Rust results. Brent and LM remain Rust-owned.

27. **Qualify the fit and its limits.** Verify objective and
    gate separately, explicit signed displacement and exact
    quote order, bounds and starts, pin feasibility and branches,
    derivative agreement and complete diagnostics. Compare
    independent noisy and observed-market slices, including
    weakly identified wings and failures. A passing fit concerns
    supplied observations and the bounded schedule. Global
    optimality, parameter identification and arbitrage control
    require separate evidence or owners.

## Independent bounded reference checks

- For quotes 0.10 and 0.40 and an absolute cap of 0.001,
  `cap/min_quote` gives 0.01. A wing error of 0.003 passes
  that relative proxy at 0.40 and fails the native cap.
  Separately, with preweights 1 and 1e−8, relative errors
  (0,0.05) score about 5e−6, versus 0.01 for (0.01,0.01).
  The smaller weighted score can hide the worse quote.
- With fractional-beta Normal at F=0.04, beta=0.5 and
  reference quote 0.006, the selected alpha seed is 0.03.
  The old beta-based Black seed would be 0.0012.
- At exact beta-zero Normal, F=0.02, alpha=0.01, nu=0.5,
  rho=0 and T=1, strikes 0.01 and 0.03 both have quote
  0.010606901994. Variance interpolation pins that value.
  The generating ATM quote is 0.010208333333.
- For Normal with F=K=1, beta=1, nu=1, rho=0 and T=6,
  the ATM map is alpha(3/2−alpha²/4). Target 23/32 has
  two positive roots, 1/2 and (sqrt(93)−1)/4≈2.160913.
  The increasing bracket [1/4,1] selects 1/2. For Black
  with F=K=1, beta=1, nu=1, rho=−0.5 and T=16, target
  1/3 has exact roots 1/4 and 2/3 on opposite slopes.
- For Normal at F=K=0.03, T=2, beta=0.5, nu=0.62, rho=−0.43
  and alpha=0.0425, high-precision differences of the pin on
  [0.01,0.09] agree with both implicit alpha derivatives.
- At F=−0.005, K=−0.01, sigma=0.20 and T=1, the same
  relative sigma gives unit Black calls about 0.00501925
  and 0.00529648 under displacements 0.02 and 0.03.
  A displacement is part of the quote's financial meaning.
- An independent numerical fit of an illustrative noisy
  Normal slice used F=0.03, T=2, beta=0.5, strikes
  [0.01,0.02,0.03,0.04,0.06] and quotes
  [0.00967,0.00860,0.00756,0.00759,0.01050]. Using the
  selected quartile starts in nu (0.001,2) and rho (−0.99,0.99),
  its weighted norm was about 0.00099142 and maximum native
  error about 0.00001297. It passes a native cap of 0.00005 with a
  nonzero objective. The standalone fit used numerical
  derivatives and an independent augmented least-squares
  step. It supplies an illustrative fit check.

## Observed scope and input domain

Single-expiry fit to ≥3 aligned positive volatility quotes with fixed user β. Normal β0 permits negative rates without shift; shifted calibration works on shifted positive F,K.

## Observed choice and rationale

Fit transformed parameters with deterministic Levenberg–Marquardt multistart and explicit relative-quote acceptance. Optional ATM pinning solves α inside a two-parameter fit; this is a numerical Newton solve, despite a source comment calling it analytic.

## Observed algorithm

Weights use market Black or Bachelier vega, divide by max, floor relative weights at1e−8, normalize total, and residual factor=sqrt(weight/total)/market_vol. Nine starts combine ν={.15,.4,.9},ρ={−.6,0,.6}, with α seeded from ATM quote×F^(1−β) (normal α=quote). Optimize log α and logistic ν∈(.001,2),ρ∈(−.99,.99). Invalid model evaluations get residual1e6. Choose lowest residual among candidates passing finite residual norm and every quote relative error≤configured tolerance. ATM quote sorts strikes, interpolates variance in strike, and clamps to endpoint outside grid. Pinning solves α by up to50 Newton steps, relative α bump1e−6, derivative floor1e−14, ±50% step cap; then fits ν,ρ and rechecks every quote.

## Observed selection and controls

Default relative tolerance1e−4,max iterations2000; high_precision1e−8/200. Inner LM tolerance=min(config,1e−10). Auto shift only triggers when min rate<0; select first of .005,.01,.02,.03,.04 providing .001 headroom; larger need errors. None and Fixed are explicit alternatives. Zero fixed shift means unshifted; nonzero final shift must satisfy parameter validation. Outcome records quote errors, winning start, iterations and bounds proximity. Residual evaluations are counted across all attempted starts. The iteration total currently omits solver errors without returned stats.

## Observed conventions

Input quotes must already match β/shift convention; no price conversion is performed. Relative tolerance scales with each quote, normal or lognormal. Shift and rate are decimals.

## Observed limitations, failure and fallback

Duplicate strikes are not rejected and conflicting duplicate quotes can fail fit. Auto shift does not fix exactly zero lognormal rates. ν bounds exclude deterministic ν=0. Pinning an interpolated ATM can conflict with an otherwise exactly representable discrete smile and cause rejection. No smoothing, cross-expiry calibration or arbitrage constraint is in this objective.

## Alternatives and tradeoffs

Free α and ATM-pinned variants are supported. Alternative price/vega weighting, β calibration or global no-arbitrage surface fitting are unimplemented choices.

## Implementation and validation evidence

[calibration.rs](../../../../finstack-quant/models/src/volatility/sabr/calibration.rs) and [tests.rs](../../../../finstack-quant/models/src/volatility/sabr/tests.rs). Deterministic repeatability/known-smile recovery tests are synthetic round trips; explicit rejected-stalled-start tests check final acceptance. Reference: [SABR formulation](../../../REFERENCES.md#hagan-2002-sabr); the exact bounds/weights/multistart policy lacks separately established adoption provenance.

The actual fit callers are the [equity surface target](../../../../finstack-quant/calibration/src/targets/vol.rs)
and [swaption cube target](../../../../finstack-quant/calibration/src/targets/swaption.rs).
See the [Python](../../../../finstack-quant-py/src/bindings/models/volatility.rs)
and [WASM](../../../../finstack-quant-wasm/src/api/models/volatility.rs)
wrappers and the [SABR notebook](../../../../finstack-quant-py/examples/notebooks/01_foundations/market_data/sabr_volatility.ipynb).
The [calibration references](../../../REFERENCES.md#sabr-atm-pin-and-fit-policy)
support the ATM-conditioning and solver context.

## Relationship to the accepted SABR source boundary

[CORE-MARKET-16's complete accepted package](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning)
preserves nu=0 and exact canonical parameter units/domains in published sources.
A fitter's selected positive-nu bounds do not invalidate stored deterministic
limits or authorize post-fit source floors, caps or retagging. Preserve actual
fit quote conventions, displacement, source clock and construction provenance;
separate those input conventions from a later explicit evaluated output.
Record 16 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's objective, bounds,
multistart, ATM pinning or fit acceptance policies.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
