# PORTFOLIO-EXECUTION-006 — Content-addressed portfolio materialization and load validation

- **Decision status:** Accepted 2026-11-15 — complete versioned materialization, bounded validation, canonical artifact and host diagnostic package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent duplicate-key, pointer, digest-order and shared-artifact examples verified. Production round trips and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio materialization and validation

1. **Version bulk and inline persistence explicitly.** Use
   `finstack_quant.portfolio_materialization/2` for the revised
   bulk shape and `finstack_quant.portfolio_spec/1` for the
   persisted inline form. Each accepts one exact marker. Both
   share Rust holding and graph admission. Keep the nested
   instrument schema, canonical digest version and producer
   metadata separate.
   [Core version and validation rules](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics).

2. **Apply bounded admission before full allocation.** Keep core
   defaults of 64 MiB, depth 96, 100,000 artifacts, one million
   holdings and 256 retained diagnostics. Apply the 100,000-book
   and 512-level limits. Count all supplied entries, including
   duplicates and unused artifacts, and stop on the first excess.
   Reject nested duplicate keys, lossy numbers and unknown
   fields in closed objects.
   [Core contract loading](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics).

3. **Use accepted holding and graph assignments.** Preserve ordered
   holdings with exact entity and artifact references. Check any
   repeated instrument ID against the decoded instrument. Admit
   only `Units` and `Percentage`. Use book `parent_id` and
   holding optional `book_id` as assignments, deriving child
   and membership lists. Admit unassigned holdings.
   [Holding identity and sizing](01-position-sizing-and-validation.md#accepted-decision-for-checked-portfolio-holdings-and-signed-sizing) ·
   [Book graph](02-book-hierarchy-and-rollups.md#accepted-decision-for-checked-book-hierarchy-and-nav-rollups).

4. **Verify artifact contents and declarations.** Distinguish the
   bundle-local artifact ID, decoded instrument ID and canonical
   `c2` digest. Reject duplicate artifact IDs and verify any claimed
   digest against the complete canonical instrument envelope.
   Distinct IDs for identical content produce a deterministic
   warning and share one immutable decode within the load. Admit
   every supplied artifact, including unused entries and cache
   hits. Compare optional dependency claims with ordered, typed
   owner declarations. Declarations support planning; selective
   repricing requires actual pricing and metric dependency proof.
   [Canonical identity](../core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity) ·
   [Selective reuse](05-dependencies-and-selective-reuse.md#accepted-decision-for-checked-portfolio-dependencies-and-selective-repricing).

5. **Share semantic admission and publish atomically.** Validation-only
   performs the same artifact, holding and graph checks as load and
   may skip final portfolio and index construction. Load publishes
   a complete portfolio only after admission. Distinguish submitted
   artifact entries, unique admitted contents and holdings. Record
   007 owns cache accounting and timing.
   [Cache and load reporting](07-artifact-cache-and-load-reporting.md).

6. **Export a fully restorable representation.** Both formats require
   canonical instrument definitions. Bulk export emits referenced
   contents, deduplicated by digest in first-holding order. Holdings
   preserve their own identities and sizes. Qualify round trips
   through the same strict loader.

7. **Expose the same bounded diagnostics across hosts.** Preserve
   ordered codes, phases and escaped JSON Pointers. A zero
   diagnostic-retention cap still reports fatal status and truncation
   when an error occurs. Existing Python and WASM load and
   validation calls use the same Rust admission. Keep bulk export
   Rust-only and qualify generated examples through actual Rust
   admission.
   [Bounded diagnostics](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics).

## Independent calculation and identity checks

- A nested duplicate entity key can produce the same map as a
  clean input after ordinary map decoding. Pair-preserving
  admission finds the duplicate before map collection.
- An entity key `desk/a~b` uses JSON Pointer segment
  `desk~1a~0b`. A zero diagnostic cap can retain an empty
  detail list while preserving fatal and truncated status.
- Reordering object keys preserves the digest for the checked
  illustrative envelope. Reversing an ordered cash-date array
  changes it. Two producer artifact IDs can identify one
  canonical content with two distinct holdings. A USD 10
  mark at holdings of `+2` and `−0.5` gives USD 20 and
  USD `−5` as separate rows.

## Observed choice and rationale at the audit revision

The materialization envelope stores reusable instrument artifacts separately from ordered holdings, with canonical content hashes and runtime-derived dependency claims. This reduces repeated decode work while ensuring that content, position identity and declared market footprint agree before valuation. It is a construction contract; it does not certify market availability or instrument pricing accuracy.

## Ordered load algorithm

1. Enforce generic JSON byte/depth limits, then deserialize a lightweight collection counter for artifacts, positions and books. If this preflight cannot parse the shape, it leaves structural error reporting to the full parse. Enforce caller `LoadLimits` for artifacts/positions and the 100,000 book ceiling.
2. Parse the current `finstack_quant.portfolio_materialization/1` schema tag and strict envelope fields. Percentage holdings use a bounded percentage wire type; nonpercentage holdings retain f64 quantity and a named unit or explicit nullable notional currency.
3. Validate duplicate artifact and position IDs, all artifact/entity references, entity map keys versus embedded IDs, and book topology. Materialized positions derive book membership from the book lists; there is no independently supplied position `book_id` to reconcile.
4. Compute each envelope's canonical content hash. Verify an optional caller hash exactly. Duplicate content under different artifact IDs is a warning, not a fatal duplicate-ID error. Prepare all artifacts, including unreferenced ones, with encoded byte counts.
5. Decode through the [artifact cache](07-artifact-cache-and-load-reporting.md), keyed by computed content hash, current instrument contract version and canonicalization version. Extract market dependencies from the actual instrument. If supplied, the claimed dependency object must equal runtime extraction; mismatches fail even on cache hits.
6. Check each position's reference instrument ID against the artifact's live ID, finite quantity and percentage bounds. Build positions through `Position::new`, attach attributes/metadata and derived book ID, rebuild indices, and run final portfolio validation. Any retained error makes the whole load fail; no partially built portfolio is returned.

Diagnostics are phase-coded, include relevant pointers/IDs, and are bounded by `LoadLimits`. Decode errors are collected in artifact order. `validate_materialization` performs decode, dependency and semantic checks but skips constructing the position vector and indices; it can warm the same cache. Materializer producer/compiler/library strings are provenance payload, not an observed compatibility-verification algorithm.

## Export and schema behavior

`to_materialization` first checks native structure, serializes every instrument through the canonical instrument registry, and deduplicates by content hash in first-position encounter order. The hash itself is the exported artifact ID. Unknown runtime instruments are rejected rather than emitted without a payload. The export retains entity/book/header metadata and position quantities. Native validation is narrower than this load boundary, so the existence of an exported typed envelope is not proof it will pass all load-time semantic checks after arbitrary native field edits.

The schema registry delegates assertion-equivalent externalization of instrument and valuation definitions to core; published examples combine real constructor round trips and illustrative typed JSON. Generated schemas describe shape and explicit wire validators, not every runtime financial invariant.

## Alternatives and limits

The simpler repeated-instrument `PortfolioSpec` reconstruction remains a separate representation with weaker identity checks; it is not a migration fallback inside this loader. Rejecting duplicate content would reduce redundant artifact entries but change the warning contract. The report's `unique_instruments` is the input artifact count, even when duplicate-content warnings identify fewer unique hashes; dependency counts are summed per artifact entry, not globally deduplicated keys.

Numerical model adoption remains with the instrument records.

## Evidence

[materialization/mod.rs](../../../../finstack-quant/portfolio/src/materialization/mod.rs), [envelope.rs](../../../../finstack-quant/portfolio/src/materialization/envelope.rs), [schema.rs](../../../../finstack-quant/portfolio/src/schema.rs) and [schema examples](../../../../finstack-quant/portfolio/src/schema/examples.rs) were inspected. [Materialization tests](../../../../finstack-quant/portfolio/tests/materialization.rs) contain round-trip, duplicate/missing-reference, hash/dependency and resource-limit cases; [schema tests](../../../../finstack-quant/portfolio/tests/materialization_schema.rs) locate percentage wire parity. These are contract/round-trip evidence, not independent pricing validation.

## Follow-up tracking

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
