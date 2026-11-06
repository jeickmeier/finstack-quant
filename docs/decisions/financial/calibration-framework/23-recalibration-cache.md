# CALIBRATION-FRAMEWORK-023: Content-keyed recalibration provider and cache lifetime

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

Recalibration is supplied to valuation through an explicit provider and caches completed results for repeated identical requests. Content keys avoid treating a curve ID as sufficient identity. This is a computational reuse choice whose correctness depends on complete dependency identity.

## Key construction and concurrency

Rate market keys include canonical source-market hash, linked-discount/forward or single-OIS IDs, and exact floating-point bump bits (including tenor vector order). Discount-only keys additionally include source curve and explicit recipe hashes. Hazard keys include hazard ID, discount ID, target discount fingerprint, optional identity-source discount fingerprint, recovery bits, requested documentation/valuation convention, bump identity and whole source hazard content hash. The source fingerprint includes its calibration/risk recipes and FX policy. Parallel/tenor/exact-index bumps are distinct variants.

The map is protected by a mutex, with one separately locked entry per key. Release map lock before computing; hold per-entry lock through calculation so concurrent same-key calls share one successful result. Different keys can compute concurrently. Poisoned locks recover their inner state. Errors propagate and leave an empty entry, so later calls retry; successful values are immutable `Arc`s. There is no eviction, size cap or automatic cache reset; provider lifetime owns memory retention.

Provider dispatch validates bump/recovery domains, optionally prepares a deal quote override, and selects linked/single rate replay, hazard spread/exact-index/risk-center/dependency/recovery/horizon action. Several hazard action results are intentionally not cached. Target discount pointer inequality triggers source identity-context use for a spread request; key identity itself uses content fingerprints.

## Limits and alternatives

Hazard caching assumes discount curve plus source hazard recipe captures all pricing dependencies. Global convention registry/calendar versions are not explicit key fields; the runtime's registry mutability and reproducibility policy must support that assumption. Exact-bit and vector-order keys distinguish numerically equivalent shocks, trading hit rate for unambiguous identity. The mutex serializes expensive same-key errors too, but failed computations are retried without shared error caching.

Alternatives include bounded LRU/request-scoped caches or dependency-version keys, adding memory/control complexity. Existing source-change tests establish selected identity dimensions; they are not exhaustive dependency completeness or concurrency stress evidence.

## Source and validation evidence

- Implementation: [recalibration/cache.rs](../../../../finstack-quant/calibration/src/recalibration/cache.rs) — per-key mutex, successful-only retention and poison recovery.
- Implementation: [recalibration/provider.rs](../../../../finstack-quant/calibration/src/recalibration/provider.rs) — explicit pricing-provider construction and action dispatch.
- Implementation: [recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — rate market/curve/recipe keys.
- Implementation: [recalibration/hazard.rs](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) — hazard/source/discount/risk-input keys.
- Test evidence: [src/recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — identical reuse, changed source markets, explicit recipes and source fixings; inspected or located, not executed.
- Test evidence: [src/recalibration/hazard.rs](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) — FX policy and deal-risk-input identity regressions; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
