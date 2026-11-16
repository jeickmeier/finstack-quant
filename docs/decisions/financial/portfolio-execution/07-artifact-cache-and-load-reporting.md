# PORTFOLIO-EXECUTION-007 — Bounded artifact decode cache and diagnostic timing

- **Decision status:** Accepted 2026-11-15 — complete immutable decode cache, checked LRU, synchronized misses, exact counts and phase timing package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent eviction, same-key concurrency, alias count and timing examples verified. Production concurrency, memory behavior and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for bounded portfolio decode caching and load reporting

1. **Retain only caller-owned immutable decoded inputs.** Cache
   validated instrument definitions and typed dependency declarations.
   Key by verified `c2` digest, nested instrument contract version
   and canonicalization version under the same decoder authority.
   Check each bundle entry's claims on every load, including
   hits. An omitted cache selects a per-call default.
   Market state, PV and risk remain with their request-local owners.
   [Materialization admission](06-materialization-validation.md#accepted-decision-for-checked-portfolio-materialization-and-validation) ·
   [Cache ownership](../../architecture/0046-portfolio-execution-operational-contract.md#decision-and-scope).

2. **Use checked LRU retention budgets.** Keep 4,096 entries and
   64 MiB of canonical encoded envelope bytes as defaults. Charge
   each retained content once. Evict least-recently-used entries
   until both limits fit. Zero limits and oversized artifacts
   bypass retention without eviction. Previously returned instruments
   remain usable. The byte budget covers retained encoded content.
   Heap, outstanding references and temporaries are measured separately.

3. **Keep synchronized decoding for retainable misses.** Same-key
   callers share a successful decode while resident. Retainable
   misses for unrelated keys serialize. Bypass decodes execute
   outside the retention lock. A failed decode preserves its typed
   cause, leaves no entry and can be retried. Independent-key
   concurrency requires representative measurement.
   [Measured performance policy](../../standards/0040-measured-performance-decisions.md#decision).

4. **Recover poisoning from known-good metadata.** Under exclusive
   access discard suspect entries, LRU order and byte accounting
   before clearing poison. Previously returned references survive.
   Use checked retention arithmetic. Keep cumulative successful
   decode telemetry separate and report it as unavailable on overflow.

5. **Count admitted content separately from work.** Report submitted
   artifact entries, distinct admitted contents, holdings, exact
   input UTF-8 bytes and typed declarations counted once per unique
   content. Count shared-cache hits and successful decodes at each
   distinct content's first resolution in a load. Same-load aliases
   reuse the admitted result without adding a shared-cache hit.

6. **Distinguish measured, unavailable and skipped durations.** Use
   checked monotonic elapsed time and preserve host clock resolution.
   Equal readings give measured zero. Missing, regressing or
   unrepresentable readings give unavailable timing. Validation-only
   skips final construction and indexing. Artifact resolution
   includes cache access and wait, so its elapsed time measures
   more than decoder CPU alone. Timing never changes load admission.

7. **Preserve exact reports across hosts.** Rust owns checked
   counters, phase status and diagnostics. Use lossless integer
   transport in Python and WASM, including full-width nanoseconds.
   Qualify cold and warm loads against the same semantic admission.
   Benchmark throughput separately under controlled conditions.
   [Core count and wire rules](../core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity).

## Independent cache and timing checks

- With two entries and ten bytes, contents A=4 and B=6 fill
  capacity. Touching A makes B oldest. An eleven-byte bypass
  leaves both in place. Loading C=5 evicts B. The cache then
  holds A and C at nine bytes.
- Eight concurrent requests for one retainable key in the
  independent synchronized model give one decode and seven hits.
  Two producer IDs for the same instrument with two holdings and
  three typed declarations give two entries, one content, two
  holdings and three declarations. Cold resolution gives zero
  hits and one decode; warm gives one hit and zero decodes.
- Clock readings of 100 and 100 ms give measured zero. Readings
  of 100 and 100.25 ms give 250,000 ns. A regressing or
  unrepresentable reading gives unavailable timing. An omitted
  build in validation-only is skipped.

## Observed choice and scope at the audit revision

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

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
