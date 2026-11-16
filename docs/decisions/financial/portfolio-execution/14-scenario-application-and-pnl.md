# PORTFOLIO-EXECUTION-014 — Scenario state isolation, selective invalidation and P&L batches

- **Decision status:** Accepted 2026-11-16 — complete independent scenario execution, fixed inventory, strict P&L endpoints, selective reuse, Money reconciliation and report package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent scenario isolation, signed FX, Money rounding and failure-order examples verified. Runtime pricing, restoration and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio scenario revaluation and MTM

1. **Apply each scenario independently to the admitted opening state.**
   Use the same immutable opening market and holdings for every scenario.
   The scenario engine owns action order, admitted effects and time rolls.
   Supply active information and calendar inputs explicitly. Reject
   required statement or binding actions on the portfolio-only route.
   Use the returned effective cut for valuation and FX, including when
   the adapter borrows the unchanged holdings.
   [Atomic scenario execution](../scenarios/02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution)
   and [scenario time rolls](../scenarios/13-market-time-roll.md#accepted-decision-for-checked-scenario-time-rolls-and-horizon-replay).

2. **Preserve the opening holding inventory.** Match each replacement to
   its exact holding occurrence, instrument ID and type. Preserve holding
   order, sizing, entities and books. Validate the complete replacement
   state and rebuild its indexes before use. Reject missing, extra or
   mismatched endpoint rows and retire their zero-fill treatment. Product
   owners determine whether a matured or settled claim has a qualified
   zero mark.
   [Instrument overrides](../scenarios/11-instrument-overrides.md#accepted-decision-for-checked-scenario-instrument-overrides).

3. **Use explicit revaluation controls and strict PV-only P&L endpoints.**
   Ordinary scenario revaluation takes the caller's metric menu and risk
   policy. Remove implicit DV01. Scenario P&L uses comparable canonical
   PV-only marks with admitted model, settings and provider and qualified
   cash FX at each endpoint. Price the opening book once for a nonempty
   batch. An empty batch returns no results without opening book pricing.
   [Valuation profiles](03-valuation-profiles-and-failures.md#accepted-decision-for-checked-portfolio-valuation-profiles-and-failures)
   and [portfolio FX](08-spot-and-forward-fx.md#accepted-decision-for-checked-portfolio-nav-and-cashflow-fx).

4. **Reuse only from verified opening and final states.** Require typed
   effect evidence, matching holding and pricing provenance, and complete
   coverage of actual sources and source selectors. Include opening and
   final credit-index dependencies, changed instrument occurrences, FX
   routes and bridge curves. A proven reporting-only change can reuse
   native PV and refresh conversion. Incomplete proof triggers fresh work,
   including for an apparently empty change manifest. Scope recalibration
   caches to the request and registered immutable market state.
   [Selective repricing](05-dependencies-and-selective-reuse.md#accepted-decision-for-checked-portfolio-dependencies-and-selective-repricing)
   and [recalibration caches](../calibration-framework/23-recalibration-cache.md#accepted-decision-for-exact-request-local-recalibration-caching).

5. **Compute headline MTM and holding reconciliation in checked Money.**
   Subtract opening NAV from stressed NAV. Compute holding differences
   from already sized and translated marks in opening holding order.
   Report the ordered holding subtotal and checked arithmetic difference
   to the headline. Preserve exact zeros and the accepted Money precision
   rules. Keep realized cash outside MTM. Elected carry remains separate
   with actual holding sizes and explicit coverage.
   [Money arithmetic](../core-conventions/012-money-decimal-precision.md)
   and [scenario carry](../scenarios/14-carry-aggregation.md#accepted-decision-for-checked-scenario-roll-carry-and-native-aggregation).

6. **Make batch failure atomic and follow input order.** Validate shared
   request structure first, then the required opening valuation. Keep at
   most eight active stressed states per wave and use one executor parallel
   axis. Select the first scenario failure in input order across application,
   planning, pricing, P&L and required report construction. Publish only
   after all scenarios succeed. Preserve each occurrence by input ordinal
   so repeated scenario IDs remain distinguishable.
   [Execution order](04-execution-order-and-reduction.md#accepted-decision-for-checked-portfolio-execution-and-nav-reduction).

7. **Pair application and valuation evidence in one checked result.**
   Retain exact scenario identity and content, opening and stressed state
   references and dates, selected controls and FX, ordered holding marks,
   effects, reuse and carry evidence, and derivable P&L. Use bounded Rust
   restoration to validate the result. Preserve exact Money and native
   ordered axes across Python tables, WASM and UI. Charts use an explicit
   numeric projection. Economic replay requires original qualified inputs.
   [Scenario reports and replay](../scenarios/17-reports-and-replay.md#accepted-decision-for-checked-scenario-reports-and-replay).

## Independent calculation and order checks

- Separate `+10%` and `−10%` scenarios on 100 yield 110 and 90. Applying
  the same actions in order in one scenario yields 99.
- With an opening and stressed large holding of USD `1e28`, a second
  holding increasing from USD `0.001` to `0.002` leaves both checked NAVs
  equal. The headline delta is zero, the holding subtotal is `+0.001`,
  and the reconciliation difference is `−0.001`.
- A signed holding mark of USD `−240` rising to `−225` gives USD `+15`
  of MTM. An ex-cash mark falling from USD 100 to USD 98 after a USD 2
  coupon gives USD `−2` MTM and zero cash-inclusive change.
- In every tested completion order, the first scenario's pricing failure
  precedes later scenario planning and application failures.

## Observed choice and scope at the audit revision

Every scenario is applied independently to the original market/portfolio state; a scenario batch is not a compounded path through previous stressed states. The scenario engine owns operation ordering and financial transformations. This bridge owns cloning, effective-date propagation, position replacement, evaluation profiles, selective reuse and P&L reduction.

## Algorithm and selection

`apply_scenario` clones the market and clones instruments only when the specification says they are required. Its execution context has no statements model, rate bindings or calendar, and starts at `portfolio.as_of`. The engine receives caller config and optionally the batch's cached recalibration provider. It returns an authoritative change report and effective date. If the scenario mutates instruments, require exactly one returned instrument per position, replace in order, rebuild dependency/state indices and update the owned portfolio's date. Otherwise borrow the original portfolio but evaluate at the returned market-state date.

The public `apply_scenario_and_revalue` uses default strict valuation with the standard DV01 menu. P&L entry points deliberately use strict `Only([])`, so their endpoints follow canonical PV-only pricing and cannot use best-effort fallback. Empty batches return empty results without a base valuation. Nonempty batches calculate one base valuation and handle scenario waves of at most eight active states. Each wave compiles to the shared evaluation plan. The batch shares a `CachedRecalibrationProvider` for scenario application; valuation states separately build their own pricing-option providers. The cache key's safety for application reuse is owned by calibration.

Selective reuse is disabled for changed effective date, portfolio shape, `all_dirty`, a stale position count, out-of-range changed instrument indices, FX targets or volatility-index targets. Other market targets map to exact portfolio keys: discount/forward/commodity/credit/inflation curves, scalar equity prices, volatility surfaces and base correlation. Union market-affected indices with changed instrument indices. Instrument changes mark the internal invalidation authoritative; the shared planner still checks compatible shape/profile and clean-row identity. A no-op scenario can reuse the entire base result.

## P&L and error ordering

For each stressed position, subtract its base value, using zero for a new position; append negated base values for removed positions. Total is stressed portfolio total minus base total using checked Money subtraction, rather than re-summing the position P&L map. Reporting is base-currency MTM difference, with no separately accumulated realized cash or transaction ledger.

Application proceeds in scenario input order until a failure within the wave. Earlier successfully applied states are still evaluated and scanned in order before that application error is returned. Therefore an earlier valuation error wins over a later application error. The public batch returns one error, not partial results. Shared execution may already have evaluated later jobs when an earlier logical error is selected.

A live source check found that plan preparation still propagates an error
before executing earlier jobs. An error from that phase can mask an earlier
scenario's valuation or P&L failure. Permitted Money rounding can also make
the ordered position differences diverge from the change in checked NAV.

## Alternatives and limitations

Sequentially applying all scenarios to one mutable market would be a different scenario composition product; it is not this batch. Full evaluation is the fallback when the manifest cannot prove selective safety. This audit does not prove complete manifests for every scenario operation; the [scenario methodology scope](../scenarios/README.md) records their owning algorithms.

The instrument replacement loop retains the position reference ID while replacing its live instrument. If an operation changes an instrument ID, the reference field can differ from the live identity checked by materialization. No such numerical failure was reproduced. Current accepted state-isolation/reuse requirements are binding under [ADR 0022](../../architecture/0022-portfolio-evaluation-state.md).

## Evidence

[scenarios.rs](../../../../finstack-quant/portfolio/src/scenarios.rs), [evaluation plan](../../../../finstack-quant/portfolio/src/evaluation/plan.rs) and [executor](../../../../finstack-quant/portfolio/src/evaluation/executor.rs). Inspected [integration scenario error-order test](../../../../finstack-quant/portfolio/tests/integration_scenarios.rs); inline tests locate no-op exact zero, one base valuation, count mismatch, new/removed positions and FX disabling selective reuse. [Fallback tests](../../../../finstack-quant/portfolio/tests/valuation_fallback.rs) locate rejection of noncanonical PV fallback for P&L. These are state/contract and same-pricer reconciliation evidence, not independent scenario-price validation.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
