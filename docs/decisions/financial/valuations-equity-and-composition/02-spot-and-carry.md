# Cash equity valuation and dividend-adjusted carry

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-02`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

Cash equity is quantity times an explicitly resolved share price. Forward reporting uses rebased discount factors and either a discrete-dividend escrow or continuous dividend yield. This is the observed carry convention; its suitability for a particular security remains a caller/adoption decision.

## Scope and input domain

Finite signed quantities (omission means one share), nonnegative quoted/market share prices, explicit currency and optional dividend scalar/cash-dividend schedule. The forward horizon is a finite nonnegative number of years in the discount curve's time coordinate.

## Algorithm

1. `quoted_spot` takes precedence over `spot_id`. A `Price` scalar is converted into instrument currency using market FX; a unitless scalar is interpreted in that currency. Missing both sources fails.
2. PV is `effective_quantity × share_price`; no discounting enters current cash-equity PV.
3. For a forward horizon `t`, compute `D = DF(curve_time(as_of)+t)/DF(curve_time(as_of))`.
4. If the discrete-dividend vector is nonempty, subtract PVs of dividends with `as_of < ex_date ≤ horizon` and divide prepaid spot by `D`; otherwise use `S exp(-q t)/D`. Omitted dividend-yield ID means zero, while a specified missing/wrong-type scalar errors.
5. Metrics expose price, effective shares, yield and forward price. Delta is the signed share quantity, also stored under the ticker component. The forward metric reads `<ticker>-FWD_T`; any lookup failure defaults to zero, and a monetary scalar is read by its amount.

## Selection, controls and conventions

The generic equity registration selects its native spot calculation. Rate risk uses shared parallel/key-rate bump calculators and ordinarily vanishes for a fixed spot. Cashflow export is `NoResidual`, with a notional hint only when a quoted spot exists; configured dividends affect forward reporting but are not emitted as this instrument's dated cashflows.

## Limitations, failures and alternatives

The forward method does not reject a negative prepaid spot after cash dividends. A nonempty dividend list suppresses continuous yield even when no dividend remains inside the horizon. Quoted-spot delta denotes economic share exposure although bumping a separate market spot cannot move that quote. Modeling dividend events as dated cashflows, or requiring a horizon scalar instead of zero fallback, are alternatives requiring agreement.

## Evidence

[Hull carry reference](../../../REFERENCES.md#hull-options-futures); [types_tests.rs](../../../../finstack-quant/valuations/equity/tests/equity/types_tests.rs) and [test_bucketed_dv01.rs](../../../../finstack-quant/valuations/equity/tests/equity/test_bucketed_dv01.rs) contain local construction/carry and rate-risk checks; [pricer_tests.rs](../../../../finstack-quant/valuations/tests/instruments/equity/pricer_tests.rs) supplies pricing cases. These are formula/behavior comparisons, not a historical dividend or corporate-action benchmark.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/spot/types.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/spot/metrics.rs)
