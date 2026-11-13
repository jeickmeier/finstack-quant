# MODELS-NUMERICAL-005: Black–Derman–Toy lognormal lattice and curve fitting

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Source and callers inspected. Independent rational and Decimal comparisons checked row fitting, target feasibility, range and option effects of rate clipping. Production pricing and risk qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked BDT curve fitting

1. **Select the constant-volatility BDT law explicitly.** Retain a
   one-factor, positive lognormal short-rate lattice with equal
   risk-neutral branch weights. Require exactly zero mean reversion
   in a shared config. Black–Karasinski owns the mean-reverting
   log-rate model. BDT takes a supplied scalar log-rate volatility
   and fits the discount curve. The original BDT yield-volatility
   calibration is a different fit problem.
   [BDT calibration references](../../../REFERENCES.md#checked-bdt-curve-fitting)

2. **Declare the quote basis, volatility and controls.** Require
   finite nonnegative constant sigma in log-rate units per
   square root of a model year, so .20 is 20% annual log-rate
   volatility. Select the base node quote compounding explicitly.
   Use checked Continuous, Simple or Periodic(m) base quotes under
   record 003. Sigma describes that selected rate quote. Changing
   compounding with the same sigma can change the model law.
   Require explicit calibration and pricing work controls and a
   fit tolerance. Product owners can qualify their own selections.
   [Checked tree discounting](tree-discounting-and-induction.md#accepted-decision-for-checked-tree-discounting-and-rollback)

3. **Use the selected uniform lattice and one target table.**
   Require positive finite horizon T and literal positive count N.
   Use h=T/N and canonical labels `T*(n/N)` with exact endpoints
   and strictly increasing represented times. Prepare one frozen
   discount vector from the actual model origin on the qualified
   source clock. Use dated source factors or an admitted map for
   undated substeps as in record 004. Reuse the prepared clock
   through target resolution, fit, rollback, OAS and risk.
   [Origin-relative targets and grid](ho-lee-curve-fit.md#accepted-decision-for-checked-holee-curve-fitting),
   [discount source evaluation](../core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction)

4. **Require strictly decreasing base targets.** Check exact
   P[0]=1 and `1>P[1]>...>P[N]>0` on the admitted targets.
   A strictly positive departing rate gives a mathematical
   base DF below one. A represented flat or rising target
   interval is outside this model, including at sigma=0.
   Admit positive subnormal targets subject to the remaining
   numerical checks. Ordinary rounding can make an individual
   model DF equal one. It does not expand the target domain.

5. **Build nodes in log-rate coordinates.** For positive sigma,
   set `a=sigma*sqrt(h)`, `x[n,j]=(n-2*j)*a` and
   `r[n,j]=exp(beta[n]+x[n,j])`. Fit one log center beta[n]
   per departing row. From stored node j, child j raises the
   centered log factor and child j+1 lowers it, each with weight
   1/2. Keep this high-to-low storage order through state prices
   and rollback. Check spacing and required node distinctions.
   Form the complete log rate before exponentiation. A detached
   level `exp(beta[n])` or multiplier `exp(a)` need not fit.

6. **Use the exact one-state limit at sigma=0.** Retain the
   selected grid, positive-rate admission and exercise policy.
   Obtain each outgoing quote directly by inverting
   `P[n+1]/P[n]` on h. Propagate one state with unit weight.
   The root of a positive-sigma tree also uses a direct
   inverse on P[1]. Check the actual outgoing quotes and
   factors and replay both routes. No solver or volatility
   floor is needed for a one-node row.

7. **Fit to the actual incoming state-price mass.** Start
   with Q[0,0]=1. For departing row n, require
   `0<P[n+1]<W_n=sum_j Q[n,j]`, using the model mass from
   prior fitted rows. An earlier tolerated fit error can
   make the next target unattainable. Fit
   `sum_j Q[n,j]*DF(r[n,j],h)=P[n+1]`
   and propagate discounted state-price contributions with
   the same transitions used in rollback. Use base discounts
   only in calibration. OAS changes the applied discount
   in pricing under record 003.

8. **Bracket the unique positive-rate fit in the log center.**
   Set A=P[n+1]/W_n and let `r*=invDF(A,h)>0` in the
   selected quote basis. The real center is bracketed by
   `log(r*)-max_j x[n,j]` and `log(r*)-min_j x[n,j]`.
   At the lower endpoint every node rate is at most r*.
   At the upper endpoint every node rate is at least r*.
   The positive state-price discount sum decreases strictly
   as the center increases. Form the log bracket with stable
   ratios and inverse arithmetic, including when the
   standalone ratio would round to one.

9. **Use bounded fallible Brent on nontrivial rows.** Use
   the log of model DF over target DF as the residual.
   Verify representable admissible endpoints and signs, or
   an admissible endpoint that meets the same fit gate.
   Bound any endpoint adjustment by admitted coordinates and
   the work budget. Use the accepted explicit positive Brent
   tolerance and update limit. Charge each evaluation to
   calibration work. Propagate a failed objective, an unresolved
   bracket or exhausted work. Validate the complete stored
   row after solving.
   [Checked Brent](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search)

10. **Preserve scale without changing the law.** Use stable
    log-rate discount evaluation and scaled or logarithmic
    positive sums for state prices and fit objectives. Carry
    the effect of small positive contributions through the
    complete sum. At publication, required node quotes and
    outgoing factors must be positive and finite under the
    selected convention. Remove the center and node clamps
    at [1e−8,5] from fitting and pricing. Rates outside
    that interval can be admitted. Unresolved required
    arithmetic is an error. No capped, shifted-rate or
    zero-rate substitute is selected.

11. **Replay the final lattice against one inclusive tolerance.**
    Recompute all maturity factors from the actual stored
    departing rates and weights using the pricing base operator.
    Check every `10000*abs(P_model[i]-P[i])/P[i]` for finiteness
    and accept only if each is at most the explicit finite
    positive relative-DF bp tolerance. Report the worst error and
    earliest attaining maturity in 1..=N. Remove BDT's
    secondary <1 bp bond-caller gate and clamp diagnostics.
    Black–Karasinski's fit-gate reconciliation is reviewed in
    [record 006](black-karasinski-trinomial.md).

12. **Keep the terminal log factor separate.** Fit N departing
    rows. Terminal state can expose its genuine centered log
    factor if a qualified payoff needs it. There is no fitted
    terminal interval rate, outgoing DF or converted spread.
    Remove the repeated last center as a terminal quote. The
    interval-rate accessor errors at the terminal step.

13. **Check resources and publish the candidate atomically.**
    Check grid, node and edge counts, storage and requested work
    before allocation. Account for actual fit and replay work,
    including nontrivial objective calls. Use adjacent
    state-price rows or equivalent linear scratch. Keep the
    selected N. Publish the prepared grid, fitted rows and
    diagnostics only after validation and replay. Preserve any
    prior calibrated state on failure.

14. **Reconcile the actual callable-bond preparation.** The
    explicit BDT selection supplies its own sigma and retains
    the bond owner's Simple base quote. Prepare relative factors
    from the valuation origin and feed the same prepared grid
    to the fit and payoff path. The bond owner qualifies dated
    event alignment and any bounded selection of effective N.
    Propagate conversion failures. A selected BDT at sigma=0
    uses its qualified deterministic limit and positive-rate
    target admission, including if the product uses an
    equivalent implementation. A different dated deterministic
    model requires its own product selection and qualification.
    [Bond rates-tree owner](../valuations-bonds/04-rates-tree-exercise.md)

15. **Use the same BDT contract across prices, OAS and risk.**
    Propagate calibration and discount errors through the
    existing Rust bond price, quote and risk paths and Python
    and WASM product surfaces. Keep OAS quote basis, volatility
    bump stencil, dated exercise and settlement with their
    owners. A bump may reach the admitted zero-volatility
    limit. Apply the same model and grid for a same-contract
    comparison. Keep the fitted lattice Rust-only.
    [Bond spread quotes](../valuations-bonds/12-spread-quotes.md),
    [bond option and vega risk](../valuations-bonds/18-embedded-option-risk.md)

16. **Qualify option prices and risk independently of curve fit.**
    Check independent finite-tree path prices, near-flat
    positive curves, shifted origins, exact zero volatility,
    high and low tails, numerical range and failed publication.
    Qualify price and risk tolerances over the intended grid,
    volatility, horizon and exercise regimes. Compare external
    cases only on aligned curve, discount, volatility and
    spread conventions. This fit identifies the row centers
    conditional on sigma. A small DF residual alone says
    nothing about option-tail accuracy.
    [BDT calibration references](../../../REFERENCES.md#checked-bdt-curve-fitting)

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| Simple h=1, sigma=log(2), P[1]=10/11 and P[2]=19/77 | Uncapped center 3 gives second-row rates 6 and 1.5. With a cap of 5, refitting the center to 262/79 gives rates 5 and 131/79 and the same exact P[2]. A year-one call on the unit year-two ZCB at strike .39 is 1/220 in the uncapped tree and zero in the capped tree. |
| h=.75, sigma=.21, row levels .031, .047 and .063 | The log-center bracket contains the recovered centers for Continuous, Simple and Periodic(2). Independent path enumeration reproduces each base curve. |
| P[1]=.98, P[2]=.9799995 and actual incoming mass .979999 | The captured targets decrease and the prior miss is about .010204 relative-DF bp. The second target is still unattainable from the actual prior row. |
| Construction-only log center −30 and step spacing 710 | The detached `exp(710)` overflows in binary64. The combined log rates 680 and −740 give finite positive rates, including a subnormal rate at the low node. |

These fixtures compare the specified laws and arithmetic. Production BDT
pricing, external case replay and risk were not exercised.

## Observed implementation and evidence

### Choice and domain

[bdt.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/bdt.rs) represents r[n,j]=α_n exp((n−2j)σ√dt) with equal binary transition probabilities. Volatility is relative lognormal volatility, unlike Ho–Lee's absolute rate volatility. N>0, T>0, finite nonnegative σ and zero mean reversion are required by [tree.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/tree.rs). Negative-rate targets are not represented by this strictly positive construction.

### Algorithm and controls

Initialize a root guess from −ln(P(dt))/dt. For each row solve ΣQ_j df(clamp(α u^(n−2j),1e−8,5),dt)=P(t_next) using default core Brent; initial guess is the root estimate or geometric mean of the already populated row. Clamp the solved α and each resulting rate to [1e−8,5]. Propagate discounted state prices with equal probabilities and populate the next row, including a synthetic terminal row using the last α. Calibration replaces the existing tree only on success.

Track maximum relative DF error ×10000 and the first rate clamp changing a node by more than 1e−6 relative. Fail on solver error or maximum fit error above configured tolerance (default 0.1 relative-DF bp). Material clamp engagement alone does not fail: an accurate curve fit can retain clamps in low-weight tails. A warning threshold of 1 bp is separate from the hard gate.

### Selection and alternatives

The [bond dispatcher](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/tree_pricer.rs) selects BDT explicitly, threads compounding and invokes an extra quality check. BK is a separate supported model for lognormal mean reversion; Ho–Lee/HW allow negative rates. Exact ZCB repricing cannot establish tail-option fidelity when rates are clamped. `error_bp > max_error_bp` is used without a per-row nonfinite check; a NaN row error would not update the finite maximum. This is a static guard limitation, not a reproduced pricing failure.

The public callable-bond BDT path selects Simple base compounding.
It uses valuation-relative horizon and exercise times, feeds the
unrebased source into calibration, and constructs a separate payoff
grid. Step selection can discard failed exercise-date conversions.
At zero volatility the bond dispatch uses a separately prepared
deterministic grid before invoking the BDT branch. The BDT vega
calculator floors its down bump at 1e−8.
[Bond model config](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/config.rs),
[bond tree preparation](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/tree_pricer.rs),
[bond vega](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/vega.rs)

### Evidence

[short-rate tests](../../../../finstack-quant/models/src/trees/short_rate_tree/tests.rs) include `test_bdt_stored_lattice_prices_zero_coupon_to_calibration_curve`, `bdt_calibration_fails_when_node_rate_clamp_engages_materially`, near-zero/high-rate cases, and `bdt_solver_failure_is_immediate_and_preserves_previous_calibration`. These test roundtrip fit and guard behavior. [Brent](../../../REFERENCES.md#brent-1973) covers the solver family; [Hull](../../../REFERENCES.md#hull-options-futures) is general background, not evidence approving [1e−8,5].

Source-backed Bloomberg callable-bond cases provide instrument-specific
comparison inputs. The BHCCN embedded-option metric has an open
benchmark issue. These fixtures were located and remain unrun here.
[IBM EUR callable case](../../../../finstack-quant/valuations/tests/golden/data/pricing/bloomberg/bond/ibm_eur_2034_callable_bloomberg.json),
[BHCCN callable case](../../../../finstack-quant/valuations/tests/golden/data/pricing/bloomberg/bond/bhccn_10_2032_callable_bloomberg.json)

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
