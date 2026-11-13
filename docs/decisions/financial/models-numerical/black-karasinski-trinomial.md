# MODELS-NUMERICAL-006: Black–Karasinski capped log-rate trinomial

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Source and callers inspected. Independent rational and Decimal references checked transition moments, capped geometry, curve replay and a finite-tree bond option. Production pricing, risk and external qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked BK trinomial fitting

1. **Select a constant-parameter Black–Karasinski lattice.**
   Write the outgoing node quote as `r=exp(beta[n]+x)`, where
   the centered log-rate factor x starts at zero and has
   mean-reverting dynamics `dx=-kappa*x*dt+sigma*dW`. Require
   finite strictly positive kappa per model year and finite
   nonnegative sigma in log-rate units per square root of a
   model year. The model fits the curve through the departing
   row centers. The original Black–Karasinski model supports
   broader time-dependent parameters and market calibration.
   [BK references](../../../REFERENCES.md#checked-bk-trinomial-fitting)

2. **Declare the quote basis and controls.** Select checked
   Continuous, Simple or Periodic(m) base node compounding.
   Sigma describes the logarithm of the selected rate quote.
   Keep volatility, compounding, calibration tolerance and
   numerical work explicit. The selected base rate is positive.
   OAS retains its separate quote and discount contract.
   [Checked tree discounting](tree-discounting-and-induction.md#accepted-decision-for-checked-tree-discounting-and-rollback)

3. **Use one prepared uniform grid and origin-relative curve.**
   Require finite T>0 and literal positive N. Use h=T/N and
   checked increasing canonical labels `T*(n/N)` with exact
   endpoints. Freeze qualified source factors from the actual
   model origin, with exact P[0]=1 and
   `1>P[1]>...>P[N]>0`. The clock, dated mapping and target
   preparation follow the accepted rates-tree contract. Reject
   represented flat or rising base intervals, including at
   zero volatility.
   [Origin-relative grid and targets](ho-lee-curve-fit.md#accepted-decision-for-checked-holee-curve-fitting),
   [positive-rate target admission](bdt-lognormal-curve-fit.md#accepted-decision-for-checked-bdt-curve-fitting)

4. **Match the exact conditional OU mean and variance.**
   For positive sigma use `delta=-expm1(-kappa*h)`,
   `v=sigma^2*integral_0^h exp(-2*kappa*u) du`
   and spacing `dx=sqrt(3*v)`. The conditional mean of
   the next centered factor is `exp(-kappa*h)*x`.
   Use checked scaled evaluation and the accepted exponential
   integral. Preserve positive required spacing and the
   complete moment arithmetic. This changes the current
   Euler moment inputs. The finite distribution remains a
   trinomial approximation.
   [Checked exponential variance integration](../core-numerics/piecewise-coefficients.md#accepted-decision-checked-piecewise-coefficients-and-exponential-integration)

5. **Set the width cap using the actual mean decrement.**
   Use the conservative Hull–White threshold 0.184. Choose
   the least positive J within N with `J*delta>=0.184`, or
   J=N when the threshold is beyond the requested horizon.
   At step n use centered indices `-min(n,J)..=min(n,J)`
   in ascending order and x=j*dx. Use checked cap comparisons
   before large quotient or integer conversion. Shift edge
   branches only at departing levels whose width has reached
   the cap and stays there. The requested horizon is never
   extended.
   [Capped branching reference](../../../REFERENCES.md#checked-bk-trinomial-fitting)

6. **Use explicit moment-matched branches.** Put
   `z=j*delta` and `M=1/3+z*z`. In the interior,
   offsets (+1,0,−1) receive
   `((M-z)/2,1-M,(M+z)/2)`. At the upper active edge,
   offsets (0,−1,−2) receive
   `(1-3*z/2+M/2,2*z-M,(M-z)/2)`.
   At the lower active edge use offsets (+2,+1,0) with
   the reversed upper-edge weights from the mirrored parent.
   Before rounding the index displacement has mean −z
   and variance 1/3 in all three cases.

7. **Check probabilities and children before valuation.**
   Interior weights are feasible for `abs(z)<=sqrt(2/3)`.
   Shifted edges are feasible for
   `1-sqrt(2/3)<=abs(z)<=1+sqrt(2/3)`.
   The selected cap keeps interior displacements below .184
   and active edge displacements between .184 and one in
   real arithmetic. Require checked finite weights
   in [0,1] and the accepted sum check in represented
   arithmetic. Check every child against the prepared next
   row. Use the same stored children and weights for calibration,
   replay and rollback. No probability repair, renormalization
   or skipped branches.

8. **Use the exact one-state limit at sigma=0.** Keep positive
   kappa admission, the selected grid and positive base
   targets. Use one centered state and weight one. Obtain
   each outgoing rate by checked inverse on `P[n+1]/P[n]`.
   Replay it with the same fit gate. For a stochastic tree,
   its one-node root also uses direct inverse. Zero volatility
   uses no stochastic width or row solver.

9. **Fit log-rate centers to actual incoming state prices.**
   Start at Q[0,0]=1 and require each positive target
   P[n+1] to be below actual incoming mass W=sum Q[n,j].
   Solve `sum_j Q[n,j]*DF(exp(beta[n]+x_j),h)=P[n+1]`
   using the base discount. Propagate discounted state
   prices with the prepared trinomial. Use the same
   positive-rate feasibility rule and scalar log-center
   calibration as in accepted BDT, with BK's own transitions.

10. **Use a checked bounded fit for multi-node rows.** With
    `r*=invDF(P[n+1]/W,h)>0`, the real center lies between
    `log(r*)-max_j x_j` and `log(r*)-min_j x_j`.
    Check represented endpoints and signs. Use the log of
    model DF over target DF as the fallible objective with
    explicit Brent tolerance and update limit, subject to
    calibration work. An admissible endpoint can satisfy
    the same fit gate. Failed brackets, failed objectives
    and exhausted work return errors.
    [Checked Brent](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search)

11. **Preserve scale through the final outgoing quotes.**
    Use stable log-rate discounts and scaled or logarithmic
    positive sums for trial fits and forward state prices.
    Form the complete log quote before exponentiating.
    Require positive finite stored base rates and outgoing
    factors. Reject unresolved required results. Remove
    the floored forward guess and synthetic guess fallback.
    Unresolved arithmetic cannot substitute a different
    rate law, width or mean reversion.

12. **Use one inclusive stored-state replay gate.** Replay
    every maturity with the final base rates and checked
    transitions. Require finite
    `10000*abs(P_model[i]-P[i])/P[i]` at each i and
    acceptance at or below the one explicit finite positive
    relative-DF bp tolerance. Report the maximum and earliest
    attaining maturity in 1..=N. Retire the remaining secondary
    <1 bp BK bond-caller gate. Together with accepted BDT,
    this retires the shared secondary fit helper.

13. **Keep only the genuine terminal state.** There are N
    departing rate rows. The terminal state can supply the
    centered factor where needed. Its outgoing interval rate,
    DF and converted OAS are absent. Remove the repeated last
    center as a terminal quote and return an error from an
    interval-rate query at step N. Product-owned displaced
    cash requires its own qualified dated discount and spread.

14. **Check work and publish atomically.** Check capped row
    widths, node and branch counts, index conversions and
    allocation sizes. For the stochastic grid the total node
    count including terminal is
    `(J+1)^2+(N-J)*(2*J+1)`. Account for the actual fit, replay and
    pricing work, including objective evaluations. Use
    adjacent state-price rows or equivalent linear scratch.
    Keep the selected N and publish the complete calibrated
    candidate only after validation. Preserve any prior
    calibrated state on failure.

15. **Reconcile the callable-bond preparation.** The explicit
    BK route keeps its selected Simple base quote and
    requires the product's supplied volatility and positive
    mean reversion. Fit relative to valuation origin and
    hand the same prepared grid to payoff valuation. The
    product owns dated cash and exercise, admissible
    alignment and any bounded selection of effective N.
    Propagate failed clock or event preparation. Selected
    BK at zero volatility must preserve its same-grid
    deterministic limit and positive-rate admission.
    [Bond tree owner](../valuations-bonds/04-rates-tree-exercise.md)

16. **Keep the same selected model for prices, OAS and risk.**
    Reconcile the actual Rust bond repricers and existing
    Python and WASM product inputs and errors. Keep runtime
    lattice state Rust-only. Product owners select volatility
    and curve bumps, OAS quote basis and price targets.
    Qualify the zero-volatility boundary, including same-grid
    nodewise OAS and actual risk stencils. Mean-reversion bumps
    can cross a cap threshold and change finite-grid prices or
    sensitivities. Propagate infeasible recalibrations.
    [Bond spread quotes](../valuations-bonds/12-spread-quotes.md),
    [bond option and vega risk](../valuations-bonds/18-embedded-option-risk.md)

17. **Qualify transition, option and risk error separately.**
    Compare the undiscounted factor moments with the OU
    identities and use independent finite-tree pricing for
    capped-edge and exercise fixtures. Fit accuracy does not
    determine option or risk accuracy. On a fixed finite grid,
    as kappa tends to zero the BK weights approach (1/6,2/3,1/6).
    BDT has two equally weighted branches. A common
    continuous-law limit requires refinement on the same
    modeled basis. Kappa=0 stays outside BK selection.
    Qualify the intended volatility, mean reversion, grid
    and horizon regimes with independent option and risk
    evidence and matching discount/OAS assumptions.
    [BK references](../../../REFERENCES.md#checked-bk-trinomial-fitting)

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| Capped edge j=J=1, kappa*h=1.5 | The existing Euler weights (1/24,5/12,13/24) are valid and give conditional next-index mean −0.5. Exact OU moments give weights about (.30312528,.61687960,.07999512) and mean .22313016. |
| Twenty intervals, kappa*h in {1e−6,.03,.19,1.5,100} | Independent branch propagation stays in the selected support and reproduces the undiscounted variance `sigma^2*(1-exp(-2*kappa*n*h))/(2*kappa)` at every level in high-precision arithmetic. |
| Simple h=1, kappa=log(2), dx=log(2), outgoing rate levels .1, .2, .3 | The cap is J=1 and the upper weights are (13/24,5/12,1/24). Independent enumeration gives P[1]=10/11, P[2]=5725/7623, P[3]=9281875/16208192. A year-one call on a unit year-three ZCB at strike .70 prices at 22757/3473184, about .006552201093. |
| N=2 at the cap switch delta=.184 | Approaching the switch from below gives terminal probability at index +2 tending to about .01526578. At the threshold J drops to one and the +2 state is outside the tree. |
| Fixed h with kappa approaching zero | For one step the centered positive-part expectation is `sqrt(3)*sigma*sqrt(h)/6` in the limiting BK tree and `sigma*sqrt(h)/2` in BDT. The first two moments agree, while the payoff expectation differs. |

## Observed implementation and evidence

### Choice and domain

[black_karasinski.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/black_karasinski.rs) uses a recombining trinomial in x=ln r. Unlike BDT it represents mean reversion through node-dependent branch probabilities. The model selection in [tree.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/tree.rs) requires κ>0 and σ>0; the κ=0 alternative is BDT, not an automatic fallback. Rates remain positive.

### Algorithm

Uniform dt gives dx=σ√(3dt), width cap jmax=max(ceil(0.184/(κdt)),1), and row width 2 min(n,jmax)+1. Interior probabilities are p_up=1/6+(j²m²−jm)/2, p_mid=2/3−j²m², p_down=1/6+(j²m²+jm)/2 with m=κdt. Thus the one-step mean offset is −jm and second moment is 1/3+(jm)². At the upper cap branches use offsets (0,−1,−2), and at the lower cap (+2,+1,0), with probabilities solved from those two moments. Nonfinite/negative probabilities fail; finite probabilities are normalized and checked to sum to one.

Each interval solves ΣQ_j df(exp(a+j dx),dt)=P_target with default Brent, seeded by ln(max(period forward rate,1e−8)). Store r=exp(a+j dx), propagate state prices with the same branches, and gate maximum relative DF error at the configured tolerance. The final a is repeated for terminal accessors. Dedicated rollback uses these capped branches and the OAS conventions in [record 003](tree-discounting-and-induction.md).

### Controls, limitations and alternatives

Step count, volatility, κ, compounding and fit tolerance are explicit. κdt can produce invalid boundary probabilities; the engine returns an error rather than clipping them. This is Euler-style OU mean/variance matching, not exact finite-step OU moments. The common probability normalization is borrowed from Hull–White. The maximum-error aggregation has the same per-row NaN visibility concern as BDT. Exact OU transitions or an uncapped nearest-mean lattice would be new variants.

The callable-bond path uses Simple base compounding and feeds
the unrebased discount source to a valuation-relative lattice.
Its payoff grid is prepared separately. The product can bypass
BK at sigma=0 using a separately dated deterministic grid.
Terminal BK callbacks currently receive an interval-converted
spread derived from the repeated rate. The bond uses that
shift when adjusting cash displaced from its actual date.
[Bond tree preparation](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/tree_pricer.rs),
[short-rate rollback](../../../../finstack-quant/models/src/trees/short_rate_tree/pricing.rs),
[bond payoff](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/bond_valuator.rs)

### Evidence

[tests.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/tests.rs) includes `bk_trinomial_reprices_curve_to_a_tenth_bp`, `bk_terminal_log_rate_dispersion_matches_ou_limit` (σ√((1−e^(−2κT))/(2κ)), and `bk_kappa_to_zero_converges_to_bdt` (cross-engine comparison). [Hull–White numerical procedures](../../../REFERENCES.md#hull-white-1994-numerical-procedures) is the cited branching background; it does not independently qualify BK option prices.

The fixed-grid BK–BDT price comparison exercises on a synthetic
terminal rate. Qualify the accepted terminal-state boundary using
a payoff on a genuinely outgoing interval or on a separately
qualified conditional claim.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
