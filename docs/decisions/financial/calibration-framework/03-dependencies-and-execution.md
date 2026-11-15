# CALIBRATION-FRAMEWORK-003: Dependency ordering, preflight and installation

- Decision status: **Accepted 2026-11-15 — complete typed dependencies, preflight, ordered execution and atomic installation package**.
- Implementation status: **Observed for the stated source scope.** Reconciliation to the accepted contract is pending.
- Validation status: **Independent typed-dependency, stale-read, failure-order and atomic-installation examples checked.** Production execution, calibration and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for calibration dependencies, preflight and ordered execution

1. **Bind reads and writes to typed market keys and roles.** Each read
   identifies the storage namespace, exact ID, required kind and
   financial role. Each output identifies its actual storage key,
   including composite updates. A same-name scalar cannot serve a
   discount read. Curve kinds sharing a storage slot conflict.
   Use one Rust dependency contract for dry run, preflight,
   scheduling, execution and replay. Keep target trial artifacts
   private.
   [Accepted typed context lifecycle](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).

2. **Resolve the actual target inputs and record consumed quotes.**
   XCCY binds its applicable discount and projection roles and
   elected FX spot authority. Student-t binds the exact tranche
   quote, index, discount and active credit inputs. Remove sole-discount
   inference. Equity targets bind explicit spot and carry, including
   the exact dividend schedule where used. Remove ticker-derived IDs
   and presence-based precedence. Hull–White binds exact evaluators
   and any expressly shared evaluator. Report exact quote IDs
   consumed. Admit qualified mixed families and expressly selected
   subsets and reject silent filtering.

3. **Preserve authored order and explicit replacement.** Read initial
   state or the latest admitted earlier output. Reject output
   collisions unless authorized through an ordered same-kind
   replacement. Validate dependent composites on the resulting
   market. Treat base correlation and its credit-index update as
   one atomic change. Preserve consumed input versions and replacement
   provenance. Reject unresolved forward dependencies in authored order.

4. **Run target-owned preflight before each solve.** Validate the
   actual preceding market, selected quote set, required roles,
   effective controls and financial context. Include Hull–White,
   cap/floor Hull–White and parametric steps. Use the applicable
   target rules for quote counts, currencies, entity, recovery,
   clock, instrument eligibility and parameter domains. A static
   dry run can report unresolved requirements. Execution resolves
   them against the actual preceding market before solving.
   [Accepted numerical and context policy](02-controls-and-market-context.md#accepted-decision-for-calibration-controls-freshness-and-initial-market).

5. **Batch only where authored execution is preserved.** Batch
   contiguous steps with known typed read and write sets. Workers
   use one immutable batch-start market. End a batch before a later
   step reads an earlier pending output or writes an overlapping
   slot, including composite effects. An earlier reader of a later
   replacement uses the earlier value if available. Resolve uncertain
   dependencies before batching. Collect by authored index and select
   the earliest failed step across preflight, solve, fit, validation,
   report and installation.
   [Rayon ordered search semantics](https://docs.rs/rayon/latest/rayon/iter/trait.ParallelIterator.html#method.find_first).

6. **Gate complete outputs and publish a complete admitted market.**
   Use the same gate in serial, parallel and replay paths. Check
   solver outcome, required financial residual and post-fit checks,
   report consistency and the complete typed output set before it
   can serve later steps. Stage multi-output effects together.
   Publish a calibrated market after all required steps pass.
   Exploratory failure may retain bounded diagnostic evidence.
   Deliberate zero-step context construction reports no fit and
   zero work. [Record 014](14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics)
   owns detailed fit acceptance. The replay owner retains
   source-reproduction requirements.
   [Accepted replay qualification](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

7. **Report the actual stage, typed cause and work.** Preserve step
   identity and outcome. A failed post-fit check is a validation
   failure even when the residual passes. Reserve warnings for
   advisory checks. Preserve authored report and trace order,
   collision-free step and quote identities and checked aggregate
   work. Share outcomes across Rust, Python, WASM and persisted data.

## Independent calculation checks

- A curve factor 0.95 values 100 at 95. With an authorized prior
  step replacing the factor by 0.97, a subsequent read must value
  the payment at 97 in both execution modes. A same-name scalar
  supplies no discount-curve dependency.
- When step A has a rejected fit and later step B raises a target
  error, the first failure is A. The observed batch path can return
  B before applying A's fit gate.
- An attempted correlation change from 0.20 to 0.30 paired with an
  index update referring to a missing hazard fails atomically.
  The previously admitted curve and index remain in force.

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
- Test evidence: [src/api/tests/engine_dependency_review.rs](../../../../finstack-quant/calibration/src/api/tests/engine_dependency_review.rs) — same-kernel serial/parallel equivalence, stale prior writer, XCCY and equity carry reads; inspected or located, not executed.
- Test evidence: [src/step_runtime.rs](../../../../finstack-quant/calibration/src/step_runtime.rs) — passing runtime validation preserves a target validation failure; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
