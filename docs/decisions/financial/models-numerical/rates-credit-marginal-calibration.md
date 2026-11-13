# MODELS-NUMERICAL-008: Rates/credit marginal calibration, hazard floor and variance loss

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Source and callers inspected. Independent rational and Decimal checks covered binary support, variance retention, hazard fitting, survival scale and separate marginal versus joint valuation. Production pricing, risk and external qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked rates-credit marginal calibration

1. **Keep two centered additive binary factors.** Each factor
   starts at centered state zero and uses its own explicit
   finite nonnegative kappa and sigma. Kappa is per model year
   and sigma is an absolute decimal rate or intensity volatility
   per square root of a model year. Fit a deterministic row
   level for each marginal. Apply mean reversion to the centered
   factor, independently of the fitted row level. The selected
   finite binary law matches the conditional mean and retains
   only the admitted fraction of the intended conditional variance.
   [Marginal calibration references](../../../REFERENCES.md#checked-rates-credit-marginal-calibration)

2. **Use one prepared uniform model grid.** Take positive
   finite T and a literal positive count N. The uniform
   mathematical width h=T/N is used throughout fitting,
   rollback and sampling. Use increasing checked coordinate
   labels with exact endpoints. Numeric labels use
   `T*(n/N)`. For dated ACT/365F grids, prove any required
   date-to-node alignment from integer day counts and retain
   the original dates. A qualifying whole-day grid may use
   those exact ACT/365F date labels. The product owns its
   chosen resolution and qualified timing treatment. No
   automatic step change or silent coordinate snapping.

3. **Bind both sources to the actual model origin.** Capture
   origin-relative discount factors D[0..N] and local integrated
   hazard H[0..N−1] from the selected market snapshots. Use
   each source's native day count, calendar, conditioning and
   tail policy after its checked model-to-source mapping.
   Keep fractional model times separate from actual event dates.
   Local integrated intensity is the canonical credit
   calibration target. Cumulative survival is a derived
   replay check. Remove ratios of rounded unconditional
   survivals as the target preparation method.
   [Relative discounts](../core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction),
   [local hazard and survival](../core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation)

4. **Admit explicit discount and intensity targets.** Require
   D[0]=1 and positive finite later D, including signed-rate
   regimes with factors above one. Require finite nonnegative
   H for every interval. The target log survival at maturity i
   is the checked accumulation `-sum_{k<i} H[k]`.
   Exact zero H represents a flat survival interval. Retain a
   positive H even if the corresponding survival rounds to one
   or zero. Recovery, OAS and coupling are outside both
   marginal target equations.

5. **Match the exact centered conditional mean with fixed binary spacing.**
   For an active factor use `delta=-expm1(-kappa*h)` and
   `a=sigma*sqrt(integral_0^h exp(-2*kappa*u) du)`.
   At step n and index j let `z=2*j-n` and `x=z*a`.
   Child j+1 moves up with `p=(1-delta*z)/2` and child j
   moves down with weight 1−p. In real arithmetic
   `E[x_next|x]=exp(-kappa*h)*x` and the conditional
   variance is `a^2*(1-(delta*z)^2)`. At exact kappa=0,
   recover the equal-probability additive law and
   `a=sigma*sqrt(h)`. The rate marginal then agrees with
   the accepted continuous-base Ho–Lee law on the same grid.
   [Exponential variance integration](../core-numerics/piecewise-coefficients.md#accepted-decision-checked-piecewise-coefficients-and-exponential-integration),
   [Ho–Lee](ho-lee-curve-fit.md#accepted-decision-for-checked-holee-curve-fitting)

6. **Admit the raw binary law by support and variance retention.**
   On each positive-sigma factor require strict interior
   branch weights and one explicit owner-qualified minimum
   retention in (0,1]. The smallest real retention over
   departing nodes is `1-((N-1)*delta)^2` when support
   is valid. Check the complete support and retention before
   fit and check the represented transitions before use.
   Evaluate represented retention from the same pair of
   admitted weights, `4*p_up*p_down`.
   Publish the worst retention and its earliest node in
   step and storage order. Remove the kappa-only 0.15
   ceiling, sigma floor and probability clamp. A grid
   that fails the test returns an unsupported-model error.

7. **Use one state for each exactly deterministic factor.**
   With sigma=0, propagate one centered zero state and one
   unit branch per time, regardless of its admitted kappa.
   Retain fitted rate or intensity variation across time.
   Raw innovation variance retention is unavailable on that
   axis. Do not create an artificial Bernoulli innovation.
   Check genuinely positive stochastic spacing and complete
   node and weight arithmetic in the other cases.

8. **Prepare the transitions once.** Reuse the admitted
   geometry, branches and weights in calibration, forward
   replay, rollback and sampling. Use stable arithmetic for
   the OU decrement, spacing and complementary probabilities.
   Preserve required positive state-price contributions in
   scale through the fit and replay. The joint law must
   preserve these two marginals. Record 009 owns the actual
   coupling, feasible correlation range and random draws.
   [Joint coupling and sampling](rates-credit-coupling-and-sampling.md)

9. **Fit the signed Continuous short-rate marginal.** At a
   departing row with centered factors x_j and discount
   state prices Q_j, fit the rate center
   `alpha=(log(sum_j Q_j*exp(-h*x_j))-log(D[n+1]))/h`.
   Start with mass one and propagate discounted state prices
   with the stored rate branches. Use the actual incoming
   discount mass and stable checked arithmetic. The root and
   deterministic rows use the same one-state identity.
   Keep the base short rate signed through pricing and
   callbacks. OAS follows the checked rollback contract.
   [Tree discount and OAS](tree-discounting-and-induction.md#accepted-decision-for-checked-tree-discounting-and-rollback)

10. **Select a positive-part effective hazard law.** Let
    d_j be the nonnegative distance from a node's centered
    factor to the row maximum. Fit the top raw intensity b
    and define raw hazard `b-d_j`, effective hazard
    `lambda_j=max(b-d_j,0)`. Use effective hazard in the
    departing survival and default calculation. The centered
    factor and its probabilities keep their own law across
    the floor. This is a local choice for this lattice,
    separate from the market hazard source and product recovery.

11. **Fit the local loss using actual survival state prices.**
    Start survival-only weights q with mass one. For
    W=sum(q), set w_j=q_j/W and fit
    `H_model=-log(sum_j w_j*exp(-h*lambda_j))=H[n]`.
    Propagate `q_j*exp(-h*lambda_j)` with the stored hazard
    branches. Do not substitute the previous quoted survival
    for W. The local fit carries any already accumulated
    residual forward for the separate cumulative replay check.

12. **Use the exact flat row and direct one-node fit.** When
    H[n]=0, choose b=0, the largest top level with every
    hazard at zero. The row preserves incoming survival mass
    exactly under the model law. All-floored equality is
    feasible and does not require a solve. For a positive
    H[n] and a single node, set b=H[n]/h. A positive
    target requires positive modeled local loss.

13. **Use a proven bounded bracket on positive multi-node rows.**
    Let `A=H[n]/h>0` and `d_max=max d_j`. The top
    level lies between A and A+d_max. At the lower bound
    all effective hazards are at most A, and at the upper
    bound all are at least A. The weighted loss is strictly
    increasing on the active region. When the target
    default complement `L=-expm1(-H[n])` is below the
    positive top-node weight, the upper bound can be tightened
    to `-log1p(-L/w_top)/h` if smaller. Use bounded,
    fallible Brent with checked endpoints, explicit
    tolerances, update and work limits. A qualified endpoint
    can meet the local gate. Failed objectives, brackets or
    solves return errors. Remove zero-shift, midpoint and
    saturated-success substitutes.
    [Checked Brent](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search)

14. **Preserve local and cumulative survival scale.** Compute
    the weighted local default complement with expm1 and
    recover log survival with log1p near one. Use a stable
    log sum of surviving terms in the tail. Preserve finite
    scaled weights and check effective hazard and local
    integrated hazard on their required interfaces. Use
    the checked source probability conventions for rounded
    output factors. Reject unresolved required arithmetic.

15. **Replay both fitted marginals before publication.** On
    the stored departing values and branches, require each
    discount relative error in bp to meet one explicit finite
    positive tolerance. Require one explicit survival
    tolerance in (0,10000) relative bp, applied to both
    local positive integrated hazard error and cumulative
    survival relative error. Exact zero-loss intervals
    retain exact modeled zero loss. Compute cumulative
    survival relative error from the checked log ratio.
    Report the worst finite errors and earliest attaining
    intervals or maturities in their respective units.

16. **Measure the floor on genuine departing hazard rows.**
    Report incoming survival-state-price mass at effective
    hazard zero and the strictly negative raw share
    separately, each normalized by actual incoming mass.
    Include the root and exclude terminal. Report exact
    zero-loss rows and the earliest worst floor share.
    On positive-volatility rows with positive local H, apply
    an explicit owner-qualified inclusive maximum zero-hazard
    share in [0,1]. A fitted plateau can legitimately put all
    mass at zero. Deterministic and exact zero-loss rows are
    outside this stochastic floor gate.

17. **Distinguish raw variance loss from the hazard-floor effect.**
    Where both next nodes have departing hazards, the
    conditional effective-intensity variance is
    `p*(1-p)*(lambda_up-lambda_down)^2`. Use it to
    qualify floor distortion separately from the checked
    raw variance retention. Floor mass, variance retention
    and fit residuals have distinct meanings. Option or
    market price tolerances require separate evidence.

18. **Keep the genuine terminal factors and check actual work.**
    Terminal state has no fitted departing rate, hazard,
    survival factor or converted OAS. Use checked indexed
    access and actual marginal widths. Bound marginal fit,
    replay and any required joint storage/work before
    allocation. Two active factors have (n+1)^2 joint states
    at level n, one active factor has n+1 and neither has
    one. Count solver work and publish the complete validated
    candidate atomically. A failed request preserves prior
    calibration. Joint publication also needs its owning
    coupling checks.

19. **Reconcile the bond, term-loan and host preparation.** Keep
    the joint bond origin at valuation and the term-loan
    origin at settlement. Use one prepared grid and native
    source factors through each selected price and payoff
    path. Preserve the bond's qualified dated final-payment
    handling and the loan's separate settlement carry.
    Product owners qualify control values, deterministic
    routes, bump stencils and OAS inversion and propagate
    infeasible requests. Reconcile existing Rust, Python and
    WASM product inputs and errors. Keep lattice state Rust-only.
    [Bond PV dispatch](../valuations-bonds/03-pv-models.md),
    [term-loan pricing](../valuations-other-fixed-income/08-term-loan-pricing.md)

20. **Keep joint economics and validation with their owners.**
    The separate fits identify risk-free discount and
    undiscounted survival marginals. Joint risky discounting
    depends on coupling. Recovery, exercise and coupon
    folding remain with record 010 and their products.
    Require independent marginal replay, tail and floor
    cases, and aligned joint price and risk qualification.
    [Joint coupon and recovery treatment](rates-credit-coupon-folding.md)

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| Active factor with delta=1/8 | At departing level four the extreme up weights are .25 and .75, and minimum raw variance retention is .75. At level eight the extreme weights are zero and one, so N=9 fails strict support. |
| Kappa=.15, T=10 | The worst absolute probability skew `(N-1)*delta` is about .52763 at N=2, 1.25363 at N=10 and 1.47392 at N=100. A finer grid can move this binary law outside its admitted support. |
| Hazard weights (1/4,3/4), distances (0,log 2), h=1 | For H=log(16/15), the fitted top intensity is log(4/3). Conditional survival is exactly 15/16 and the zero-hazard share is 3/4. H=0 gives the all-zero effective row and survival one. |
| Local H=2^−60 | Binary64 survival rounds to one. The local default complement remains about 8.67361738e−19. |
| Unconditional integrated hazard exceeds 1000 before model origin. Next local H=.02. | Both unconditional endpoint survivals round to zero. The local survival is about .9801986733. |
| Equally likely discount values (.90,1.05) and survival values (.80,1) | The marginal means are unchanged across couplings. Joint discounted survival is .8775 independently, .885 with like pairs and .87 with crossed pairs. |

These checks distinguish the marginal fit, admitted geometry and joint
valuation. Production execution, external joint prices and actual risk
paths remain open.

## Observed implementation and evidence

### Choice and domain

[calibration.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/calibration.rs) fits rate and hazard marginals separately to conditional discount factors and survival probabilities. [mod.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/mod.rs) defaults to 100 steps and zero volatility, mean reversion and correlation. Inputs have N+1 equally spaced times starting at zero, finite positive factors starting at one, nonincreasing survival≤1, and recovery in [0,1]. Volatilities are absolute normal rate/intensity units per sqrt(year); κ is annual and bounded to [0,0.15]. Negative rates are allowed; effective hazard is max(raw hazard,0).

### Algorithm and controls

Set each root to −ln(target[1])/dt. Rows are affine base+node·2σ√dt; next base starts one σ√dt below the current base. Reversion changes up probability to clamp(1/2−κ(x−x_ref)√dt/(2 max(σ,1e−12)),0,1), with x_ref fixed at the first-period rate/intensity. Calibration and pricing use the same probabilities on raw factor levels. Discount rate state prices by exp(−r dt), hazard state prices by exp(−max(h,0)dt).

For the next row's target maturity, unfloored drift has the analytical log-ratio solution; invalid/nonpositive model sums fall back to zero shift. Floored hazard drift solves ΣQ exp(−max(base+θ,0)dt)=target. Its lower bracket floors every node; if target≥incoming mass, return that all-floored shift and increment `unreachable_steps` (even equality is counted). Otherwise expand the upper bracket up to 200 times and use Brent. **Solver failure returns the bracket midpoint**, without a reported root error or final repricing gate. Build a candidate and publish only after correlation feasibility succeeds, but these internal numerical fallbacks remain successful calibration paths.

### Diagnostics, alternatives

Floor diagnostics report state-price-weighted mass at zero and worst step. Variance retention reports min 4p(1−p), clamp count/share and worst step over all pricing rows, without probability-mass weighting. A κ cap alone cannot control κT; long horizons can have deterministic tail moves. Additive dynamics plus a nonlinear hazard floor are an observed approximation. Strict failure/fit gates, acceptance limits for floor mass/variance loss, and positive-intensity alternatives such as CIR/lognormal lattices are not implemented here. Terminal rows are extrapolated with zero new shift.

The shared product adapter currently divides unconditional
survivals to prepare relative targets. The joint bond path uses
valuation-relative dates. Term-loan preparation uses settlement,
but its payoff grid independently computes discount timing from
unrebased factors. The bond vega regression exercises a saturated
down-volatility bump. That observed stencil requires qualification
under the proposed support and variance limits.
[Shared rates-credit target preparation](../../../../finstack-quant/valuations/core/src/instruments/pricing/rates_credit.rs),
[joint bond preparation](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/tree_pricer.rs),
[term-loan payoff preparation](../../../../finstack-quant/valuations/fixed-income/src/term_loan/pricing/tree_engine.rs),
[bond vega](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/vega.rs)

### Evidence

[tests.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/tests.rs) includes `calibration_quality_survival_matching` (forward survival reconstruction to 1e−6), `calibration_quality_zcb_repricing`, `hazard_floor_saturation_reports_binding_floor`, `variance_retention_is_bounded_by_one_minus_kappa_t_squared`, and `long_horizon_clamping_is_reported_not_silent`. They establish selected invariants and expose distortion, not independent callable-credit accuracy. [Brent](../../../REFERENCES.md#brent-1973) covers the root method; the midpoint fallback, κ limit and floored marginal construction lack a dedicated adoption reference.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
