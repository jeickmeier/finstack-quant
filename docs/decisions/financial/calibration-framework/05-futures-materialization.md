# CALIBRATION-FRAMEWORK-005: Listed rate futures and reference periods

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

Futures retain listed price and contract economics rather than being reduced to a generic FRA. Reference period, expiry, fixing and settlement are constructed separately. The code's rationale is observable in explicit variant handling and date checks; no claim of current exchange compliance follows.

## Ordered construction

1. Resolve the futures contract and its rate index. Adjust supplied expiry on the contract calendar using the index business-day convention.
2. ForwardStarting: add contract settlement business days, adjust, then add delivery months. ImmQuarterInArrears: period end is the third Wednesday of expiry month; expiry must equal its preceding business day; period start is the third Wednesday delivery-months earlier. CalendarMonthInArrears: use first calendar day to next month first, with expiry the last business day of the reference month. BusinessMonthInArrears: enforce the same expiry, but adjust both month boundaries Following.
3. Simple-compounding fixing is period start minus index reset-lag business days. Other compounding uses period end minus one calendar day. Settlement is expiry for simple futures and period end otherwise.
4. Build `ListedFutureTerms` long, with contracts = calibration notional / face value, multiplier = tick value / tick size, convention currency, and the original quoted price. Face value, tick size, delivery months and supplied convexity adjustment are retained in `FutureContractSpecs`; quote authoring defaults convexity to zero. Explicit discount/forward IDs and index/compounding metadata are preserved.
5. The prepared pillar is **reference-period end**, not expiry. Its year fraction uses the target curve clock. Pricing and quote-bump conversion are owned by the instrument/quote modules, not recomputed here.

## Limits and alternatives

Wrong expiry for an arrears contract fails; the builder does not silently infer a new contract month. The calendar-month computation is performed after expiry adjustment. Alternatives are synthetic FRA approximation (simpler but drops listed economics) or direct exchange settlement schedules (stronger provenance, larger data contract).

## Source and validation evidence

- Implementation: [build/rates.rs](../../../../finstack-quant/calibration/src/build/rates.rs) — four period variants, fixing/settlement dates and listed economics.
- Implementation: [build/prepared.rs](../../../../finstack-quant/calibration/src/build/prepared.rs) — future pillar selection.
- Implementation: [api/authoring.rs](../../../../finstack-quant/calibration/src/api/authoring.rs) — default zero convexity.
- Test evidence: [src/build/rates.rs](../../../../finstack-quant/calibration/src/build/rates.rs) — contract reference-period regressions located; inspected or located, not executed.
- Test evidence: [src/build/tests_rates.rs](../../../../finstack-quant/calibration/src/build/tests_rates.rs) — SOFR and EURIBOR future construction fields; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
