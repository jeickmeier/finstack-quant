# VALUATIONS-FX-AND-EXOTICS-FX-LISTED-CONTRACTS: FX listed futures and futures-option delegation

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-LISTED-CONTRACTS`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

FX futures use a CIP model mark and the common listed-contract mark-to-market convention. FX futures options instantiate the shared futures-option implementation rather than embedding a separate FX spot-option model.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Fair futures quote is `S D_f/D_d` to the contract settlement date. `ListedFutureTerms` resolves explicit mark versus model mark and cash value from the difference to entry/last settled terms using contract count, position and point value. The family supplies quote currency as `terms.currency`; base currency must differ.

Futures delta is the common point delta with respect to a one-unit futures-quote move. FX01 reprices spot exposure and generic DV01 reprices curve dependence. A quoted market mark can therefore suppress modeled curve/spot sensitivity while point delta remains a contract-unit sensitivity.

Futures-option macro expansion reuses the listed forward/futures option kernel, with Black-76/normal and exercise/settlement behavior defined in the shared source. It is not Garman–Kohlhagen pricing with an extra foreign discount factor. See the shared source links for the complete common algorithm; the local choice is reuse of that common listed contract.

## Selection and controls

`FxFuture` delegates quote/settlement-mark selection and NPV to `ListedFutureTerms`. The `define_future_option!` invocation selects the FX instrument/family and common option metric registration. The common listed implementation owns exercise, settlement and option-model dispatch.

FX entry, quoted, settlement and spot prices must be finite and positive. Common listed terms validate contract dates, quote units and resolved settlement behavior. The instrument emits no residual cashflow schedule for futures.

## Limitations, failure and fallback

CIP is used as the futures fair quote without a stochastic-rate futures-versus-forward convexity adjustment. Contractual point delta differs from model spot delta. The macro's local selection and family registration are inspected; shared kernels are audited in the valuations-framework slice.

## Alternatives and tradeoffs

Exchange quoted marks are supported and avoid treating a modeled CIP quote as an observed futures mark. A stochastic-rate futures model is not implemented in this family. Maintaining one common futures-option kernel reduces duplicated settlement conventions but requires coordinated family tests.

## Evidence

- [finstack-quant/valuations/fx/src/fx_future/metrics.rs](../../../../finstack-quant/valuations/fx/src/fx_future/metrics.rs)
- [finstack-quant/valuations/fx/src/fx_future/types.rs](../../../../finstack-quant/valuations/fx/src/fx_future/types.rs)
- [finstack-quant/valuations/fx/src/fx_future_option.rs](../../../../finstack-quant/valuations/fx/src/fx_future_option.rs)

Delegated implementation dependencies:

- [finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/types.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/types.rs)
- [finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/instrument_impl.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/instrument_impl.rs)

No separate independent numerical benchmark was established in this slice. Inline tests in the owning implementation provide only the assertions present there; they are not proof of accuracy outside those cases.

Canonical reference index: [Black 1976](../../../REFERENCES.md#black-1976); [Hull](../../../REFERENCES.md#hull-options-futures). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
