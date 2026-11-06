# TARN target truncation, early redemption and HW1F term rates

- Record: `VALUATIONS-FX-AND-EXOTICS-TARN-TARGET`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

The TARN is an inverse-floating coupon note whose cumulative accrual, measured as a fraction of notional, is capped at a target. It redeems principal at the payment of the target-reaching coupon.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

For each accrual period compute `c=max(fixed_rate-term_rate,coupon_floor)×accrual_fraction`. With cumulative paid fraction `C` and target `T`, actual coupon is `min(c,max(T-C,0))`. When `c>=T-C`, set knocked-out and stop future coupons. Add actual coupon×notional to pending payment and redeem notional once at that coupon payment; otherwise redeem at final maturity. A zero target therefore triggers at the first coupon, not an immediate valuation-date redemption.

Term rates use the same HW affine bond-to-simple-forward calculation and deterministic additive projection basis as snowballs: `basis=market_forward(reset)-discount_model_forward(reset)`. The model process fits the discount curve and starts at its short rate. Reset events are at period starts; pending coupon/redemption is discounted when flushed at the next payment event using reciprocal simulated bank account. Maturity is appended to settle the final pending amount. If all remaining resets are deterministic, use contract DFs and report zero standard error.

Target state starts at zero on every path and is reset between paths. The pricer rejects seasoned valuation because cumulative coupon, past fixing and prior redemption state are absent. Contractual accrual day count differs from shared model time. Local risk registration is combined DV01 and triangular bucketed DV01; no target-risk or short-rate-volatility metric is added here.

## Selection and controls

Default/registered method is strict-parameter `MonteCarloHullWhite1F` via the shared note adapter. A deterministic branch is used only when no future stochastic reset remains; no independent discounting model is registered.

Target and floor finite/nonnegative; nonempty strictly increasing payment periods; index tenor must match the forward curve. Scalar HW parameters must be complete, positive and previously supplied/fitted; sigma schedules error. Shared MC count/antithetic/event-substep and seed controls apply. At or beyond final payment return zero.

## Limitations, failure and fallback

Target units, exact coupon truncation, first-coupon zero-target redemption and same-day settlement follow the observed contract above. Deterministic basis is not a multi-factor forward-rate model. Seasoned contracts are unsupported. Internal parity does not establish independent target-boundary, event-timing or stochastic-discounting accuracy.

## Alternatives and tradeoffs

A tree with cumulative-target state is a possible independent benchmark; production local pricing remains MC. Overshooting target, paying full final coupon, or postponing principal to final maturity are different contractual conventions and are not implemented alternatives.

## Evidence

- [finstack-quant/valuations/exotics/src/exotics/tarn/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/tarn/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/tarn/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/tarn/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/tarn/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/tarn/types.rs)
- [finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs](../../../../finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs)

- [finstack-quant/valuations/exotics/tests/exotic_harness/tarn_tree_parity.rs](../../../../finstack-quant/valuations/exotics/tests/exotic_harness/tarn_tree_parity.rs): Tree-versus-MC comparison uses an unreachably high target and nonbinding floor, so validates the linear non-knockout term-rate/discounting limit, not target truncation or path-dependent knockout. It is an internal alternate-engine comparison.

Canonical reference index: [Hull White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives); [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
