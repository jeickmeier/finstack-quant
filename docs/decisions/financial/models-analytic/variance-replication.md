# MODELS-ANALYTIC-21: Discrete variance replication, log-contract anchor and wing extension

**Decision status:** Accepted (complete package adopted 2026-11-10).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and caller paths inspected. Independent scalar and high-resolution numerical checks support the proposed distinctions. Production and end-to-end qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked variance replication and explicit Black wings

All 26 decisions were accepted on **2026-11-10**. Implementation reconciliation
and independent complete-path qualification remain open.

1. **Define the single-expiry variance quantity.** Estimate annualized
   log-contract variance from the valuation origin to one positive
   horizon T. For a strictly positive underlying with a continuous
   multiplicative diffusion, deterministic proportional carry and
   deterministic discounting, the full continuum strip gives
   `(2/T) × integral Q(K)/K² dK`. Here Q is the undiscounted OTM
   premium per underlying unit in the same price units as F and K.
   The split is at F. The identity uses a true forward-normalized
   martingale and the required log payoff and stochastic-integral
   integrability. Under those conditions the strip gives expected
   continuous integrated variance divided by T.

2. **Require qualified forward, clock and quote inputs.** Use finite
   positive F and T, with F in the same price units as strike.
   The caller binds the actual underlying, horizon, forward basis
   and model clock. Use the actual unshifted Black annual-volatility
   strike source under record 16 and ADR-0045. Keep physical
   observation dates and source coordinates with the dated owner.

3. **Keep one fallible volatility callback.** Bind the active source
   and expiry in the Rust adapter. The model requests sigma by
   strike and receives a checked result. Evaluate only required
   strike samples inside the finite strip. The product owner
   supplies the checked expiry resolution and any explicitly
   supported source boundary policy. Remove the generic option
   price callback and risk-free-rate argument.

4. **Validate a two-sided original strike grid.** Require at least
   three finite positive strictly increasing original strikes,
   including a strike strictly below and above F. Reject duplicate,
   one-sided and thin grids before invoking the source. Use the
   original represented levels, with no minimum strike or
   moneyness threshold.

5. **Split at the exact forward.** Retain F if already present,
   otherwise insert the exact represented F between its enclosing
   strikes. Sample it once from the same source. At this split
   put and call premiums agree, so use one common ATM premium.
   The log-contract anchor correction is exactly zero at F.

6. **Use OTM Black premiums throughout.** Use the put below F and
   call above F, with the shared ATM sample at F. Price them on
   the same F, T and sampled sigma in the canonical checked
   record 02 forward law. Use undiscounted unit premium, so
   discount conversion is unnecessary in the strip integral.
   Cboe index strike selection and correction are separately
   defined benchmark conventions.

7. **Require checked quote and premium arithmetic.** Each sampled
   annual sigma must be finite and nonnegative. Zero is admitted
   and gives the exact zero OTM or ATM premium. Propagate the
   first source or model error with its strike. Remove the
   minimum-volatility floor and unchecked cash premium path.
   Use the same checked Black law and the private log-OTM geometry
   in record 03 when needed for a representable weighted
   contribution. The requested output is variance.

8. **Validate all controls before sampling.** Validate the original
   axes, F, T, selected wing mode, estimator budgets and checked
   work limits before callback execution. Set the split and
   refinement coordinates before pricing. Invalid geometry or
   collapsed required refinement errors at this stage.

9. **Use cash-strike trapezoids on the forward-split mesh.** For
   each panel of width h use `h/2 × (Q_a/K_a² + Q_b/K_b²)`.
   Put and call panels meet at F. Each outer endpoint has
   its own half-panel weight. Keep original knot values shared
   between adjacent panels.

10. **Use one deterministic midpoint refinement.** For each
    forward-split panel insert one checked, strictly interior
    arithmetic midpoint. Evaluate its implied sigma with the same
    source and its own OTM side. Evaluate each required strike
    once in ascending order and reuse values for the coarse and
    fine estimates. Return the fine finite-strip estimate.

11. **Require an explicit finite-strip mesh check.** Supply a
    finite nonnegative absolute tolerance in annual variance.
    Compare the absolute difference between the fine and coarse
    finite-strip estimates after annualization. Equality passes.
    A larger difference errors. Report the measured difference.
    This is an estimator agreement test on the supplied mesh.

12. **Keep complete panel contributions in scale.** Use checked
    widths and positive complete premium products with inverse
    strike squares and annualization. Avoid forming raw K²
    or 2/T when that loses the requested contribution. Sum in
    deterministic order using checked scaled compensated
    accumulation. A lost positive result or unresolved arithmetic
    errors.

