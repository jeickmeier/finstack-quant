# VALUATIONS-FX-AND-EXOTICS-FX-LISTED-CONTRACTS: FX listed futures and futures-option delegation

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-LISTED-CONTRACTS`
- Decision status: **Accepted 2026-11-14** — complete listed FX futures and futures-option package.
- Implementation status: **Observed** in owning paths below. Reconciliation remains pending.
- Validation status: Independent quote, variation, delivery, option premium and exercise examples checked. Runtime, venue, clearing and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked listed FX futures and futures options

1. **Bind the exact series and quote scale.** Require the
   ordered FX pair, series, side, contract count, base lot,
   tick and relevant clearing rules. For price quoted per
   `U` base units, one unit of the numeric quote changes
   cash by `base_lot / U` per contract. Multiply this by
   the tick for cash per tick. Separate analytical fractional
   exposures from eligible clearing lots.
   [CME FX product guide](https://www.cmegroup.com/markets/fx/fx-product-guide).

2. **Elect one live futures mark.** Select the exact
   qualified observed mark or an explicit deterministic
   covered-interest-parity proxy. The proxy uses the
   accepted spot-to-cash bridge and the contract's
   qualified delivery or benchmark value date. It omits
   stochastic-rate and daily-margin convexity. A
   cash-settled proxy requires compatible benchmark
   and curve economics. An unsuccessful selected
   source does not activate fallback.
   [Deliverable FX basis](fx-delivery-cip.md#accepted-decision-for-checked-deliverable-fx-and-covered-interest-parity).

3. **Separate live variation, final variation and delivery.**
   Use undiscounted live mark less fill or last applied
   clearing mark, scaled by side, contracts and multiplier.
   After the trading cutoff require the official final
   mark where needed. Retain eligible unpaid variation
   as dated quote cash and exclude applied variation.
   On physical delivery, use the qualified invoice to
   book both native amounts and value the remaining
   exchange under the deliverable FX decision.
   Cash-settled contracts use qualified net settlement.
   [Shared listed contracts](../valuations-framework/listed-products.md#accepted-decision-for-checked-listed-futures-and-options) ·
   [CME FX delivery](https://www.cmegroup.com/markets/fx/fx-delivery) ·
   [CME clearing calculations](https://www.cmegroup.com/clearing/files/CME-Money-Calculations-Futures-and-Options.pdf).

4. **Reuse the shared listed-option kernel and authoritative
   exercise.** Bind the exact underlying futures series,
   delivery ratio, quote scale and settlement currency.
   Select Black-76 or Bachelier and apply the shared
   exercise, premium, margining and discount rules.
   Authoritative exercise and assignment govern delivery,
   including a confirmed action at zero intrinsic value.
   A delivered future enters at strike and passes to the
   futures owner once. Preserve identified unpaid option
   premium, exercise cash and variation separately.
   [CME EUR/USD futures option rulebook](https://www.cmegroup.com/rulebook/CME/III/250/261A/261A.pdf).

5. **Report risk on the active mark or claim.** Live point
   delta is signed contracts times the quote multiplier,
   including the observed-mark route. That route has zero
   modeled FX and curve mark risk on the held quote.
   The CIP route uses FX01 and DV01 on its exact active
   sources. Fixed final marks end live point delta.
   Qualified unpaid cash or physical delivery can retain
   discount or FX exposure. Option Greeks use the
   futures quote. Market FX or mapped rate risk requires
   the selected underlying source's qualified response.

6. **Use the resolved state in pricing, risk and export.**
   Export qualified outstanding clearing cash and delivery
   obligations with dates and provenance. An unresolved
   contingent decomposition is unavailable. A confirmed
   closed state can provide an empty residual. Use the
   Rust owner in the existing Python and WASM
   instrument-envelope routes.
   [Valuation cashflow export](../valuations-framework/cashflow-export.md).

## Observed behavior

### Choice, rationale and scope

FX futures use a CIP model mark and the common listed-contract mark-to-market convention. FX futures options instantiate the shared futures-option implementation rather than embedding a separate FX spot-option model.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

### Algorithm and conventions

Fair futures quote is `S D_f/D_d` to the contract settlement date. `ListedFutureTerms` resolves explicit mark versus model mark and cash value from the difference to entry/last settled terms using contract count, position and point value. The family supplies quote currency as `terms.currency`; base currency must differ.

Futures delta is the common point delta with respect to a one-unit futures-quote move. FX01 reprices spot exposure and generic DV01 reprices curve dependence. A quoted market mark can therefore suppress modeled curve/spot sensitivity while point delta remains a contract-unit sensitivity.

Futures-option macro expansion reuses the listed forward/futures option kernel, with Black-76/normal and exercise/settlement behavior defined in the shared source. It is not Garman–Kohlhagen pricing with an extra foreign discount factor. See the shared source links for the complete common algorithm; the local choice is reuse of that common listed contract.

### Selection and controls

`FxFuture` delegates quote/settlement-mark selection and NPV to `ListedFutureTerms`. The `define_future_option!` invocation selects the FX instrument/family and common option metric registration. The common listed implementation owns exercise, settlement and option-model dispatch.

FX entry, quoted, settlement and spot prices must be finite and positive. Common listed terms validate contract dates, quote units and resolved settlement behavior. The instrument emits no residual cashflow schedule for futures.

### Limitations, failure and fallback

CIP is used as the futures fair quote without a stochastic-rate futures-versus-forward convexity adjustment. Contractual point delta differs from model spot delta. The macro's local selection and family registration are inspected; shared kernels are audited in the valuations-framework slice.

### Alternatives and tradeoffs

Exchange quoted marks are supported and avoid treating a modeled CIP quote as an observed futures mark. A stochastic-rate futures model is not implemented in this family. Maintaining one common futures-option kernel reduces duplicated settlement conventions but requires coordinated family tests.

### Evidence

- [finstack-quant/valuations/fx/src/fx_future/metrics.rs](../../../../finstack-quant/valuations/fx/src/fx_future/metrics.rs)
- [finstack-quant/valuations/fx/src/fx_future/types.rs](../../../../finstack-quant/valuations/fx/src/fx_future/types.rs)
- [finstack-quant/valuations/fx/src/fx_future_option.rs](../../../../finstack-quant/valuations/fx/src/fx_future_option.rs)

Delegated implementation dependencies:

- [finstack-quant/valuations/core/src/instruments/listed/future_option/types.rs](../../../../finstack-quant/valuations/core/src/instruments/listed/future_option/types.rs)
- [finstack-quant/valuations/core/src/instruments/listed/future_option/instrument_impl.rs](../../../../finstack-quant/valuations/core/src/instruments/listed/future_option/instrument_impl.rs)

No separate independent numerical benchmark was established in this slice. Inline tests in the owning implementation provide only the assertions present there; they are not proof of accuracy outside those cases.

References for the original audit are [Black 1976](../../../REFERENCES.md#black-1976) and [Hull](../../../REFERENCES.md#hull-options-futures).

## Independent numerical checks

- Three long EUR 125,000 contracts marked at 1.10010
  against an applied clearing reference of 1.10000
  have USD 37.50 live variation. After applying that
  mark, final 1.10100 leaves USD 337.50 outstanding
  variation. The illustrative physical invoice receives
  EUR 375,000 and pays USD 412,875. At cash FX 1.102,
  delivery factors .999 in EUR and .998 in USD, and
  a .9995 factor for unpaid variation, combined PV is
  USD 1,124.83125.
- A confirmed EUR/USD call exercised at strike 1.08
  and zero immediate intrinsic delivers one long
  EUR 125,000 future. Marking it later at 1.09 gives
  USD 1,250 of variation.
- For two EUR 125,000 contracts with futures quote
  1.12, strike 1.10, .5 model years, discount factor
  .98 and Black volatility .20, the option call and
  put are USD 17,904.411513 and USD 13,004.411513.
  The difference is USD 4,900. Normal volatility
  .22 quote units per square root year gives a call
  of USD 17,780.396884 on the same basis.

## Follow-up tracking

Implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
