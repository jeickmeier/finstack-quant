# VALUATIONS-FX-AND-EXOTICS-QUANTO-DRIFT: Quanto option drift adjustment, fixed conversion and correlation risk

- Record: `VALUATIONS-FX-AND-EXOTICS-QUANTO-DRIFT`
- Decision status: **Accepted 2026-11-14** — complete dated fixed-conversion quanto pricing, settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent dated price, payoff, quote orientation and sensitivity calculations checked. Production market sources, lifecycle, risk and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked dated quanto pricing and risk

1. **Book explicit fixed-conversion terms.** Require European call or put,
   holder long or short, exact underlying and asset price unit, positive
   foreign strike and quantity, distinct asset and payout currencies, and
   positive fixed conversion `κ` in payout currency per asset currency.
   Price the cash payoff as signed `quantity × κ × intrinsic`. Retire
   the `notional/strike` fallback. Derive any reference notional as
   `quantity × κ × strike`. Use checked payout-currency Money and apply
   only explicitly contractual rounding.

2. **Bind ordered FX and qualified market sources.** `X` is payout currency
   per asset currency. Correlation is between the asset log-return shock
   and the log-return shock of `X` on the common model clock. A qualified
   reciprocal of the same modeled process reverses correlation once and
   preserves relative volatility. A reciprocal smile requires a separately
   qualified mapping. Bind the asset spot, foreign financing, dividend
   treatment, payout discounting and directed FX quote and volatility
   sources explicitly. Use the selected asset volatility scalar or
   qualified strike grid at contractual strike. Sample the qualified
   unshifted Black FX strike grid at the ATM forward for the actual expiry
   and FX value date. Bridge any later-value FX quote with its dated base
   and quote factors. Preserve total variance when mapping both quote
   clocks. Use checked lookup on admitted grid support and boundary terms.
   [Shared contract controls](../valuations-framework/shared-contract-controls.md),
   [dated FX carry](fx-delivery-cip.md),
   [variance clocks and fixing eligibility](../conventions/0045-financial-clocks-and-date-boundaries.md).

