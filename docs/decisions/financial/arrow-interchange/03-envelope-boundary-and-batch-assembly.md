# ARROW-003: Envelope admission, batch assembly, resource limits and export direction

**Decision status:** Accepted 2026-11-17 for checked single-batch export and UTF-8 size validation.

**Implementation status:** Single-batch assembly and error mapping are implemented. Core validation at Arrow entry and UTF-8 size preflight remain pending.

**Validation status:** Both Arrow batch unit tests and all seven core table unit tests passed on 2026-11-17. The UTF-8 overflow path was verified in pinned dependency source. Arrow entry-point rejection tests, zero-row typed batches, and UTF-8 boundary tests remain pending.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope and exact coverage](README.md).

## Accepted decision

1. Validate the envelope through the [core table contract](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics) at `to_record_batch` before allocating Arrow buffers. Reject blank or duplicate column names, inconsistent lengths or row counts, and positive row counts with zero columns. Preserve valid 0×0 tables and zero-row tables with typed columns.
2. Keep Arrow `Utf8` for both string variants. Before constructing arrays, use checked byte counting to enforce `i32::MAX` total UTF-8 payload bytes per column. Count present strings, with empty strings and nulls contributing zero bytes. Return a column-specific `Error::Validation` when the limit would be exceeded. Ordinary allocator exhaustion remains outside the returned-error contract.
3. Return one fully materialized `RecordBatch` on success. Supply the validated row count explicitly and retain Arrow's final consistency checks and the existing Python `ValueError` mapping for validation failures. The Rust adapter exports from `TableEnvelope`; Python's separate `ArrowTable` API owns its IPC features.

## Choice, domain and rationale

`to_record_batch(&TableEnvelope)` currently exports one fully materialized batch and documents a validated-input precondition. Core construction, serialization and deserialization call `TableEnvelope::validate`. Public Rust fields can still produce invalid in-memory states. The exporter currently relies on typed array construction followed by Arrow consistency checks.

Passing the explicit row count lets Arrow construct a 0×0 batch. Owned arrays give the resulting batch an independent lifetime. Core owns table-name and shape invariants. The Arrow boundary owns representability in its `Utf8` arrays.

## Ordered computation and actual controls

1. **Upstream checked path:** `TableEnvelope::new` derives row count from the first column, or zero for no columns. Core validation checks matching lengths, unique exact nonblank names, and zero rows for tables with no columns. Custom serde invokes those checks at serialization and restoration. A Rust struct literal or public-field mutation can bypass them.
2. **Materialize columns:** Allocate vectors for fields and arrays, then convert columns in declaration order using [ARROW-001](01-typed-values-and-null-semantics.md). The exporter copies payloads and serializes annotations before validating the overall batch. It therefore may allocate all column buffers before rejecting an inconsistent declared row count.
3. **Create schema:** Assemble ordered fields and metadata using [ARROW-002](02-schema-order-and-metadata.md). The pinned `arrow-schema` constructor stores the supplied fields; it does not reject duplicate names.
4. **Fix row count:** Set `RecordBatchOptions::new().with_row_count(Some(table.row_count))`. A no-column, zero-row envelope yields an empty batch; a raw no-column envelope declaring a positive row count also retains that count. Zero-row tables with typed columns retain their schema. Nonempty tables must have each array length equal to the supplied row count.
5. **Validate batch:** Pinned Arrow 57.3.2 `RecordBatch::try_new_impl` checks field/array counts, presence of a row-count source, nonnullable fields versus null counts, column lengths versus selected row count, and array/field type consistency. The adapter generates matching types and nullability together. Arrow catches raw length and row-count mismatches, while an equal-length raw envelope with duplicate or blank names can pass this path.
6. **Map errors and return:** A metadata serialization error stops at its column or table stage. A returned Arrow error becomes `Error::Validation("arrow record batch construction: …")`. No partial batch, truncation, deduplication, null filling, type widening or fallback result is returned. Python `from_envelope` passes this error through `core_to_py`, whose validation mapping is `ValueError`; panics and process allocation failures do not traverse that mapping.

