# Snowball and inverse-floater coupon recursion and HW1F discounting

- Record: `VALUATIONS-FX-AND-EXOTICS-SNOWBALL-HW`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

Interest-rate snowballs recursively accumulate coupon rates; inverse floaters apply gearing to a term floating rate. Both default to HW1F MC, with an explicit deterministic-forward approximation only for noncallable inverse floaters.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Period boundaries are start date followed by payment dates. Snowball raw coupon is `previous_coupon+fixed_rate-floating_rate`; inverse-floater raw coupon is `fixed_rate-gearing×floating_rate`. Apply floor then optional cap. The snowball formula does **not** use gearing. Multiply the annual decimal coupon by the contractual day-count accrual and notional; add principal at final payment.

Forward discounting samples the tenor-validated forward curve at each reset, applies the coupon rule recursively, and discounts payment amounts by the discount curve. Applying floor/cap to the forward is an approximation to the expected nonlinear coupon, not an option model.

HW MC derives a curve-fitted short-rate process and term-forward bond coefficients. At future reset `t`, model term rate receives a deterministic additive basis equal to quoted forward at reset minus the discount-curve model forward. At start equal to valuation, use the projected term fixing deterministically. Reset events generate pending coupon; next period start/payment event flushes it through reciprocal simulated bank account. Append maturity event so last coupon and principal discount to actual payment. Store previous coupon separately on every path. If no stochastic reset remains, return an exact deterministic estimate.

The shared note adapter attaches MC diagnostics. Only combined parallel DV01 and triangular bucketed DV01 are registered; they reprice the stochastic model with the selected seed/config. A volatility surface dependency can be declared but does not itself calibrate parameters in the pricing path.

## Selection and controls

`MonteCarloHullWhite1F` is the default for both variants. `Discounting` is registered but rejects the snowball variant and all callable structures. Shared HW parameter/config resolution is the same CapFloor-family strict resolver described in the callable range record.

Reject valuation strictly after start (unless already matured, which is zero), because realized coupon/fixing state is unavailable. Callable snowballs error even though a call provision is representable. Payment dates strictly increase; gearing positive, floor finite/nonnegative, cap above floor, and snowball initial coupon nonnegative. Shared defaults: 10,000 estimator streams, antithetic true, four minimum inter-event steps.

## Limitations, failure and fallback

Snowball gearing omission and deterministic-basis dynamics are observed choices. The explicit discounting route approximates cap/floor convexity. Seasoned and callable contracts are unsupported, not silently approximated. Payment date is treated as already settled on that date (`as_of>=final_payment`), unlike FX end-of-day treatment. Declared volatility dependencies do not imply surface-based sigma risk.

## Alternatives and tradeoffs

Deterministic discounting is supported only as an explicit inverse-floater approximation. A dedicated callable/seasoned augmented-state engine is absent. Stochastic basis or separate projection-rate dynamics could alter option value relative to deterministic additive spread.

## Evidence

- [finstack-quant/valuations/exotics/src/exotics/snowball/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/snowball/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/snowball/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/snowball/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/snowball/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/snowball/types.rs)
- [finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs](../../../../finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs)

- [finstack-quant/valuations/exotics/tests/exotic_harness/hw1f_curve_repricing.rs](../../../../finstack-quant/valuations/exotics/tests/exotic_harness/hw1f_curve_repricing.rs): HW discount-curve consistency and pathwise bank-account evidence; does not validate every recursive coupon contract.

Canonical reference index: [Hull White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives); [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
