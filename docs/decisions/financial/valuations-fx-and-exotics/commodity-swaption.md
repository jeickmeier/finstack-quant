# VALUATIONS-FX-AND-EXOTICS-COMMODITY-SWAPTION: Commodity swaption discounted-forward averaging and annuity Black-76

- Record: `VALUATIONS-FX-AND-EXOTICS-COMMODITY-SWAPTION`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

A commodity swaption treats the underlying fixed-for-floating swap's discounted weighted average forward price as lognormal and prices a European option on that scalar swap rate.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

For underlying payment periods compute projected business-day average price `F_i`, payment DF `D_i`, annuity `A=ΣD_i`, and swap forward `F=ΣD_i F_i/A`. Quantities are per period, so no interest-style accrual factor multiplies annuity. Return arithmetic mean if the weight sum is nonpositive, but the Black input validator subsequently rejects nonpositive annuity when pricing.

At expiry the value is `Q A max(±(F-K),0)`. Before expiry Black-76 uses `A` in place of a single payment DF: `Q A [FΦ(d1)-KΦ(d2)]` for call. After expiry value is zero. Dates require expiry no later than underlying start, and underlying start before maturity.

Forward delta is `Q A Φ(d1)` (put signed form); at zero time/volatility use a strict intrinsic derivative. Vega is `0.01 Q A F n(d1)√t`; gamma uses percentage bumps to the whole price curve divided by `(F×bump)²`. Curve DV01 includes effects on payment annuity and weighted forward, through generic repricing.

## Selection and controls

Default/family selection is `Black76`. The same business-day average-period schedule as commodity swaps supplies the forward; the expiry/strike volatility is a single surface quote or active override.

Forward, strike and annuity must be positive and finite; volatility nonnegative, time nonnegative. Quantity positive; default time Act365F; calendar/BDC controls inherited averaging. Negative forwards error with guidance to use normal/shifted pricing, but those models are not actually implemented in this local type.

## Limitations, failure and fallback

One scalar swap volatility does not model individual forward-volatility/correlation structure. The annuity weights equal quantities per period. The arithmetic-mean fallback in the helper is not an accepted way to price invalid discount factors; pricing rejects the annuity afterward.

## Alternatives and tradeoffs

A normal or shifted-lognormal swap-rate model would cover nonpositive prices but is absent here. A joint forward-curve simulation would model delivery-period correlation at materially greater calibration cost.

## Evidence

- [finstack-quant/valuations/exotics/src/commodity/commodity_swaption/metrics.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_swaption/metrics.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_swaption/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_swaption/types.rs)

No separate independent numerical benchmark was established in this slice. Inline tests in the owning implementation provide only the assertions present there; they are not proof of accuracy outside those cases.

Canonical reference index: [Black 1976](../../../REFERENCES.md#black-1976). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
