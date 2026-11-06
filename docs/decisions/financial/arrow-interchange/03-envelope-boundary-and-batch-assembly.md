# ARROW-003: Envelope admission, batch assembly, resource limits and export direction

**Decision status:** Draft — observed implementation; adoption pending for the admission/resource policies described below.

**Implementation status:** Observed in `finstack-quant/arrow-interchange/src/lib.rs:69–72,75–124,176–196`, with core public/serde construction and the pinned Arrow constructor inspected as boundary evidence. Checked construction and raw-envelope export do not enforce identical invariants.

**Validation status:** Implementation, pinned dependency code and named tests inspected, not executed. No large allocation, panic reproduction, invalid-envelope execution or host IPC test was run.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope and exact coverage](README.md).

## Choice, domain and rationale

`to_record_batch(&TableEnvelope)` exports one fully materialized batch. Its argument documentation says the envelope is validated. The core constructor is one way to satisfy that precondition, but public fields and derived `Deserialize` allow other states. The exporter does not reconstruct the envelope or call a validator; it relies on typed array construction followed by Arrow consistency checks.

Passing the explicit row count is documented as the way to retain column-less table shape. Ownership of arrays gives the resulting batch an independent lifetime. Neither this rationale nor the use of a checked Arrow constructor establishes complete input admission or a fallible resource contract.

## Ordered computation and actual controls

1. **Upstream checked path:** `TableEnvelope::new` sets row count to the first column length, or zero for no columns; it checks every length and rejects duplicate exact column names. It does not require nonempty names, finite floating values or role/data compatibility. A raw literal, public-field mutation or derived serde deserialization does not invoke those checks.
2. **Materialize columns:** Allocate vectors for fields and arrays, then convert columns in declaration order using [ARROW-001](01-typed-values-and-null-semantics.md). The exporter copies payloads and serializes annotations before validating the overall batch. It therefore may allocate all column buffers before rejecting an inconsistent declared row count.
3. **Create schema:** Assemble ordered fields and metadata using [ARROW-002](02-schema-order-and-metadata.md). The pinned `arrow-schema` constructor stores the supplied fields; it does not reject duplicate names.
4. **Fix row count:** Set `RecordBatchOptions::new().with_row_count(Some(table.row_count))`. A no-column, zero-row envelope yields an empty batch; a raw no-column envelope declaring a positive row count also retains that count. Zero-row tables with typed columns retain their schema. Nonempty tables must have each array length equal to the supplied row count.
5. **Validate batch:** Pinned Arrow 57.3.1 `RecordBatch::try_new_impl` checks field/array counts, presence of a row-count source, nonnullable fields versus null counts, column lengths versus selected row count, and array/field type consistency. It does not check unique field names. The adapter generates matching field and array types/nullability together, so raw-envelope length/row-count mismatches are the main accessible consistency errors here. A raw equal-length duplicate-name envelope passes this static check path, unlike `TableEnvelope::new`.
6. **Map errors and return:** A metadata serialization error stops at its column or table stage. A returned Arrow error becomes `Error::Validation("arrow record batch construction: …")`. No partial batch, truncation, deduplication, null filling, type widening or fallback result is returned. Python `from_envelope` passes this error through `core_to_py`, whose validation mapping is `ValueError`; panics and process allocation failures do not traverse that mapping.

The function has no runtime options besides the envelope. Row count is a `usize`; no adapter-specific row, column or total-memory cap exists. The one-batch method does not stream or chunk input.

## Resource and failure boundary

Both string arms select `StringArray`/`Utf8` with signed 32-bit offsets. Inspection of pinned Arrow 57.3.1 shows `GenericByteArray::from_iter_values` checks the accumulated byte length with an `expect("offset overflow")`; nullable collection uses `GenericByteBuilder::next_offset`, which likewise expects a representable offset. A column whose aggregate present UTF-8 bytes exceed `i32::MAX` can therefore panic before `RecordBatch::try_new_with_options`. The adapter does not preflight this limit or catch the panic. This is a confirmed static failure path, not a reproduced multi-gigabyte allocation. Ordinary allocation failure is also outside the returned validation-error contract.

The input's checked constructor validates names and lengths, not this string-size boundary. Consequently even a constructor-produced envelope is not guaranteed to export through a returned `Result` in all resource states. Rust `deny(clippy::panic)` on this crate does not remove dependency panics. The nonnullable iterator is a mapped vector iterator with an upper size bound, so Arrow's separate unsized-iterator panic is not an additional reachable concern for this call site.

The export copies numeric vectors and string contents; subsequent `Arc` ownership does not make envelope-to-Arrow conversion zero-copy. The inspected Python C-stream wrapper clones a completed batch and shares its buffers for consumers; that later handoff has a different ownership boundary. No full consumer-memory benchmark or end-to-end zero-copy assertion is made here.

## Supported direction, alternatives and tradeoffs

The Rust crate exports only. It has no Arrow-to-envelope conversion, import casting, IPC encoding/decoding or schema negotiation. Python `ArrowTable` has separate IPC methods and C-stream behavior in its own binding file; those do not expand this crate's API. There is no supported inverse round trip on which to claim exporter idempotence.

Enforcing core invariants at export would make raw and checked admission consistent; restricting construction through a validated core representation would place that guarantee earlier. Either requires a deliberate ownership choice, not a compatibility fallback. For large strings, an explicit checked size limit would retain today's schema and yield a clear error; a larger-offset canonical representation or producer chunking would support larger payloads but change schema/batch assumptions. No alternative is selected by this draft.

## Evidence and limitations

- **Owned source:** [complete adapter](../../../../finstack-quant/arrow-interchange/src/lib.rs), including the post-test README-doctest item at lines 303–311, which adds no production algorithm.
- **Admission evidence:** [core `TableEnvelope` fields, serde derive and constructor](../../../../finstack-quant/core/src/table.rs). Its `table_validates_column_lengths` and `table_rejects_duplicate_column_names` tests inspect the checked constructor only.
- **Dependency evidence:** [Cargo.lock](../../../../Cargo.lock) pins `arrow-array`/`arrow-schema` 57.3.1. Exact local sources inspected: `arrow-array/src/record_batch.rs:310–371`, `arrow-schema/src/schema.rs:236–241`, `arrow-array/src/array/byte_array.rs:217–249,606–615`, and `arrow-array/src/builder/generic_bytes_builder.rs:85–123`. These establish constructor checks and offset failure paths as source evidence, not independent conformance testing or runtime qualification.
- **Adapter test evidence:** `to_record_batch_preserves_row_count_for_zero_column_table` tests only constructor-produced 0×0. The nine-column fixture tests a valid two-row batch. Neither tests raw mismatched lengths, duplicate names, nonzero zero-column row count, zero-row typed columns or resource limits.
- **Host boundary:** [Python wrapper](../../../../finstack-quant-py/src/bindings/core/table.rs) and [error mapping](../../../../finstack-quant-py/src/errors.rs); [Python interop tests](../../../../finstack-quant-py/tests/test_arrow_interchange.py) exercise ordinary statement/portfolio outputs and reuse. IPC methods were located to distinguish scope, not audited as part of this owned algorithm.
- **Step verdict:** Explicit row-count and consistency checks match the observed batch assembly. Full core-envelope admission is not repeated, and UTF-8 construction has a panic boundary outside the mapped validation errors. Policy and test qualification remain open.

## Follow-up tracking

Implementation reconciliation, pending choices, and qualification work are
tracked in the [Arrow-interchange follow-ups](../FOLLOW_UPS.md#arrow-interchange).
