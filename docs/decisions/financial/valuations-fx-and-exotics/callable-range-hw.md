# Callable rate range-accrual HW1F projection and issuer LSMC

- Record: `VALUATIONS-FX-AND-EXOTICS-CALLABLE-RANGE-HW`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

Callable rate range-accrual notes use a curve-fitted one-factor Gaussian short-rate process, term-forward observations and issuer exercise. Principal is part of this note, unlike the standalone asset coupon-only range accrual.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Require explicit index/forward curve/tenor, and forward ID equal to discount ID: projection is single-curve. Build a union of future observation, eligible call and final payment dates. At each observation compute a simple tenor forward from HW affine bond coefficients `L=(1/P(t,t+τ)-1)/τ`; inclusive lower≤L≤upper increments count. Observation occurs before same-day exercise snapshot.

Coupon is `N×coupon_rate×accrual_year_fraction×(past+simulated in-range)/(past+future observations)`. At final payment path value includes coupon plus principal, discounted by reciprocal simulated bank account; deterministic DF is the fallback when the bank marker is absent. Exercise stores in-range count and the HW bond price to final payment. The local intrinsic is `call_price_fraction×N+accrued_coupon×(1-P(call,final))`; value-after subtracts accrued coupon discounted at final payment, preserving its settlement treatment during issuer exercise comparisons.

Shared LSMC regresses continuation using `ShortRateBasis`; degree-two row is `[1,r,r²,t r]`. The local basis omits accrued-count state even though the note payoff tracks it. Default shared config is 10,000 independent estimator streams, antithetic true (twice the raw simulated paths), four minimum substeps between events, degree two and in-sample LSMC (`oos_lsmc=false`). A custom pricer config can enable split-sample estimation. Instrument path/scenario overrides replace count/derive seed. The short-rate process is prepared from the discount curve over the event horizon; initial state comes from its short rate, while the scalar `spot_id` supplies the reference rate for relative bounds.

DV01 and bucketed DV01 reprice combined curve exposures. Shared diagnostics attach estimate/error information. No dedicated local volatility-risk metric is registered.

## Selection and controls

Default `MonteCarloHullWhite1F` invokes shared `Hw1fNotePricer`. No-call schedules use plain HW MC; eligible future calls use HW LSMC. Parameters resolve in order: complete instrument kappa/sigma pair, explicit pricer pair, then complete pre-fitted CapFloor-family market scalars keyed by discount curve. No volatility-surface fitting occurs during pricing.

Scalar kappa/sigma are strictly positive; partial overrides/calibrated pairs and supplied sigma schedules error. The optional surface field is not an on-demand fit source. Calls after final payment error; historical observation totals must match elapsed dates. Call dates on/before valuation are excluded from future exercise.

## Limitations, failure and fallback

The regression basis omits accumulated in-range count; residual coupon subtraction alone does not establish Markov sufficiency for issuer decisions. Single-curve projection and in-sample policy estimation are observed defaults. Past issuer-call/redemption state is not represented by merely excluding past call dates.

## Alternatives and tradeoffs

A basis including accrued count, split-sample policy evaluation and a tree with augmented coupon state could improve exercise assessment at additional cost. Multi-curve basis-adjusted projection is not implemented here (unlike snowball/TARN additive deterministic basis).

## Evidence

- [finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/types.rs)
- [finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs](../../../../finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs)

Delegated implementation dependencies:

- [finstack-quant/valuations/core/src/instruments/pricing/hw1f_params.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/hw1f_params.rs)
- [finstack-quant/models/src/rates/hw1f/config.rs](../../../../finstack-quant/models/src/rates/hw1f/config.rs)
- [finstack-quant/models/src/rates/hw1f/exercise.rs](../../../../finstack-quant/models/src/rates/hw1f/exercise.rs)
- [finstack-quant/models/data/defaults/pricer_defaults.v1.json](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json)

- [finstack-quant/valuations/exotics/tests/exotic_harness/hw1f_curve_repricing.rs](../../../../finstack-quant/valuations/exotics/tests/exotic_harness/hw1f_curve_repricing.rs): Curve-repricing and simulated bank-account discount tests; tests process consistency, not independent callable-note exercise accuracy.

Canonical reference index: [Hull White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives); [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
