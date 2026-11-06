# PORTFOLIO-EXECUTION-004 — Request identities, deterministic execution and valuation reduction

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and rationale

A request owns separate immutable market and portfolio states and deduplicates exact evaluation jobs. Execution chooses one portfolio-level parallel axis, then preserves input order for errors and reduction. These mechanisms implement portions of accepted [ADR 0022](../../architecture/0022-portfolio-evaluation-state.md), [ADR 0046](../../architecture/0046-portfolio-execution-operational-contract.md) and [INVARIANTS 2.6](../../../../INVARIANTS.md#26-portfolio-evaluation-execution). Acceptance of the execution policy does not adopt every observed threshold or certify this record's implementation and validation coverage.

## Algorithm and selection

Every registration creates a fresh numeric state ID, including a repeated reference to the same object. State registration does not content-deduplicate markets. A prepared market owns or borrows the snapshot and effective date and creates fresh pricing options with caller config and a recalibration cache. Clones of those options can share within that market state.

An evaluation key is `(market-state ID, portfolio-state ID, exact normalized metric profile and risk policy)`. Metric order is significant. A repeated exact key returns the existing job; requested execution mode and selective seed are not part of that key, so the first registration determines its execution. Jobs produce indexed per-job `Result<Arc<PortfolioValuation>>`; failure in one job does not prevent scheduling the remaining jobs. Consumers scan results in logical job order.

Automatic position execution is serial below 64 positions. At 64 or more it uses ordered parallel mapping. Selective execution uses actual dirty count, or all positions when FX conversion refresh is required; it parallelizes only when work is at least 64 and `4 * work >= position_count`. A plan with at least eight jobs and every portfolio at most 63 positions parallelizes states and forces each position loop serial. Otherwise it runs states in order with each job's position policy. WASM does not use the native state-parallel branch. Shared cashflow/metric/primitive collection uses the same 64-item threshold helper.

The ordered position vector is reduced serially. Position results enter `IndexMap` in portfolio order; entities appear on first encounter. Each entity's base PV amounts are Neumaier-summed into Money, then entity amounts are Neumaier-summed into portfolio Money. All stored native/base PVs already passed Money construction. There is no currency-minor-unit rounding in `Money::new`; these are f64 projection/construction boundaries. A different entity partition can still alter floating-point reduction grouping.

## Raw endpoint variant and limits

Factor-stress endpoints use a financially distinct raw executor returning unscaled base-currency f64 PVs, rejecting non-finite endpoints. It uses the same position scheduler. Its optional seed is vector-index based and has no own profile/instrument provenance check; its internal caller must establish identity, dirty indices and unchanged conversion semantics. It falls back to repricing when the prior endpoint index is absent. The higher-level [factor stress record](../portfolio-attribution/14-factor-stress-and-position-what-if.md) owns that proof.

IDs use `u32::try_from(len).unwrap_or(u32::MAX)` rather than an explicit overflow error; the practical memory bound normally precedes this, but the implementation does not enforce a formal maximum job count. No benchmark in this audit establishes 64/eight/25% as optimal thresholds. Any performance change must preserve financial identity and logical error/reduction order; completion-order reducers or nested parallel fan-out are not approved alternatives under current invariants.

## Evidence and its limits

[plan.rs](../../../../finstack-quant/portfolio/src/evaluation/plan.rs), [executor.rs](../../../../finstack-quant/portfolio/src/evaluation/executor.rs) and [state.rs](../../../../finstack-quant/portfolio/src/evaluation/state.rs) are implementation evidence. Inline plan tests locate exact-job deduplication and strict/best-effort separation. Inspected [selective tests](../../../../finstack-quant/portfolio/tests/selective_repricing.rs) compare 63/64 dirty counts and full-pricing results; [numerical stability tests](../../../../finstack-quant/portfolio/tests/numerical_stability.rs) locate compensated portfolio totals. These are deterministic/invariant comparisons, not independent valuation benchmarks. [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) and [Kahan](../../../REFERENCES.md#kahan-1965) are conceptual numerical references; no thread-count/platform execution was performed in this audit.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
