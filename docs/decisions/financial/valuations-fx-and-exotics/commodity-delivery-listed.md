# VALUATIONS-FX-AND-EXOTICS-COMMODITY-DELIVERY-LISTED: Commodity forwards, listed futures and delivery/fixing state

- Record: `VALUATIONS-FX-AND-EXOTICS-COMMODITY-DELIVERY-LISTED`
- Decision status: **Accepted 2026-11-14** — complete commodity forward, futures and listed option package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent forward, physical invoice, variation and option calculations checked. Runtime, clearing, delivery, market data, risk and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for commodity delivery and listed contracts

1. **Book forward terms and elect a qualified price source.** Require
   explicit side, cash or physical settlement, finite positive quantity and
   multiplier, exact commodity and price unit, applicable grade and location,
   currency and finite booked price. Admit signed prices under the contract
   terms. Book the fixing or delivery date separately from payment. Resolve
   any authored calendar, lag and adjustment once. An at-market quote can
   establish the fixed price at booking. Live valuation selects a qualified
   dated outright, matched price curve or explicit financing-only spot proxy
   where zero net nonfinancing carry is justified. The latter carries the
   admitted spot to delivery on its matched funding factor. Propagate
   failures of the elected route.
   [CME on commodity carry](https://www.cmegroup.com/education/courses/introduction-to-ferrous-metals/what-is-contango-and-backwardation).

2. **Value the forward's actual remaining legs.** With signed quantity `sQ`
   including the multiplier, contractual price `K` and dated payment factor
   `D_P`, cash value is `sQ × (F−K) × D_P`. Authorized fixing locks the
   signed cash amount. For physical settlement using a qualified
   cash-on-delivery mark, value `sQ × (F × D_delivery − K × D_P)`.
   Keep the commodity obligation and signed invoice separate. Actual
   fulfillment removes delivered commodity units from instrument PV and
   leaves any unpaid invoice. Eligible same-day unpaid cash uses factor
   one. Partial, failed or overdue obligations require a qualified residual
   and valuation basis. Confirmed discharge removes the relevant claim.
   Direct live mark delta uses the factor on the leg driven by that mark.
   Fixed unpaid cash retains discount and timing risk. Reprice each active
   funding or discount source once for curve risk.
   [Dated discounting](../core-conventions/016-discounting.md).

3. **Bind the futures series, quote scale and exact fixing rule.** Require
   qualified venue and series, position, count, quotation denomination,
   point multiplier, applicable lot, tick and rounding terms, trading
   cutoff and cash or physical delivery terms. Elect an exact observed
   series mark or explicit deterministic matched price-curve projection.
   The projection omits daily-margin convexity. For a single fixing or
   equal-weight average preserve each contractual observation with its
   exact reference contract or roll and authorized phase and version.
   Use the full denominator, with one contribution per required slot.
   Hold qualified published fixes fixed. Project pending slots only on a
   qualified basis and require eligible missing data to be resolved. At
   the post-trading cut require the official final mark when it is needed.
   [ICE WTI single fixing](https://www.ice.com/products/6753564/WTI-Bullet-Future),
   [ICE Brent average and roll](https://www.ice.com/products/6753532/Brent).

4. **Separate live variation, clearing claims and delivery.** Live
   undiscounted variation is signed contracts times point multiplier times
   `mark−reference`. Use the fill for a new position and the last applied
   official clearing mark for a carried position. Retain unpaid variation
   on its actual payment date and apply each clearing interval once. For
   physical delivery resolve the qualified invoice and remaining commodity
   obligation separately. Futures `delta` is signed cash per live futures
   point for either mark route. Report `price_curve_delta` separately for
   a parallel absolute shift of the elected curve. It equals point delta
   times the projected sample fraction and is zero on a held observed
   mark. An official final mark ends live mark risk. Remaining commodity
   and cash claims retain their own exposures.
   [CME clearing calculations](https://www.cmegroup.com/clearing/files/CME-Money-Calculations-Futures-and-Options.pdf),
   [NYMEX crude delivery](https://www.cmegroup.com/rulebook/NYMEX/2/200.pdf).

5. **Reuse the checked listed futures-option framework.** Bind the exact
   underlying future, mark, delivery ratio, exercise dates and cutoffs,
   margining, settlement form, native volatility and model clock. Select
   Black-76 for eligible positive live futures price and strike or
   Bachelier for finite signed prices and strikes where the terms admit
   them. Use the accepted European-equivalent law or qualified American
   delivery lattice as applicable. Exercise and assignment require
   authoritative action, including eligible at-the-money or contrary
   instructions. A delivered position enters at strike and transfers once
   to the futures owner. Preserve identified unpaid premium, exercise cash
   and option or futures variation. Report delta and gamma per futures
   point and its square and vega per `+0.01` native volatility. Theta across
   an event uses a qualified horizon state.
   [Listed products](../valuations-framework/listed-products.md#accepted-decision-for-checked-listed-futures-and-options),
   [NYMEX crude option rules](https://www.cmegroup.com/rulebook/NYMEX/3/310.pdf),
   [CME exercise rules](https://www.cmegroup.com/clearing/contrary-option-exercise-instructions.html).

6. **Export the resolved obligations on their owning basis.** Export fixed
   net derivative cash as `DerivativeSettlement` and clearing variation as
   `VariationMarginReceive` or `VariationMarginPay` according to its role.
   Attribute the physical invoice separately and retain typed commodity
   delivery with quantity, unit, date and provenance. Complete cash-only
   export is unavailable for an outstanding physical exchange or unresolved
   contingent payoff. Confirmed closure permits an empty residual. Reconcile
   pricing, risk, cash and commodity attribution in canonical Rust and the
   existing Python and WASM instrument envelopes.
   [Cashflow export](../valuations-framework/cashflow-export.md).

## Independent calculations

- A long 1,000-unit forward at `K=70` marked at `F=74` with delivery
  factor `0.990` and payment factor `0.985` has cash PV USD `3,940`
  or physical exchange PV USD `4,310` under the corresponding terms.
  A cash fixing of `68` at remaining factor `0.998` gives USD `−1,996`.
  The separate physical contract after fulfilled delivery has invoice-only
  PV USD `−69,860` at that factor.
- Three long 1,000-barrel futures with equal slots `−10`, `70` and
  `75`, only the first fixed, give mark `45`, live variation USD `9,000`
  versus fill `42`, point delta USD `3,000`, and projected-curve delta
  USD `2,000`. After applying mark `45`, official final mark `46` fixes
  USD `3,000` unpaid variation, worth USD `2,997` at factor `0.999`.
- Two European Bachelier options at `F=−4`, `K=−2`, native volatility
  `6` points per square root year, time `0.25`, discount `0.99` and
  multiplier `1,000` give call PV USD `897.650704` and put PV USD
  `4,857.650704`. Call delta is USD `499.9352` per point. A separate
  recorded two-lot call exercise at `−2` with first applied mark `−1`
  creates two long futures and USD `2,000` initial variation. A later
  mark of `0.5` leaves USD `3,000` new variation against the `−1` reference.

## Observed behavior

### Choice, rationale and scope

The inspected implementation discounts the marked delivery spread, values
listed futures through variation, and admits finite signed linear prices.

### Algorithm and conventions

Forward `PV=position_sign×quantity×multiplier×(F-K)×DF(payment)`. Delivery price is evaluated at maturity before delivery; on/after maturity the price-curve spot or spot scalar is used. Without a price curve, spot fallback is `S/DF(delivery)` with no storage/convenience yield. Payment date adds settlement business days to maturity and applies BDC. Override → commodity convention → default precedence selects settlement lag (default two days), calendar and Following BDC.

Physical forward cashflow export is the contractual invoice `-sign×K×quantity×multiplier`; cash settlement exports net `(F-K)`. Economic PV still includes the asset/price difference. This distinction is material when summing exported cashflows.

A listed single-fixing future uses required realized price for observation dates strictly before valuation and curve projection on/after valuation. An arithmetic-average future uses equal-weight realized/projected fixing values and Neumaier summation. No observation-day fixing is required until the next day. `ListedFutureTerms` controls quoted mark, last trading date, settlement mark, contract position/point value and MTM cash value. Curve delta is point delta times the remaining projected fixing fraction; it is zero after trading or when quoted mark is active. The futures-option macro delegates Black/normal, exercise and settlement to the shared listed implementation.

### Selection and controls

Forward pricing selects quoted forward first, then `PriceCurve`, then an explicitly configured spot scalar carried by `1/DF`. Missing contract price creates an at-market template. Futures marks use common listed terms; commodity futures options instantiate the common listed-option macro.

Validate positive quantity/multiplier, finite linear prices, unique sorted fixing dates, no fixing after settlement, and exact finite past fixings. Forward payment is live on payment date and zero afterward. Forward delta is the contractual coefficient `position_sign×quantity×multiplier×DF(payment)` until settlement, even if the price is quoted or contract price omitted; it is not a numerical derivative of the at-market-template value. Futures register only mark-price and curve delta locally. Forward generic DV01/key-rate measures reprice the selected source.

### Limitations, failure and fallback

The forward's delivered-but-unpaid mark uses current spot instead of a required locked delivery fixing. Its contractual delta remains nonzero when omitted strike makes the value identically zero: it represents coefficient risk rather than a value derivative. Spot fallback excludes storage costs/convenience yield. Physical invoice-only cashflows do not reproduce economic PV without the commodity delivery. Futures averaging uses a different same-day fixing cutoff from commodity Asian options.

### Alternatives and tradeoffs

An explicit delivery fixing would lock the post-delivery cash claim. Full carrying-cost models or direct market forward quotes are alternatives; only quoted-forward/curve and risk-free spot carry are implemented here. Futures daily margin convexity is not separately modeled.

### Implementation evidence

- [finstack-quant/valuations/exotics/src/commodity/commodity_forward/metrics.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_forward/metrics.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_forward/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_forward/types.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_future/metrics.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_future/metrics.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_future/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_future/types.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_future_option.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_future_option.rs)

Delegated implementation dependencies:

- [finstack-quant/valuations/core/src/instruments/listed/terms.rs](../../../../finstack-quant/valuations/core/src/instruments/listed/terms.rs)
- [finstack-quant/valuations/core/src/instruments/listed/future_option/types.rs](../../../../finstack-quant/valuations/core/src/instruments/listed/future_option/types.rs)

- [finstack-quant/valuations/exotics/tests/commodity/test_commodity_forward.rs](../../../../finstack-quant/valuations/exotics/tests/commodity/test_commodity_forward.rs): Forward price/settlement and physical-cashflow behavior assertions.

Canonical reference index. [Hull](../../../REFERENCES.md#hull-options-futures), [Black 1976](../../../REFERENCES.md#black-1976).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
