# ARROW-002: Ordered schema, semantic roles and JSON metadata

**Decision status:** Accepted 2026-11-17 for ordered schema and advisory metadata.

**Implementation status:** The exporter implements the accepted schema and metadata contract. The crate README now describes the typed serialization boundary.

**Validation status:** The Arrow schema and metadata fixture and the core role spelling regression passed on 2026-11-17. Nested and omitted metadata and host-consumer fidelity remain to be qualified.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope and exact coverage](README.md).

## Accepted decision

1. Preserve producer row and column order, exact column names, declared types and nullability. The [core table contract](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics) governs name and shape admission.
2. Carry optional field roles under `finstack:role` using the core role spellings. Carry nonempty typed JSON metadata objects under `finstack:metadata` on the field or schema where they belong. Omit absent roles and empty maps. Treat both roles and metadata as advisory annotations. Serialize embedded strings as text.
3. Repeated exports of the same envelope preserve equivalent fields, values and annotations. JSON member order, Arrow metadata key order and IPC byte identity are outside the contract.

## Choice, domain and rationale

Each field retains the envelope column's name, declared type and nullability; optional semantic information travels separately in Arrow metadata. This supports identifiers, time-like axes, model measures and attributes without interpreting their economic meaning. The [crate README](../../../../finstack-quant/arrow-interchange/README.md) documents the two fixed metadata keys and omission of absent information. No metadata-based pricing, unit conversion or table reconstruction is selected.

Carrying annotations alongside the typed arrays lets consumers discover finstack hints without changing how they read the values. Metadata retention depends on the host consumer.

## Ordered construction and selection

1. Iterate `table.columns` in its `Vec` order and obtain each field/array pair from `column_to_arrow`. Push both to matching positions. Row values remain in each source vector's order. There is no sorting, key alignment, name trimming/case-folding, deduplication, pivot, aggregation or row-key inference.
2. Build a fresh field metadata `HashMap`. If `column.role` is present, insert `finstack:role` with `TableColumnRole::as_str()`: `dimension`, `index`, `measure` or `attribute`. Absent role produces no key. The typed enum prevents an unknown role string through this API, but neither constructor nor adapter checks compatibility between role and data type; a role is a hint.
3. If the column metadata `IndexMap<String, Value>` is nonempty, serialize the entire map with `serde_json::to_string` and store that JSON object text under the single key `finstack:metadata`. Do not flatten its members into Arrow metadata. An inner member named `finstack:role` cannot overwrite the outer role key. Empty maps omit the key; a nonempty map containing a null-valued member remains present.
4. Construct `Field::new(column.name.clone(), data_type, nullable).with_metadata(metadata)`. Names, including empty strings and case distinctions, are copied exactly. Unique-name admission is handled as described in [ARROW-003](03-envelope-boundary-and-batch-assembly.md); it is not enforced in this step.
5. Independently serialize nonempty `table.metadata` under `finstack:metadata` in a schema metadata `HashMap`, then build `Schema::new_with_metadata(fields, schema_metadata)`. Field-level and table-level JSON are distinct scopes; no merge, inheritance, override or conflict resolution exists between them.

## Ordering, errors and repeated export

Source `IndexMap` iteration determines the serialized top-level JSON member order, as the fixture's exact strings demonstrate. No recursive canonicalization or key sort is called. Nested JSON objects retain whatever representation they already have; this audit does not assign them an insertion-order guarantee. The surrounding Arrow metadata maps are `HashMap`s, so their key iteration order is not an export contract. Preserving table column order therefore does not imply deterministic serialized IPC bytes or a content-hash canonical form.

The function borrows the envelope and builds fresh arrays/schema; it does not mutate, cache, consume or normalize the input. Repeating export of an unchanged envelope selects the same field names/types/values and annotations. This is semantic repeatability, not a proven byte-level or NaN-equality idempotence property. There is no reverse conversion in this crate.

Serialization failure is mapped to `Error::Validation` with either `column '<name>': metadata serialization failed: …` or `table metadata serialization failed: …`. The adapter serializes typed `Value` maps. A string containing malformed JSON text is serialized as a JSON string. No reachable failure fixture for serialization of the actual metadata type was identified.

## Alternatives and tradeoffs

Flattening annotations into individual Arrow metadata entries would make common hints easier to discover but would change namespacing and typed JSON-value preservation. A versioned metadata schema could validate units and provenance but would require an explicit contract and producer updates. Canonical JSON/ordered metadata would be appropriate for content-addressed bytes, but neither the exporter nor the existing tests establish that requirement. These are alternatives, not approved changes.

No currency, monetary scale, date unit, sign convention or metadata schema is enforced. Consumers must read metadata deliberately and must not infer financial correctness merely from the `measure` role. Empty-versus-absent metadata cannot be distinguished after export because both omit the key.

## Evidence and limitations

- **Implementation:** [metadata constants and export construction](../../../../finstack-quant/arrow-interchange/src/lib.rs); [core `TableColumnRole::as_str`, constructors and `IndexMap` types](../../../../finstack-quant/core/src/table.rs). The complete ordered loop and schema construction were inspected.
- **Local tests:** `to_record_batch_maps_types_nullability_and_metadata` passed. It asserts all field names/types/nullability, `dimension` and `index` role values, and exact JSON text for one column map and one table map. Core `column_role_as_str_matches_serde_rename` also passed and checks all four role spellings against serde.
- **Host tests:** `test_field_metadata_carries_roles` checks one `dimension` role through pyarrow; `test_arrow_table_is_reusable_across_consumers` checks repeated consumption of one finite result. [Test source](../../../../finstack-quant-py/tests/test_arrow_interchange.py). Neither covers arbitrary nested metadata, absent keys, all roles through every host, or canonical byte identity.
- **Step verdict:** Ordered schema and metadata emission match the accepted contract. Host preservation of annotations remains to be qualified.

## Follow-up tracking

Metadata qualification is tracked in the
[Arrow-interchange follow-ups](../FOLLOW_UPS.md#arrow-interchange).
