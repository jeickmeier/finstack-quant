# SCENARIOS-013: Time-roll dates, fixing materialization and hazard replay

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: horizon market evolution realizes curve forwards and projects crossed coupon observations before advancing curve origins. It is not an unchanged-curve lookup at a later valuation date. A completed roll can be followed by market shocks; `apply_shocks=false` terminates after this roll.

## Date domain and ordered algorithm

Parse the tenor. `CalendarDays` advances actual months/years/days without adjustment. `BusinessDays` advances the tenor then uses **ModifiedFollowing**, supplying the provided calendar or `WEEKENDS_ONLY`; it does not mean counting N business days for a tenor such as `1D`. `Approximate` converts tenor to approximate days and adds that integer. Reject any resolved negative day shift. A zero shift returns an empty carry report without fixing projection, market roll or hazard replay.

For an advancing roll:

1. Collect additional `(hazard,discount)` replay targets requested by upcoming solve-to-par ParCDS shocks. Also scan every calibrated hazard curve and read its recipe's `discount_curve_id`; reject missing recipe routing or missing discount data. Deduplicate hazard targets.
2. If an inventory is supplied, collect every instrument's canonical pre-roll cashflow schedule. Any schedule failure aborts the roll before committing state. Materialize required coupon observations crossed in `[old_date,new_date]` against the pre-roll market; existing exact-date observations retain priority. Missing/conflicting projections error. This projection is not simulated future fixings.
3. Call shared `MarketContext::roll_forward(day_shift)` on the projected market. Curve-specific clocks shift knots/base dates and realize forwards: discount curves renormalize by the starting DF, forward curves preserve future forward levels, inflation rebases CPI and price/index curves take spot from the old forward. Shared curve-roll details remain in [core market lifecycle](../core-market/context-lifecycle.md).
4. Replay each calibrated/requested hazard using its original hazard and original source market, the rolled target market and resolved discount curve, with `TimeRollReplay`. Provider failure aborts before context assignment.
5. Compute carry rows using original and rolled markets, then assign the rolled market and new as-of date. Carry-row failures are reported/excluded but do not block this final commit.

## Limits, controls and alternatives

The raw market roll leaves FX spot, scalar prices, observations, dividends, hierarchy and volatility surfaces/cubes with their expiry axes unchanged; only explicitly materialized coupon fixings are added beforehand. It does not model stochastic spot/vol evolution, forward variance conditioning or new realized option histories. Instruments themselves are not state-transitioned or exercised by this operation; their valuation/schedule interfaces determine lifecycle support.

The roll's construction is atomic with respect to its market/date assignment, but the overall multi-operation scenario is not. Pre-roll schedule projection is stricter than carry collection: one schedule failure aborts the whole roll, whereas one endpoint-value/cashflow failure after a constructed roll removes only that carry row. Expiring all useful curve pillars or replaying beyond supported calibration maturities may error rather than extrapolate indefinitely.

## Evidence

Owning source: [adapters/time_roll.rs](../../../../finstack-quant/scenarios/src/adapters/time_roll.rs), [engine/mod.rs](../../../../finstack-quant/scenarios/src/engine/mod.rs). Shared projection: [fixing materialization](../cashflows/16-fixing-materialization.md). Inspected inline regression verifies failed projection leaves market/date unchanged. [Production historical-roll audit](../../../../finstack-quant/scenarios/tests/production_historical_roll_audit.rs) and [ParCDS replay tests](../../../../finstack-quant/scenarios/tests/par_cds_bump.rs) locate exact-observation priority, horizon quote replay and zero-day no-replay evidence. These are replay/lifecycle invariants, not a historical market forecasting model.

Approximate month/year days and holiday-aware tenor rolls are different models; their intended usage remains unadopted.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
