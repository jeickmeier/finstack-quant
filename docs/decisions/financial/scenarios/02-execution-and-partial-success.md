# SCENARIOS-002: Execution phases, effect ordering and partial success

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: apply operations to the caller's mutable market/model/inventory, preserving order within defined phases. Adapters first generate effects, and the engine commits them in operation order. This supports dependencies such as a rate shock followed by hazard quote replay. Overall application is explicitly non-atomic.

## Ordered algorithm and controls

1. Validate the spec and reject missing instrument inventory for any instrument-mutating operation before changing state. Expand all hierarchy targets against the opening market.
2. Execute the sole time roll first wherever it appears in the list. If `apply_shocks=false`, return its report immediately; other market and statement operations are skipped, although initial validation and hierarchy expansion have already run.
3. Traverse expanded operations, deferring forecast and inline rate-binding operations. Flush pending `MarketBump`s before every market operation. Price writes and curve replacements also flush before committing. Adjacent effects within an operation can share a batch; separate user operations observe prior completed changes.
4. Apply existing context rate bindings against the final shocked market. Map key must equal `binding.node_id`; mismatch is a hard error. Binding errors otherwise become warnings.
5. Apply deferred inline bindings and forecast operations in their original relative order. An inline binding updates the context map only when a map already exists, and immediately assigns its calculated rate. Successful assignments/adjustments count; missing-node or calculation failures warn and execution continues.
6. Reevaluate the statement model when at least one deferred operation counted or an initial binding map exists. Evaluation failures warn. The adapter returns evaluator warnings, not computed statement results.

Market lookup, curve construction and replay failures generally propagate. Missing statements can therefore hard-fail after prior market writes. Equity misses, hierarchy misses, unsupported instrument override routing, statement failures and individual carry failures are warning pathways with distinct semantics; an `Ok(report)` is not proof all requested effects influenced valuation.

## Parallel replacement selection and failure boundary

The engine recognizes leading contiguous runs of ParCDS/inflation operations with distinct curve IDs. On native targets, a run with at least two ParCDS operations generates all effects through `try_map_ordered` before committing any run member; inflation-only and single-ParCDS runs stay serial. WASM always stays serial. Repeated IDs terminate the independent run. The first error follows operation order.

**Confirmed static discrepancy:** if a later operation in such a native batch fails generation, preceding run members have not been committed. In serial/WASM execution those earlier operations can already be committed. Thus error identity/order can agree while mutable state after `Err` differs. This record does not assert a successful-result difference or executed reproduction. Prior operations outside the failed run remain applied on both paths.

The engine retains each hazard curve's calibration-source market until that hazard is rebuilt, preventing preceding rate shocks from erasing the source used for quote replay. A cached recalibration provider is installed by every constructor; its lifetime follows the engine and a caller can replace/share it. Source documentation permits sharing across an immutable batch. Accepted [ADR 0022](../../architecture/0022-portfolio-evaluation-state.md) requires request/applicable-market-state cache isolation: the engine lifetime boundary does not by itself establish that isolation. No stale-cache result is established by static inspection.

## Alternatives and evidence

Cloning all contexts and committing once gives transactional execution at memory/copy cost; uniform operation-sized commits preserve partial success but require matching native/serial failure semantics. Warning promotion policies would be a separate behavior change.

Owning source: [engine/mod.rs](../../../../finstack-quant/scenarios/src/engine/mod.rs), [effects.rs](../../../../finstack-quant/scenarios/src/engine/effects.rs). Inspected replacement-run unit tests classify distinct IDs and native/WASM selection. Inspected [ParCDS regression](../../../../finstack-quant/scenarios/tests/par_cds_bump.rs) `several_failing_par_cds_ops_report_the_first_op_in_order` checks first-error identity with all hazards missing; it does **not** exercise an earlier success followed by failure or compare surviving mutable state. Missing-model/inventory and warning paths are located in [engine tests](../../../../finstack-quant/scenarios/src/engine/tests.rs).

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
