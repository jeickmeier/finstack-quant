# PORTFOLIO-EXECUTION-007 — Bounded artifact decode cache and diagnostic timing

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and scope

`InstrumentArtifactCache` retains immutable decoded instruments plus their extracted dependencies. It contains no market, PV, Greek or calibrated valuation state, so its reuse differs from the request-local financial-cache restriction in [ADR 0022](../../architecture/0022-portfolio-evaluation-state.md). The caller owns the cache lifetime; portfolios retain independent `Arc` references to decoded instruments after eviction.

## Algorithm and controls

The default limits are 4,096 entries and 64 MiB of encoded instrument-envelope bytes. A key contains canonical content hash, instrument contract version and canonicalization version. A map stores entries and a `VecDeque` records least-recently-used order.

Every `get_or_decode` obtains the write lock. A hit clones the artifact and moves its key to the queue tail. A miss executes the decoder while still holding that lock, increments the successful decode counter, and evicts oldest entries until both bounds permit insertion. Queue touch searches and removes the existing key linearly. Zero entry/byte limits or an artifact larger than the byte bound still allow decoding and use, but suppress retention. Failed decodes are returned and not cached; their attempts do not increment the successful-decode counter.

Holding the lock across decode makes concurrent requests for a key decode once, at the cost of serializing misses for unrelated keys and blocking cache reads during decode. Lock poisoning is recovered through `into_inner` for both read and write locks rather than disabling the cache. Counts and byte additions use saturation where coded; there is no separate heap-allocation estimator. Encoded-size bounds do not establish a strict memory bound for live instruments, shared `Arc`s or decoding temporaries.

## Reporting semantics

Materialization records parse, validation, decode, position-build and index-build times. Native timing uses monotonic `Instant`, converting nanoseconds to u64 with saturation. WASM uses optional `performance.now()`, validates nonnegative finite elapsed milliseconds, rounds to nanoseconds and caps at u64 maximum. Missing or unusable clock readings become zero phase values with `timing_available=false`; those zeroes are not measured zero-cost phases. Validation-only calls fold position semantic time into validation and leave build/index durations zero.

Counts describe artifacts/positions/dependency keys as specified in [06](06-materialization-validation.md); they are not a valuation-performance benchmark. A warm cache hit still goes through outer limits, canonical hashing, references and dependency-claim checks.

## Alternatives and evidence limits

Per-key single-flight decoding would allow independent-key concurrency but adds synchronization complexity; it is not implemented. Exact heap accounting or count-only policies offer different resource guarantees. Failed-decode memoization would change error lifetime and must be designed explicitly rather than inferred from this cache.

This audit inspected mechanisms and tests, without measuring latency, memory or parallel scheduling.

## Evidence

[cache.rs](../../../../finstack-quant/portfolio/src/materialization/cache.rs), [report.rs](../../../../finstack-quant/portfolio/src/materialization/report.rs) and the [load caller](../../../../finstack-quant/portfolio/src/materialization/mod.rs). Inspected [materialization tests](../../../../finstack-quant/portfolio/tests/materialization.rs) cover 50 artifacts/5,000 holdings cold and warm, capacity/byte LRU eviction, retained instruments after eviction and concurrent same-key decode. Inline cache tests check version-axis separation and lock poisoning. [Benchmark contract tests](../../../../finstack-quant/portfolio/tests/materialization_benchmark_contract.rs) are a further evidence location, not a performance result from this audit.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
