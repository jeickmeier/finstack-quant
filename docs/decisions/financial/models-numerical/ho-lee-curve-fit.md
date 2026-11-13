# MODELS-NUMERICAL-004: Ho–Lee additive lattice and curve-fitting drift

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Source and callers inspected. Independent exact and Decimal references were executed for target anchoring, row fitting, state prices and a finite-tree bond option. Production, external option and risk qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked Ho–Lee curve fitting

1. **Select the additive Ho–Lee law explicitly.** Use the
   one-factor recombining short-rate lattice with equal risk-neutral
   up and down weights. Mean reversion is exactly zero and is
   not a Ho–Lee control. If a shared configuration still takes
   mean reversion, validate exact zero for Ho–Lee. Hull–White
   owns the separate normal mean-reverting model.
   [Ho–Lee calibration references](../../../REFERENCES.md#checked-holee-curve-fitting)

2. **Declare the rate basis and absolute volatility.** Require
   finite nonnegative constant sigma in annual decimal rate
   units per square root of a model year. Select Continuous,
   Simple or Periodic(m) for the node quotes and discounts.
   Sigma describes the quoted node rate on the selected basis.
   Changing the basis with the same numeric sigma can change
   the rate law and option PV. Admit signed rates and positive
   discounts above one where the selected convention permits
   them. Use the checked base discount and OAS rules in
   [record 003](tree-discounting-and-induction.md#accepted-decision-for-checked-tree-discounting-and-rollback).

3. **Require a literal positive uniform step count.** Take finite
   T>0 and positive N. Use h=T/N for every outgoing interval
   and canonical model labels `T*(n/N)` with exact endpoints and
   strictly increasing represented times. Use the same prepared
   width, grid and node ordering in target resolution, fit,
   rollback, OAS and risk. Check representability before fitting.
   Product owners resolve maturity and cash or exercise timing.
   A matured claim follows its lifecycle before calibration.

4. **Prepare targets relative to the model's actual origin.**
   Resolve the correct discount source and validate its coverage
   at the origin and all target coordinates. Capture one
   immutable sequence `P[0..=N]` with exact P[0]=1 and finite
   positive P[n] for each later knot. A target is the source's
   relative factor from the model origin to that knot. Dated
   knots use the source's dated context. Undated uniform nodes
   require an explicit mapping to admitted source coordinates.
   Adding model-relative time to the source origin is valid only
   when the selected clock justifies that mapping. Propagate
   missing context or source coverage errors. Positive targets
   may increase or be subnormal. No re-query, date snapping,
   monotonicity repair or unrebased `D(model_time)` fallback.
   [Source-owned discount evaluation](../core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction),
   [model-to-curve clock bridge](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection)

5. **Build each recombining row from a common center.** For
   positive sigma use `a=sigma*sqrt(h)`,
   `x[n,j]=(2*j-n)*a` and `r[n,j]=alpha[n]+x[n,j]`.
   At node j the children are j and j+1 with weight 1/2 each.
   Fit one absolute center alpha[n] per departing row. The
   difference between adjacent centers is the deterministic
   increment. Construct a node once from its index and center.
   Reject unresolved required spacing, state or rate arithmetic.
   A positive sigma never substitutes a deterministic tree.

6. **Use an exact deterministic route at sigma=0.** Use one
   state per time and branch weight one. Set each outgoing
   rate to the checked inverse discount quote for its one-step
   target ratio `P[n+1]/P[n]`. Preserve the requested times,
   eligibility, base compounding and fit gate. Zero volatility
   needs no row solver. A one-node outgoing row also uses the
   checked direct inverse, including the root for positive sigma.

7. **Fit to Arrow–Debreu state prices.** Start with Q[0,0]=1.
   For outgoing row n solve
   `sum_j Q[n,j]*DF(r[n,j],h)=P[n+1]`.
   Propagate its discounted state-price contribution to each
   child with the owned weights. Q includes prior base discount.
   Use actual model state prices from the fitted preceding rows.
   The curve fit uses no OAS, credit or product payoff.

8. **Fit Continuous rows directly.** For positive-volatility
   multi-node rows compute
   `alpha[n]=(log(sum_j Q[n,j]*exp(-h*x[n,j]))-log(P[n+1]))/h`.
   Use stable positive sums and logarithmic accumulation so
   unshifted factors and intermediate products do not force an
   avoidable range failure. Check reconstructed node quotes
   and their required outgoing factors.

9. **Fit Simple and Periodic rows inside their common domain.**
   Parameterize the row by its minimum rate rho and fixed
   nonnegative offsets. Let W be the sum of Q on the row,
   w_min the normalized state-price weight at the minimum-rate
   node, and A=P[n+1]/W. With `inv` the selected inverse DF on h,
   the real root is bracketed by `inv(A/w_min)` and `inv(A)`.
   At the lower bound the minimum-rate node's contribution equals
   the target before adding the other terms. At the upper bound
   every node's DF is at most A.
   The row discount sum is strictly decreasing on the common
   domain, with rho>−1/h for Simple and rho>−m for Periodic(m).
   Never require an unshifted row or an initial guess to be valid.

10. **Use checked bounded Brent for nontrivial noncontinuous rows.**
    Evaluate the bracket and residual with scaled or logarithmic
    arithmetic as needed. Use the log model-to-target ratio as
    the residual. Require a representable valid bracket and
    checked sign information, or an admissible endpoint already
    satisfying the fit gate. Allow representable outward endpoints
    only within the admitted domain and selected work limits.
    Require the accepted explicit positive Brent tolerance and
    update limit. Count objective evaluations in the calibration
    work budget. Report failed arithmetic, absence of a usable
    bracket, or work exhaustion. Do not floor a trial base or
    treat a failed objective as a sign. A numerical root still has
    to pass replay on the prepared tree.
    [Checked Brent](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search)

11. **Preserve the scale of the complete fit.** Accumulate positive
    state prices and discounted contributions with stable scale
    or log arithmetic. Avoid premature loss of tail mass,
    cancellation in quote construction, and overflow or underflow
    of a detached intermediate. Required outgoing DFs, states
    and diagnostics must be representable on their admitted
    interfaces. Remove the hardcoded node-factor window
    [1e−30,1e6]. Reject unresolved required values, invalid
    compounding domains and nonfinite diagnostics.

12. **Use one inclusive fit gate on the actual prepared state.**
    Replay every target maturity from the final stored outgoing
    quotes and weights using the same base operator as pricing.
    For each maturity i compute
    `e_i=10000*abs(P_model[i]-P[i])/P[i]`
    in relative discount-factor bp. Require every finite e_i
    to be at most the one explicit finite positive tolerance.
    Publish the worst error and earliest attaining maturity
    index in 1..=N. Remove the separate hardcoded <1 bp check
    from the Ho–Lee contract. Shared BDT/BK diagnostic callers
    are reconciled with their own model records.

13. **Separate the terminal factor from interval rates.** Fit
    exactly N departing rows. The terminal level can expose
    its genuine centered random-walk state for qualified payoffs.
    It has no fitted interval rate, outgoing DF, or interval OAS
    shift. The rate accessor errors at the terminal level.
    Product owners handle terminal and later-dated cash under
    the accepted rollback contract.

14. **Check resources and publish calibration atomically.**
    Require explicit positive work controls for calibration and
    pricing. Check node, branch and allocation sizes for the
    selected geometry and account for row solver evaluations.
    Use adjacent state-price scratch rows or equivalent linear
    workspace for propagation. Preserve the literal selected
    N. Publish validated grid, fitted state and diagnostics
    together after replay. Any failure leaves the previous
    calibrated state usable.

15. **Reconcile the live product owners and hosts.** The callable
    term-loan rates-only path prepares its targets from settlement
    and keeps settlement-to-valuation carry as a separate product
    operation. The private/direct bond Ho–Lee route prepares
    targets from its valuation origin. Feed the same checked
    grid and factors to calibration and product cash preparation
    where they represent the same discount assumption. Keep
    event mapping, lifecycle, OAS quote, defaults and risk
    experiments with their owners. Retain the public bond model
    selection boundary and Rust-only lattice state. Update existing
    product, risk and host error paths.
    [Term-loan pricing](../valuations-other-fixed-income/08-term-loan-pricing.md),
    [bond tree pricing](../valuations-bonds/04-rates-tree-exercise.md)

16. **Qualify option and risk accuracy separately from the fit.**
    Test shaped and shifted-origin curves, signed rates, exact
    zero volatility, near-domain noncontinuous rows, tiny times
    and failed publication. Use independent state prices or path
    enumeration for the finite tree. Compare supported option
    regimes with independently qualified prices on the selected
    clock and law. For Continuous Ho–Lee, use continuous-time
    bond options as a convergence comparator. The initial curve
    fit determines the centers for the supplied sigma. Option
    calibration, finite-grid exercise error and risk tolerances
    require separate evidence.
    [Ho–Lee calibration references](../../../REFERENCES.md#checked-holee-curve-fitting)

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| Source D(.5)=.99, D(1)=.97, D(1.5)=.92. Root at source time .5 and h=.5. | Prepared targets are 1, 97/99 and 92/99. An undiscounted terminal payment of 100 gives a root PV of about 92.92929293. The unrebased targets .99 and .97 give 97 for that payment. |
| Continuous h=.5, sigma=.02, targets 1, 1.01, 1.015, 1.02 | Independent path factorization gives outgoing centers about −.01990066170634, −.00982656369783 and −.00962803627117. Path enumeration recovers all three targets. |
| Simple h=1, sigma=2, P[1]=10/11, P[2]=100/99 | Fitted root rate .1 and next rates −.5 and 3.5. An unshifted row built from the root has minimum rate −1.9 outside the valid Simple domain. |
| Periodic(2), h=1, sigma=4, P[1]=100/121, P[2]=16400/9801 | Fitted root rate .2 and next rates −1 and 7. An unshifted row built from the root has minimum rate −3.8 outside the valid periodic domain. |
| Continuous N=T=1, sigma=20, P[1]=1 | Root rate is zero and the centered terminal states are −20 and 20. The terminal unit payoff has PV 1. The existing terminal guard would compute a fictional factor of exp(20) at one terminal state and reject it. |
| Continuous source D(u)=exp(−.01u−.02u²), root u=1, h=.5, sigma=.02. Call at t=.5 on a unit ZCB maturing at t=1, strike .965. | Finite-tree PV ≈ .001253395039045. The continuous-time Gaussian Ho–Lee comparator gives ≈ .001081502988713 on the same re-anchored curve and volatility. |

## Observed implementation and evidence

### Choice, scope and rationale

[ho_lee.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/ho_lee.rs) fits each grid discount factor using additive normal moves and a deterministic row shift. This permits negative rates and exact recombination without mean reversion. [config.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/config.rs) defaults to Ho–Lee, N=100, absolute normal σ=0.01 per sqrt(year), continuous compounding and a 0.1 bp relative-DF fit tolerance. These are observed defaults, not adopted calibrations.

### Algorithm and controls

Use uniform dt=T/N. Set r0 by inverting the configured compounding against P(0,dt). From each row propagate Arrow–Debreu prices with probability 1/2 and configured node DF, and build next base rates r±σ√dt. To fit the following maturity, continuous compounding uses θ=(ln ΣQ_next exp(−r_base dt)−ln P_target)/dt. Other conventions solve ΣQ_next df(r_base+θ,dt)=P_target with core `BrentSolver::new`, initialized by the continuous expression. Add θ to the whole next row. The terminal row has θ=0 because no post-expiry discount interval is used.

Recompute all maturity DFs by forward induction on the stored lattice. Error is |modelDF−targetDF|/targetDF×10000, not yield basis points. Reject error above configured tolerance or any node DF outside [1e−30,1e6], including terminal-row diagnostic DFs. [tree.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/tree.rs) validates finite positive targets and horizon, nonnegative volatility, positive steps/tolerance, and calibrates a candidate before replacing previous state. Ho–Lee rejects mean reversion with magnitude above 1e−12.

### Alternatives and limitations

The supported normal mean-reverting alternative is the separate Hull–White engine. Fitting the discount curve does not calibrate σ to option prices or validate callable-option accuracy. `TreeCalibrationResult::is_acceptable` uses a separate hardcoded <1 bp threshold, distinct from the configured default 0.1 bp. Core Brent tolerances are inherited rather than exposed in this tree config.

The bond dispatch calls the additional diagnostic helper for BDT
and Black–Karasinski. Its private Ho–Lee route builds time from
valuation date and passes the original curve into calibration.
The rates-only callable term-loan route builds time from settlement
and also passes the original curve. The fit reads its factors at
local grid times. On a shaped curve, a passing fit can therefore
match factors anchored at the source base date. Loan payoff
preparation reconstructs its own local times and factors.
[Bond tree preparation](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/tree_pricer.rs),
[term-loan tree preparation](../../../../finstack-quant/valuations/fixed-income/src/term_loan/pricing/tree_engine.rs)

### Evidence

[tests.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/tests.rs): `ho_lee_stored_lattice_prices_zero_coupon_to_calibration_curve` checks calibration/rollback duality to 1e−8; `ho_lee_noncontinuous_compounding_reprices_curve` tests Simple and several periodic conventions to 0.1 relative-DF bp; failure tests verify preservation of old state. [Brent](../../../REFERENCES.md#brent-1973) is the root-method reference and [Brigo–Mercurio](../../../REFERENCES.md#brigo-mercurio-2006-interest-rate-models) is background. These are not independent option-price benchmarks.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
