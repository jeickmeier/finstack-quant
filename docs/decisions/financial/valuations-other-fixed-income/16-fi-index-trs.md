# VALUATIONS-OTHER-FIXED-INCOME-16: Fixed-income index TRS carry returns and duration risk proxy

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

[FI index TRS pricing](../../../../finstack-quant/valuations/fixed-income/src/fi_trs/pricer.rs) uses continuously compounded index yield for projected total return. Source explicitly selects carry analytics to retain yield sensitivity and explicitly disclaims a full index mark-to-market model. Both native and registered generic pricing use this same kernel; no alternative model selector supplies omitted underlying price risk. That documented intent is not approval for fair-value or margin uses.

## Algorithm and selection

1. [Validate](../../../../finstack-quant/valuations/fixed-income/src/fi_trs/types.rs) finite nonnegative notional, matching underlying currency, schedule/financing inputs and positive finite optional initial level. Read yield from a strict unitless scalar if an identifier exists; configured missing/wrong-type data error. No identifier means intentional zero carry.
2. For fully future return period `[start,end]`, project `exp(y×year_fraction(start,end))−1` on the schedule day count. Yield must already be an annual continuous decimal; the engine does not convert a supplied bond-equivalent yield.
3. For a period in progress, resolve its exact start reset from fixing series, or initial level only at schedule start. Read live positive index level in underlying currency or unitless form, then use `live_level×exp(y×remaining_time)/reset_level−1`. For a completed period whose delayed payment remains relevant, return exact end/start fixing ratio minus one. Missing endpoint fixings error; no interpolation of realized index levels occurs.
4. Shared [TRS engine](../valuations-framework/trs-legs.md) multiplies period returns by trade notional and discounts on the financing discount curve. Financing uses its configured index, fixings, spread and day count through that engine. Receive-total-return PV is return-leg PV minus financing PV; pay side reverses it. Both cashflow legs are aggregated by payment date with the same signs and financing projected-fixing metadata.
5. [ParSpread](../../../../finstack-quant/valuations/fixed-income/src/fi_trs/metrics/par_spread.rs) divides return PV less zero-spread financing PV by notional-weighted discounted financing accrual and reports basis points, always from the receiver's perspective. FinancingAnnuity is the money-times-year denominator; multiplying it by `.0001` gives a one-bp financing-spread PV.

## Risk and margin conventions

[DurationDv01](../../../../finstack-quant/valuations/fixed-income/src/fi_trs/metrics/mod.rs) requires explicit finite unitless index duration in years. It reports `−N×D×1e−4` for receiving total return and positive for paying. This measures intended economic long-bond yield risk; it is **not** the derivative of the carry-only pricing function. There is no fabricated default duration. The resolver checks finiteness but does not impose nonnegative duration.

Shared parallel/bucketed DV01 shocks financing discount/forward curves and reprices the carry model. It therefore captures financing/discount effects, not a marked-to-market underlying bond basket. [SIMM input](../../../../finstack-quant/valuations/fixed-income/src/fi_trs/marginable.rs) emits only the signed duration proxy into one currency IR tenor bucket selected from duration; it does not combine full financing key rates, index spread/credit risk or the index constituents. VM mark uses the carry-only PV.

## Limitations, alternatives and evidence

No constituent cashflows, roll-down, duration/convexity repricing, spread migration/default, or rebalancing dynamics are projected. Using yield as expected total return is a scenario/carry assumption, not a demonstrated risk-neutral price. The risk proxy and PV can have inconsistent yield responses by construction. A constituent-based forward mark, full price-index curve or joint rates/credit model are unimplemented alternatives; observed fixing-based realized returns are supported.

[TRS integration cases](../../../../finstack-quant/valuations/tests/instruments/trs/test_fi_index_trs.rs) and colocated `duration_risk_requires_a_duration_input` provide located examples and required-input checks. [ISDA definitions](../../../REFERENCES.md#isda-2006-definitions) and [SIMM](../../../REFERENCES.md#isda-simm) are reference context, not certification of this proxy.  No independent constituent-index price reconciliation was established.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
