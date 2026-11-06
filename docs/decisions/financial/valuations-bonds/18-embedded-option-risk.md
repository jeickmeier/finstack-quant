# VALUATIONS-BONDS-18: Holder embedded-option value and short-rate model vega

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and controls

Embedded option value is the holder's net value of call/put/return-floor rights under the chosen model. Resolve fixed OAS from computed OAS, a quote-based solve, or zero; remove price drivers. Price the original optional bond and a clone with only call/put and return floor removed, keeping other model inputs and dispatch. Return `P_optional−P_straight` in currency; no rights returns zero. A call-only right normally gives negative holder value, a put-only right positive, while combined rights report net value.

## Vega algorithm

Vega is the sensitivity of that **option-value difference**, not total bond PV, to the selected short-rate volatility. Optional bonds require Tree or RatesCredit; other models error. RatesCredit reads `hw1f_sigma` (default zero). Rates-only trees resolve Hull–White scalar sigma/mean reversion, BDT sigma or BK sigma. Ho–Lee is rejected. Hull–White mean reversion is pinned on bumped clones so only sigma changes; the straight leg retains bumped rate volatility and all credit inputs.

With configured absolute decimal vol bump `b`, use `σ_up=σ+b`, `σ_down=max(σ−b,1e−8)` and the actual width. Return `(V_up−V_down)/notional·100/width·0.01`, price points per one absolute volatility percentage point. Effectively zero width returns zero. A small/zero sigma yields an asymmetric secant, not a symmetric derivative. Normal-rate sigma and lognormal-rate sigma are different model parameters despite this shared reporting unit.

## Limits, alternatives and evidence

Fixed OAS is calibrated only once; no market-volatility surface is shocked/recalibrated. Straight-leg subtraction removes model-rate volatility exposure unrelated to exercise. For joint simulation, differences can include sampling/regression error; no vega confidence interval or automatic bump-convergence check is produced. Total-PV vega and implied-market-quote vega would be distinct additional metrics.

Owning code: [embedded value](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/embedded_option_value.rs), [vega](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/vega.rs), [tree resolver](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/config.rs). Inline vega tests locate preservation of joint-model inputs on the straight leg, saturated marginal behavior and equivalent clean/dirty quote normalization. These are model consistency tests, not independent vega accuracy. [Hull–White 1990](../../../REFERENCES.md#hull-white-1990-pricing-ird) and [numerical procedures](../../../REFERENCES.md#hull-white-1994-numerical-procedures) supply model background.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
