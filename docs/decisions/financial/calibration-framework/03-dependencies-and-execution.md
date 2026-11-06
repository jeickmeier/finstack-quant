# CALIBRATION-FRAMEWORK-003: Dependency ordering, preflight and installation

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

The plan is executed in caller order. A static graph checks availability and forms safe contiguous batches; it does not reorder steps or infer a new plan. The observed rationale is to preserve deterministic dependencies while allowing independent solves against one immutable market snapshot.

## Ordered algorithm and selection

1. Collect prior-object and nonquote datum IDs. Check unique step IDs, quote IDs across all quote kinds, duplicate nonquote IDs within each kind, quote-set existence/resolution, and validation of referenced quote payloads.
2. Start each node from `StepParams::io`. Add XCCY projection IDs from convention data; credit-index constituent hazards; Student-t selected tranche index and, when discount is omitted, all previously available discount IDs; and equity carry dependencies. Explicit dividend yield takes precedence over matching schedules, with fallback ID `<ticker>-DIVS`. SVI without discount requires the yield scalar even when a cash schedule exists.
3. Check every read against initial IDs plus earlier writes. Missing dependencies accumulate errors. Repeated writes are allowed in order. The graph compares strings, while typed retrieval and full numeric validity occur later.
4. Preflight requires rate observations for discount/forward; a discount dependency and entity/currency/recovery-consistent CDS observations for hazard; positive inflation notional/base CPI and matching existing index currency/lag/base value; required vol discount curves; positive shifted-lognormal displacement; SVI positive grids with at least five strikes; positive XCCY FX spot; and Student-t/base-correlation target dependencies. HW, cap-HW and parametric checks occur at execution. Mixed quote families are later extracted by their target.
5. Sequential execution solves, validates and gates each step before insertion. Parallel batching stops for a read of a prior batch write, repeated writes, or preflight that needs a pending output. Ordered parallel mapping solves against the same context. With strict bad-fit policy every report in a batch is checked before any batch result is applied; outputs are then installed in plan order.
6. Outputs include curves, multiple XCCY curves, surfaces, cubes, scalar model parameters and HW sigma series. Base correlation also updates credit-index state. The final market is serialized from this local context; errors do not return a partial market to the caller.

## Failure and reporting contract

Preflight, configuration, target, solver and context stages have structured error details. `fail_on_bad_fit=false` allows unsuccessful reports and their outputs; successful fit alone need not imply passed financial validation in Warn mode. Internal replay `execute_params_and_apply` applies to a cloned market before its caller enforces fit acceptance. Reports aggregate using their typed positive finite tolerance; missing tolerance fails even after local insertion. Empty plans produce a valid empty context and success by vacuous all-step checks; schema examples rely on this.

## Alternatives

A typed dependency graph could distinguish a scalar from a curve with the same ID; a topological scheduler could accept out-of-order plans but would alter this deliberate sequence. `SolverNotConverged`'s message always states max residual exceeds tolerance even when failure is validation or final parameter clamping; the structured report holds the real cause, which should remain accessible.

## Source and validation evidence

- Implementation: [api/validate.rs](../../../../finstack-quant/calibration/src/api/validate.rs) — dependency expansion, duplicate checks and ordered availability.
- Implementation: [api/schema.rs](../../../../finstack-quant/calibration/src/api/schema.rs) — declared read/write contracts.
- Implementation: [api/engine.rs](../../../../finstack-quant/calibration/src/api/engine.rs) — serial/parallel scheduling, strict gates and staged errors.
- Implementation: [validation/preflight.rs](../../../../finstack-quant/calibration/src/validation/preflight.rs) — type-specific financial preconditions.
- Implementation: [step_runtime.rs](../../../../finstack-quant/calibration/src/step_runtime.rs) — target dispatch, output installation and replay entry point.
- Implementation: [api/errors.rs](../../../../finstack-quant/calibration/src/api/errors.rs) — structured error wire mapping.
- Test evidence: [src/api/dependency_review_tests.rs](../../../../finstack-quant/calibration/src/api/dependency_review_tests.rs) — same-kernel serial/parallel equivalence, stale prior writer, XCCY and equity carry reads; inspected or located, not executed.
- Test evidence: [src/step_runtime.rs](../../../../finstack-quant/calibration/src/step_runtime.rs) — passing runtime validation preserves a target validation failure; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
