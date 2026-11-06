# Commodity forwards, listed futures and delivery/fixing state

- Record: `VALUATIONS-FX-AND-EXOTICS-COMMODITY-DELIVERY-LISTED`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

Commodity forwards discount the marked delivery-minus-contract cash value; listed futures value variation-margin mark changes and have distinct single/average fixing schedules. Negative commodity prices are allowed where the linear instrument requires only finiteness.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Forward `PV=position_sign×quantity×multiplier×(F-K)×DF(payment)`. Delivery price is evaluated at maturity before delivery; on/after maturity the price-curve spot or spot scalar is used. Without a price curve, spot fallback is `S/DF(delivery)` with no storage/convenience yield. Payment date adds settlement business days to maturity and applies BDC. Override → commodity convention → default precedence selects settlement lag (default two days), calendar and Following BDC.

Physical forward cashflow export is the contractual invoice `-sign×K×quantity×multiplier`; cash settlement exports net `(F-K)`. Economic PV still includes the asset/price difference. This distinction is material when summing exported cashflows.

A listed single-fixing future uses required realized price for observation dates strictly before valuation and curve projection on/after valuation. An arithmetic-average future uses equal-weight realized/projected fixing values and Neumaier summation. No observation-day fixing is required until the next day. `ListedFutureTerms` controls quoted mark, last trading date, settlement mark, contract position/point value and MTM cash value. Curve delta is point delta times the remaining projected fixing fraction; it is zero after trading or when quoted mark is active. The futures-option macro delegates Black/normal, exercise and settlement to the shared listed implementation.

## Selection and controls

Forward pricing selects quoted forward first, then `PriceCurve`, then an explicitly configured spot scalar carried by `1/DF`. Missing contract price creates an at-market template. Futures marks use common listed terms; commodity futures options instantiate the common listed-option macro.

Validate positive quantity/multiplier, finite linear prices, unique sorted fixing dates, no fixing after settlement, and exact finite past fixings. Forward payment is live on payment date and zero afterward. Forward delta is the contractual coefficient `position_sign×quantity×multiplier×DF(payment)` until settlement, even if the price is quoted or contract price omitted; it is not a numerical derivative of the at-market-template value. Futures register only mark-price and curve delta locally. Forward generic DV01/key-rate measures reprice the selected source.

## Limitations, failure and fallback

The forward's delivered-but-unpaid mark uses current spot instead of a required locked delivery fixing. Its contractual delta remains nonzero when omitted strike makes the value identically zero: it represents coefficient risk rather than a value derivative. Spot fallback excludes storage costs/convenience yield. Physical invoice-only cashflows do not reproduce economic PV without the commodity delivery. Futures averaging uses a different same-day fixing cutoff from commodity Asian options.

## Alternatives and tradeoffs

An explicit delivery fixing would lock the post-delivery cash claim. Full carrying-cost models or direct market forward quotes are alternatives; only quoted-forward/curve and risk-free spot carry are implemented here. Futures daily margin convexity is not separately modeled.

## Evidence

- [finstack-quant/valuations/exotics/src/commodity/commodity_forward/metrics.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_forward/metrics.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_forward/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_forward/types.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_future/metrics.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_future/metrics.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_future/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_future/types.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_future_option.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_future_option.rs)

Delegated implementation dependencies:

- [finstack-quant/valuations/core/src/instruments/common_impl/listed/terms.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/terms.rs)
- [finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/types.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/types.rs)

- [finstack-quant/valuations/exotics/tests/commodity/test_commodity_forward.rs](../../../../finstack-quant/valuations/exotics/tests/commodity/test_commodity_forward.rs): Forward price/settlement and physical-cashflow behavior assertions.

Canonical reference index: [Hull](../../../REFERENCES.md#hull-options-futures); [Black 1976](../../../REFERENCES.md#black-1976). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
