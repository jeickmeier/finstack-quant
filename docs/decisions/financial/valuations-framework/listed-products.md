# VALUATIONS-FRAMEWORK-03 — Listed futures and options: pricing, exercise, and settlement

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the shared listed-product terms and instrument macro.
- Validation status: Evidence inspected/located, not executed; no external price comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, domain, and rationale

`ListedFutureTerms` values a signed contract position by variation-margin mark to market, `s × contracts × multiplier × (mark − entry_price)`. `FutureOptionTerms` separately makes price model, exercise, premium margining, and cash versus future delivery explicit. The models are Black-76 with positive futures price/strike and decimal lognormal volatility, or Bachelier with unrestricted finite futures price/strike and normal volatility in price points per square-root year. Contract count and multiplier must be finite and positive; direction is `Position`.

For an American option whose exercise cash is paid on a fixed contractual date, the code deliberately uses the European expectation. Its documented rationale is the convex payoff of a martingale futures price with deterministic rates: waiting dominates exercise when the payment date does not change. This is an economic assumption needing adoption, not a general claim about all American cash options.

## Algorithm and lifecycle

1. A live future resolves an explicit `quoted_price` before a supplied model-price closure. Strictly after last trading, it requires `settlement_price`. Strictly after settlement it returns zero; settlement-date MTM remains included. No discounting is applied. Physical asset/quantity is validated metadata; this helper does not value a delivery basket.
2. A live option requires an explicit implied-volatility quote in instrument overrides, including a supplied zero. Time uses the terms' day count. Premium-paid options discount to cash payment, or to expiry for future delivery; futures-style options use discount factor one.
3. European and fixed-date cash American calls use `DF[F N(d1) − K N(d2)]` under Black-76, or `DF[(F−K)N(d)+σ√T φ(d)]` under Bachelier; puts use the corresponding opposite payoff. At zero time or volatility the branch returns discounted intrinsic.
4. American future delivery with positive volatility uses a martingale futures lattice. Black-76 delegates to Leisen–Reimer with dividend yield equal to the effective rate `−ln(DF)/T`. Normal uses additive moves `±σ√Δt`, probability one half, and backward induction `max(intrinsic, exp(−rΔt)(Vdown+Vup)/2)`. Futures-style effective rate is zero. The deterministic delivery branch returns `intrinsic × max(DF,1)` to choose immediate versus terminal exercise.
5. Position value is the signed scale times the unit quote; futures-style additionally subtracts `option_reference_price`, which is required and nonnegative. This is MTM from the reference quote, not an undiscounted premium balance.
6. From expiry onward, an exercise/expiry observation is mandatory unless the terminal settlement date has passed. European observations must be on expiry; American observations may be earlier. A recorded exercise delivers a future only if the observed price was strictly in the money. Cash uses the observed intrinsic paid on the contractual date. Delivered futures use `call/put sign × (current mark−strike)`; after underlying last trading they require official settlement price. All branches return zero strictly after terminal settlement.

## Risk selection and controls

Official American PV defaults to 401 tree steps; finite-difference risk defaults to 201. `tree_steps` overrides both. Delta uses `(V(F+h)−V(F−h))/(2h)` with `h=max(|F|×1e−4,1e−4)`. Gamma uses the centered second difference with `h=max(|F|×1e−3,1e−3)`. Black bumps are capped at half the positive futures price. Vega is the cash change from `σ+0.01`, without division; this is one absolute volatility point in the selected model's units. Theta is next-calendar-day value less current value, capped at expiry, on the same market curves. All are scaled by signed contracts/multiplier.

After exercise, gamma/vega/theta are zero. Delta on a delivered in-the-money future is signed contracts/multiplier until last trading, then zero after verifying settlement price. At expiry without an observation the Greek functions return zero whereas PV errors; callers must not use zero Greeks as evidence that the lifecycle observation exists. The interest-rate option macro additionally requires `underlying_price_change_per_bp` for rate-risk mapping.

## Limitations, alternatives

- Bermudan exercise is rejected because no exercise schedule is supplied. Stochastic rates, exercise-dependent cash-payment dates, and delivery choice/basis are not modeled by these terms.
- American risk is computed on a different default grid from official PV.
- The additive normal tree and effective constant rate approximate the supplied discount term structure. A term-structured lattice or PDE is an unimplemented alternative here.
- A future's post-settlement zero branch precedes terms validation; settled malformed terms need not produce an error through that path.

## Evidence

- [FutureOptionTerms](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/types.rs): owning formulas, 401/201 defaults, exercise and settlement branches, finite differences.
- [Instrument macro](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/instrument_impl.rs): cross-asset delegation, overrides and rate-risk requirement.
- [ListedFutureTerms](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/terms.rs): explicit mark priority, MTM formula and date boundaries.
- [Future-option implied-volatility tests](../../../../finstack-quant/valuations/tests/instruments/future_option/implied_volatility.rs): located cross-asset behavior/roundtrip evidence; a roundtrip is not an independent price benchmark.
- [Black (1976)](../../../REFERENCES.md#black-1976), [Bachelier (1900)](../../../REFERENCES.md#bachelier-1900), and [Hull](../../../REFERENCES.md#hull-options-futures) provide canonical model context. These citations do not establish approval of the lifecycle rules or tree controls.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
