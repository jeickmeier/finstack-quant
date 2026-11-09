# VALUATIONS-FX-AND-EXOTICS-FX-DELIVERY-CIP: FX cash positions, covered-interest-parity forwards and two-leg swaps

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-DELIVERY-CIP`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

FX spot is a quote-currency receipt position; FX forwards and swaps value exchanges using domestic/quote and foreign/base discount curves. Date-based discount-factor ratios preserve curve rebasing and instrument/curve day-count differences. Contract rates and model spot are distinct inputs.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

For quote-per-base spot `S`, compute `F(T)=S D_f(as_of,T)/D_d(as_of,T)`. Forward value is `N D_d(F-K)` in quote currency; a missing strike uses `K=F`. The spot position emits `N S` at its effective spot settlement, discounted on an optional domestic curve; absent that curve it is undiscounted. It does not subtract an opposite-currency acquisition leg.

The swap receives base and pays quote on the near date, reverses both on the far date: `N S(D_f,n-D_f,f)+N(K_f D_d,f-K_n D_d,n)`, including only unsettled legs. Near/far rates omitted from the contract are the CIP rates for those dates. Forward points are the resolved contract far rate minus the resolved contract near rate (each omitted rate defaults to its model forward). Carry PV is the included near `N S D_f,n` minus included far `N F_model,f D_d,f`.

Forward-points construction sets `contract_rate=spot+points`; pips first multiply by the core pair-specific pip size before that addition. Trade-date constructors delegate pair-specific spot lag, joint calendars, tenor addition, business-day adjustment and end-of-month rules to core FX date helpers. FX events remain live on their event date (`event_date < as_of` means occurred). Explicit spot settlement is adjusted only when a calendar is configured; without explicit settlement, settlement is recomputed from the valuation date.

FX01 uses the shared economic base-to-quote spot bump route. Generic DV01 uses parallel combined discount sensitivities and bucketed DV01 triangular key rates. Swap domestic/foreign IR01 instead scales each relevant DF by `exp(-b t)` for `b=±0.0001`, reprices the corresponding currency leg and returns `(PV_up-PV_down)/2`; omitted contract rates are recomputed in each scenario. Thus the reported sensitivity of an at-market template differs from that of fixed-rate contractual exchanges.

## Selection and controls

Default discounting paths call the instrument value methods. Forward quoted spot overrides FX-matrix spot; a swap requires the FX matrix even if contract near/far rates are supplied. Omitted contract rates are reset to current model forwards, producing at-market terms rather than a fixed historical trade.

Discount factors and spot used by the shared forward helper must be finite and positive. The swap separately rejects domestic DF below `1e-12`. Spot valuation supports an optional discount curve, and forward/spot quote bumps modify explicit quoted spot when that is the active source. SpotRate is computed as base PV divided by base notional; InverseRate is its reciprocal, with near-zero guards. Therefore these reported metrics contain settlement discounting (and can become zero/error after settlement), rather than directly returning the FX-market quote. BaseAmount returns the contractual base notional. Cashflow representations distinguish projected, contractual and no residual schedules.

## Limitations, failure and fallback

The spot-receipt interpretation and valuation-date-relative default settlement are observed choices, not an adopted trade convention. SpotRate/InverseRate use discounted PV per base amount: enabling discounting changes a metric named as a market rate. Omitted forward/swap contract rates are scenario-dependent at-market terms; their zero/near-zero risk is not the risk of a booked trade. No stochastic-rate FX futures convexity or cross-currency basis dynamics are included by this CIP calculation.

## Alternatives and tradeoffs

A fixed trade date/settlement and mandatory contract rates would remove template ambiguity but change the API contract. Collateral-aware cross-currency curve construction can feed these discount factors; it is not an additional local pricing model. Stochastic basis/FX-rate models remain unimplemented here.

## Evidence

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

Canonical reference index: [Hull](../../../REFERENCES.md#hull-options-futures); [Clark FX Options](../../../REFERENCES.md#clark-fx-options). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Relationship to the accepted FX quote-source boundary

[CORE-MARKET-15's complete accepted package](../core-market/fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs)
separates option expiry E, actual FX delivery L and market spot settlement P.
The source clock maps volatility to expiry; a qualified FX owner supplies the
forward for the actual delivery contract, with the same explicit quote-per-base
pair. Reconcile actual spot-settlement/discount dependencies there rather than
constructing a local constant-rate expiry forward inside smile conversion.
Source metadata and successful strike inversion do not qualify the cash-spot,
collateral or delivery model. Record 15 was accepted on **2026-11-08**, with
reconciliation/qualification pending; this record's remaining valuation and
date-policy choices remain Draft.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
