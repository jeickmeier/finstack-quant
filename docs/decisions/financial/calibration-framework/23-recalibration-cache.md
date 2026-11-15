# CALIBRATION-FRAMEWORK-023: Content-keyed recalibration provider and cache lifetime

- Decision status: **Accepted 2026-11-15 — complete request/state-local cache identity, concurrency, retention and qualification package**.
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent key examples and isolated checks of the live memo helper verified.** Production replay, cache equivalence, concurrency and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for exact request-local recalibration caching

1. **Own caches by request and immutable market state.** Create fresh
   rate and hazard caches for each registered evaluation state. Cloned
   pricing options may share them within that state and request. End
   retention when that scope ends. Reusable providers, scenario engines
   and horizon analyzers obtain caches from the active execution.
   Use this same ownership from existing Python and WASM entry points.
   [Accepted portfolio execution](../../architecture/0046-portfolio-execution-operational-contract.md#decision-and-scope) ·
   [Accepted isolated state](../../architecture/0022-portfolio-evaluation-state.md#decision).

2. **Key source qualification separately from target replay.** Bind
   the original evaluator and construction record, source snapshot,
   typed dependencies, effective conventions and calendars, dates
   and information cut, consumed fixings and seeds, and effective
   fit and reproduction policy. The target key adds the explicit
   target state, action, exact selectors and represented moves,
   applicable recovery or deal override, horizon and coverage policy,
   and requested output and report scope. A complete-market result
   also binds the entire target snapshot carried into its output.
   [Accepted replay provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification) ·
   [Accepted rate replay](19-rate-quote-replay.md#accepted-decision-for-exact-rate-quote-replay-and-publication) ·
   [Accepted hazard replay](21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication) ·
   [Accepted horizon replay](22-hazard-deal-and-horizon.md#accepted-decision-for-deal-overrides-and-horizon-hazard-replay).

3. **Validate first and require exact identity.** Preserve action
   discriminants, meaningful optional fields, exact represented
   values and semantic ordering. Original-form zero replay and the
   zero-shock par-risk center have distinct identities. Use versioned
   canonical content hashes to accelerate lookup and require exact
   canonical equality or a verified immutable identity for reuse.
   Propagate invalid inputs and key-construction failures at their
   actual stage, including at zero.
   [Canonical Rust serialization](../../../../finstack-quant/core/src/canonical.rs) ·
   [Rust hash and equality contract](https://doc.rust-lang.org/std/hash/trait.Hash.html#hash-and-eq).

4. **Share one active attempt per admitted key.** Keep the map lock
   out of financial work and allow unrelated keys to proceed
   independently. Retain complete accepted results with their
   evidence. Give callers waiting on a failed attempt its original
   stage error, retire that slot and admit later retries. Release
   waiters on aborted work and verify bookkeeping before reusing
   poisoned state.
   [Rust mutex poisoning](https://doc.rust-lang.org/std/sync/struct.Mutex.html#poisoning).

5. **Bound retention independently of financial acceptance.** Bound
   active and completed entry counts across the source and target
   caches, and bound retained storage. Keep admitted successes
   until the scope ends. At capacity execute new misses through the
   same checked replay without retention. Return an accepted oversized
   result to its callers and release the slot. Report actual work and
   cache reuse separately from the original fit evidence.

6. **Qualify reuse on the shared Rust path.** Compare delivered
   evaluators, reports and typed failures with caching enabled and
   disabled. Exercise changed snapshots, conventions, fixings, actions
   and tolerances, forced hash collisions, concurrent success and
   failure, capacity exhaustion, and teardown across states and
   requests. Use the same path for valuation, scenarios and hosts.

## Independent checks

- On one million notional, an admitted one-day ACT/360 fixing move
  from 4% to 5% changes coupon cash by `250/9`. Curve and quote
  identities can agree across both requests.
- A horizon integrated-hazard error of .005 passes a .01 tolerance
  and fails a .001 tolerance under the same delivered numerical fit.
- An isolated harness importing the current memo helper verified
  one computation and one immutable result across eight simultaneous
  same-key callers, progress on another key during a blocked
  calculation, and successful retry after a failure. A thousand
  distinct computed failures left a thousand empty slots.

## Observed choice and rationale at the audit revision

Recalibration is supplied to valuation through an explicit provider and caches completed results for repeated identical requests. Content keys avoid treating a curve ID as sufficient identity. This is a computational reuse choice whose correctness depends on complete dependency identity.

## Observed key construction and concurrency

Rate market keys include canonical source-market hash, linked-discount/forward or single-OIS IDs, and exact floating-point bump bits (including tenor vector order). Discount-only keys additionally include source curve and explicit recipe hashes. Hazard keys include hazard ID, discount ID, target discount fingerprint, optional identity-source discount fingerprint, recovery bits, requested documentation/valuation convention, bump identity and whole source hazard content hash. The source fingerprint includes its calibration/risk recipes and FX policy. Parallel/tenor/exact-index bumps are distinct variants.

The map is protected by a mutex, with one separately locked entry per key. Release map lock before computing; hold per-entry lock through calculation so concurrent same-key calls share one successful result. Different keys can compute concurrently. Poisoned locks recover their inner state. Errors propagate and leave an empty entry, so later calls retry; successful values are immutable `Arc`s. There is no eviction, size cap or automatic cache reset; provider lifetime owns memory retention.

Provider dispatch validates bump/recovery domains, optionally prepares a deal quote override, and selects linked/single rate replay, hazard spread/exact-index/risk-center/dependency/recovery/horizon action. Several hazard action results bypass the cache.

In the live source, hazard source qualification and target results have
separate maps. Their dependency fingerprint includes the discount curve
and an optional same-ID hazard seed. Spread replay compares this tuple
by content when deciding whether to pass a distinct identity context.

## Observed limits and alternatives

Hazard caching assumes discount curve plus source hazard recipe captures all pricing dependencies. Global convention registry/calendar versions are not explicit key fields; the runtime's registry mutability and reproducibility policy must support that assumption. Exact-bit and vector-order keys distinguish numerically equivalent shocks, trading hit rate for unambiguous identity. The mutex serializes expensive same-key errors too, but failed computations are retried without shared error caching.

Alternatives include bounded LRU/request-scoped caches or dependency-version keys, adding memory/control complexity. Existing source-change tests establish selected identity dimensions; they are not exhaustive dependency completeness or concurrency stress evidence.

## Source and validation evidence

- Implementation: [recalibration/cache.rs](../../../../finstack-quant/calibration/src/recalibration/cache.rs) — per-key mutex, successful-only retention and poison recovery.
- Implementation: [recalibration/provider.rs](../../../../finstack-quant/calibration/src/recalibration/provider.rs) — explicit pricing-provider construction and action dispatch.
- Implementation: [recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — rate market/curve/recipe keys.
- Implementation: [recalibration/hazard.rs](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) — hazard/source/discount/risk-input keys.
- Test evidence: [src/recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — identical reuse, changed source markets, explicit recipes and source fixings; inspected or located, not executed.
- Test evidence: [src/recalibration/hazard.rs](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) — FX policy and deal-risk-input identity regressions; inspected or located, not executed.
- The independent checks cover selected financial key inputs and the
  isolated memo helper. Complete-path replay, bounded retention,
  collision handling and host concurrency require qualification.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