13. **Require a wing policy explicitly.** Finite strip evaluates
    only the original strike range and reports both wings as
    omitted, with their amounts unknown. Full flat-Black wings
    extend the lower and upper endpoints to 0 and infinity
    respectively using each endpoint's own sampled sigma. This
    adds modeled wing contributions without further source queries.
    There is no fixed strike
    multiplier, narrow-grid heuristic or implicit tail choice.

14. **Use a positive finite-domain formula for full Black wings.**
    For each boundary let `ell = log(K_edge/F)`,
    `v = sigma_edge sqrt(T)`, and `s = v sqrt(r)`.
    Its annual variance contribution is
    `sigma_edge² × integral_0^1 Phi(eta (ell/s + s/2)) dr`,
    where eta is +1 for the lower put wing and −1 for the
    upper call wing. The r=0 limit is zero because both
    endpoints strictly straddle F. A zero edge sigma contributes
    exact zero after full validation.

15. **Use the Black vega identity behind the wing formula.** In
    normalized strike `x = K/F`, the derivative in total
    standard deviation of the normalized Black premium is
    `phi(d1)`, and `phi(d1)/x = phi(d2)`. Integrating this
    derivative against dx/x gives `s Phi(eta (ell/s+s/2))`
    on the selected wing. Integrate from zero total standard
    deviation and set `r = s²/v²`. This avoids cancellation
    between truncated lognormal moments and synthetic strike queries.

16. **Use bounded checked quadrature on the wing integrand.** Use
    the accepted scalar adaptive Simpson rule. The lower-wing
    integrand increases with r. The upper-wing integrand peaks
    at `r = 2 ell/v²` when this lies strictly inside (0,1).
    In that case split at its represented peak to sample the
    feature. If an interior split cannot be represented,
    propagate a computation error. Process lower then upper
    panels in increasing r.

17. **Give both wings independent numerical budgets.** Require
    separate finite nonnegative absolute estimator tolerances in
    annual variance, plus explicit max depth and callback
    evaluation limit per initial panel. Conserve the upper
    tolerance when splitting it across two panels. Apply
    CORE-NUMERICS-04's checked resource and convergence rules.
    Preflight at most three panels and their total work.
    Exhaustion or estimator failure errors. Passing the numerical
    gate does not establish the financial tail assumption.

18. **Preserve complete weighted wing probabilities.** Compute
    the edge log ratio from original coordinates and form
    `sigma_edge² Phi(...)` with scaled factors or checked
    log-tail evaluation. Preserve representable positive samples
    even when the probability alone would underflow.
    Use legitimate endpoint limits and the checked quadrature
    sample range. A failed required sample, lost geometry or
    a zero computed wing with positive edge sigma is a computation
    error. An exact zero edge sigma is the zero-wing identity.

19. **Keep core and wing amounts in annual-variance units.** Return
    the checked sum of the refined finite-strip estimate and
    any selected wing amounts. All contributions are
    nonnegative. Preserve genuine zero, including a zero
    sampled strip and zero modeled wings. Fail on a nonfinite,
    negative or lost-positive complete result.

20. **Return truthful diagnostics and work.** Carry the actual F,
    T, original strike range, forward insertion, coarse and
    fine core estimates and mesh difference, wing mode, edge
    sigmas and applicable modeled wing amounts and controls.
    Record actual quote and wing callback counts. Represent
    omitted wings explicitly. Report configured wing estimator
    budgets without inventing a realized error estimate.

21. **Use typed failures and preserve the requested method.** A
    bad source, failed mesh or wing gate, range failure or
    incompatible clock or convention remains visible to the
    caller. No implicit ATM variance, width floor, successful
    zero tail on error or change of selected wing policy.

22. **Reconcile the actual equity variance caller.** Replace its
    clamped sigma sentinel and unchecked discounted spot option
    callback with the qualified strike source and checked
    forward kernel. Preserve the explicitly selected flat-IV
    route, dated forward and funding, lifecycle, realized
    blending and forward-start total-variance subtraction at
    the equity product owner.

23. **Reconcile the actual FX variance caller.** Require the
    product's qualified dated forward, horizon and Black
    strike source. Remove the unchecked discounted Garman–
    Kohlhagen callback from the strip. The FX owner retains
    the cumulative variation, carry, partial-return,
    observation and annualization rules and downstream PV
    discounting.

24. **Keep the shared model Rust-only.** Reconcile the existing
    product and typed WASM instrument paths through their
    owning pricers, retaining error context. The shared
    callback, controls and diagnostics introduce no parallel
    Python, WASM or UI calculation.

