# VALUATIONS-FX-AND-EXOTICS-COMMODITY-AVERAGE-SWAP: Commodity business-day averaging and fixed-for-floating swaps

- Record: `VALUATIONS-FX-AND-EXOTICS-COMMODITY-AVERAGE-SWAP`
- Decision status: **Accepted 2026-11-14** — complete commodity averaging swap schedule, fixing, net settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent schedule, publication, cash and curve-risk calculations checked. Runtime, fixing and market data, settlement and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for commodity averaging swap pricing and settlement

1. **Book the contractual grid and period quantities.** Require cash
   settlement, explicit pay-fixed or receive-fixed side, exact commodity
   benchmark and reference contract or roll, currency, price and quantity
   units, and finite fixed price. Compile one unadjusted contractual period
   grid from explicit dates, cadence, roll and stub. Bind finite positive
   total quantity to each regular or stub period. An explicitly flat
   amount is valid. Apply each booked quantity directly to the price
   difference without interest accrual or inferred stub proration.
   [Schedule generation](../core-conventions/008-schedule-generation.md),
   [CFTC WTI commodity swap example](https://www.govinfo.gov/content/pkg/CFR-2025-title17-vol1/pdf/CFR-2025-title17-vol1-part20.pdf).

2. **Separate observation windows and payment dates.** Require an exact
   observation calendar and an explicit nonnegative calendar-day lookback.
   Shift both raw window boundaries before applying the observation calendar.
   Windows include their start and exclude their end. Include the final
   shifted endpoint only when expressly elected and eligible. Apply the
   separate payment calendar and convention to the raw period end, then
   any booked business-day lag. Require valid event ordering from required
   observation to payment. Reject empty windows, duplicate slots, unsupported
   terms and required date or calendar failures. Keep payments separate from
   the observation grid.
   [Dated events and calendar rules](../core-conventions/008-schedule-generation.md).

3. **Compute the checked equal-weight average on exact slots.** Give each
   contractual slot one contribution on the full denominator. Bind its
   actual benchmark, reference series or roll, authorized source version
   and publication phase. Consume an exact finite signed fixing once
   eligible. Use the elected matched price curve for future or explicitly
   pending slots only where its exact dates, basis and original source
   clock are qualified. Missing eligible fixings, unknown publication
   status and failed required projections error. At a qualified same-day
   publication, consume the official value in place of projection.
   [Fixing authority](../core-market/scalar-series-and-fixings.md),
   [price curves](../core-market/price-curves.md).

4. **Price and export the net period settlement.** Pay-fixed cash is
   `Q × (average−fixed_price)`. Reverse sign once for receive-fixed. Apply
   contractual rounding at its specified stage and discount once to actual
   payment. Skip confirmed discharged periods before inactive fixing or curve
   queries. Fully fixed unpaid cash needs only its eligible payment basis
   once the amount is known. Eligible same-day unpaid cash uses factor one.
   A failed, partial or overdue obligation needs a qualified residual.
   Export one signed `DerivativeSettlement` per included period, with fixed
   or projected provenance. Gross fixed and floating values are attribution.
   Historical reporting uses qualified archived settlements. Price and export
   the actual open cash on the same prepared basis.
   [ICE swap methodology](https://idd.ice.com/CM/CMHelp/Content/FM/Swap.htm),
   [cashflow classification](../core-conventions/015-cashflow-semantics.md).

5. **Report risk on the actual source and claim.** `delta` is the signed
   PV change per native price unit from matched `±1` additive pillar
   reprices. Preserve the selected source clock and interpolation and hold
   authorized fixings, quantities, settlement state and discount factors
   fixed. Propagate required reprice failures. The projected sample fraction
   gives the direct level-shift coefficient. It also gives the pillar
   coefficient when the selected curve translates each queried level one
   for one. Fully fixed periods have zero commodity delta. Unpaid cash
   retains applicable signed parallel and bucketed discount DV01. Use
   qualified closing observations and eligible signed payments for horizon
   theta.
   [Price-curve shocks](../core-market/price-curves.md),
   [dated curve risk](../valuations-framework/finite-difference-rates.md),
   [dated theta](../valuations-framework/carry-theta-breakeven.md).

6. **Share the prepared underlying basis and host state.** Commodity
   swaptions use the exact underlying periods, quantities, observation slots
   and fixing resolution. Qualify the actual first observation phase before
   using an all-projected underlying. The swaption owner retains its annuity
   and option model. Reconcile one checked Rust calculation through the
   existing Python and WASM instrument envelopes.
   [Accepted commodity swaption pricing and exercise](commodity-swaption.md#accepted-decision-for-commodity-swaption-pricing-and-exercise).

## Independent calculations

- For raw periods January 6–13 and January 13–16, 2025, take a three-day
  calendar lookback, a synthetic January 8 closure, and include the final
  shifted endpoint. Qualified slots are January 3, 6, 7 and 9, then January
  10 and 13. A separate one-business-day payment lag gives January 14 and 17.
  The regular period has quantity `1,000`, observations `68,70,72,74` and
  discount factor `0.999`. The stub has quantity `250`, observations `70`
  and projected `80`, and payment factor `0.997`. Pay-fixed at `70` gives
  PV USD `2,245.25`. Publication of the last fixing at `78` gives USD
  `1,996.00` PV on the same factors and zero remaining commodity delta.
- With quantity `100`, fixed price `19`, payment factor `0.98`, a known
  fixing `18` and a pending midpoint between log-linear pillars `10` and
  `40`, the pending level is `20` and base PV is zero. Matched `±1` native
  pillar reprices give delta USD `61.29328137` per native price unit. A
  direct one-unit shift of the pending level gives USD `49.00`.

## Observed behavior

### Choice, rationale and scope

The inspected swap uses an equal quantity across all periods and arithmetic
daily business-day averages. The swaption consumes its averaging helper.

### Algorithm and conventions

Build short-back-stub periods using Act365F schedule metadata, no EOM, no payment lag, no accrual-date adjustment and no roll rule. Iterate observation dates in `[accrual_start-lag,accrual_end-lag)`, with the last period also including its end. `index_lag_days` shifts by calendar days, not business days. Include weekdays accepted by the optional strict calendar. Average with compensated summation; if no business days exist, substitute the midpoint date price.

For date `<as_of`, require a supplied exact past fixing; for date `>=as_of`, project from the price curve. Each period pays `Q×average_price` floating and `Q×fixed_price` fixed: there is no accrual-year-fraction multiplier in these commodity cash amounts. Discount only payments on/after valuation. Pay-fixed value is floating PV minus fixed PV; Receive reverses sign.

The raw cashflow schedule exports separate signed fixed and floating flows for all periods, with accrual metadata and projected representation. A parallel additive price-curve bump of ±1 currency price-unit gives swap delta `(PV_up-PV_down)/2`. Generic discount DV01/key-rate measures reprice the same schedule; the shared commodity bump helper supports multiplicative percentage bumps for option sensitivities.

### Selection and controls

Discounting is the only local swap valuation method. Optional calendar selects business-day filtering and date adjustment; no calendar still excludes weekends from observations but forces unadjusted payment scheduling.

Quantity is positive, fixed price finite (negative prices are supported), schedule strictly ordered, frequency nonzero, calendar known, and fixing dates unique/finite. Same-day observations remain projected. No observation-price interpolation is used for missing historical fixings.

### Limitations, failure and fallback

Equal quantity per stub/regular period, final endpoint inclusion, calendar-day lag and midpoint fallback are observed conventions. Raw full-history schedules can require fixings for periods already paid, while PV filters paid periods first. Relative observation cutoff differs from Asian products; this is not automatically a bug or an approved convention.

### Alternatives and tradeoffs

Volume-weighted or day-proportional delivery quantities, daily physical deliveries and publication calendars are unimplemented alternatives. Explicit empty-window rejection would avoid a synthetic midpoint observation but change current behavior.

### Implementation evidence

- [finstack-quant/valuations/exotics/src/commodity/averaging.rs](../../../../finstack-quant/valuations/exotics/src/commodity/averaging.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_swap/metrics.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_swap/metrics.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_swap/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_swap/types.rs)

The original audit inspected source and inline assertions. The independent calculations above cover the stated examples. Production qualification is pending.

Canonical reference index. [Hull](../../../REFERENCES.md#hull-options-futures).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
