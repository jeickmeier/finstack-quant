# ARROW-002: Ordered schema, semantic roles and JSON metadata

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed in `finstack-quant/arrow-interchange/src/lib.rs:54–67,126–150,176–192`. No parsing or semantic validation of metadata is implemented, despite the crate README's malformed-JSON error statement.

**Validation status:** Implementation and named test bodies inspected, not executed. No canonical-byte, IPC, inverse-conversion or cross-language metadata conformance certification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope and exact coverage](README.md).

## Choice, domain and rationale

Each field retains the envelope column's name, declared type and nullability; optional semantic information travels separately in Arrow metadata. This supports identifiers, time-like axes, model measures and attributes without interpreting their economic meaning. The [crate README](../../../../finstack-quant/arrow-interchange/README.md) documents the two fixed metadata keys and omission of absent information. No metadata-based pricing, unit conversion or table reconstruction is selected.

Separating annotations from typed payload permits consumers to use ordinary Arrow arrays while retaining finstack hints. This is the implementation's observable tradeoff, not an adopted guarantee that every host honors those hints.

## Ordered construction and selection

1. Iterate `table.columns` in its `Vec` order and obtain each field/array pair from `column_to_arrow`. Push both to matching positions. Row values remain in each source vector's order. There is no sorting, key alignment, name trimming/case-folding, deduplication, pivot, aggregation or row-key inference.
2. Build a fresh field metadata `HashMap`. If `column.role` is present, insert `finstack:role` with `TableColumnRole::as_str()`: `dimension`, `index`, `measure` or `attribute`. Absent role produces no key. The typed enum prevents an unknown role string through this API, but neither constructor nor adapter checks compatibility between role and data type; a role is a hint.
3. If the column metadata `IndexMap<String, Value>` is nonempty, serialize the entire map with `serde_json::to_string` and store that JSON object text under the single key `finstack:metadata`. Do not flatten its members into Arrow metadata. An inner member named `finstack:role` cannot overwrite the outer role key. Empty maps omit the key; a nonempty map containing a null-valued member remains present.
4. Construct `Field::new(column.name.clone(), data_type, nullable).with_metadata(metadata)`. Names, including empty strings and case distinctions, are copied exactly. Unique-name admission is handled as described in [ARROW-003](03-envelope-boundary-and-batch-assembly.md); it is not enforced in this step.
5. Independently serialize nonempty `table.metadata` under `finstack:metadata` in a schema metadata `HashMap`, then build `Schema::new_with_metadata(fields, schema_metadata)`. Field-level and table-level JSON are distinct scopes; no merge, inheritance, override or conflict resolution exists between them.

## Ordering, errors and repeated export

Source `IndexMap` iteration determines the serialized top-level JSON member order, as the fixture's exact strings demonstrate. No recursive canonicalization or key sort is called. Nested JSON objects retain whatever representation they already have; this audit does not assign them an insertion-order guarantee. The surrounding Arrow metadata maps are `HashMap`s, so their key iteration order is not an export contract. Preserving table column order therefore does not imply deterministic serialized IPC bytes or a content-hash canonical form.

The function borrows the envelope and builds fresh arrays/schema; it does not mutate, cache, consume or normalize the input. Repeating export of an unchanged envelope selects the same field names/types/values and annotations. This is semantic repeatability, not a proven byte-level or NaN-equality idempotence property. There is no reverse conversion in this crate.

Serialization failure is mapped to `Error::Validation` with either `column '<name>': metadata serialization failed: …` or `table metadata serialization failed: …`. The README statement that malformed metadata JSON produces that error is inaccurate for this API: metadata is already a typed `Value` map, and the adapter serializes it without parsing strings as JSON. A string containing malformed JSON text is a valid JSON string value and is not interpreted as an embedded document. No reachable failure fixture for serialization of the actual metadata type was identified; the mapped branch is not evidence of a parser or schema validator.

## Alternatives and tradeoffs

Flattening annotations into individual Arrow metadata entries would make common hints easier to discover but would change namespacing and typed JSON-value preservation. A versioned metadata schema could validate units and provenance but would require an explicit contract and producer updates. Canonical JSON/ordered metadata would be appropriate for content-addressed bytes, but neither the exporter nor the existing tests establish that requirement. These are alternatives, not approved changes.

No currency, monetary scale, date unit, sign convention or metadata schema is enforced. Consumers must read metadata deliberately and must not infer financial correctness merely from the `measure` role. Empty-versus-absent metadata cannot be distinguished after export because both omit the key.

## Evidence and limitations

- **Implementation:** [metadata constants and export construction](../../../../finstack-quant/arrow-interchange/src/lib.rs); [core `TableColumnRole::as_str`, constructors and `IndexMap` types](../../../../finstack-quant/core/src/table.rs). The complete ordered loop and schema construction were inspected.
- **Local tests:** `to_record_batch_maps_types_nullability_and_metadata` asserts all field names/types/nullability, `dimension` and `index` role values, and exact JSON text for one column map and one table map. Core `column_role_as_str_matches_serde_rename` checks all four role spellings against serde. These tests were inspected, not run.
- **Host tests:** `test_field_metadata_carries_roles` checks one `dimension` role through pyarrow; `test_arrow_table_is_reusable_across_consumers` checks repeated consumption of one finite result. [Test source](../../../../finstack-quant-py/tests/test_arrow_interchange.py). Neither covers arbitrary nested metadata, absent keys, all roles through every host, or canonical byte identity.
- **Step verdict:** Ordered schema and metadata emission agree with the implementation's documented map. The malformed-JSON error statement does not match the typed serialization boundary. Metadata's economic meaning and preservation by all supported consumers remain unqualified.

## Follow-up tracking

Implementation reconciliation, pending choices, and qualification work are
tracked in the [Arrow-interchange follow-ups](../FOLLOW_UPS.md#arrow-interchange).