3. **Use the checked quanto kernel with separate fixing and payment roles.**
   Compute `B_f` from the dated foreign log financing less integrated
   dividend yield to fixing. On the common fixing clock use
   `F_q=S × exp(B_f−ρσ_assetσ_FX T)`, applying the covariance correction
   once. Price through the selected checked terminal Black quanto law with
   the independent payout factor to the actual payment date. At zero asset
   volatility use intrinsic on the deterministically carried level. Zero
   FX volatility or correlation gives exactly zero covariance correction.
   The selected law holds the two effective volatilities and correlation
   constant over the fixing horizon. Source smile samples supply effective
   parameters for that law.
   [Checked quanto kernel](../models-analytic/quanto-drift.md#accepted-decision-for-fixed-conversion-quanto-premiums),
   [Haugh's measure-change derivation](https://www.columbia.edu/~mh2078/ContinuousFE/FX_Quanto.pdf).

4. **Resolve fixing, entitlement and discharge on their actual dates.**
   Bind the authorized fixing, published version, event phase and information
   cut. Require explicit payment at or after fixing with ordered phases on
   a common civil date. An eligible missing fixing requires authorized
   resolution. A pending fixing can be modeled only with a qualified positive
   residual horizon. Use authorized fixing intrinsic to establish the signed
   claim. After fixing, resolve cash before inactive live inputs. Retain known
   cash through payment or discharge and use factor one for eligible same-day
   unpaid cash. Require a qualified residual and valuation basis for partial,
   failed or overdue settlement. Export known cash as one dated signed
   `DerivativeSettlement` with fixing provenance. Confirmed zero payoff or
   discharge gives `NoResidual`. Complete contractual cash export is
   unavailable while the payoff is contingent.
   [Dated discounting](../core-conventions/016-discounting.md),
   [cashflow export](../valuations-framework/cashflow-export.md).

5. **Measure asset, FX and correlation risk on the selected live basis.**
   Report delta and gamma per asset spot unit and unit squared. Asset
   vega, vanna and volga use the active asset volatility source and include
   the induced covariance change. Report separate FX vega per `+0.01`
   annual FX volatility. Use checked matched reprices at declared widths.
   Use fixed `±1%` moves on the selected ordered FX quote for `FX01` and
   report half the PV difference. Rebuild the dated ATM forward and
   resample the held strike grid with contractual conversion fixed. A
   qualified flat FX smile gives zero FX01. Use the checked analytic
   `correlation01` per `+0.01` oriented correlation with the inward
   derivative at `±1`. Zero covariance loading gives exact zero.
   [Option sensitivities](../valuations-framework/option-sensitivities.md),
   [Wystup's quanto sensitivities](https://www.mathfinance.com/wp-content/uploads/2024/10/wystup_quanto_eqf.pdf).

6. **Separate local rate risk, actual curve risk and horizon theta.**
   Report payout and foreign local rho per `+1 bp` on declared payment
   and fixing rate factors, respectively. Move only the prepared payment
   discount or foreign carry with sampled volatilities held fixed. Dated
   parallel and bucketed DV01 reprice each actual curve once across its
   active roles, including the FX smile lookup when its forward moves.
   Horizon theta uses qualified opening and closing cuts, authoritative
   observations and eligible signed cash. A fixed unpaid claim retains its
   applicable payout discount and timing risk. Reconcile canonical Rust
   pricing, risk, export and validation through the existing typed WASM
   and Python JSON-envelope routes.
   [Dated curve risk](../valuations-framework/finite-difference-rates.md),
   [dated theta](../valuations-framework/carry-theta-breakeven.md).

## Independent calculations

- For asset spot JPY 38,000 and strike JPY 39,000, quantity 125,
  contractual conversion `0.0072 USD/JPY`, fixing time `1`, foreign
  financing factor `0.982`, integrated dividend yield `0.013`, payment
  factor `0.965`, asset and FX volatilities `0.22` and `0.11`, and
  correlation `0.55` versus USD per JPY, the adjusted forward is JPY
  `37,691.7082654`. The closed form and direct payoff integration give
  USD `2,383.80775241` for the long call and `3,520.05912390` for the
  long put. Reversing correlation on the same quote raises call value to
  USD `2,830.88233620`. Reorienting a qualified reciprocal preserves the
  original result.
- With an authorized fixing of JPY 40,400 on those terms, the call
  fixes USD `1,260.00` cash. At the remaining dated factor `0.997` its
  PV is USD `1,256.22`. On the eligible unpaid due day its PV is USD
  `1,260.00`.
- A long call with spot `100`, strike `105`, quantity `7`, conversion
  `1.3`, fixing time `1.25`, payment time `1.30`, payout and foreign
  flat rates `0.03` and `0.05`, yield `0.01`, asset volatility `0.24`
  and correlation `0.6` illustrates the risk basis. The ordered FX
  quote `1.20` at value time `0.01` projects to `1.17013784346` at
  value time `1.27`. Use FX volatility `0.15+0.15×(K−1.17013784346)`
  near the queried strikes `K`. Directed FX01 is `−0.14576362` in payout
  currency. A flat smile at `0.15` gives zero. The sloped case gives
  local payout rho `−0.01112467` per bp and actual payout-curve DV01
  `−0.01296130` per bp because the latter also reprices the FX ATM lookup.

## Observed behavior

### Choice, rationale and scope

The inspected implementation uses a foreign-asset lognormal model under
the payout measure with a correlation drift correction and fixed conversion.

### Algorithm and conventions

Recover domestic and foreign rates from their date-based DFs. Resolve asset spot and optional dividend yield. FX forward is `S_fx exp[(r_d-r_f)t]`. Query FX volatility there, then the shared quanto kernel prices with asset carry `r_f-q-ρ σ_asset σ_fx` and domestic discounting. The Black equivalent dividend yield is `r_d-r_f+q+ρσ_assetσ_fx`.

Payoff scale is `quantity×payoff_fx_rate` when both are provided; if neither is supplied it is `domestic_notional/strike`. Supplying only one errors. At expiry use scaled asset intrinsic; after expiry zero. No terminal stochastic FX conversion is applied.

Correlation01 reprices at `ρ±h`, where `h=min(standard_bump,1-ρ,1+ρ)`; divide by `2h` and rescale to one correlation percentage point. Exactly ±1 errors because no symmetric bump exists. FX vega bumps the dedicated FX surface and normalizes by `2×decimal_bump×100`, yielding one-vol-point cash P&L. Asset vega/vanna/volga use active asset-volatility repricing; generic curve sensitivities propagate discount and quanto-carry effects.

### Selection and controls

`QuantoBS` is the selected analytical model. Asset volatility uses the instrument active quote/surface; FX volatility requires a dedicated surface and is sampled at the CIP ATM-forward FX strike. Missing FX surface errors.

Correlation is finite in [-1,1]; spot, strike, quantity/fixed conversion and required volatilities obey validated domains. Rates use instrument time; FX surface requires Black lognormal strike coordinates. The fixed conversion ratio is distinct from live FX spot used for selecting FX volatility.

### Limitations, failure and fallback

The fallback scale is `notional/strike`, and equity–FX correlation depends on its supplied sign/quote convention. Freezing correlation and sampling one FX volatility is a joint-dynamics approximation. Existing regression evidence verifies consistency/scaling rather than a calibrated joint-market model.

### Alternatives and tradeoffs

A two-factor joint asset/FX model can support stochastic conversion and richer smile dynamics, but differs economically from fixed quanto conversion. It is not implemented in this local pricer. A one-sided boundary Greek would extend ±1 correlation risk but changes the metric convention.

### Implementation evidence

- [finstack-quant/valuations/fx/src/quanto_option/metrics/correlation01.rs](../../../../finstack-quant/valuations/fx/src/quanto_option/metrics/correlation01.rs)
- [finstack-quant/valuations/fx/src/quanto_option/metrics/fx_vega.rs](../../../../finstack-quant/valuations/fx/src/quanto_option/metrics/fx_vega.rs)
- [finstack-quant/valuations/fx/src/quanto_option/metrics/mod.rs](../../../../finstack-quant/valuations/fx/src/quanto_option/metrics/mod.rs)
- [finstack-quant/valuations/fx/src/quanto_option/pricer.rs](../../../../finstack-quant/valuations/fx/src/quanto_option/pricer.rs)
- [finstack-quant/valuations/fx/src/quanto_option/types.rs](../../../../finstack-quant/valuations/fx/src/quanto_option/types.rs)

- [finstack-quant/valuations/fx/tests/quanto_option/regression.rs](../../../../finstack-quant/valuations/fx/tests/quanto_option/regression.rs): FX-vega scale, correlation boundary, quote-source and expiry-theta regression assertions; internal numerical consistency evidence.

Canonical reference index. [Wystup FX Options](../../../REFERENCES.md#wystup-fx-options), [Haug](../../../REFERENCES.md#haug-2007-option-pricing-formulas).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
