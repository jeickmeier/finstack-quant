# VALUATIONS-FX-AND-EXOTICS-FX-DIGITAL: FX cash-or-nothing and asset-or-nothing digitals

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-DIGITAL`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

European FX digitals use analytical lognormal distribution probabilities and separately derived sensitivities. Payout currency and base notional have different roles for cash and asset digitals.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Using Garman–Kohlhagen `d1,d2`, a cash call pays `A D_d Φ(d2)` and an asset call `N S D_f Φ(d1)`; puts use negative arguments. On expiry only strict in-the-money conditions pay: equality pays zero. After expiry value and Greeks are zero.

Cash delta is `±A D_d n(d2)/(S σ√t)`, gamma `∓A D_d n(d2)d1/(S²σ²t)`, and vega `∓A D_d n(d2)d1/(100σ)`. Asset delta differentiates `SΦ(d1)`, gamma uses `d2`, and vega is `∓N S D_f n(d1)d2/(100σ)`. Cash theta differentiates `d2` in remaining time; nondegenerate asset theta differentiates `d1`. Domestic/foreign rhos differentiate both discounting and distribution drift and divide by 100.

At zero volatility away from the deterministic boundary, cash Greeks reduce to discount-only theta/rho and asset Greeks to discounted-asset delta/foreign rho. At deterministic ATM the Greek function errors as undefined. The deterministic asset theta branch uses a one-day shorter remaining maturity, floored at zero, and returns the price difference.

## Selection and controls

Default digital pricing delegates shared FX inputs to the local analytic kernel. Greek registration chooses the local `OptionGreeksProvider` for delta/gamma/vega/theta/domestic and foreign rho.

Inputs share date-based DF recovery and active surface/override selection with vanilla FX. Cash payout is a quote-currency money amount; asset payout converts the base notional at terminal spot. Time/day-count and theta-year denominator are explicit controls.

## Limitations, failure and fallback

The expiry equality rule and the zero-volatility distribution limit are discontinuous conventions. A smooth finite bump through a digital jump is not an independent derivative. No smile-gradient term is added to the analytic Greeks.

## Alternatives and tradeoffs

Tight call-spread replication would introduce a finite width and potentially smile-sensitive digital price. It is not the chosen kernel here. Full smile-density or local-volatility methods are not implemented locally.

## Evidence

- [finstack-quant/valuations/fx/src/fx_digital_option/metrics.rs](../../../../finstack-quant/valuations/fx/src/fx_digital_option/metrics.rs)
- [finstack-quant/valuations/fx/src/fx_digital_option/pricer.rs](../../../../finstack-quant/valuations/fx/src/fx_digital_option/pricer.rs)
- [finstack-quant/valuations/fx/src/fx_digital_option/types.rs](../../../../finstack-quant/valuations/fx/src/fx_digital_option/types.rs)

No separate independent numerical benchmark was established in this slice. Inline tests in the owning implementation provide only the assertions present there; they are not proof of accuracy outside those cases.

Canonical reference index: [Garman Kohlhagen](../../../REFERENCES.md#garman-kohlhagen-1983); [Haug](../../../REFERENCES.md#haug-2007-option-pricing-formulas). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