25. **State the financial and numerical scope.** Mesh and wing
    estimator gates concern their selected numerical approximations.
    A finite strip leaves unknown tails. Full flat-Black wings
    integrate the chosen continuation. Market wing fit, continuous
    versus discrete realized payoff, jumps, source arbitrage and
    product suitability require separate qualification.

26. **Require independent and end-to-end evidence.** Cover the
    exact forward split, Black parity and zero-sigma limits,
    irregular panels, maturity effects and common price-scale invariance,
    finite-strip refinement failure, full-wing identities and
    cutoff bias, narrow peak and range cases, callback and
    work failures, dated source reconciliation and independent
    product repricing. See the
    [variance replication references](../../../REFERENCES.md#checked-variance-replication-and-explicit-black-wings).

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| F=T=1, edge strikes 0.8 and 1.2, both edge sigmas 0.25 | Normalized unannualized lower and upper wing integrals are about 0.003487728615 and 0.003600596963. The positive wing transform agrees with independent truncated-lognormal moments. |
| F=T=1, flat sigma 1, original strike range [0.9,1.1] | The current synthetic endpoints are 0.3 and 3.3. The Black annual variance outside them is about 0.11740842, independently of finite-strip quadrature bias. |
| F=T=1, flat sigma 0.25, original strikes (0.7,0.9,1.2,1.5) | Inserting F then bisecting each panel gives a coarse core estimate of 0.06606708 and a refined estimate of 0.06121262. Their annual-variance difference of 0.00485446 breaches a mesh tolerance of 0.002. |

The checks concern scalar formulas and numerical examples. Production
source resolution, valuation, integration and product qualification remain
pending.

## Observed implementation and evidence

### Scope and input domain

Annualized forward variance from≥3 strictly increasing positive strikes straddling positive forward, positive T and supplied vol/discounted-option callbacks.

### Choice and rationale

Carr–Madan/Demeterfi-labelled OTM replication uses trapezoids, an exact log-contract anchor and finite flat-vol wing extension. No automatic switch to ATM variance occurs on failure.

### Algorithm

Choose K0=largest strike≤F. Use puts below K0,calls above,mean(call,put) at K0. Sum ΔK Q(K)/K² with half neighboring widths, including half-width endpoints, using Neumaier accumulation. Query vols at nodes; require>1e−6. Extend lower tail to Kmin/3 using boundary put vol and upper to3Kmax using boundary call vol, each64 trapezoid subdivisions. Return (2 exp(rT)/T)integral−(2/T) × (F/K0−1−ln(F/K0)) if finite positive. Warn if either wing log-moneyness<.35.

### Selection and controls

Fixed wing factor3 and64 subdivisions; user controls input grid and callbacks but not those constants. Invalid grid/vol, nonpositive/invalid variance yields None. Forward/K0 must exceed1e−9. Invalid tail endpoint arithmetic returns zero tail contribution rather than a distinct error.

### Conventions

Callbacks must use compatible forward, discount, option type and annual volatility. The exp(rT) factor presumes prices already discounted at supplied continuous r. Returned quantity is variance, not volatility.

### Limitations, failure and fallback

Finite flat-vol wings omit farther tails and assume no wing slope; narrow-grid warning does not reject. Callback option prices are not individually checked for sign/upper bounds; only final aggregate is tested. The mean-at-K0/discrete anchor is an approximation to continuum replication and needs grid convergence.

### Alternatives and tradeoffs

ATM σ², adaptive continuum integration or extrapolated smile tails are alternative methods, not fallbacks here. Fixed tail extension buys limited truncation correction at bounded cost.

### Implementation and validation evidence

[variance_replication.rs](../../../../finstack-quant/models/src/volatility/variance_replication.rs). Exact-anchor test is a direct formula identity; dense-grid flat-vol and wing-bias tests reuse production BS prices and test numerical integration consistency, not real-market replication. Reference: [Demeterfi](../../../REFERENCES.md#demeterfi-1999-volatility-swaps).

The equity and FX pricers are the two production callers. Both currently
sample clamped implied sigma and price discounted spot options in callbacks.
The equity owner combines endpoint total variances for a forward-start
term. The FX owner also uses cumulative variation for its discrete
forecast. See the
[equity variance pricer](../../../../finstack-quant/valuations/equity/src/variance_swap/pricer.rs),
[FX variance pricer](../../../../finstack-quant/valuations/fx/src/fx_variance_swap/pricer.rs),
[equity product record](../valuations-equity-and-composition/15-variance-swap.md)
and [FX product record](../valuations-fx-and-exotics/fx-variance.md).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
