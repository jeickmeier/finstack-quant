# MODELS-NUMERICAL-010: Joint credit rollback and reset-to-payment coupon folding

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Independent scalar and finite-lattice checks of the proposed formulas and boundaries were executed. Production pricing, risk, error and host paths remain unqualified.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked joint credit rollback and coupon folding

1. **Consume the prepared lattice.** Use the exact event nodes, actual positive
   outgoing widths, rate and hazard axes and calibrated marginals from
   [008](rates-credit-marginal-calibration.md#accepted-decision-for-checked-rates-credit-marginal-calibration).
   Use the child states and checked joint probability masses from
   [009](rates-credit-coupling-and-sampling.md#accepted-decision-for-checked-rates-credit-coupling-and-path-sampling).
   A deterministic axis has one state. Validate state identity, edges and
   applicable horizons before indexing. Use the raw signed short rate and
   prepared effective intensity. Remove the callback short-rate floor and
   local reconstruction of the fitted intensity.

2. **Use one applied discount per interval.** Follow
   [003](tree-discounting-and-induction.md#accepted-decision-for-checked-tree-discounting-and-rollback)
   for the native OAS quote, applied interval discount and checked weighted
   expectation. The live stochastic joint routes accept Continuous OAS.
   Deterministic routes can use a different explicit quote basis where their
   product and 003 qualify it. Use the same applied log discount in the
   hold, coupon fold and any qualified recovery on that interval. Keep OAS
   out of fitting and rate-only coupon projection.

3. **Hand off a pre-survival continuation.** At an alive departing node
   with width \(h\), let \(A=-\log D_{\mathrm{applied}}\) and
   \(H=\lambda h\ge 0\) be the canonical applied discount exponent
   and local integrated hazard. Carry both from the admitted interval
   calculation before materializing any rounded factors. Derive survival \(S=e^{-H}\) and default
   probability \(-\operatorname{expm1}(-H)\) from \(H\).
   Rollback supplies \(D_{\mathrm{applied}}\sum_j p_j V_j\) before
   survival, plus the authoritative outgoing interval terms. Folded coupon
   contributions at a fixing enter the same pre-survival hold. Apply the
   current interval survival once in the product hold, then add eligible
   within-interval recovery. Preserve the scale of complete products even
   when a separately rounded probability or factor loses information.

4. **Resolve dated cash and exercise in contractual order.** The product
   sets alive-at-node cash entitlement, exercise and settlement cash, and
   the ordering of coupon, principal, call and put rights. Compare the
   applicable exercise values with the survival-and-recovery-adjusted
   hold. Current due cash paid in all alternatives is common to those
   alternatives and has no outgoing survival. Cash conditional on the
   chosen alternative is part of that alternative's payoff. The terminal
   callback receives dated payoff/cash and genuine terminal state, with
   no outgoing rate, intensity, discount or OAS conversion.

5. **Use local fractional recovery of par when qualified.** A product
   selecting recovery at default with constant local intensity and applied
   equivalent rate over the interval supplies finite, nonnegative
   eligible outstanding par \(N_{\mathrm{rec}}\) in the pricing
   currency and a fraction \(R\in[0,1]\). The product
   determines the eligible base after its current principal events. Its
   recovery PV at the node is \(R N_{\mathrm{rec}} W\), where
   \[
   W = H \frac{-\operatorname{expm1}(-(A+H))}{A+H}.
   \]
   Use \(W=0\) at \(H=0\) and \(W=H\) at \(A+H=0\).
   Validate required inputs before zero shortcuts. For
   an admitted deterministic non-Continuous quote this convention uses
   the constant equivalent rate \(A/h\) for within-interval recovery.
   Preserve small \(H\), signed rates and representable complete recovery
   across intermediate underflow or overflow. Reject an invalid fraction
   or unresolved required result. Rounded survival ratios, rounded
   discount products and fraction clamps are removed from callers claiming
   this contract. The product owns recovery eligibility, par, default
   timing, accrued-on-default and the selection of a recovery model.
   [Intensity and recovery references](../../../REFERENCES.md#checked-joint-credit-rollback-and-coupon-folding)

6. **Admit one simple in-advance term observation per folded coupon.**
   Require a uniquely identified parent settlement coupon and an explicit
   observation whose amount dependencies are determined at its admitted
   fixing phase. Keep fixing \(f\), index effective date \(s\), index
   maturity \(e\), and payment \(p\) distinct from coupon accrual
   dates. Require \(0\le f\le s<e\) and \(f<p\) on exact nodes of
   the same prepared model clock. Coverage must reach both \(e\) and
   \(p\). The payment may precede index maturity when the product
   qualifies that timing and the full amount is known at fixing.
   A published historical fixing is literal known cash once its other
   dependencies are resolved. An unpublished past observation, unresolved
   publication lag, missing required fixing or unresolved clock follows
   the owner's explicit policy and cannot become a future tree reset.
   [Resolved term observations](../cashflows/06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures),
   [Parent settlement and timing](../cashflows/08-floating-replay.md#accepted-decision-parent-bound-replay-exact-full-settlement-and-explicit-timing-ownership)

7. **Project on the fitted rate marginal.** Let \(B(t,u)\) be the
   conditional unit bond from exact node \(t\) to exact node \(u\)
   under the fitted rate marginal and raw Continuous base discounting.
   Roll it back using rate probabilities and actual widths, excluding
   OAS, hazard, cash, recovery and exercise. Return \(B(t,t)=1\) after
   state admission. Reject reversed or off-grid requests. Use the
   contractual index effective and maturity nodes in the gross factor
   \(B(f,s)/B(f,e)\). Remove the reset-to-payment proxy, coupon-accrual
   denominator and timing-scale adjustment from index projection.

8. **Normalize the node index gross to its explicit market source.**
   Let \(G_0(s,e)>0\) be the gross growth from the admitted index
   projection source. A single-curve source is valid when explicitly
   qualified for that index. Use
   \[
   G_f(s,e)=G_0(s,e)
     \frac{B(f,s)/B(f,e)}{B(0,s)/B(0,e)}.
   \]
   The denominator is the achieved origin ratio under the same prepared
   rate tree. Replace the additive forward-rate bump. Use checked log
   ratios and \(\operatorname{expm1}(\log G_f)/\tau_{\mathrm{idx}}\)
   under the index's contractual positive fraction and gross admission.
   At the origin or with a deterministic rate axis, preserve the admitted
   market index rate algebraically after validation. The normalization
   matches the initial gross in the tree's index-maturity forward
   measure. Its scope is that first moment of the uncomposed index.
   Nonlinear rate controls, displaced payment, risky valuation and
   exercise have their own effects. The tree has one stochastic rate
   factor and the basis is deterministic.
   [Index projection factors](../core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates),
   [Analytic term projection](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection),
   [Multicurve reference](../../../REFERENCES.md#checked-joint-credit-rollback-and-coupon-folding)

9. **Compose and settle one full parent coupon.** Use the accepted
   checked binary64 raw-index admission and shortest-text conversion
   into exact rate controls, with the required positive raw gross
   check before bounds. Apply the contractual index bounds, gearing,
   spread and all-in bounds in the canonical cashflow order. Use the
   separate parent accrual fraction, eligible fixed principal and
   canonical Money/Decimal settlement, including only owner-specified
   rounding. Admit signed settlement cash and convert at the existing
   checked pricing boundary. Remove the deterministic base plus
   floating increment split from this route.
   [Coupon settlement](../cashflows/05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization),
   [Rate composition](../cashflows/06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures)

10. **Fold the full coupon through a joint unit claim.** Seed one at
    its actual payment node conditional on being alive there. Each
    outgoing interval before payment applies
    \(K_n U=D_n S_n\sum_j p_{nj}U_j\) using the same OAS discount,
    joint transitions and local hazard as hold. Roll back through
    intervals strictly after fixing. At fixing multiply the full
    coupon cash by \(D_f\sum_j p_{fj}U_j\) and add it to the
    pre-survival hold. The product's single fixing-interval survival
    completes \(K_f\). Adjacent fixing and payment use the unit seed
    directly. Payment may be terminal or interior and has no outgoing
    survival or discount in the coupon claim. The claim has no
    separate recovery cash. Use scaled complete products and checked
    signed sums for the continuation, coupons, recovery and due cash.

11. **Replace one price contribution per parent.** Match the fold to
    the exact ordinary payment and parent coupon identity. Exclude
    that one ordinary contribution from pricing while preserving its
    contractual dates and reporting identity. Known coupon cash,
    unrelated due cash and principal retain their own entries. Include
    each eligible same-node parent once in stable event order.
    Reject missing, conflicting or duplicate identities before rollback.

12. **Qualify exercise and entitlement before folding.** A live
    termination at fixing may forfeit the future coupon under this
    profile only when the product establishes that ordering and the
    termination payoff can be computed without the folded amount.
    The coupon due at payment must be common to all admissible
    alternatives and additive to their payoffs. Decline the fold
    for a live intervening exercise or termination right, or an
    intervening event that changes the fixed amount, entitlement or
    relevant payoff state. Scheduled events independent of the
    claim may coexist. Default is represented by survival and
    separate product recovery. Decline the fold if that recovery or
    any intervening or endpoint exercise payoff depends on the pending
    coupon. Remove the deletion of interior legal term-loan call
    opportunities. An alternative pricing route requires explicit
    product selection and qualification.

13. **Keep path-dependent coupon economics with the product.** A
    coupon requiring multiple observations, an in-arrears or
    compounded overnight path, future state-dependent principal or
    accrual, or stochastic capitalization is outside the supported
    fold. It requires the relevant product state or an explicitly
    qualified alternative. Use the owner's publication and amount
    rules for historical, overnight and payment cash. A discount
    proxy or fabricated fixing cannot complete missing dependencies.
    [Overnight observations](../cashflows/07-overnight-compounding.md#accepted-decision-resolved-overnight-blocks-cumulative-growth-and-parent-allocation)

14. **Validate and share bounded work on the prepared model.** Check
    exact dates, phases, endpoint coverage, controls and resource
    sizes using actual row shapes. Reuse rate unit rollbacks by
    required target node and joint unit rollbacks by payment node
    within the same model and applied OAS. Capture the required
    fixing rows and check allocations. Reuse requires the same
    applicable model, basis and inputs. Fail the request on invalid
    state, unsupported entitlement, missing required data or
    unresolved arithmetic. Preserve a genuine signed or zero price.

15. **Reprice the same contract for OAS and risk.** Recompute the
    applied discount, joint coupon folds and applicable recovery
    for each trial spread. Reuse rate-only factors on the same
    admitted base model. Replace bond and term-loan OAS objective
    sentinels with checked price evaluations, the accepted fallible
    root solver and the owner's final price residual gate. Fixed-quote
    risk retains the selected quote and refits or rebuilds its
    affected model terms. Propagate failures across price, OAS and
    risk without a fabricated result or implicit model switch.

16. **Reconcile the owners and existing host paths.** Reconcile joint
    term-loan rollback and relevant bond deterministic, sampled
    bullet and sampled exercise routes where they claim this
    interval economics. Reconcile any other live consumers of the
    shared recovery helper at their owning product. Exercise regression,
    recovery-model choice, calendars, curves, cashflow materialization,
    currencies and risk bump policy remain with their existing owners.
    Keep lattice state and the fold Rust-only. The existing product
    APIs and host error mappings carry the same supported economics and
    failures.

17. **Qualify the complete contract independently.** Compare with
    direct finite-path enumeration for signed rates, zero/tiny
    hazard, applied OAS, terminal and interior payments, coincident
    events, multiple coupons and invalid entitlement. Check the
    actual index dates, distinct fractions, separate projection curves,
    nonlinear controls and deterministic limit against independent
    expected values. Include OAS repricing and independently computed
    risk, required-data errors and host propagation. A passing
    finite-law comparison qualifies the stated fold for that law
    and contract. Market model quality and production convergence
    require separate evidence.

## Observed choice and input domain

[pricing.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/pricing.rs)
rolls claims on the full square joint lattice. It discounts at the raw
calibrated short rate plus continuously compounded OAS in bp/10000.
The `TreeValuator` callback applies ordinary survival and recovery.
Callback rates are floored to 1e−8. Discounting and sampling use raw
rates. The callback uses max(raw hazard, 0).

## Algorithm and conventions

For each `NodeCoupon`, require reset<payment≤N, finite inputs, positive accrual and timing scale. Compute conditional risk-free discount factors from reset to payment by rate-marginal rollback of a unit claim, excluding OAS and hazard. Node forward is (1/P−1)/accrual. Add node-forward minus supplied base discount-forward to the supplied base index rate, then pass it through canonical cashflow `calculate_floating_rate` (including its coupon composition). Increment amount is notional·accrual·timing_scale·(composed bumped rate−composed base rate).

Rollback a joint unit claim from payment to just after reset using joint probabilities, OAS discounting and hazard survival, with no recovery or product exercise. At reset take the discounted joint expectation without reset-interval survival and multiply by that node's increment. Add the result to main continuation before the callback; the callback then applies reset survival and exercise. The supplied base cashflow must remain in the product's ordinary flow stream. Multiple reset increments add. OAS inversion must recompute these folded claims because their discounting depends on OAS.

## Limitations and alternatives

This fold is not a general path-dependent coupon engine: valuing termination/exercise between reset and payment can require the coupon fixing as state or another method. Rate floors passed to callbacks differ from projection rates and can change rate-dependent payoffs. The models engine carries recovery as calibrated metadata but does not choose recovery-of-par versus recovery-of-market-value. Broader decisions belong to instrument valuators; they must not be inferred from this continuation engine. Carrying fixings in augmented state or using sampled regression is an alternative, not an automatic option here.

## Evidence

- Exact finite-path enumeration with coupon cash 10, fixing discount
  0.9 and survival 0.8, and two equally likely subsequent states with
  discount/survival pairs (1, 0.5) and (0.5, 1) gives 3.6. Omitting
  the fixing survival gives 4.5 and duplicating it gives 2.88. An
  interior termination forfeiting payment on one branch gives 1.8.
  The [term-loan tree engine](../../../../finstack-quant/valuations/fixed-income/src/term_loan/pricing/tree_engine.rs#L596)
  currently removes interior exercise dates before rolling back.
- With index fraction 1/4, market gross 1.01, achieved origin ratio
  0.98 and conditional ratio 0.95, the proposed index rate is
  \(-41/490\). For principal 100, parent fraction 1/2, gearing 2,
  spread 100 bp, index floor −810 bp and all-in floor −1510 bp,
  the cash is −7.55. The observed additive rate-bump formula in
  the same setup produces cash of −7.50. An exact two-state
  forward-measure check recovers the input gross 1.01. The
  linear end-date identity cannot be generalized to delayed payment
  or nonlinear composition even at zero rates-credit correlation.
- For \(H=2^{-60}\) and \(A=-H\), the recovery weight equals
  \(2^{-60}\) despite rounded survival of one. For \(A=H=500\),
  the weight is effectively one-half despite underflow of the
  combined factor. The observed recovery helper reconstructs
  exponents from rounded factors.
  [Shared recovery helper](../../../../finstack-quant/valuations/core/src/instruments/pricing/rates_credit.rs)

[sampling.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/sampling.rs) owns `conditional_discount_factors`; [mod.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/mod.rs) owns descriptor validation. [tests.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/tests.rs) includes `conditional_discount_factors_match_curve_at_zero_rate_vol`, `pure_forward_increment_folds_to_zero_without_correlation`, and `interior_node_coupon_payment_matches_cashflow_under_positive_hazard`. These establish consistency with the same lattice/cashflow assumptions, not independent option accuracy. [multi-curve background](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling) does not validate this folding approximation.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
