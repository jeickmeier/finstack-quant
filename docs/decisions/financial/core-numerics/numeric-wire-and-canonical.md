# CORE-NUMERICS-24: Numeric wire representations and canonical content identity

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`wire/*.rs`, `canonical.rs`, `canonical/value_serializer.rs`: typed serialized numeric boundaries, nonfinite sentinels, schema-version labels, deterministic JSON bytes and SHA-256 identity. This is representation methodology, not pricing arithmetic or a claim of cryptographic authentication.

## Choice and rationale

Decimal values serialize as strings to preserve decimal representation; ordinary bounded floats validate at wire conversion. Explicit nonfinite-aware fields serialize sentinels rather than silently converting undefined/infinite results to null. Canonical content rejects raw nonfinite floats and uses a versioned hash domain.

## Algorithm

1. Bounded f64 wrappers require finiteness plus the named bound: positive, nonnegative, [0,1], (0,1), [-1,1], or percentage quantity [-100,100]. Serializer helpers validate as well as deserializers. Optional positive values preserve None.
2. Decimal/optional decimal delegate to `rust_decimal::serde::str`; DateWire delegates to time::Date with schema date annotation. Count serialization converts usize to u32 (or i64 to i32 for signed count) and errors on overflow, but deserialization accepts the host-sized usize/i64 domain.
3. Explicit nonfinite fields write finite numbers or strings `inf`, `-inf`, `nan`. Reads trim/lowercase those three sentinel spellings; other strings and null error. NaN payload/sign is not preserved by the sentinel.
4. SchemaVersion accepts exactly integer 1. Serde label/tag helpers reuse enum serialization; tags accept a string or single-key externally tagged object, otherwise error.
5. Canonical serialization recursively builds serde_json values, rejects raw nonfinite f32/f64 values and floating map keys, and restricts numeric i128/u128 values to JSON's supported i64/u64 domain. Arrays retain order, object keys follow serde_json's ordered map in this build, enum representations follow serde, and finite floating encoding/signed zero are preserved by serde_json rather than normalized to a mathematical equivalence class.
6. Canonical bytes are compact serde_json bytes. Hash input is `finstack-canon/` + `c1` + NUL + canonical bytes; output is lowercase `sha256:` hex. Custom raw-JSON serialization is delegated to serde_json's RawValue handling.

## Selection, controls and conventions

Numeric wire behavior is selected by wrapper/serde annotation, not globally for all f64 fields. Sentinels are strings before canonicalization and therefore can be canonicalized as such; only raw nonfinite floats are rejected. The crate depends on serde_json without preserve_order for sorted keys.

## Limitations, failure and fallback

Canonical equality is serialized identity, not economic equivalence: 1 and 1.0, Decimal scale strings, signed zero, array ordering and caller serde annotations can matter. Duplicate map keys with identical serialized strings overwrite in the intermediate map. Count read/write ranges differ on 64-bit hosts. Domain constraints do not automatically apply to unwrapped public f64 fields.

## Alternatives and tradeoffs

Strict raw-float rejection and explicit sentinel representation are implemented for different surfaces. Automatically converting all nonfinites to null, normalizing all numbers, or migrating schema versions is not performed. This canonical format is repository-specific, not asserted to implement an external JSON canonicalization standard.

## Evidence

- [Wire adapters](../../../../finstack-quant/core/src/wire/mod.rs), [wire tests](../../../../finstack-quant/core/src/wire/tests.rs), [canonical serializer](../../../../finstack-quant/core/src/canonical/value_serializer.rs), [canonical API tests](../../../../finstack-quant/core/tests/canonical_api.rs), [contract canonical tests](../../../../finstack-quant/core/tests/contract/canonical.rs).
- `non_finite_f64_round_trips_infinities`, `non_finite_f64_rejects_null`, and exact-schema-spelling cases test the wire contract. `canonical_bytes_sort_map_keys_recursively_without_reordering_arrays` and `content_hash_has_a_stable_versioned_sha256_fixture` test implementation stability; they are not independent cryptographic proof or universal cross-language conformance evidence. [ADR 0020](../conventions/0020-numerical-representation-and-rounding.md) covers representation policy at its accepted scope.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
