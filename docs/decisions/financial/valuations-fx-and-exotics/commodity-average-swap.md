# Commodity business-day averaging and fixed-for-floating swaps

- Record: `VALUATIONS-FX-AND-EXOTICS-COMMODITY-AVERAGE-SWAP`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

Commodity swaps pay an equal quantity each payment period against an arithmetic average of daily business-day commodity prices. The helper also defines the averaging convention reused by commodity swaptions.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Build short-back-stub periods using Act365F schedule metadata, no EOM, no payment lag, no accrual-date adjustment and no roll rule. Iterate observation dates in `[accrual_start-lag,accrual_end-lag)`, with the last period also including its end. `index_lag_days` shifts by calendar days, not business days. Include weekdays accepted by the optional strict calendar. Average with compensated summation; if no business days exist, substitute the midpoint date price.

For date `<as_of`, require a supplied exact past fixing; for date `>=as_of`, project from the price curve. Each period pays `Q×average_price` floating and `Q×fixed_price` fixed: there is no accrual-year-fraction multiplier in these commodity cash amounts. Discount only payments on/after valuation. Pay-fixed value is floating PV minus fixed PV; Receive reverses sign.

The raw cashflow schedule exports separate signed fixed and floating flows for all periods, with accrual metadata and projected representation. A parallel additive price-curve bump of ±1 currency price-unit gives swap delta `(PV_up-PV_down)/2`. Generic discount DV01/key-rate measures reprice the same schedule; the shared commodity bump helper supports multiplicative percentage bumps for option sensitivities.

## Selection and controls

Discounting is the only local swap valuation method. Optional calendar selects business-day filtering and date adjustment; no calendar still excludes weekends from observations but forces unadjusted payment scheduling.

Quantity is positive, fixed price finite (negative prices are supported), schedule strictly ordered, frequency nonzero, calendar known, and fixing dates unique/finite. Same-day observations remain projected. No observation-price interpolation is used for missing historical fixings.

## Limitations, failure and fallback

Equal quantity per stub/regular period, final endpoint inclusion, calendar-day lag and midpoint fallback are observed conventions. Raw full-history schedules can require fixings for periods already paid, while PV filters paid periods first. Relative observation cutoff differs from Asian products; this is not automatically a bug or an approved convention.

## Alternatives and tradeoffs

Volume-weighted or day-proportional delivery quantities, daily physical deliveries and publication calendars are unimplemented alternatives. Explicit empty-window rejection would avoid a synthetic midpoint observation but change current behavior.

## Evidence

- [finstack-quant/valuations/exotics/src/commodity/averaging.rs](../../../../finstack-quant/valuations/exotics/src/commodity/averaging.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_swap/metrics.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_swap/metrics.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_swap/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_swap/types.rs)

No separate independent numerical benchmark was established in this slice. Inline tests in the owning implementation provide only the assertions present there; they are not proof of accuracy outside those cases.

Canonical reference index: [Hull](../../../REFERENCES.md#hull-options-futures). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
