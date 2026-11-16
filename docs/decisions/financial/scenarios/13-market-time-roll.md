# SCENARIOS-013: Time-roll dates, fixing materialization and hazard replay

- Decision status: **Accepted 2026-11-15 — complete date admission, fixing treatment, market conditioning and explicit horizon replay package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent date, fixing eligibility, factor ratio and local survival examples verified.** Production roll, replay, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for checked scenario time rolls and horizon replay

1. **Resolve one explicit target date.** Accept the exact target civil
   date or a positive calendar tenor with an explicit business-day
   convention and a qualified calendar for adjusted dates. Compute the raw
   calendar endpoint once, then adjust it once. Report the actual elapsed
   calendar days. Retire scenario-level approximation and implicit
   weekends-only/calendar adjustment defaults.
   [Tenor and date arithmetic](../core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions) ·
   [Calendar adjustments](../core-conventions/006-business-day-adjustments.md#accepted-decision-rolling-and-joint-calendar-conventions).

2. **Make identity and continuation explicit.** Reject a resolved target
   before the opening date. An equal target with preserved information is
   an admitted identity with zero elapsed days and empty carry evidence.
   Horizon requoting requires a later date. Same-day factual or
   hypothetical information changes use their fixing owners. Require an
   explicit continuation choice and admit one roll per scenario. A
   roll with `apply_shocks=false` is the sole operation and ends after
   any elected carry.
   [Scenario specification](01-specification-and-composition.md#accepted-decision-for-checked-scenario-specifications-and-ordered-composition).

3. **Elect fixing treatment over exact observation keys.** Preserve the
   selected factual snapshot, cutoff and prior hypothetical records, or
   expressly elect qualified raw projection realization from the frozen
   pre-roll market. Determine required keys under the active product
   information rules. Discover dependencies independently of full
   payoff schedules. A qualified empty set is valid. Failed or unsupported
   required discovery and missing required published data fail. Admit the
   complete batch before qualified exact-observation precedence. Retain
   the full paired information and hypothetical provenance for downstream
   pricing, later rolls and restoration.
   [Fixing materialization](../cashflows/16-fixing-materialization.md#accepted-decision-qualified-hypothetical-fixing-materialization) ·
   [Fixing knowledge](../conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility).

4. **Condition the original curve evaluators at the same target date.**
   Preserve full original evaluators, clocks and tails. Condition discount
   and projection factors as `C(T)/C(v)` on the original source
   coordinates. For hazard use conditional survival from local integrated
   intensity on the original source clock, including when absolute
   survival has underflowed. Rebind affected credit-index dependencies
   canonically to the admitted dated sources.
   [Market context lifecycle](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle) ·
   [Hazard conditioning](../core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation).

5. **Keep frozen sources and product eligibility explicit.** Retain the
   original absolute CPI, commodity and volatility-index sources with
   their clocks, along with selected observations, stored spot and scalar
   data, dividend dates, volatility axes and correlation basis. Held FX
   requires captured quotes with coverage for the applicable queries.
   Active frozen vol or history use requires product qualification.
   Exercise, settlement, rebalancing and stochastic evolution stay with
   their product owners.
   [Market context horizon and FX rules](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).

6. **Require an express horizon hazard requote.** Identify each exact
   hazard and discount dependency in an ordered, duplicate-free election.
   Qualify the original fit on its immutable source market and bind the
   horizon discount and information state independently. Requote surviving
   exact contractual maturities, report expiries and require a nonempty
   live set and complete native quote repricing. Check local integrated
   hazard at every civil date through the elected end date within explicit
   tolerance and work limits. Ordinary rolls retain conditioned intensity.
   An attached recipe or later CDS shock does not elect a refit.
   [Horizon hazard replay](../calibration-framework/22-hazard-deal-and-horizon.md#accepted-decision-for-deal-overrides-and-horizon-hazard-replay) ·
   [Scenario hazard actions](07-hazard-quote-shocks.md#accepted-decision-for-native-hazard-shocks-and-exact-cds-scenario-replay).

7. **Share preparation and publish atomically.** Direct scenarios and
   instrument-horizon analysis use the same date and fixing preparation.
   Stage the date, market, fixing provenance, fit/replay evidence and
   report together. Required failures preserve the opening scenario
   state. Record 014 owns optional carry admission and failure policy.
   Record 017 owns report and replay representation. Apply the checked
   Rust contract across Python and WASM.
   [Atomic execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution) ·
   [Carry record 014](14-carry-aggregation.md) ·
   [Reporting record 017](17-reports-and-replay.md).

## Independent calculation checks

- March 31, 2025 plus two calendar months gives May 31, 61 days
  later. Modified Following on a weekends-only calendar gives May
  30, 60 days later. May 30 plus one day under the same convention
  resolves to May 30. If the selected calendar also closes that
  Friday, the same request resolves to May 29 and fails.
- A Monday observation released Tuesday at 08:00 is ineligible at
  Tuesday 07:00 and eligible at 09:00 in a snapshot with that release.
  A Tuesday observation released Wednesday remains unpublished at
  either Tuesday cutoff.
- Source factors `.97` at horizon and `.92` at payment imply
  a conditioned factor of about `.948453608247423`. At constant
  intensity 1000, absolute survival at years 1 and 1.1 underflows
  in binary64. The local conditional survival is
  `exp(−100) ≈ 3.720075976020836e−44`.

## Observed choice and rationale at the audit revision

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

## Evidence and current call path

Owning source: [adapters/time_roll.rs](../../../../finstack-quant/scenarios/src/adapters/time_roll.rs), [engine/mod.rs](../../../../finstack-quant/scenarios/src/engine/mod.rs). Shared projection: [fixing materialization](../cashflows/16-fixing-materialization.md). Inspected inline regression verifies failed projection leaves market/date unchanged. [Production historical-roll audit](../../../../finstack-quant/scenarios/tests/production_historical_roll_audit.rs) and [ParCDS replay tests](../../../../finstack-quant/scenarios/tests/par_cds_bump.rs) locate exact-observation priority, horizon quote replay and zero-day no-replay evidence. These are replay/lifecycle invariants, not a historical market forecasting model.

The current adapter calls `condition(new_date)` and delegates canonical
credit-index rebinding to core. It still discovers fixings via complete
cashflow schedules, applies a generic observation-date window, and infers
replay from recipes or later CDS operations. The older rebasing and
`roll_forward(day_shift)` description is retained above as audit evidence.

## Relationship to the accepted frozen CPI boundary

[CORE-MARKET-10's complete accepted package](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks) retains absolute CPI levels
on their original source base/clock, with an independent valuation reference.
It removes CPI rebasing, interpolated new origin levels and expired-pillar
deletion; original annual-parameter shocks retain their original coordinates.
A projection cannot become a published fixing merely because the valuation
date advances. Step 3 above records the audit-era runtime. Record 10 was
accepted on **2026-11-08**; its reconciliation and qualification remain pending.

## Relationship to the accepted observation knowledge boundary

[CORE-MARKET-11's complete accepted package](../core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability) keeps an immutable selected
fixing snapshot and knowledge cutoff K distinct from valuation date v. Moving
v alone cannot make a projection a published observation or advance actual
revision knowledge. A new cutoff/version requires explicitly supplied data;
any hypothetical projected-fixing materialization must remain labelled as such.
Reconcile availability routing and provenance with the accepted CPI source
boundary. Record 11 was accepted on **2026-11-08** with reconciliation and
qualification pending.

## Relationship to the accepted rate fixing dataset boundary

[CORE-MARKET-12's complete accepted package](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings) keeps exact observation
identity distinct from projection identity and prohibits ordinary generic bumps
of fixing histories, including no-ops. Explicit pre-roll model materialization
must retain hypothetical-dataset provenance; advancing valuation date or
inserting FIXING data is not actual publication knowledge. Reconcile strict
series sampling policies and financial eligibility without copying the monthly
CPI snapshot representation into a universal fixing store. Record 12 was
accepted on **2026-11-08**, with reconciliation and qualification pending.

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

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#scenarios).
