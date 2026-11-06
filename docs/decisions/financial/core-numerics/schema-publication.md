# CORE-NUMERICS-26: Canonical schema publication and lossy LLM projection

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`schema/*` and `bin/gen_core_schemas.rs`: Rust-derived JSON Schema generation, assertion-equivalent external references, deterministic artifact synchronization, LLM-oriented projection and union-error expansion. Schema support is feature-gated; detailed diagnostics additionally require `jsonschema-validate`. This machinery describes/validates data shapes, not financial model correctness.

## Choice and rationale

Schemars derives canonical schemas from Rust/serde types. Shared definitions are externalized only after an expanded assertion view matches. The LLM projection intentionally relaxes some canonical constraints and requires separate canonical validation. Keeping these two surfaces distinct is necessary to avoid treating a generation aid as the authoritative contract.

## Algorithm

1. A SchemaArtifact generates Rust schema, sets versioned ID/dialect/title/description, applies its packager, collapses a single-object oneOf only when all sibling keys are annotations, and appends example values. Examples serialize actual values and round floating examples to 12 significant digits; this affects examples only.
2. Externalization removes annotations/IDs/defs for comparison, recursively expands local references including suffixes, marks recursive revisits symbolically and combines ref siblings using allOf. Only matching assertion trees may replace local refs with external URIs. Prune definitions unreachable from root references afterward.
3. Generator CLI requires exactly one of write/check/list. Validate normalized relative paths, non-overlapping owned schemas roots and unique IDs/paths; render pretty JSON plus newline in deterministic ordered maps. Check compares expected bytes/missing/extra generated files. Write updates changed files, deletes extra matching-suffix files under owned roots, and removes empty nested directories. No schema generation was run in this audit.
4. LLM projection defaults to 16 KiB per-definition inline budget and 240 description characters. Resolve external refs in sorted order; inline small schemas, replace oversized ones with typed descriptive handles, preserve unresolved refs, rewrite known refs and prune unreachable definitions. Oversized local definitions become handles too (only when budget>0).
5. Fold eligible string-const unions to enum, shorten Rustdoc descriptions while preserving selected standards citations, drop nonportable keywords including format/uniqueItems/default, describe omitted defaults, replace fixed tuples by a fixed-length homogeneous/union items schema, and make every object property required with additionalProperties=false. This tuple rewrite loses positional type constraints; handle substitution loses nested validation constraints.
6. Diagnostic expansion handles anyOf/oneOf failures to depth four, follows references, prefers branches whose const tags match, otherwise selects the failing branch with fewest failures, and prefixes nested instance pointers. If resolution/building fails, retain the original error. This is a heuristic explanation of canonical validation failure, not altered acceptance logic.

## Selection, controls and conventions

Canonical generation and LLM projection are explicit APIs. Output-root and profile budgets are caller controls; artifact registry chooses domain types/examples. `max_inline_bytes=0` skips local oversized substitution but makes resolved nonempty external documents exceed the external-inline budget; it is not a uniform unlimited-budget convention.

## Limitations, failure and fallback

Assertion comparison is structural after specified expansion, not a general JSON-Schema logical-equivalence solver. LLM projection is not assertion-preserving and may retain unresolved external refs or permissive handles. Description limits are soft to preserve whole leading sentences/citations. External definition-name collisions retain the first body via map insertion behavior, requiring registry discipline. Canonical Rust/schema/runtime validation equivalence still requires tests.

## Alternatives and tradeoffs

Publishing canonical schemas directly retains constraints but can exceed downstream generation-tool limits. Projection trades completeness for size/shape compatibility; it must not replace canonical validation. Strict registry path validation and deterministic drift checks are implemented; no remote schema retrieval or automatic semantic validation is added by this module.

## Evidence

- [Registry](../../../../finstack-quant/core/src/schema/registry.rs), [externalization](../../../../finstack-quant/core/src/schema/externalize.rs), [projection](../../../../finstack-quant/core/src/schema/llm.rs), [generator](../../../../finstack-quant/core/src/schema/generator.rs), [diagnostics](../../../../finstack-quant/core/src/schema/diagnostics.rs), [schema tests](../../../../finstack-quant/core/src/schema/tests.rs).
- `packaging_rejects_name_only_shape_substitution` covers assertion mismatch; `packaging_compares_recursive_typed_definition_graphs` covers recursive comparison; `projection_rewrites_tuples_and_drops_non_portable_keywords` covers intentional projection loss; `generator_renders_once_and_preserves_unowned_files` covers publication/drift boundaries. These are representation/transform invariants and do not prove every generated schema matches all domain constructors. JSON Schema dialect is explicitly draft 2020-12 in source; external standards conformance was not independently assessed here.

Successful schema validation does not approve a financial methodology.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
