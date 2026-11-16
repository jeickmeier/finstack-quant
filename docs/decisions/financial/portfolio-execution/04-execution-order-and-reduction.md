# PORTFOLIO-EXECUTION-004 — Request identities, deterministic execution and valuation reduction

- **Decision status:** Accepted 2026-11-15 — complete request identities, one-axis execution, error order, Money NAV and raw endpoint package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent scheduling boundary, failure-order, seed and NAV precision examples verified. Production execution, thread-count behavior and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio execution and NAV reduction

1. **Register immutable states with checked request-local IDs.**
   Register market snapshots and effective dates separately from
   ordered portfolio holdings. Each registration earns a fresh ID.
   Reuse an existing registration by passing its handle. Reject
   unknown or cross-request handles and ID exhaustion. Bind the
   selected provider and settings. Rate and hazard caches are local
   to the registered market state and request.
   [Recalibration cache contract](../calibration-framework/23-recalibration-cache.md).

2. **Coalesce only identical financial requests.** Use the registered
   states, ordered metric menu, risk policy, currency and FX basis,
   and effective valuation controls. Reject conflicting explicit
   scheduling constraints. Apply record 005 reuse checks before
   coalescing. Share identical admitted seeds. Differing seeds or an
   explicit requirement for fresh work select fresh valuation,
   independently of registration order.
   [Accepted valuation profiles](03-valuation-profiles-and-failures.md#accepted-decision-for-checked-portfolio-valuation-profiles-and-failures) ·
   [Reuse record 005](05-dependencies-and-selective-reuse.md).

3. **Use at most one portfolio parallel axis per wave.** Choose states,
   positions or factors. State and factor workers use serial position
   loops. Explicit controls take precedence over the automatic choice.
   Keep the current private heuristics: full position work becomes
   eligible at 64 positions, selective at 64 or more positions and at
   least 25% of the book, and state work at eight or more jobs each with
   fewer than 64 positions. A required FX refresh counts every position.
   WASM uses the same calculation and ordering on its serial path.
   [Accepted executor](../../architecture/0046-portfolio-execution-operational-contract.md#decision-and-scope).

4. **Select failures in logical order.** Collect indexed outcomes.
   Scan jobs and holdings in their registered order and preserve the
   metric owner's dependency order within each holding. A failed job
   leaves other independent outcomes available to the calling workflow.
   [Reproducibility guarantees](../../standards/0021-reproducibility-guarantees.md#decision).

5. **Reduce headline NAV directly in checked base Money.** Fold
   qualified holding rows in portfolio order. Compute entity subtotals
   independently in each entity's holding order, emitting entities on
   first encounter. Empty NAV is base-currency zero. Currency or
   representability failure aborts assembly. Book and entity
   reconciliations require compatible reduction and rounding rules.
   ADR 0046 and the execution invariants scope Neumaier to
   binary64 reductions.
   [Monetary arithmetic boundary](../conventions/0020-numerical-representation-and-rounding.md) ·
   [Book rollups](02-book-hierarchy-and-rollups.md#accepted-decision-for-checked-book-hierarchy-and-nav-rollups).

6. **Use the shared executor for qualified raw factor endpoints.**
   Require finite, unscaled raw PVs from the selected model and
   effective state. Apply the elected reporting FX query directly
   to the raw endpoint. Remove the Money round trip and fail if
   qualified raw pricing is unavailable. Prove holding, state,
   profile and FX compatibility before reuse. The factor owner
   applies the signed holding scale once to the endpoint difference
   and uses checked ordered Neumaier accumulation.
   [Accepted raw repricing basis](../valuations-framework/pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle).

## Independent calculation and policy checks

- Full work at 64 positions qualifies for the current automatic
  position threshold and 63 does not. At 64 selective positions,
  256 total qualifies and 257 total does not. Eight jobs of
  63 positions qualify for the automatic state threshold.
- In all tested completion orders, a failure at job 0, holding 1
  precedes a failure at job 1, holding 0. Identical admitted
  seed hints can be shared. Conflicting hints in either order
  select fresh valuation.
- USD `9007199254740993` and USD `−9007199254740992` net
  to USD 1 in Money and zero after binary64 projection. Raw
  endpoints `1e−30` and `2e−30` have a nonzero binary64
  difference outside the Money input scale.

## Observed choice and rationale at the audit revision

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

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
