# VALUATIONS-FX-AND-EXOTICS-QUANTO-DRIFT: Quanto option drift adjustment, fixed conversion and correlation risk

- Record: `VALUATIONS-FX-AND-EXOTICS-QUANTO-DRIFT`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

Quanto options use a foreign-asset lognormal model under the domestic payoff measure with an equity–FX correlation drift correction and a fixed contractual conversion scale.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Recover domestic and foreign rates from their date-based DFs. Resolve asset spot and optional dividend yield. FX forward is `S_fx exp[(r_d-r_f)t]`. Query FX volatility there, then the shared quanto kernel prices with asset carry `r_f-q-ρ σ_asset σ_fx` and domestic discounting. The Black equivalent dividend yield is `r_d-r_f+q+ρσ_assetσ_fx`.

Payoff scale is `quantity×payoff_fx_rate` when both are provided; if neither is supplied it is `domestic_notional/strike`. Supplying only one errors. At expiry use scaled asset intrinsic; after expiry zero. No terminal stochastic FX conversion is applied.

Correlation01 reprices at `ρ±h`, where `h=min(standard_bump,1-ρ,1+ρ)`; divide by `2h` and rescale to one correlation percentage point. Exactly ±1 errors because no symmetric bump exists. FX vega bumps the dedicated FX surface and normalizes by `2×decimal_bump×100`, yielding one-vol-point cash P&L. Asset vega/vanna/volga use active asset-volatility repricing; generic curve sensitivities propagate discount and quanto-carry effects.

## Selection and controls

`QuantoBS` is the selected analytical model. Asset volatility uses the instrument active quote/surface; FX volatility requires a dedicated surface and is sampled at the CIP ATM-forward FX strike. Missing FX surface errors.

Correlation is finite in [-1,1]; spot, strike, quantity/fixed conversion and required volatilities obey validated domains. Rates use instrument time; FX surface requires Black lognormal strike coordinates. The fixed conversion ratio is distinct from live FX spot used for selecting FX volatility.

## Limitations, failure and fallback

The fallback scale is `notional/strike`, and equity–FX correlation depends on its supplied sign/quote convention. Freezing correlation and sampling one FX volatility is a joint-dynamics approximation. Existing regression evidence verifies consistency/scaling rather than a calibrated joint-market model.

## Alternatives and tradeoffs

A two-factor joint asset/FX model can support stochastic conversion and richer smile dynamics, but differs economically from fixed quanto conversion. It is not implemented in this local pricer. A one-sided boundary Greek would extend ±1 correlation risk but changes the metric convention.

## Evidence

- [finstack-quant/valuations/fx/src/quanto_option/metrics/correlation01.rs](../../../../finstack-quant/valuations/fx/src/quanto_option/metrics/correlation01.rs)
- [finstack-quant/valuations/fx/src/quanto_option/metrics/fx_vega.rs](../../../../finstack-quant/valuations/fx/src/quanto_option/metrics/fx_vega.rs)
- [finstack-quant/valuations/fx/src/quanto_option/metrics/mod.rs](../../../../finstack-quant/valuations/fx/src/quanto_option/metrics/mod.rs)
- [finstack-quant/valuations/fx/src/quanto_option/pricer.rs](../../../../finstack-quant/valuations/fx/src/quanto_option/pricer.rs)
- [finstack-quant/valuations/fx/src/quanto_option/types.rs](../../../../finstack-quant/valuations/fx/src/quanto_option/types.rs)

- [finstack-quant/valuations/fx/tests/quanto_option/regression.rs](../../../../finstack-quant/valuations/fx/tests/quanto_option/regression.rs): FX-vega scale, correlation boundary, quote-source and expiry-theta regression assertions; internal numerical consistency evidence.

Canonical reference index: [Wystup FX Options](../../../REFERENCES.md#wystup-fx-options); [Haug](../../../REFERENCES.md#haug-2007-option-pricing-formulas). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
