# STATEMENTS-CONTRACTS-001: Model timeline and explicit-value invariants

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice README](README.md)).

## Choice and input domain

A financial model is an insertion-ordered map of named Value, Calculated or Mixed nodes over an explicit ordered list of half-open reporting periods. Model semantic validation establishes timeline and declared-value consistency before formula evaluation. It does not establish economic correctness or complete data coverage.

Periods must be nonempty as a list, each start must precede its end, identifiers must increase strictly, and successive date intervals must not overlap. Actual periods must form a prefix before forecast periods. These requirements prevent a later actual observation becoming a forecast's historical anchor. Date gaps are permitted; this validator does not require one frequency or reconstruct dates from each PeriodId.

## Algorithm and controls

1. Collect model period IDs. Reject any explicit observation outside that set. An availability-date entry must name both a model period and an explicit observation.
2. Reject explicit values on Calculated nodes, formulas on Value nodes, and forecasts on Calculated nodes. Mixed nodes may contain all three sources.
3. Infer each nonempty explicit series as either Scalar or Monetary with one exact currency. Mixed scalar/money series and mixed currencies error; no FX conversion occurs. Empty series provide no inference. A declared type that disagrees with the inferred type errors.
4. Validate every map key against the reserved-ID rules, and require the embedded node_id to equal its map key. Validate forecast parameter shapes even when explicit observations will take precedence.
5. Fill absent value_type fields from explicit observations, then repeatedly infer untyped formula nodes until no additional type resolves. Parse identical source text once per semantic-analysis invocation. Compile formulas and scalar-valued where clauses after inference.
6. Validate capital-structure restrictions and construct the dependency graph. A successfully constructed graph is checked for cycles. If graph construction reports an unknown identifier, log a warning and retain that error for later evaluation; validate_semantics itself succeeds on this branch.

The validation method mutates inferred types in the model. The public new constructor, add_node, direct field mutation and ordinary derived Serde deserialization do not automatically call this full semantic pipeline. ModelBuilder::build, from_json, from_value and from_slice_strict do.

## Limitations and alternatives

Direct Scalar(f64) construction accepts non-finite values, and this validation stage checks series type rather than finiteness. Missing observations and unresolved formula units can remain. An empty node map is permitted. Gaps, unusual PeriodId/date combinations and unknown references therefore are not proof of a usable evaluation.

A fully closed model contract could reject unknown references and unresolved dimensions at construction; that would prevent intentional incremental authoring. Conversely, preserving a partially described model needs a visible distinction between structurally accepted and evaluation-ready. These alternatives are not implemented here.

## Evidence

[Model implementation](../../../../finstack-quant/statements/src/types/model.rs), [value inference](../../../../finstack-quant/statements/src/types/value.rs), [model regressions](../../../../finstack-quant/statements/tests/finance_model_regressions.rs) and [model tests](../../../../finstack-quant/statements/tests/spec/model_spec_tests.rs) provide exact-input/type/timeline rejection evidence. Inline tests pin actual-prefix ordering and from_json validation; finance_model_regressions covers off-timeline observations, orphan availability dates, node-map identity and nonoverlapping intervals. These are contract regressions, not independent accounting validation.

The invalid/missing/undefined distinctions of [ADR 0009](../conventions/0009-missing-and-invalid-data.md) remain in force; they do not independently approve this statements-specific warning policy.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
