# MODELS-NUMERICAL-007: Hull–White event-aligned trinomial and affine node bonds

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Source and callers inspected. Independent exact and Decimal references checked grid admission, conditional moments, curve fitting and the finite-tree versus affine bond distinction. Production pricing, external comparison and risk qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked Hull–White event lattices and node bonds

1. **Select the centered Gaussian law with Continuous base discounting.**
   Use `x(0)=0` and `dx=-kappa*x*dt+sigma(t)*dW`, with finite
   nonnegative constant kappa in inverse model years. Admit exact
   kappa=0 as the no-reversion limit of this trinomial. Use signed
   continuous interval quotes for lattice discounting. Remove
   Simple and Periodic base alternatives from this Hull–White
   lattice. OAS retains its separately selected Continuous or
   Periodic quote basis under record 003.
   [Gaussian model](../models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures),
   [tree discounting and OAS](tree-discounting-and-induction.md#accepted-decision-for-checked-tree-discounting-and-rollback)

2. **Select exactly one volatility representation.** Supply a
   nonnegative finite scalar sigma or one validated right-continuous
   piecewise-constant schedule on the model clock. Sigma is in
   absolute decimal rate units per square root of a model year.
   Admit zeros and retain the schedule's original origin and
   support. Remove the unused scalar input for scheduled requests
   and generic numerical defaults for active parameters and work.

3. **Use one declared model origin and curve clock.** Dated
   instances use ACT/365F from the owner-supplied origin under
   record 24. Evaluate the discount source on its actual native
   coordinate with its own context, including when the source
   base differs from the origin. Use the accepted clock bridge
   for fractional model days. A numeric-only instance needs a
   qualified curve and coefficients on its declared common clock.
   Coupon accrual and eligibility remain with the product.
   [Model clocks and curve fit](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection)

4. **Take a prepared immutable time grid.** The owner selects the
   numerical resolution and supplies a checked TimeGrid. Use its
   literal knots, exact endpoints and actual adjacent widths.
   Preserve the realized interval count. Admit one interval and
   the explicit [0] terminal-only grid. A terminal-only request
   still validates its direct inputs and requires no future fit
   or outgoing interval.
   [Checked TimeGrid](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots)

5. **Require exact nodes for mandatory events and volatility knots.**
   The owner resolves the actual required cash, fixing, exercise
   and other event times and includes every in-horizon schedule
   knot in the prepared grid. Check every required time in [0,T]
   and require exact grid membership. Multiple events can share
   a knot while retaining their identity and ordering. Distinct
   represented times stay distinct. Reject missing nodes and
   unsupported geometry. Remove tolerance-based admission,
   nearest-time lookup and implicit subdivision of an already
   prepared grid. Post-grid cash needs an owner-qualified dated
   treatment or an explicitly longer prepared grid.

6. **Use the exact OU conditional moments on each interval.**
   For h=t[n+1]−t[n] set `rho=exp(-kappa*h)`,
   `mean=rho*x` and
   `v=integral_[t[n],t[n+1]] sigma(u)^2*exp(-2*kappa*(t[n+1]-u)) du`.
   Use the checked integral and preserve complete scale, including
   the kappa=0 limit. This replaces Euler mean and variance.
   The departing-node discount remains a finite-grid approximation
   to the continuous stochastic bank account.
   [Exponential variance integration](../core-numerics/piecewise-coefficients.md#accepted-decision-checked-piecewise-coefficients-and-exponential-integration),
   [lattice references](../../../REFERENCES.md#checked-hullwhite-event-lattices-and-node-bonds)

7. **Use a nearest-mean trinomial when the interval has noise.**
   Use arriving spacing `dx=sqrt(3*v)`, evaluated without
   requiring a detached variance to fit when the spacing fits.
   Choose the nearest signed integer k to mean/dx, with exact
   half ties away from zero. Let `b=mean/dx-k`. Offsets
   (+1,0,−1) from k have weights
   `((1/3+b*b+b)/2,2/3-b*b,(1/3+b*b-b)/2)`.
   Validate the represented center, child positions, residual
   and weights. Use the contiguous ascending signed index span
   between required children for the arriving row.

8. **Propagate zero-noise intervals deterministically.** Map every
   current centered factor x to rho*x with unit branch weight.
   Preserve the ordered states from earlier stochastic intervals,
   using their checked contracted coordinates. A row reached
   from a wholly deterministic history remains one state at zero.
   A zero-volatility segment does not erase existing state
   dispersion. Reject unresolved required separation. Positive
   noise cannot substitute a deterministic step.

9. **Validate geometry and bound work before allocation.** Check
   signed index conversions, child arithmetic, positive spacing
   and distinct required states. Check each next span and the
   cumulative node, edge and allocation work against the explicit
   limits. Use record 003's finite weight and sum checks with
   one prepared branch law. Require every child to exist and
   preserve stored weights without clipping or renormalization.
   Tight event spacing or a sharp drop in sigma can make the
   dense grid too wide. Return a geometry or resource error
   without moving an event or changing the requested grid.

10. **Freeze origin-relative discount targets.** Capture the
    admitted sequence `D(t[n])` from the bound curve before
    fitting. Require exact D(0)=1 after source admission and
    positive finite factors at every later knot. Admit flat
    or increasing targets and factors above one. Use the same
    captured factors for fit and replay. Conditional evaluation
    uses the same immutable model source.

11. **Fit one continuous interval shift per departing row.**
    Start at Q[0,0]=1. With `r[n,j]=x[n,j]+alpha[n]`, set
    `alpha[n]=(log(sum_j Q[n,j]*exp(-h*x[n,j]))-log(D(t[n+1])))/h`.
    Use the actual incoming state-price mass, stable scaled
    log sums and checked complete rate and discount arithmetic.
    The one-state expression reduces to a checked log
    ratio. Propagate the discounted state prices with the
    prepared branches and base discounts. Keep OAS and
    product cash outside this fit.

12. **Replay the stored lattice against one inclusive tolerance.**
    Use the actual stored departing rates, interval factors
    and branches. Require every maturity's
    `10000*abs(D_model-D_target)/D_target` to be finite and
    at most the explicit finite positive tolerance in relative-DF
    bp. Report the maximum and the earliest maturity attaining it.
    For [0] there is no positive-maturity fit error. Remove the
    hardcoded 0.1 bp gate and publish the candidate only after
    all geometry, arithmetic and replay checks pass.

13. **Bind the complete prepared state.** Retain the exact
    model origin, immutable source snapshot and parameter form,
    grid and required-event provenance, selected discount law,
    branches and calibration result together. Keep construction
    fallible and atomic. Reuse requires the same selected source
    and parameters, exact admitted required nodes and qualified
    horizon coverage. When a frozen grid is requested, require
    its exact identity. A new source, parameter set or origin
    needs fresh preparation.

14. **Expose checked state, outgoing intervals and terminal payoff.**
    Indexed queries fail on invalid level, node, child or
    interval. Keep centered state separate from interval quote.
    Terminal state has no departing width, rate, discount factor
    or converted OAS. Remove the repeated alpha and last-interval
    and zero-value fallbacks. Use one fallible rollback and
    propagate callback errors and nonfinite values under record 003.

15. **Reuse the checked centered affine bond on its own basis.**
    From an admitted node at t with centered factor x and
    maturity u≥t, use the accepted continuous law
    `log P(t,u)=log(D(u)/D(t))-B(t,u)*(x+J(0,t))-B(t,u)^2*V(0,t)/2`.
    B, J and V are the accepted loading, state/integrated-state
    covariance and state variance on the bound model and
    schedule. Use a checked relative source factor or log
    ratio as needed. Interval alpha and OAS are absent from
    this law. Return one at exact equality after admission.
    Reject earlier maturities and invalid or unresolved
    results. A genuine terminal factor may value a later
    affine bond when the bound curve and coefficient law
    support the request.
    [Centered conditional projection](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection)

16. **Name the payoff basis explicitly.** For a claim defined
    by the finite lattice, an on-grid conditional unit bond
    uses the same base discounted rollback from its payment
    node. Retain the continuous affine underlying for the
    swaption exercise path, with finite-lattice discount and
    exercise. Treat that combination as an explicit product
    pricing choice subject to independent qualification.
    A continuous affine node bond is a different quantity
    from the finite-grid unit claim. Curve replay alone
    does not measure their difference.

17. **Use one coherent projection state at exercise.** Resolve
    the actual fixing, accrual and payment dates at the product.
    Keep conditional bonds and any qualified payment-measure
    adjustment on the same bound source, clock and active
    volatility representation. Propagate failed curve, rate,
    payoff or annuity evaluation. The existing swaption product
    parameter and projection restrictions stay with its owner.
    Scheduled payoffs require the corresponding checked
    schedule-aware cashflow law before admission.
    [Hull–White swaption products](../valuations-rates/08-hull-white-swaptions.md)

18. **Reconcile the live callable-bond path.** Prepare the
    valuation-origin ACT/365F lattice and source-native relative
    factors once. Use the actual prepared grid for the payoff.
    Include required adjusted payments inside the selected
    horizon or apply the separately qualified product rule for
    displaced cash and OAS. Preserve actual exercise windows,
    entitlement and same-day ordering. A selected deterministic
    Hull–White path uses the same qualified grid and discount
    semantics. Product owners select resolution and admissible
    timing treatment.
    [Callable bond rates-tree owner](../valuations-bonds/04-rates-tree-exercise.md)

19. **Reconcile European and Bermudan exercise and diagnostics.**
    Use exact exercise nodes and checked payoff callbacks for
    actual European and Bermudan Hull–White routes. Use the
    same checked continuation, policy and branch destinations
    for Bermudan exercise diagnostics as for PV. Propagate
    failures. Exercise today, ties, settlement and the
    interpretation of no exercise stay with the product.

20. **Use explicit grid and source choices in risk and hosts.**
    Preserve the actual grid for qualified same-grid risk.
    Rebuild rolled event coordinates from original dates
    on the new origin. Curve or parameter shocks must resolve
    the correct bound snapshot before repricing. Reconcile
    actual bond and swaption price, OAS, exercise and risk
    paths through existing Rust, Python and WASM surfaces.
    Keep the prepared lattice Rust-only.

21. **Qualify the finite operator and continuous comparison separately.**
    Check exact events, sparse arrivals with large dense spans,
    mixed zero and positive noise, signed discounts, state-price
    replay, terminal and past-date boundaries, and failure
    propagation. Use independent finite-tree valuations and
    continuous Gaussian or matched external benchmarks.
    Qualify the affine-exercise/lattice-rollback combination,
    options and risk over the selected grid and model regimes.
    Matching OU moments does not integrate the stochastic bank
    account exactly or guarantee derivative convergence on
    an arbitrary event grid.

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| ACT/365F date three days from the old origin and a one-day roll | Subtracting the old year fractions differs by one binary64 ULP from converting the remaining two days. Rebuild dates on the new origin before exact lookup. |
| Grid [0,1,2,4], kappa=log(2), scheduled step variances 4/3, 0 and 1/75 | Row widths are 1, 3, 3, 5. The zero-noise interval maps states (−2,0,2) to (−1,0,1). The final undiscounted factor variance is 41/1200. |
| kappa=0, scalar sigma=1 and grid [0,1,nextUp(1)] | The second arriving spacing is smaller by 2^26. The contiguous final row requires 134,217,731 nodes. The grid requires an explicit resource check even though its knots and moment formulas are admissible. |
| kappa=.2, sigma=.02, grid [0,.5,1.25], D(t)=exp(.015t−.004t²) | The fitted tree reproduces D(1.25) ≈ 1.0125784515406344. Weighting the continuous affine .5-to-1.25 bonds with the lattice state prices gives ≈ 1.0125465199521751, a gap of about .315 relative-DF bp. |
| Continuous row h=1, x=(−720,0,720), Q=exp(−720)*(1/6,2/3,1/6), alpha=15 | The fitted target is about 5.09837200836e−8. All outgoing factors fit in binary64 while the detached unshifted exp(720) overflows. |

These checks exercise the proposed mathematical contracts. Product
convergence, runtime pricing, hosts and risk remain open.

## Observed implementation and evidence

### Choice and domain

[hull_white_tree.rs](../../../../finstack-quant/models/src/trees/hull_white_tree.rs) supports negative normal short rates with κ>0, σ>0 and at least two requested steps. A positive piecewise-constant volatility schedule overrides scalar σ in interval evolution, but scalar σ is still validated. Default configuration is κ=0.03, σ=0.01, N=100, continuous compounding. Times are year fractions supplied by callers; the tree performs no calendar conversion.

### Construction, calibration and selection

Combine finite mandatory dates with interior volatility knots; discard dates within 1e−9 of endpoints or outside the horizon, sort/deduplicate, and subdivide each segment using max(round(segment/(T/N)),1). Actual N can differ from requested N. Set the arriving level's spacing dx=σ_i√(3dt_i). For node x choose central next index k=round(x(1−κdt)/dx_next), residual η=x(1−κdt)−k dx_next, A=(σ²dt+η²)/dx_next², B=η/dx_next, and probabilities ((A+B)/2,1−A,(A−B)/2). Reject negative/nonfinite probabilities rather than clipping.

Fit interval r=x+α to P(t_next). Continuous α=(ln ΣQ exp(−x dt)−ln P)/dt; other compounding uses Brent. Propagate Arrow–Debreu prices and reject nonfinite/negative masses or relative DF error>1e−5 (0.1 bp). Repeat the last α on the terminal row. Rollback uses the same stored branches and compounding then a callback. Bond and swaption callers pass coupon/exercise dates as mandatory times; `step_at_time` enforces matching while `time_to_step` snaps nearest, with ties to the lower point.

### Conditional bond method

`bond_price` is a separate continuous affine formula: P(t,T)=P(0,T)/P(0,t) exp(−B(x+c(t))−Var[x(t)]B²/2). B and integrated variance come from HW helpers; constant c(t)=σ²B(0,t)²/2. Scheduled c uses the difference of weighted variance integrals divided by κ, with a small-κ integral branch. It deliberately uses the centered x and instantaneous covariance shift rather than lattice interval α.

### Limitations, alternatives and evidence

The lattice uses Euler mean/variance while node bonds use exact integrated model moments; finite-grid consistency, particularly with noncontinuous discounting, is not established by curve fit alone. No final-maturity drift is calibrated. Invalid node-bond indices/nonfinite maturity yield NaN; matured bonds return 1; nonpositive P(0,t) returns 0. General accessors can return zero, a last dt or equal probabilities on invalid indices.

Inline `test_tree_calibration`, `calibrate_places_mandatory_dates_on_grid`, `scheduled_tree_includes_volatility_knots_in_grid`, `root_forward_swap_rate_matches_curve_par_rate_within_one_bp`, and unit-payoff tests cover curve consistency and dates, not universal derivative accuracy. [HW 1994](../../../REFERENCES.md#hull-white-1994-numerical-procedures) and [HW 1990](../../../REFERENCES.md#hull-white-1990-pricing-ird) are owning references. Exact OU moment branching, rejection-only accessors and grid-converged node bond values are unimplemented alternatives.

The callable-bond preparation uses the discount curve's day count
from valuation to contractual maturity, passes the unrebased source
to calibration, and can have adjusted cash after that horizon.
Swaption preparation uses a model-clock adapter. The payoff wraps
the affine lookup as infallible and uses scalar volatility for its
delayed-coupon correction. Bermudan exercise diagnostics duplicate
part of the rollback, and rolled risk grids can include old times
shifted by a floating-point elapsed fraction.
[Callable-bond preparation](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/tree_pricer.rs),
[swaption cashflows](../../../../finstack-quant/valuations/rates/src/swaption/pricing/hw_cashflows.rs),
[exercise diagnostics](../../../../finstack-quant/valuations/rates/src/swaption/pricing/tree_valuator.rs),
[prepared Bermudan model and risk](../../../../finstack-quant/valuations/rates/src/swaption/bermudan.rs)

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
