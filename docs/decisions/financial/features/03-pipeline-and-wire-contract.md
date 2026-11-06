# FEATURES-003: Sequential panel pipelines and typed/JSON result contracts

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope, selection and algorithm

`transform_panel_json` deserializes `PanelTransformSpec`, calls `transform_panel`, then serializes `PanelTransformResult`. Only scalar time-series and cross-sectional families are pipeline variants; grouped, pairwise and regression helpers remain separate APIs.

1. Validate all operation names before evaluating any operation: reject whitespace-only names, exact reserved `values`, and exact duplicates. Names are case-sensitive and are not trimmed or normalized for storage/lookup; a nonblank padded name is distinct.
2. Execute operations in request order. Missing `input` reads the previous output, or raw `values` for the first operation. Explicit `values` selects raw input; another name must refer to an already evaluated output. Forward/self/unknown references fail.
3. Require entity and order only for the time-series operation being executed, or time_key for cross-sectional execution. Delegate length, parameter, domain and final numerical checks to that transform. Later errors abort the call, so no partial pipeline is exposed.
4. Append a `{name, values}` entry for each result. Return `{"columns":[...]}` in operation order, with null for missing entries and finite numbers for computed values. `get_column` uses exact, first-match lookup and returns None when absent.

The module-level source prose says the JSON entry point returns an object mapping operation names to columns; actual code, result schema and tests use the ordered `columns` array. This is an observed contract-documentation discrepancy, not a proposed wire change.

## Construction, serde and validation boundaries

The four structs/enums have public fields and serde derives, with unknown fields rejected for specs/operations/results/columns. Deserialization validates types and enum tags, but not equal lengths, valid operation names, dependency order or operation-specific parameter domains. The `params` schema is an arbitrary JSON value; runtime validation is stronger. There is no checked spec/result constructor. An empty pipeline returns an empty columns array without checking unused key lengths or input finiteness. Unused keys are similarly unchecked in nonempty pipelines.

A manually constructed or deserialized `PanelTransformResult` may have duplicate names, unequal column lengths or Rust nonfinite numeric fields; `get_column` does not revalidate it. Guaranteed output invariants apply to the transform entry points. JSON nonfinite literals are not valid input tokens; raw serde serialization outside these entry points is not a feature-layer numerical validator. Keys, original input values and operation metadata are absent from the result, so alignment/provenance must travel separately.

## Rationale, alternatives and evidence

The documented purpose is explicit sequential composition with branching to previous named columns and stable operation order. Proposed rationale is reproducible cleaning pipelines; an independent-raw-input default or graph evaluator would yield different results and is not implemented. A validated immutable result type and operation-specific parameter schemas would enforce more at construction, with a larger API surface; neither is an adopted requirement.

- Owning [pipeline and structs](../../../../finstack-quant/features/src/panel.rs): `transform_panel`, `resolve_input`, `validate_operation_names`, `PanelTransformResult::get_column`.
- [Schema registry](../../../../finstack-quant/features/src/schema.rs), [generator](../../../../finstack-quant/features/src/bin/gen_schemas.rs) and [exports](../../../../finstack-quant/features/src/lib.rs) are support, not additional validation engines. Five roots describe selectors, input and output; `json-schema` gates the registry. The generator delegates to core and was not run.
- Inspected [spec schema](../../../../finstack-quant/features/schemas/features/1/panel_transform_spec.schema.json) and [result schema](../../../../finstack-quant/features/schemas/features/1/panel_transform_result.schema.json) corroborate shape only.
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs): ordered typed/JSON results, sequential return-to-zscore chaining, explicit raw input, duplicate rejection before evaluation, unknown input and reserved name. These are executable contract examples, not validation of every invalid constructed object.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
