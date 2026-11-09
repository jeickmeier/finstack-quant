# CORE-NUMERICS-24: Numeric wire representations and canonical content identity

- Decision status: **Accepted — complete numeric-wire/canonical-identity package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent literal-byte/hash and representation-boundary examples and read-only wire/canonical/caller/reference reviews are reported below; they do not qualify production writers or financial results.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`wire/*.rs`, `canonical.rs`, `canonical/value_serializer.rs`: typed serialized numeric boundaries, nonfinite sentinels, schema-version labels, deterministic JSON bytes and SHA-256 identity. This is representation methodology, not pricing arithmetic or a claim of cryptographic authentication.

## Accepted decision: strict numeric wire and canonical content identity

Accepted on **2026-11-08** after whole-record review. It inherits accepted
[012's exact monetary inputs](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity),
[023's config/metadata authority](rounding-and-config.md#accepted-decision-explicit-rounding-config-local-tolerances-and-producer-metadata)
and [0020's representation boundaries](../conventions/0020-numerical-representation-and-rounding.md).
Implementation, existing caller/host/schema reconciliation and independent
qualification remain pending. Observed behavior is retained separately below.

1. **Retain the six finite bounded-f64 wire domains.** Require finite represented
   binary64 values with exactly the named bounds: Positive >0, NonNegative >=0,
   ClosedUnitInterval [0,1], OpenUnitInterval (0,1), Correlation [-1,1] and
   PercentageQuantity [-100,100]. Validate construction, restoration, helpers
   and serialization, not only schema shape. Preserve admitted signed zero and
   subnormals; reject rather than clamp, floor or infer percent/bp conversion.
   Bounds apply after declared f64 conversion, not to an arbitrary-precision
   lexical decimal. Optional positive None remains optional, not zero. Product
   and cross-field financial validity remain with the consuming owner.

2. **Make annotated count reads and writes share one integer domain.** count
   and counts use u32 0..=4,294,967,295, converted checked to/from usize;
   signed_count uses i32 -2,147,483,648..=2,147,483,647, stored as i64. Use the
   integer-typed deserializer on reads too: JSON 3 is accepted, 3.0/3e0,
   fractions, strings, booleans and null are rejected. Do not coerce or cast.
   Reconcile fields that currently annotate only serialization and enforce the
   same bounds at owner admission. JSON Schema integer/range describes values,
   but alone cannot enforce this stricter integer-token input rule. Native host
   values admitted as annotated counts emit integer tokens; native JS Number
   admission checks integrality/range, not unrecoverable original spelling.
   True u64 seeds/IDs
   keep their exact owner contract; this is not a universal u32 count limit.

3. **Use accepted exact Decimal admission and normalized string output.**
   DecimalWire/decimal/optional_decimal use 012's one exact fixed/scientific
   parser, final coefficient/scale limits and normalized fixed-point writer.
   Reject nonzero digit loss and monetary JSON numbers; never detour exact
   integers/text through f64. Some writes a string, None writes null or is
   omitted only by the enclosing field policy. Typed Decimal 1.00 writes "1";
   an ordinary user string "1.00" remains unchanged by canonicalization.
   Remove permissive rounded wire parsing and stale schema grammar without
   introducing another monetary precision/rounding policy.

4. **Make date JSON a checked canonical calendar-date string.** Keep the
   existing time::Date proleptic Gregorian domain and its human-readable writer
   spelling, including year zero and supported negative years; do not narrow
   it to unsigned four-digit RFC 3339 spelling or enable a larger date range.
   Read only valid dates in exactly that writer spelling: no ordinal arrays,
   numbers, outer whitespace or alternate date forms. Reject invalid leap/
   month/day/range values. Route actual Date JSON fields through the same date
   adapter; a schemars stand-in alone cannot tighten their deserializers.
   Replace the misleading RFC3339 `format: date` with the actual project string
   grammar/range description and runtime calendar validation. Date-looking
   ordinary strings are not parsed or normalized by the canonicalizer.

5. **Keep nonfinite wire values explicit and field-selected.** Finite numbers
   remain JSON numbers; only explicitly nonfinite-aware fields accept exact
   lowercase strings "inf", "-inf", "nan". Remove trimming/case aliases;
   reject numeric strings, null and other spellings. Numeric arms must be finite
   even with a native deserializer capable of supplying NaN/infinity. The nan
   token preserves undefined classification, not payload/sign. FiniteOrNanWire
   permits only its finite numeric domain or "nan", never infinity tokens.
   Product owners decide which result can be infinite/undefined; this wire
   package does not convert calculation errors into sentinels or authorize a
   sentinel on every f64 field. Optional missing/null remains field-owned.

6. **Close direct finite-arm serialization bypasses.** Validate public
   NonFiniteF64Wire::Finite and FiniteOrNanWire<T>::Finite on serialization as
   well as helpers/restoration, including direct variant construction. A raw
   NaN/infinity hidden in Finite must error, never become null or silently change
   variants. Restrict supported T to f64 and the six bounded-f64 wire types;
   serialization, deserialization and schema must enforce the same finite
   numeric domain/bounds. String/object generic instantiations are unsupported.
   No second sentinel vocabulary or new generic conversion framework.

7. **Keep version axes and enum labels independent.** SchemaVersion remains
   integer 1 only for owners explicitly using that v1-only type. It does not
   govern version-2 RoundingContext, canonical c2, package versions, extension
   suffixes or every domain artifact. Owners choose their own exact supported
   version; reject unsupported values without migration/defaulting. Keep serde
   label/tag helpers as conversions of the owning enum's declared serde form
   (string unit label or one-key external tag), not a financial default or an
   independent case/alias registry. Any caller normalization remains explicit
   owner policy.

8. **Define canonical equality as serialized identity.** Preserve arrays,
   ordinary strings, Unicode scalar sequences, producer enum forms, omitted
   versus null fields, signed floating zero, integer-versus-floating form and
   extension metadata.
   In particular integer 1 and float 1.0 differ; 0.0 and -0.0 differ. No Unicode
   normalization, financial rounding, array sorting, timestamp/metadata removal
   or generic Decimal/date interpretation. Producers normalize typed economic
   concepts before serialization when their owner requires it. Equal digests
   do not establish economic equivalence or complete financial inputs.

9. **Sort every object's decoded keys explicitly by UTF-8 bytes.** Use the
   maintained serde_json Value::sort_all_objects operation before compact UTF-8
   encoding; preserve array order. Do not rely on preserve_order remaining off
   under Cargo feature unification. Sort actual key strings, not their escaped
   JSON spellings, locale or UTF-16 order. Keep the repository-specific profile;
   it is not RFC8785/JCS, which has different ordering and numeric normalization.

10. **Freeze the admitted numeric and byte-writing profile.** Numeric integer
    values must fit i64::MIN..=u64::MAX, with checked i128/u128 admission; this is
    our implementation domain, not JSON's general number limit. Raw f32/f64 must
    be finite. Preserve the current exact f32-to-f64 widening before binary64
    formatting: 0.1f32 writes 0.10000000149011612. Use the locked serde_json
    1.0.151/zmij 1.0.23 compact string/number encoding behavior as the c2 baseline,
    including .0 float distinction, signed zero, escaping and exponent notation.
    Correct the stale Ryu attribution. Independent exact byte vectors qualify
    implementations; a dependency update must preserve c2 bytes or select a new
    canonical version, rather than silently changing persisted identity. The
    supported c2 serde_json profile has arbitrary_precision off; feature
    unification must not reinterpret Number's private payload as ordinary data.

11. **Reject ambiguous map keys and duplicate object names.** Retain string/
    char, boolean, integer and unit-enum-label keys and delegating newtypes,
    converted to their exact string forms. Integer map keys may use full i128/
    u128 decimal strings: they are object names, not numeric values. Reject all
    floating map keys, including finite ones, and unsupported composite keys.
    Reject every duplicate final key after conversion, including string/integer/
    boolean collisions, custom struct fields, flatten output and struct variants.
    Never keep first/last silently. Reject malformed custom map serialization
    (key-key, value-before-key or a dangling key) without partial output.

12. **Remove raw-JSON bypasses and reject lossy ingress before Value.** Remove
    the RawValue pseudo-struct delegation; no production consumer was found.
    Explicitly reject SerializeStruct names `$serde_json::private::RawValue`
    and `$serde_json::private::Number`, without reparsing their payloads. Under
    arbitrary_precision even fitting Number values emit the latter token; fail
    rather than assign them alternate object identity. Ordinary JSON strings and
    ordinary objects with a similarly named key remain ordinary data. Add no
    raw-JSON canonicalization API. Existing raw/host ingress must reject duplicate
    names, raw nonfinites and out-of-range integer tokens before generic Value
    materialization can erase them or promote integers to f64. Float/exponent
    tokens are the declared finite f64 input form; integer tokens have the exact
    integer range above. A prebuilt Value cannot recover lost duplicate names,
    a previously rounded integer or NaN already changed to null. Do not claim
    hashing such a tree proves the original input was lossless.

13. **Bound canonical container recursion and check resource arithmetic.**
    Cap emitted JSON container nesting at 96, with a root object/array at depth
    1 and scalar root at 0; count enum-created containers too. This fixed writer
    ceiling aligns with the existing default loader depth, without adopting all
    record 025 loader controls. Check growth/capacity arithmetic; do not allocate
    directly from an untrusted Serialize length hint. Use available fallible
    reservations where supported and return no bytes/hash on failure. Enclosing
    owners retain byte/width/work limits. This does not sandbox arbitrary custom
    Serialize callbacks, guarantee all allocator failures are recoverable, or
    prevent recursion inside a producer before it emits a container. No new
    public limits/configuration framework or universal stack-safety claim.

14. **Select c2 and retain the exact versioned SHA-256 preimage.** Change the
    single active CANONICAL_VERSION from c1 to **c2** for the tightened admission
    and deterministic writer contract. Hash exactly ASCII `finstack-canon/`,
    ASCII `c2`, one NUL byte and the canonical UTF-8 bytes; write lowercase
    `sha256:` plus 64 hex digits. Keep canonical version alongside digests in
    manifests/cache identity; the digest prefix alone does not encode c2.
    Remove c1 writing/compatibility/fallback paths, do not relabel old c1 evidence
    or silently rehash it as historical identity. Future byte/admission-profile
    changes require explicit version review. SHA-256 identity is not a signature,
    authentication, financial validation, collision impossibility or replay proof.

15. **Serialize once per call and report truthful failures.** Capture one
    checked tree/byte sequence from the original value; content_hash hashes those
    bytes rather than calling the original Serialize implementation twice.
    Producers must supply deterministic, side-effect-free serialization of frozen
    state. If persistence and hashing use separate calls, the owner must ensure
    the same immutable typed content; no generic archive/bundle framework is
    selected. Invalid numeric domains/keys, duplicates and exceeded limits are
    input failures; malformed custom Serde protocol, callback/encoding failures
    are computation failures with causes. Do not flatten all failures to Internal,
    repair values, return a digest of partial output or retry with another writer.

16. **Reconcile actual identity and cache consumers.** Route existing Python/
    WASM canonical_json/content_hash operations through the same Rust authority,
    including calibration artifacts, FinancialModelSpec and valuation envelopes.
    Replace the composite cache's hash-plus-Debug-defaults suffix by canonical
    serialization/hashing of its complete typed cache-key payload, including
    actual effective pricing controls. Its owner must identify all result-changing
    context within that cache scope; hashing cannot supply omitted inputs or
    approve defaults. Reconcile c2 manifests, fixtures and cache keys together;
    do not claim existing caches are complete or archive-compatible.

17. **Reconcile existing hosts/schema without new binding math or aliases.**
    Preserve exact Python integer/Decimal and JavaScript BigInt admission before
    generic JSON conversion; reject out-of-range integers instead of silently
    rounding through serde_json Value. An untyped JS Number cannot prove the
    original exact integer beyond 2^53-1; exact-integer input paths reject unsafe
    integral Numbers and use the existing BigInt/owner-supported exact path.
    Keep the generic JS bridge's safe-integral Number-to-integer mapping, except
    preserve floating -0 rather than casting it to integer zero. Other finite
    Numbers retain the declared f64 path. JS Number cannot recover lexical
    1-versus-1.0 intent: use original JSON text through an existing text input
    when that distinction matters, or the owner's typed representation. Do not
    assert arbitrary native-object equality across hosts. Count-specific native
    admission does not justify coercing all floating values to count tokens.
    Native host objects and wire JSON are different
    boundaries: JSON.stringify of native NaN/infinity cannot preserve sentinels.
    NonFiniteFields revival applies only to declared fields/sequences, never
    recursively to arbitrary strings. Reconcile adapters, actual date/count
    consumers, sentinel schemas, schema/declaration fixtures and the parity ledger
    in the later slice. Retain Rust as byte/hash authority; add no new hash class,
    duplicated host canonicalizer, compatibility parser or version migration.

18. **Qualify representation separately from financial correctness.** Require
    independent exact bytes/digests and boundary cases for every adapter,
    integer/float/signed-zero/f32 distinction, UTF-8 versus UTF-16 ordering,
    escaping/Unicode, duplicate/conversion collisions, RawValue rejection,
    malformed Serde protocols, depth/resources, both Cargo map feature modes and
    rejection of the unsupported arbitrary_precision profile/private Number token.
    Qualify actual Rust/Python/WASM paths and original-before-Value rejection;
    writer-reader roundtrips and shared implementation hashes alone are not
    independent conformance. Bounded literal calculations/source review do not
    certify production writers, financial accuracy, authentication or replay.
    This review changes no runtime, dependency, test, schema, fixture or host.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| u32 maximum / maximum+1 count | First admitted both ways; second rejected before storing/writing. |
| Count JSON 3 / 3.0 / "3" | Integer token admitted; float token/string rejected. |
| Typed Decimal 1.00 / ordinary string "1.00" | Wire "1" / unchanged "1.00"; no generic string normalization. |
| Date year 0000 / supported negative year | Valid canonical Gregorian spelling admitted; no RFC3339-only narrowing. |
| Date ordinal array / impossible leap day | Reject, including ordinary fields that formerly used permissive time serde. |
| Exact "inf" / " INF " / null at required nonfinite field | Explicit infinity / input error / input error. |
| Finite(NaN) constructed directly | Serialization error; never null. |
| Integer 1 / float 1.0 / 0.0 / -0.0 | Four distinct canonical byte strings and versioned digests. |
| U+E000 and U+10000 object keys | U+E000 sorts first by UTF-8; different from JCS UTF-16 order. |
| Integer key 1 plus string key "1" | Duplicate-name error after key conversion. |
| Finite float map key / duplicate struct field | Reject both; no display-based float keys or overwrite. |
| u128 value 2^64 / u128 map key 2^64 | Numeric value rejected; decimal object-name string admitted absent collision. |
| RawValue or private Number pseudo-struct / JSON string containing JSON-looking text | Reserved pseudo-struct rejected / ordinary string preserved. |
| Native JS Number 1 / JSON text 1.0 / native -0 | Generic bridge integer 1 / float 1.0 / preserved float -0.0; no recovered lexical intent. |
| Raw integer token 18446744073709551617 entering Value bridge | Reject before conversion can round it to f64. |
| Emitted container depth 96 / 97 | First within canonical ceiling; second errors. Owner width/byte limits still apply. |
| RoundingContext 2 / SchemaVersion 1 / canonical c2 | Independent exact version axes; no global schema-version increment. |

## Observed choice and rationale

Decimal values serialize as strings to preserve decimal representation; ordinary bounded floats validate at wire conversion. Explicit nonfinite-aware fields serialize sentinels rather than silently converting undefined/infinite results to null. Canonical content rejects raw nonfinite floats and uses a versioned hash domain.

## Observed algorithm

1. Bounded f64 wrappers require finiteness plus the named bound: positive, nonnegative, [0,1], (0,1), [-1,1], or percentage quantity [-100,100]. Serializer helpers validate as well as deserializers. Optional positive values preserve None.
2. Decimal/optional decimal delegate to `rust_decimal::serde::str`; DateWire delegates to time::Date with schema date annotation. Count serialization converts usize to u32 (or i64 to i32 for signed count) and errors on overflow, but deserialization accepts the host-sized usize/i64 domain.
3. Explicit nonfinite fields write finite numbers or strings `inf`, `-inf`, `nan`. Reads trim/lowercase those three sentinel spellings; other strings and null error. NaN payload/sign is not preserved by the sentinel.
4. SchemaVersion accepts exactly integer 1. Serde label/tag helpers reuse enum serialization; tags accept a string or single-key externally tagged object, otherwise error.
5. Canonical serialization recursively builds serde_json values and rejects raw nonfinite f32/f64 values. Contrary to the earlier draft description, finite floating map keys are accepted via Display; nonfinite keys error. Numeric i128/u128 values must fit the implementation's i64/u64 domain. f32 widens to f64 before number construction. Arrays retain order, object keys follow serde_json's ordered map in this build, enum representations follow serde, and finite floating encoding/signed zero are preserved rather than normalized to a mathematical equivalence class. Cargo.lock selects serde_json 1.0.151 and zmij 1.0.23; the module's Ryu attribution is stale.
6. Canonical bytes are compact serde_json bytes. Hash input is `finstack-canon/` + `c1` + NUL + canonical bytes; output is lowercase `sha256:` hex. Custom raw-JSON serialization is delegated to serde_json's RawValue handling.

## Observed selection, controls and conventions

Numeric wire behavior is selected by wrapper/serde annotation, not globally for all f64 fields. Sentinels are strings before canonicalization and therefore can be canonicalized as such; only raw nonfinite floats are rejected. The crate depends on serde_json without preserve_order for sorted keys.

## Observed limitations, failure and fallback

Canonical equality is serialized identity, not economic equivalence: 1 and 1.0, current Decimal scale strings, signed zero, array ordering and caller serde annotations can matter. Duplicate map/struct/variant keys overwrite; key-key calls replace the pending key. RawValue reparsing can erase duplicates and generic Value ingress can promote oversized integer tokens to f64. Existing Value cannot reconstruct lost original information. Count read/write ranges differ on 64-bit hosts. time::Date reads ordinal arrays as well as strings, despite DateWire's string/date schema. Public finite enum arms can carry nonfinites past derived serialization. Domain constraints do not automatically apply to unwrapped fields; one schema wrapper is not universal parser enforcement. The canonical tree builder trusts length hints and has no emitted-container depth guard. Existing writer tests and digest fixtures do not establish cross-language or feature-unification stability.

## Observed alternatives and tradeoffs

Strict raw-float rejection and explicit sentinel representation are implemented for different surfaces. Automatically converting all nonfinites to null, normalizing all numbers, or migrating schema versions is not performed. This canonical format is repository-specific, not asserted to implement an external JSON canonicalization standard.

## Evidence

- [Wire adapters](../../../../finstack-quant/core/src/wire/mod.rs), [wire tests](../../../../finstack-quant/core/src/wire/tests.rs), [canonical serializer](../../../../finstack-quant/core/src/canonical/value_serializer.rs), [canonical API tests](../../../../finstack-quant/core/tests/canonical_api.rs), [contract canonical tests](../../../../finstack-quant/core/tests/contract/canonical.rs).
- `non_finite_f64_round_trips_infinities`, `non_finite_f64_rejects_null`, and exact-schema-spelling cases test the wire contract. `canonical_bytes_sort_map_keys_recursively_without_reordering_arrays` and `content_hash_has_a_stable_versioned_sha256_fixture` test implementation stability; they are not independent cryptographic proof or universal cross-language conformance evidence. [ADR 0020](../conventions/0020-numerical-representation-and-rounding.md) covers representation policy at its accepted scope.
- [Counts](../../../../finstack-quant/core/src/wire/counts_wire.rs),
  [dates](../../../../finstack-quant/core/src/wire/date_wire.rs),
  [Decimals](../../../../finstack-quant/core/src/wire/decimal_wire.rs),
  [sentinels](../../../../finstack-quant/core/src/wire/non_finite.rs) and
  [canonical API](../../../../finstack-quant/core/src/canonical.rs) were inspected
  with their serializers. Existing tests were located, **not executed**. RawValue
  usage was found only in canonical tests, not a production consumer. Existing
  [composite cache](../../../../finstack-quant/valuations/src/instruments/composite/instrument.rs)
  appends Debug defaults to a digest; Python/WASM schema/content-hash entry points
  and generic JSON bridges require coordinated reconciliation, not new host math.
- [RFC8259 sections 4/6](https://www.rfc-editor.org/rfc/rfc8259.html#section-4)
  explain duplicate-name interoperability and JSON number limits; the chosen
  u32/i32/i64/u64 domains are repository policy. [RFC8785](https://www.rfc-editor.org/rfc/rfc8785.html#section-3.2.3)
  specifies JCS UTF-16 sorting and different number normalization, not this
  profile. [FIPS180-4](https://csrc.nist.gov/pubs/fips/180-4/upd1/final)
  defines SHA-256, not the repository domain prefix or financial validity.
  The pinned [serde_json sorting implementation](https://raw.githubusercontent.com/serde-rs/json/v1.0.151/src/value/mod.rs)
  supplies sort_all_objects across map feature modes; its
  [Number implementation](https://raw.githubusercontent.com/serde-rs/json/v1.0.151/src/number.rs)
  explains the unsupported arbitrary_precision pseudo-struct. Cargo.lock,
  rather than an outdated source comment, identifies the baseline formatter.
  [RFC3339 section 5.6](https://www.rfc-editor.org/rfc/rfc3339.html#section-5.6)
  supplies the date-format comparison; it is not a reason to discard supported
  proleptic dates. [Record 025](contracts-and-diagnostics.md) separately owns
  loader/diagnostic policy; its default depth 96 motivates the fixed writer cap,
  not a claim that its whole draft package is already accepted.
- Independent **2026-11-08** Python standard-library checks used seven literal
  byte sequences and hashlib, not production writers/canonical APIs. All seven
  c2 digests were distinct; exact promoted-f32 rational identity, UTF-8/UTF-16
  ordering contrast, integer limits, oversized-integer float loss and exact
  sentinel counterexamples passed. A second read-only calculation matched the
  five vectors below. These are accepted byte/hash targets, not evidence that
  production currently writes c2 or satisfies every numeric format boundary.

| Literal canonical UTF-8 bytes | Expected SHA-256 hex after `finstack-canon/c2` + NUL prefix |
| --- | --- |
| `1` | `68fe447fdd097424ed321c44f359947982a487261dc404579e96738b4efeff79` |
| `1.0` | `74eeb9967af2daf657a8278663dc4a35aba72031afd6ead0119c1d5f19329cbd` |
| `0.0` | `a3e2eff77f83898e994b08a9a8c53c8ec4b43dd5ff53fe3197a8d0b7beb372e7` |
| `-0.0` | `65dbe1872529c515fa06199fe326a51a05247592b655af4d9ed40361bb4f5bea` |
| `{"a":"value","b":[3,2,1]}` | `592b4a40fbac33f6fb20768943454fde8df73f342eed06f63681766499f668ef` |

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
