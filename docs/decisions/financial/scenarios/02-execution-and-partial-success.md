# SCENARIOS-002: Execution phases, effect ordering and partial success

- Decision status: **Accepted 2026-11-15 — complete ordered execution, atomic publication, failure and replay-state package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent state-transition and error-order checks completed.** Production execution, rollback and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for ordered and atomic scenario execution

1. **Preflight active prerequisites and freeze hierarchy membership.**
   Validate the accepted scenario specification, required inventories and
   statement model, configured binding keys and applicable calendar inputs.
   Expand hierarchy membership once against the opening snapshot. Bind
   retained active targets against the candidate state at the point each
   action executes.
   [Accepted specification](01-specification-and-composition.md#accepted-decision-for-checked-scenario-specifications-and-ordered-composition) ·
   [Accepted hierarchy contract](../core-market/hierarchy-selection.md#accepted-decision-typed-selection-and-explicit-ordered-overlap).

2. **Run one ordered sequence of phases.** Execute the elected roll
   and its admitted carry calculation first. Apply market and instrument
   actions in expanded order. Each action sees every completed preceding
   effect in its phase. Apply the configured rate bindings against the
   final shocked market, then apply inline bindings and forecast actions
   in supplied relative order. An admitted roll-only request ends after
   its roll and carry.
   [Roll record 013](13-market-time-roll.md) · [Carry record 014](14-carry-aggregation.md).

3. **Use serial action order across hosts.** Retire cross-operation
   replacement batches. Preserve deterministic ordering within each
   action. Return the first typed failure in canonical phase and operation
   order, including active target binding, effect calculation and admission.
   Rust, Python and WASM use the same financial execution contract.
   [Host reproducibility](../../standards/0021-reproducibility-guarantees.md).

4. **Evaluate the candidate model once when the statement phase is active.**
   Use configured bindings as ordered request inputs. An inline binding
   assigns its rate for this run without editing the configured binding
   map. An empty binding map alone activates no statement work. Use the
   canonical evaluator with the elected snapshot or dated cutoff and
   applicable market inputs. Retain the checked result and its provenance
   alongside the candidate statement inputs.
   [Accepted statement evaluation](../statements-evaluation/01-graph-precedence-and-prepared-plans.md) ·
   [Binding record 012](12-statement-bindings.md).

5. **Publish the admitted scenario atomically.** Stage the market,
   effective date, affected inventory and statement model privately.
   Publish those states together with the checked success result only
   after every required phase passes. A failed application preserves
   the opening caller-visible state.
   [Accepted atomic market lifecycle](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).

6. **Distinguish mandatory failures, intentional no-effects and advisories.**
   Missing prerequisites, unsupported active effects, failed calculations
   and failed required evaluation abort publication. Report admitted
   no-effects and genuine advisories distinctly. Optional carry measurements
   may be incomplete only under record 014's admitted scope, with that
   status visible. Record 017 owns reporting and replay representation.
   [Carry record 014](14-carry-aggregation.md) · [Reporting record 017](17-reports-and-replay.md).

7. **Bind replay and caching to the actual execution state.** Keep the
   qualified original calibration source distinct from the current staged
   target. Advance the source binding only when an admitted replay supplies
   matching construction evidence. Require an explicit horizon-requote
   election. Scope financial caches to the request and registered immutable
   market state. Use the shared pricing executor for ordinary position
   valuation.
   [Accepted hazard replay](../calibration-framework/21-hazard-quote-replay.md) ·
   [Accepted horizon replay](../calibration-framework/22-hazard-deal-and-horizon.md) ·
   [Accepted cache contract](../calibration-framework/23-recalibration-cache.md#accepted-decision-for-exact-request-local-recalibration-caching) ·
   [Accepted portfolio execution](../../architecture/0046-portfolio-execution-operational-contract.md#decision-and-scope).

## Independent state-transition checks

- A `+10%` equity shock on `100` gives candidate `110`. A later
  required statement failure leaves the published price at `100`
  under the accepted transaction. The live missing-model route can
  reach that error after publishing the price at `110`.
- If the first action fails effect generation and a second action
  fails active target binding, the serial action order returns the
  first action's error. Whole-batch prebinding can return the second
  action's error first.

## Observed choice and rationale at the audit revision

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

The live engine now stages market and effective date privately through the
market phase. It still borrows the caller's inventory for instrument effects
and publishes market and date before the statement phase. Native replacement
runs prebind active targets across the run before generating its first effect.

The engine retains each hazard curve's calibration-source market until that hazard is rebuilt, preventing preceding rate shocks from erasing the source used for quote replay. A cached recalibration provider is installed by every constructor; its lifetime follows the engine and a caller can replace/share it. Source documentation permits sharing across an immutable batch. Accepted [ADR 0022](../../architecture/0022-portfolio-evaluation-state.md) requires request/applicable-market-state cache isolation: the engine lifetime boundary does not by itself establish that isolation. No stale-cache result is established by static inspection.

## Alternatives and evidence

Cloning all contexts and committing once gives transactional execution at memory/copy cost; uniform operation-sized commits preserve partial success but require matching native/serial failure semantics. Warning promotion policies would be a separate behavior change.

Owning source: [engine/mod.rs](../../../../finstack-quant/scenarios/src/engine/mod.rs), [effects.rs](../../../../finstack-quant/scenarios/src/engine/effects.rs). Inspected replacement-run unit tests classify distinct IDs and native/WASM selection. Inspected [ParCDS regression](../../../../finstack-quant/scenarios/tests/par_cds_bump.rs) `several_failing_par_cds_ops_report_the_first_op_in_order` checks first-error identity with all hazards missing; it does **not** exercise an earlier success followed by failure or compare surviving mutable state. Missing-model/inventory and warning paths are located in [engine tests](../../../../finstack-quant/scenarios/src/engine/tests.rs).

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios).
