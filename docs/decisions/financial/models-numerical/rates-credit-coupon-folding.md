# MODELS-NUMERICAL-010: Joint credit rollback and reset-to-payment coupon folding

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and input domain

[pricing.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/pricing.rs) rolls claims on the full square joint lattice. It discounts at raw calibrated short rate plus continuously compounded OAS (bp/10000); it does not itself apply ordinary claim survival/recovery. Those are `TreeValuator` responsibilities. Callback state rates are floored to 1e−8 although discounting and sampled states use raw rates; hazard state is max(raw,0). This intentional difference matters for rate-dependent callback payoffs.

## Algorithm and conventions

For each `NodeCoupon`, require reset<payment≤N, finite inputs, positive accrual and timing scale. Compute conditional risk-free discount factors from reset to payment by rate-marginal rollback of a unit claim, excluding OAS and hazard. Node forward is (1/P−1)/accrual. Add node-forward minus supplied base discount-forward to the supplied base index rate, then pass it through canonical cashflow `calculate_floating_rate` (including its coupon composition). Increment amount is notional·accrual·timing_scale·(composed bumped rate−composed base rate).

Rollback a joint unit claim from payment to just after reset using joint probabilities, OAS discounting and hazard survival, with no recovery or product exercise. At reset take the discounted joint expectation without reset-interval survival and multiply by that node's increment. Add the result to main continuation before the callback; the callback then applies reset survival and exercise. The supplied base cashflow must remain in the product's ordinary flow stream. Multiple reset increments add. OAS inversion must recompute these folded claims because their discounting depends on OAS.

## Limitations and alternatives

This fold is not a general path-dependent coupon engine: valuing termination/exercise between reset and payment can require the coupon fixing as state or another method. Rate floors passed to callbacks differ from projection rates and can change rate-dependent payoffs. The models engine carries recovery as calibrated metadata but does not choose recovery-of-par versus recovery-of-market-value. Broader decisions belong to instrument valuators; they must not be inferred from this continuation engine. Carrying fixings in augmented state or using sampled regression is an alternative, not an automatic option here.

## Evidence

[sampling.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/sampling.rs) owns `conditional_discount_factors`; [mod.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/mod.rs) owns descriptor validation. [tests.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/tests.rs) includes `conditional_discount_factors_match_curve_at_zero_rate_vol`, `pure_forward_increment_folds_to_zero_without_correlation`, and `interior_node_coupon_payment_matches_cashflow_under_positive_hazard`. These establish consistency with the same lattice/cashflow assumptions, not independent option accuracy. [multi-curve background](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling) does not validate this folding approximation.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
