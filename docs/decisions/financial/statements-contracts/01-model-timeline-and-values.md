# STATEMENTS-CONTRACTS-001: Model timeline and explicit-value invariants

**Decision status:** Accepted for the full model timeline, explicit value and readiness contract (2026-11-15).

**Implementation status:** Reconciliation pending. The audited path can accept gaps, unresolved references and unguarded model construction.

**Validation status:** Existing source and regression evidence mapped. Independent model-admission qualification remains pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for model timelines, values and readiness

1. Require a canonical, nonempty `PeriodPlan` with one reporting cadence and calendar context, contiguous half-open periods and bounds that match their period identifiers. Actuals form a prefix followed by forecasts. All-actual and all-forecast plans are valid. [Reporting periods and fiscal context](../core-conventions/014-fiscal-periods-ltm.md#accepted-decision-reporting-periods-fiscal-context-and-ltm-windows).
2. Value nodes use explicit observations as their only value source. Calculated nodes require a formula as their value source. Mixed nodes may contain explicit observations, formulas and forecasts. Structurally validate every configured source, including sources that may be unused during evaluation. Validate node identifiers and require each node's embedded ID to match its map key. Explicit observations must name a period in the plan. An availability-date entry must match an explicit observation for that node and period.
3. Admit finite scalar observations or native `Money` in one currency per series. Resolve a supported unit by declaration or inference and require observations to agree with it. Represent missing cells by absence, including in a typed empty series. Each consuming operation determines the observation coverage it needs. [Missing and invalid data](../conventions/0009-missing-and-invalid-data.md).
4. Allow an incomplete draft during authoring. Use one complete guarded validation path to build or load a runnable model. Resolve references and supported types, require scalar `where` clauses, reject same-period dependency cycles and validate the capital structure. A runnable model needs at least one node or an evaluable capital structure.

For example, a quarterly USD series on a canonical Q1–Q3 plan may omit its Q2 observation. Omitting Q2 from the plan or referencing a misspelled node fails runnable-model validation.

## Audited input domain

A financial model is an insertion-ordered map of named Value, Calculated or Mixed nodes over an explicit ordered list of half-open reporting periods. Model semantic validation establishes timeline and declared-value consistency before formula evaluation. It does not establish economic correctness or complete data coverage.

Periods must be nonempty as a list, each start must precede its end, identifiers must increase strictly, and successive date intervals must not overlap. Actual periods must form a prefix before forecast periods. These requirements prevent a later actual observation becoming a forecast's historical anchor. Date gaps are permitted; this validator does not require one frequency or reconstruct dates from each PeriodId.

## Audited algorithm and controls

1. Collect model period IDs. Reject any explicit observation outside that set. An availability-date entry must name both a model period and an explicit observation.
2. Reject explicit values on Calculated nodes, formulas on Value nodes, and forecasts on Calculated nodes. Mixed nodes may contain all three sources.
3. Infer each nonempty explicit series as either Scalar or Monetary with one exact currency. Mixed scalar/money series and mixed currencies error; no FX conversion occurs. Empty series provide no inference. A declared type that disagrees with the inferred type errors.
4. Validate every map key against the reserved-ID rules, and require the embedded node_id to equal its map key. Validate forecast parameter shapes even when explicit observations will take precedence.
5. Fill absent value_type fields from explicit observations, then repeatedly infer untyped formula nodes until no additional type resolves. Parse identical source text once per semantic-analysis invocation. Compile formulas and scalar-valued where clauses after inference.
6. Validate capital-structure restrictions and construct the dependency graph. A successfully constructed graph is checked for cycles. If graph construction reports an unknown identifier, log a warning and retain that error for later evaluation; validate_semantics itself succeeds on this branch.

The validation method mutates inferred types in the model. The public new constructor, add_node, direct field mutation and ordinary derived Serde deserialization do not automatically call this full semantic pipeline. ModelBuilder::build, from_json, from_value and from_slice_strict do.

## Limitations and alternatives

Direct Scalar(f64) construction accepts non-finite values, and this validation stage checks series type rather than finiteness. Missing observations and unresolved formula units can remain. An empty node map is permitted. Gaps, unusual PeriodId/date combinations and unknown references therefore are not proof of a usable evaluation.

The accepted contract checks structural completeness at build or load after incremental authoring. Reconciliation of those entry points remains pending.

## Evidence

[Model implementation](../../../../finstack-quant/statements/src/types/model.rs), [value inference](../../../../finstack-quant/statements/src/types/value.rs), [model regressions](../../../../finstack-quant/statements/tests/finance_model_regressions.rs) and [model tests](../../../../finstack-quant/statements/tests/spec/model_spec_tests.rs) provide exact-input/type/timeline rejection evidence. Inline tests pin actual-prefix ordering and from_json validation; finance_model_regressions covers off-timeline observations, orphan availability dates, node-map identity and nonoverlapping intervals. These are contract regressions, not independent accounting validation.

The invalid/missing/undefined distinctions of [ADR 0009](../conventions/0009-missing-and-invalid-data.md) remain in force; they do not independently approve this statements-specific warning policy.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
