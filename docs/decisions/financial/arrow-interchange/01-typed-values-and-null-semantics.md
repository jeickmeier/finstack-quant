# ARROW-001: Typed values, validity, precision and transport boundaries

**Decision status:** Accepted 2026-11-17 for typed Arrow export and preservation of missing and nonfinite results.

**Implementation status:** Direct Arrow conversion and nonnullable Float64 JSON implement the accepted mapping. Nullable Float64 JSON reconciliation remains pending.

**Validation status:** Both Arrow batch unit tests and the core nonnullable Float64 nonfinite round-trip regression passed on 2026-11-17. Nullable Float64 JSON behavior was reviewed in source. Payload edge cases and host fidelity remain to be qualified.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope and exact coverage](README.md).

## Accepted decision

1. Map each declared table column variant to its corresponding plain Arrow type. Preserve source order, exact integer values and widths, and declared field nullability. Encode `None` as an Arrow null and retain present `NaN` and infinities as floating-point values.
2. Apply the [accepted core table wire contract](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics) to canonical envelope JSON. Encode finite Float64 values as numbers and present nonfinite values with the existing `"nan"`, `"inf"` and `"-inf"` sentinels. Reserve `null` for missing values in nullable columns. The owning operation defines when each state is valid under [ADR 0009](../conventions/0009-missing-and-invalid-data.md).
3. Keep units, currency, time conventions and contractual monetary precision with the producer. Plain Arrow primitives carry the values the producer supplies. Exact monetary exports require a producer-level representation that preserves the precision required by [ADR 0020](../conventions/0020-numerical-representation-and-rounding.md).

## Choice, domain and rationale

All supported result producers use the same enum-driven conversion. There is no instrument-specific dispatch or estimation. Numeric representation, units, currency, sign, period convention and missing-data meaning are supplied by the producer. The adapter neither calculates nor rescales a value, and role/metadata hints do not select another Arrow representation.

The [crate contract](../../../../finstack-quant/arrow-interchange/README.md) selects plain Arrow types and preserves nonfinite floating values. Direct Arrow export builds arrays from the typed representation. Each producer owns the precision and interpretation of the values it supplies.

## Ordered conversion and selection

1. Match `TableColumnData` exhaustively. `String` and `NullableString` produce `Utf8`; `Float64` variants produce `Float64`; `UInt32` variants produce `UInt32`; `Int64` variants produce `Int64`; `Boolean` produces `Boolean`. There is no nullable Boolean variant in the input enum, no unsupported-type fallback, and no sampling-based inference.
2. Traverse each string vector in source order. Nonnullable strings use `StringArray::from_iter_values`; nullable strings map `Option<String>` to `Option<&str>` and collect an array. Text is copied into Arrow storage. Empty text remains an empty string; `None` is distinct from empty text. Rust strings already have UTF-8 validity; the adapter performs no text normalization.
3. Clone each numeric/Boolean vector into the corresponding Arrow array constructor. Source `u32` and `i64` retain their integer widths and signs; integers never pass through `f64`, including `i64` values beyond binary64's exact-integer range. Floating values remain binary64; no cast, rounding, currency quantization or compensated accumulation occurs. Primitive bit patterns are handed to Arrow without arithmetic; bit-pattern preservation through every downstream consumer is not independently qualified here.
4. Set field nullability from the enum arm, not the values. `NullableFloat64([Some(x)])` remains a nullable field; an empty nullable column also retains that declaration. Each `None` becomes an invalid/null slot. `Some(NaN)`, `Some(+∞)` and `Some(−∞)` remain valid/present floating slots; nonnullable Float64 likewise allows them. Neither zero nor NaN is synthesized for a missing value. Physical values under null slots are not a consumer-facing payload guarantee.
5. Hand the typed array and field to [schema construction](02-schema-order-and-metadata.md) and [batch assembly](03-envelope-boundary-and-batch-assembly.md). Every column has the same row order as its input; the adapter performs no alignment, filtering or imputation.

## Precision, time and JSON boundaries

The envelope has no Date, Timestamp, Duration, Decimal, Binary, list, struct, categorical or extension-type variant. A date-looking string remains `Utf8`; an epoch-like `Int64` remains an integer with no inferred time unit or timezone. The inspected statements producer formats `PeriodId` as text before calling this boundary. No day-count or calendar operation occurs here.

Float64 columns do not acquire Decimal precision from a currency label. The inspected portfolio producer calls `Money::amount()` before constructing its floating column; this adapter cannot recover precision already lost upstream. Contractual amount fidelity and rounding must be qualified at that producer boundary under ADR 0020. Similarly, metadata such as `unit`, `currency` or `scale` is copied, not enforced.

Direct export builds Arrow arrays from typed values. For canonical envelope JSON, the nonnullable `Float64` variant already uses the core nonfinite sequence adapter and round-trips all three sentinel spellings. `NullableFloat64` still uses ordinary serde for `Vec<Option<f64>>`. The pinned serde_json 1.0.151 serializer emits `null` for those present nonfinite floats, so restoring the JSON yields `None`. Applying the accepted core sentinel contract to nullable columns resolves this remaining loss of information. [Core column types and tests](../../../../finstack-quant/core/src/table.rs) · [Pinned serializer source](https://docs.rs/serde_json/1.0.151/src/serde_json/ser.rs.html#168-180).

## Alternatives, limitations and failures

Plain Float64 preserves model outputs, including undefined values, without deciding their financial validity. Mapping NaN to null would merge distinct states unless a producer explicitly selects that policy; rejecting all nonfinite values would also exclude outputs that legitimately report undefined results. ADR 0009 requires a stated policy at the owning operation, not either global replacement.

Decimal and temporal Arrow types would express richer contracts but require corresponding canonical envelope variants and explicit producer conventions. They cannot safely be inferred from a column name or metadata. Lossless text representations are another producer-level choice with different host arithmetic behavior. Neither alternative is selected here.

Supported primitive values have no adapter range check beyond their Rust type; no finite-value error is returned. UTF-8 storage limits and allocation failures occur during array construction and are addressed in ARROW-003. There is no import adapter to establish Arrow-to-envelope round-trip or idempotence; repeated direct exports are read-only reconstructions, not a transform that consumes its own output.

## Evidence and limitations

- **Implementation:** [adapter `column_to_arrow`, lines 75–124](../../../../finstack-quant/arrow-interchange/src/lib.rs), [core variants and serde attributes, lines 236–271](../../../../finstack-quant/core/src/table.rs), [statement producer](../../../../finstack-quant/statements/src/evaluator/export.rs), and [portfolio producer](../../../../finstack-quant/portfolio/src/dataframe.rs). These establish transformation and responsibility boundaries.
- **Local test evidence:** `to_record_batch_maps_types_nullability_and_metadata` in [adapter tests](../../../../finstack-quant/arrow-interchange/src/lib.rs) constructs all nine variants and asserts their types/nullability, but only checks null counts for the nullable string and Float64 columns. It does not assert all array values or integer extrema.
- **Host evidence:** [Python tests](../../../../finstack-quant-py/tests/test_arrow_interchange.py) compare ordinary finite statement values between Arrow and pandas/Polars and check scalar-null columns. These comparisons share producer data; they are not independent financial reference values or exhaustive transport qualification.
- **Step verdict:** Enum dispatch and copying match the direct export contract by inspection. Nullable envelope JSON still collapses present nonfinite values and missing values into the same representation.

## Follow-up tracking

Nullable JSON reconciliation and transport qualification are tracked in the
[Arrow-interchange follow-ups](../FOLLOW_UPS.md#arrow-interchange).
