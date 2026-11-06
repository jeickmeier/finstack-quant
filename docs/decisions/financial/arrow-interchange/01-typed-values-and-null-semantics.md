# ARROW-001: Typed values, validity, precision and transport boundaries

**Decision status:** Draft — observed implementation; adoption pending. [ADR 0009](../conventions/0009-missing-and-invalid-data.md) and [ADR 0020](../conventions/0020-numerical-representation-and-rounding.md) retain their accepted scope.

**Implementation status:** Observed in `finstack-quant/arrow-interchange/src/lib.rs:75–124`, with the core table variants and their serde path inspected as boundary evidence. The direct Arrow and JSON-envelope paths have different nonfinite semantics.

**Validation status:** Implementation and the identified tests inspected, not executed. No numerical accuracy, exhaustive bit-fidelity, or consumer-conformance certification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope and exact coverage](README.md).

## Choice, domain and rationale

All supported result producers use the same enum-driven conversion. There is no instrument-specific dispatch or estimation. Numeric representation, units, currency, sign, period convention and missing-data meaning are supplied by the producer. The adapter neither calculates nor rescales a value, and role/metadata hints do not select another Arrow representation.

The [crate contract](../../../../finstack-quant/arrow-interchange/README.md) explicitly selects plain Arrow types and preservation of nonfinite floating values. Keeping the typed representation avoids a JSON stage in direct Arrow export. That is documented behavior; adopting it as a sufficient contract for every financial producer remains a separate decision.

## Ordered conversion and selection

1. Match `TableColumnData` exhaustively. `String` and `NullableString` produce `Utf8`; `Float64` variants produce `Float64`; `UInt32` variants produce `UInt32`; `Int64` variants produce `Int64`; `Boolean` produces `Boolean`. There is no nullable Boolean variant in the input enum, no unsupported-type fallback, and no sampling-based inference.
2. Traverse each string vector in source order. Nonnullable strings use `StringArray::from_iter_values`; nullable strings map `Option<String>` to `Option<&str>` and collect an array. Text is copied into Arrow storage. Empty text remains an empty string; `None` is distinct from empty text. Rust strings already have UTF-8 validity; the adapter performs no text normalization.
3. Clone each numeric/Boolean vector into the corresponding Arrow array constructor. Source `u32` and `i64` retain their integer widths and signs; integers never pass through `f64`, including `i64` values beyond binary64's exact-integer range. Floating values remain binary64; no cast, rounding, currency quantization or compensated accumulation occurs. Primitive bit patterns are handed to Arrow without arithmetic; bit-pattern preservation through every downstream consumer is not independently qualified here.
4. Set field nullability from the enum arm, not the values. `NullableFloat64([Some(x)])` remains a nullable field; an empty nullable column also retains that declaration. Each `None` becomes an invalid/null slot. `Some(NaN)`, `Some(+∞)` and `Some(−∞)` remain valid/present floating slots; nonnullable Float64 likewise allows them. Neither zero nor NaN is synthesized for a missing value. Physical values under null slots are not a consumer-facing payload guarantee.
5. Hand the typed array and field to [schema construction](02-schema-order-and-metadata.md) and [batch assembly](03-envelope-boundary-and-batch-assembly.md). Every column has the same row order as its input; the adapter performs no alignment, filtering or imputation.

## Precision, time and JSON boundaries

The envelope has no Date, Timestamp, Duration, Decimal, Binary, list, struct, categorical or extension-type variant. A date-looking string remains `Utf8`; an epoch-like `Int64` remains an integer with no inferred time unit or timezone. The inspected statements producer formats `PeriodId` as text before calling this boundary. No day-count or calendar operation occurs here.

Float64 columns do not acquire Decimal precision from a currency label. The inspected portfolio producer calls `Money::amount()` before constructing its floating column; this adapter cannot recover precision already lost upstream. Contractual amount fidelity and rounding must be qualified at that producer boundary under ADR 0020. Similarly, metadata such as `unit`, `currency` or `scale` is copied, not enforced.

Direct export never serializes column values as JSON. In contrast, core `TableColumnData` derives ordinary serde for `Vec<f64>` and `Vec<Option<f64>>`, without the core nonfinite adapters. The pinned serde_json 1.0.151 serializer emits `null` for nonfinite floats. Thus a caller's nullable envelope JSON round trip turns `Some(nonfinite)` into `None` before Arrow sees it; a nonnullable float vector cannot deserialize that JSON null as an `f64`. These are static consequences of the selected serde types and serializer, not executed reproduction results. [Pinned serializer source](https://docs.rs/serde_json/1.0.151/src/serde_json/ser.rs.html#168-180).

## Alternatives, limitations and failures

Plain Float64 preserves model outputs, including undefined values, without deciding their financial validity. Mapping NaN to null would merge distinct states unless a producer explicitly selects that policy; rejecting all nonfinite values would also exclude outputs that legitimately report undefined results. ADR 0009 requires a stated policy at the owning operation, not either global replacement.

Decimal and temporal Arrow types would express richer contracts but require corresponding canonical envelope variants and explicit producer conventions. They cannot safely be inferred from a column name or metadata. Lossless text representations are another producer-level choice with different host arithmetic behavior. Neither alternative is selected here.

Supported primitive values have no adapter range check beyond their Rust type; no finite-value error is returned. UTF-8 storage limits and allocation failures occur during array construction and are addressed in ARROW-003. There is no import adapter to establish Arrow-to-envelope round-trip or idempotence; repeated direct exports are read-only reconstructions, not a transform that consumes its own output.

## Evidence and limitations

- **Implementation:** [adapter `column_to_arrow`, lines 75–124](../../../../finstack-quant/arrow-interchange/src/lib.rs), [core variants and serde derives, lines 172–198](../../../../finstack-quant/core/src/table.rs), [statement producer](../../../../finstack-quant/statements/src/evaluator/export.rs), and [portfolio producer](../../../../finstack-quant/portfolio/src/dataframe.rs). These establish transformation and responsibility boundaries.
- **Local test evidence:** `to_record_batch_maps_types_nullability_and_metadata` in [adapter tests](../../../../finstack-quant/arrow-interchange/src/lib.rs) constructs all nine variants and asserts their types/nullability, but only checks null counts for the nullable string and Float64 columns. It does not assert all array values or integer extrema.
- **Host evidence:** [Python tests](../../../../finstack-quant-py/tests/test_arrow_interchange.py) compare ordinary finite statement values between Arrow and pandas/Polars and check scalar-null columns. These comparisons share producer data; they are not independent financial reference values or exhaustive transport qualification.
- **Step verdict:** Enum dispatch and copying match the documented direct-export contract by inspection. Cross-transport preservation is incomplete: the JSON-envelope path does not preserve the same nonfinite/missing distinction. Financial appropriateness remains producer-specific.

## Follow-up tracking

Implementation reconciliation, pending choices, and qualification work are
tracked in the [Arrow-interchange follow-ups](../FOLLOW_UPS.md#arrow-interchange).
