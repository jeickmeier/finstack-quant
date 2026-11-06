# PORTFOLIO-EXECUTION-014 — Scenario state isolation, selective invalidation and P&L batches

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and scope

Every scenario is applied independently to the original market/portfolio state; a scenario batch is not a compounded path through previous stressed states. The scenario engine owns operation ordering and financial transformations. This bridge owns cloning, effective-date propagation, position replacement, evaluation profiles, selective reuse and P&L reduction.

## Algorithm and selection

`apply_scenario` clones the market and clones instruments only when the specification says they are required. Its execution context has no statements model, rate bindings or calendar, and starts at `portfolio.as_of`. The engine receives caller config and optionally the batch's cached recalibration provider. It returns an authoritative change report and effective date. If the scenario mutates instruments, require exactly one returned instrument per position, replace in order, rebuild dependency/state indices and update the owned portfolio's date. Otherwise borrow the original portfolio but evaluate at the returned market-state date.

The public `apply_scenario_and_revalue` uses default strict valuation with the standard DV01 menu. P&L entry points deliberately use strict `Only([])`, so their endpoints follow canonical PV-only pricing and cannot use best-effort fallback. Empty batches return empty results without a base valuation. Nonempty batches calculate one base valuation and handle scenario waves of at most eight active states. Each wave compiles to the shared evaluation plan. The batch shares a `CachedRecalibrationProvider` for scenario application; valuation states separately build their own pricing-option providers. The cache key's safety for application reuse is owned by calibration.

Selective reuse is disabled for changed effective date, portfolio shape, `all_dirty`, a stale position count, out-of-range changed instrument indices, FX targets or volatility-index targets. Other market targets map to exact portfolio keys: discount/forward/commodity/credit/inflation curves, scalar equity prices, volatility surfaces and base correlation. Union market-affected indices with changed instrument indices. Instrument changes mark the internal invalidation authoritative; the shared planner still checks compatible shape/profile and clean-row identity. A no-op scenario can reuse the entire base result.

## P&L and error ordering

For each stressed position, subtract its base value, using zero for a new position; append negated base values for removed positions. Total is stressed portfolio total minus base total using checked Money subtraction, rather than re-summing the position P&L map. Reporting is base-currency MTM difference, with no separately accumulated realized cash or transaction ledger.

Application proceeds in scenario input order until a failure within the wave. Earlier successfully applied states are still evaluated and scanned in order before that application error is returned. Therefore an earlier valuation error wins over a later application error. The public batch returns one error, not partial results. Shared execution may already have evaluated later jobs when an earlier logical error is selected.

## Alternatives and limitations

Sequentially applying all scenarios to one mutable market would be a different scenario composition product; it is not this batch. Full evaluation is the fallback when the manifest cannot prove selective safety. This audit does not prove complete manifests for every scenario operation; the [scenario methodology scope](../scenarios/README.md) records their owning algorithms.

The instrument replacement loop retains the position reference ID while replacing its live instrument. If an operation changes an instrument ID, the reference field can differ from the live identity checked by materialization. No such numerical failure was reproduced. Current accepted state-isolation/reuse requirements are binding under [ADR 0022](../../architecture/0022-portfolio-evaluation-state.md).

## Evidence

[scenarios.rs](../../../../finstack-quant/portfolio/src/scenarios.rs), [evaluation plan](../../../../finstack-quant/portfolio/src/evaluation/plan.rs) and [executor](../../../../finstack-quant/portfolio/src/evaluation/executor.rs). Inspected [integration scenario error-order test](../../../../finstack-quant/portfolio/tests/integration_scenarios.rs); inline tests locate no-op exact zero, one base valuation, count mismatch, new/removed positions and FX disabling selective reuse. [Fallback tests](../../../../finstack-quant/portfolio/tests/valuation_fallback.rs) locate rejection of noncanonical PV fallback for P&L. These are state/contract and same-pricer reconciliation evidence, not independent scenario-price validation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
