# VALUATIONS-FX-AND-EXOTICS-FX-DELIVERY-CIP: Booked deliverable FX and covered interest parity

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-DELIVERY-CIP`
- Decision status: **Accepted 2026-11-14** — complete deliverable FX package.
- Implementation status: **Observed** in the owning paths below. Reconciliation remains pending.
- Validation status: Independent spot-date, booked swap PV, same-day and risk examples checked. Runtime, market-data and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked deliverable FX and covered interest parity

1. **Book fixed native exchanges.** Spot and forward each
   exchange two signed native amounts on a fixed value date.
   An FX swap reverses the same positive base principal on
   distinct near and far dates, with a fixed quote amount for
   each exchange. Preserve the ordered base and quote pair
   and the explicit base receive or pay side. Materialize
   the booked Money amounts once from the agreed outright
   or points under the contractual rounding policy. A market
   quote or at-market preview can supply a rate at booking.
   Pricing, risk and exports use the stored cash amounts.
   [New York Fed FX instrument definitions](https://www.newyorkfed.org/fxc/fx-volume-survey).

2. **Fix dates at booking and use the selected settlement cut.**
   Use actual booked dates or derive them once from the
   selected trade date and explicit lag, calendars, adjustment
   and end-of-month terms. Keep the same dates across reprices.
   Ordinary clean-close valuation excludes same-day cash.
   An explicit entitled cut includes known same-day payments
   at factor one. Failed or partial settlements require
   qualified residual claims. Fully settled trades have zero
   residual value without inactive market lookups.
   [FX dates](../core-conventions/011-fx-date-and-quote-conventions.md).

3. **Bind one ordered quote and collateral basis.** Select
   the exact effective base and quote curves on a coherent
   collateral or funding basis. At valuation date `V`, use
   qualified cash FX `X_V` in quote units per base, or derive
   it from a market outright `S_P` at its actual spot value
   date `P`: `X_V = S_P * D_Q(V,P) / D_B(V,P)`. The market
   forward for actual delivery `L` is
   `F_L = X_V * D_B(V,L) / D_Q(V,L)`. The same factors apply
   for a live delivery before the market spot date. Discount
   each native payment on its own matched curve and convert
   discounted base cash at `X_V`. Check finite positive
   factors and representable arithmetic, and remove the
   arbitrary factor floor. Curve construction and market
   basis calibration remain with their owners.
   [Cross-currency discount and FX basis](../valuations-rates/16-cross-currency-swaps.md) ·
   [BIS on covered interest parity and basis](https://www.bis.org/publications/qr-201609/covered-interest-parity-lost-understanding-cross-currency-basis).

4. **Identify quotes and PV attribution precisely.** Report
   the selected market outright as `spot_rate`, with its
   basis and value date, and its checked reciprocal as
   `inverse_rate`. Report the positive booked base magnitude
   as `base_amount` with the side separate. Booked swap
   points are `K_far - K_near`. Market points are
   `F_far - F_near` for the qualified market dates. Convert
   pips only with matching explicit metadata. Replace
   `carry_pv` with `base_leg_pv`, the signed translated
   PV of outstanding base cash.
   [Metric identifiers](../valuations-framework/0010-metric-identifiers.md).

5. **Reprice only the selected market sources.** Keep booked
   amounts and dates fixed. FX01 moves the selected ordered
   quote in matched +1% and −1% scenarios and reports
   `(PV_up - PV_down) / 2` in quote currency per positive
   percentage point. Use shared rate-risk rules for per-curve,
   combined and bucketed DV01. Bind the actual spot-date
   and live payment-date factor dependencies. Hold the
   selected FX quote fixed under rate shocks and recompute
   the cash-equivalent conversion where required. Bump each
   active source once.
   [FX01](../valuations-framework/option-sensitivities.md#8-keep-fx01-a-fixed-directional-scenario) ·
   [Rate risk](../valuations-framework/finite-difference-rates.md).

6. **Use the booked cash in export and host routes.** Preserve
   signed native amounts and value dates. Use the same
   entitled cash, discount factors and FX conversion in the
   valuation attribution. Rust validates the existing typed
   and instrument-envelope paths in Python and WASM. NDFs,
   listed futures and options retain their owning product
   decisions.
   [Valuation cashflow export](../valuations-framework/cashflow-export.md).

## Observed behavior

### Choice, rationale and scope

FX spot is a quote-currency receipt position; FX forwards and swaps value exchanges using domestic/quote and foreign/base discount curves. Date-based discount-factor ratios preserve curve rebasing and instrument/curve day-count differences. Contract rates and model spot are distinct inputs.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

### Algorithm and conventions

For quote-per-base spot `S`, compute `F(T)=S D_f(as_of,T)/D_d(as_of,T)`. Forward value is `N D_d(F-K)` in quote currency; a missing strike uses `K=F`. The spot position emits `N S` at its effective spot settlement, discounted on an optional domestic curve; absent that curve it is undiscounted. It does not subtract an opposite-currency acquisition leg.

The swap receives base and pays quote on the near date, reverses both on the far date: `N S(D_f,n-D_f,f)+N(K_f D_d,f-K_n D_d,n)`, including only unsettled legs. Near/far rates omitted from the contract are the CIP rates for those dates. Forward points are the resolved contract far rate minus the resolved contract near rate (each omitted rate defaults to its model forward). Carry PV is the included near `N S D_f,n` minus included far `N F_model,f D_d,f`.

Forward-points construction sets `contract_rate=spot+points`; pips first multiply by the core pair-specific pip size before that addition. Trade-date constructors delegate pair-specific spot lag, joint calendars, tenor addition, business-day adjustment and end-of-month rules to core FX date helpers. FX events remain live on their event date (`event_date < as_of` means occurred). Explicit spot settlement is adjusted only when a calendar is configured; without explicit settlement, settlement is recomputed from the valuation date.

FX01 uses the shared economic base-to-quote spot bump route. Generic DV01 uses parallel combined discount sensitivities and bucketed DV01 triangular key rates. Swap domestic/foreign IR01 instead scales each relevant DF by `exp(-b t)` for `b=±0.0001`, reprices the corresponding currency leg and returns `(PV_up-PV_down)/2`; omitted contract rates are recomputed in each scenario. Thus the reported sensitivity of an at-market template differs from that of fixed-rate contractual exchanges.

### Selection and controls

Default discounting paths call the instrument value methods. Forward quoted spot overrides FX-matrix spot; a swap requires the FX matrix even if contract near/far rates are supplied. Omitted contract rates are reset to current model forwards, producing at-market terms rather than a fixed historical trade.

Discount factors and spot used by the shared forward helper must be finite and positive. The swap separately rejects domestic DF below `1e-12`. Spot valuation supports an optional discount curve, and forward/spot quote bumps modify explicit quoted spot when that is the active source. SpotRate is computed as base PV divided by base notional; InverseRate is its reciprocal, with near-zero guards. Therefore these reported metrics contain settlement discounting (and can become zero/error after settlement), rather than directly returning the FX-market quote. BaseAmount returns the contractual base notional. Cashflow representations distinguish projected, contractual and no residual schedules.

### Limitations, failure and fallback

The spot-receipt interpretation and valuation-date-relative default settlement are observed choices, not an adopted trade convention. SpotRate/InverseRate use discounted PV per base amount: enabling discounting changes a metric named as a market rate. Omitted forward/swap contract rates are scenario-dependent at-market terms; their zero/near-zero risk is not the risk of a booked trade. No stochastic-rate FX futures convexity or cross-currency basis dynamics are included by this CIP calculation.

### Alternatives and tradeoffs

A fixed trade date/settlement and mandatory contract rates would remove template ambiguity but change the API contract. Collateral-aware cross-currency curve construction can feed these discount factors; it is not an additional local pricing model. Stochastic basis/FX-rate models remain unimplemented here.

### Evidence

- [finstack-quant/valuations/fx/src/fx_forward/metrics.rs](../../../../finstack-quant/valuations/fx/src/fx_forward/metrics.rs)
- [finstack-quant/valuations/fx/src/fx_forward/types.rs](../../../../finstack-quant/valuations/fx/src/fx_forward/types.rs)
- [finstack-quant/valuations/fx/src/fx_spot/metrics.rs](../../../../finstack-quant/valuations/fx/src/fx_spot/metrics.rs)
- [finstack-quant/valuations/fx/src/fx_spot/types.rs](../../../../finstack-quant/valuations/fx/src/fx_spot/types.rs)
- [finstack-quant/valuations/fx/src/fx_swap/metrics/ir01.rs](../../../../finstack-quant/valuations/fx/src/fx_swap/metrics/ir01.rs)
- [finstack-quant/valuations/fx/src/fx_swap/metrics/mod.rs](../../../../finstack-quant/valuations/fx/src/fx_swap/metrics/mod.rs)
- [finstack-quant/valuations/fx/src/fx_swap/pricing_helper.rs](../../../../finstack-quant/valuations/fx/src/fx_swap/pricing_helper.rs)
- [finstack-quant/valuations/fx/src/fx_swap/types.rs](../../../../finstack-quant/valuations/fx/src/fx_swap/types.rs)
- [finstack-quant/valuations/fx/src/shared.rs](../../../../finstack-quant/valuations/fx/src/shared.rs)

- [finstack-quant/valuations/fx/tests/fx_swap/pricing.rs](../../../../finstack-quant/valuations/fx/tests/fx_swap/pricing.rs): CIP and near/far PV formula assertions; implementation-level financial identities.
- [finstack-quant/valuations/fx/tests/fx_swap/metrics.rs](../../../../finstack-quant/valuations/fx/tests/fx_swap/metrics.rs): Bump-risk and forward-point assertions; no independent vendor calibration.
- [finstack-quant/valuations/fx/tests/fx_forward/test_fx_forward_types.rs](../../../../finstack-quant/valuations/fx/tests/fx_forward/test_fx_forward_types.rs): Builder/date and dependency tests; limited price-accuracy evidence.

References for the original audit are [Hull](../../../REFERENCES.md#hull-options-futures) and [Clark FX Options](../../../REFERENCES.md#clark-fx-options).

## Independent numerical checks

- In a USD-per-EUR swap with valuation before the
  near date and near before market spot, use spot 1.20
  and quote/base factor pairs of `(.98,.99)` at spot,
  `(.99,.995)` at near and `(.94,.96)` at far.
  Cash FX is `196/165`. The near and far market
  outrights are 1.193878175696 and 1.213152804642.
  Receiving EUR 1,000 at booked near 1.18 and
  returning it at far 1.21 gives USD 10.775757575758,
  matching the four discounted native payments.
- Same-day base EUR 10,000 bought at 1.17 with
  cash FX 1.22 gives USD 500 when explicitly entitled.
  The ordinary clean-close value is zero.
- A separate EUR 10,000 swap uses near equal to spot,
  spot 1.20, strikes 1.19 and 1.23, quote/base factors
  `(.99,.98)` at spot and `(.96,.94)` at far, and
  spot clock times .10 and .125 with both far times 1.
  Centered relative FX01 is USD +4.848979591837.
  Centered 1-bp rate checks give quote, base and joint
  DV01s of USD −1.067838981558, +.997071429844 and
  −.070767551228.

## Relationship to the accepted FX quote-source boundary

[CORE-MARKET-15's complete accepted package](../core-market/fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs)
separates option expiry E, actual FX delivery L and market spot settlement P.
The source clock maps volatility to expiry; a qualified FX owner supplies the
forward for the actual delivery contract, with the same explicit quote-per-base
pair. Reconcile actual spot-settlement/discount dependencies there rather than
constructing a local constant-rate expiry forward inside smile conversion.
Record 15 was accepted on **2026-11-08**. The deliverable FX
package supplies the dated owner basis for this reconciliation.
Runtime reconciliation and independent market qualification
remain pending.

## Follow-up tracking

Implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
