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

## Relationship to the accepted frozen CPI boundary

[CORE-MARKET-10's complete accepted package](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks) retains absolute CPI levels
on their original source base/clock, with an independent valuation reference.
It removes CPI rebasing, interpolated new origin levels and expired-pillar
deletion; original annual-parameter shocks retain their original coordinates.
A projection cannot become a published fixing merely because the valuation
date advances. Step3 above records the current runtime, and broader fixing
materialization/context/replay choices remain separately Draft. Record10 was
accepted on **2026-11-08**; its reconciliation and qualification remain pending.

## Relationship to the accepted observation knowledge boundary

[CORE-MARKET-11's complete accepted package](../core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability) keeps an immutable selected
fixing snapshot and knowledge cutoff K distinct from valuation date v. Moving
v alone cannot make a projection a published observation or advance actual
revision knowledge. A new cutoff/version requires explicitly supplied data;
any hypothetical projected-fixing materialization must remain labelled as such.
Reconcile availability routing and provenance with the accepted CPI source
boundary. Record11 was accepted on **2026-11-08** with reconciliation and
qualification pending; this scenario's remaining fixing/time evolution
methodology stays Draft.

## Relationship to the accepted rate fixing dataset boundary

[CORE-MARKET-12's complete accepted package](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings) keeps exact observation
identity distinct from projection identity and prohibits ordinary generic bumps
of fixing histories, including no-ops. Explicit pre-roll model materialization
must retain hypothetical-dataset provenance; advancing valuation date or
inserting FIXING data is not actual publication knowledge. Reconcile strict
series sampling policies and financial eligibility without copying the monthly
CPI snapshot representation into a universal fixing store. Record12 was accepted
on **2026-11-08**, with reconciliation/qualification pending; this scenario's
remaining time/materialization policy stays Draft.

## Relationship to the accepted context lifecycle boundary

[CORE-MARKET-20's accepted package](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle)
replaces per-source day-offset roll with explicit target-date source conditioning
and atomic canonical composite rebinding. Retain actual frozen observation/event/
volatility clocks and knowledge cutoffs; active horizon pricing still requires
its qualified model/eligibility. Recipe presence does not authorize automatic
current-fit/replay claims; replay qualifies the actual request under accepted06.
Cloned FX can retain a live date-sensitive provider, so held-FX scenarios need
explicit qualified quote capture/coverage. Accepted record 20 does not
adopt this record's date resolution, projected fixings, replay selection, carry
rows or broader scenario financial model.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