The function has no runtime options besides the envelope. Row count is a `usize`; no adapter-specific row, column or total-memory cap exists. The one-batch method does not stream or chunk input.

## Resource and failure boundary

Both string arms select `StringArray`/`Utf8` with signed 32-bit offsets. Pinned Arrow 57.3.2 checks accumulated byte length in `GenericByteArray::from_iter_values` with `expect("offset overflow")`. Nullable collection checks the offset in `GenericByteBuilder::next_offset`. A column above the byte limit can currently panic before `RecordBatch::try_new_with_options`. The accepted preflight will return a validation error at that boundary. The overflow path was established by source inspection.

The input's checked constructor validates names and lengths, not this string-size boundary. Consequently even a constructor-produced envelope is not guaranteed to export through a returned `Result` in all resource states. Rust `deny(clippy::panic)` on this crate does not remove dependency panics. The nonnullable iterator is a mapped vector iterator with an upper size bound, so Arrow's separate unsized-iterator panic is not an additional reachable concern for this call site.

The export copies numeric vectors and string contents; subsequent `Arc` ownership does not make envelope-to-Arrow conversion zero-copy. The inspected Python C-stream wrapper clones a completed batch and shares its buffers for consumers; that later handoff has a different ownership boundary. No full consumer-memory benchmark or end-to-end zero-copy assertion is made here.

## Supported direction, alternatives and tradeoffs

The Rust crate exports only. It has no Arrow-to-envelope conversion, import casting, IPC encoding/decoding or schema negotiation. Python `ArrowTable` has separate IPC methods and C-stream behavior in its own binding file; those do not expand this crate's API. There is no supported inverse round trip on which to claim exporter idempotence.

Reusing core validation aligns checked construction, wire restoration and Arrow admission. The byte limit preserves the agreed `Utf8` schema and single-batch API. Larger offsets would change the field type, and chunking would change the return contract.

## Evidence and limitations

- **Owned source:** [complete adapter](../../../../finstack-quant/arrow-interchange/src/lib.rs), including the post-test README-doctest item at lines 303–311, which adds no production algorithm.
- **Admission evidence:** [core `TableEnvelope` fields, checked serde and constructor](../../../../finstack-quant/core/src/table.rs). Core tests cover constructor rejection of inconsistent lengths and duplicate names, and rejection of malformed restored or mutated state through serde.
- **Dependency evidence:** [Cargo.lock](../../../../Cargo.lock) pins `arrow-array`/`arrow-schema` 57.3.2. The inspected Arrow sources include `arrow-array/src/record_batch.rs:302–368`, `array/byte_array.rs:217–242`, and `builder/generic_bytes_builder.rs:92–119` for consistency and offset checks.
- **Adapter test evidence:** Both tests passed. `to_record_batch_preserves_row_count_for_zero_column_table` covers constructor-produced 0×0, and the nine-column fixture covers a valid two-row batch. Add coverage for raw shape and name errors, zero-row typed columns, and both sides of the UTF-8 size limit with bounded test inputs.
- **Host boundary:** [Python wrapper](../../../../finstack-quant-py/src/bindings/core/table.rs) and [error mapping](../../../../finstack-quant-py/src/errors.rs); [Python interop tests](../../../../finstack-quant-py/tests/test_arrow_interchange.py) exercise ordinary statement/portfolio outputs and reuse. IPC methods were located to distinguish scope, not audited as part of this owned algorithm.
- **Step verdict:** Existing assembly matches the accepted single-batch path. Core validation and UTF-8 preflight still need implementation and regression coverage.

## Follow-up tracking

Implementation reconciliation and qualification work are
tracked in the [Arrow-interchange follow-ups](../FOLLOW_UPS.md#arrow-interchange).
