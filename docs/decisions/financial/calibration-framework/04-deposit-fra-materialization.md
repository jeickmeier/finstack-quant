# CALIBRATION-FRAMEWORK-004: Deposit/FRA construction and prepared quote clocks

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

A quote becomes the same instrument type used by valuation, with dates resolved once and retained in `PreparedQuote`. The explicit deposit NPV branch avoids using the deposit's redemption-value convention as a zero-PV calibration objective. Shared construction reduces schedule drift; it does not make same-kernel repricing an independent benchmark.

## Inputs, stages and conventions

`BuildCtx` supplies as-of/trade date, numeric notional, required named curve roles, and optional CDS/OIS overrides. Missing curve roles fail. Index ID lookup in `ConventionRegistry` selects currency, calendar, business-day convention, settlement and reset lags, and day count. `resolve_spot_date` adds business days to as-of then applies the convention adjustment.

For deposits, tenor pillars start from spot and explicit dates are adjusted on the market calendar. The built deposit uses the convention currency/day count, decimal fixed rate, and required discount curve. For FRA, start and end pillars are independently resolved from the same spot date; fixing is start minus the convention business-day reset lag. It is a Receive-side FRA with explicit discount/forward roles, fixing calendar and convention. Float-to-decimal conversion errors propagate. Date ordering is enforced through instrument construction rather than an additional bespoke date repair here.

Prepared rate quotes retain the original quote in an `Arc`, the built instrument, resolved pillar date, and target-selected year fraction from base date to pillar. Deposit/FRA pillars are their end dates. `calibration_value_raw` calls deposit `npv_raw`; all other rates call instrument `value_raw`. This preserves numeric residual precision and does not prematurely round into `Money`. Swap payment-lag and futures reference-period pillars are detailed separately.

## Limits and alternatives

Convention data, builders and pricing kernels are external dependencies of this slice; this record does not certify exchange or ISDA compliance. A standalone closed-form calibration formula would give an independent implementation but risks drifting schedules; an external date/cashflow fixture would qualify the shared builder without duplication. Curve time, accrual day count and fixing date are distinct roles and must not be interchanged.

## Source and validation evidence

- Implementation: [build/context.rs](../../../../finstack-quant/calibration/src/build/context.rs) — required curve roles and overrides.
- Implementation: [build/helpers.rs](../../../../finstack-quant/calibration/src/build/helpers.rs) — business-day spot calculation.
- Implementation: [build/rates.rs](../../../../finstack-quant/calibration/src/build/rates.rs) — deposit/FRA dates and instrument orientation.
- Implementation: [build/prepared.rs](../../../../finstack-quant/calibration/src/build/prepared.rs) — cached pillar year fraction and deposit raw NPV override.
- Test evidence: [src/build/tests/rates.rs](../../../../finstack-quant/calibration/src/build/tests/rates.rs) — deposit/FRA fields and EUR/USD construction assertions; inspected or located, not executed.
- Test evidence: [src/build/tests/quote_construction.rs](../../../../finstack-quant/calibration/src/build/tests/quote_construction.rs) — quote-family construction smoke evidence; inspected or located, not executed.
- Test evidence: [src/build/prepared.rs](../../../../finstack-quant/calibration/src/build/prepared.rs) — deposit calibration-value convention regression; inspected or located, not executed.
- Reference context: [ISDA 2021 definitions](../../../REFERENCES.md#isda-2021-definitions) — convention context; no mapping of every registry entry to a versioned source is established; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
