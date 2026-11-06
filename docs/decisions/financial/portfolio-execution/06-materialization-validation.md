# PORTFOLIO-EXECUTION-006 — Content-addressed portfolio materialization and load validation

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and rationale

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

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
