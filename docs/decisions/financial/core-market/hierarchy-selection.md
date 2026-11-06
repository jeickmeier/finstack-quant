# CORE-MARKET-21: Hierarchical scenario targeting and overlap resolution

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

The market-data hierarchy resolves scenario targets by path and optional local tags. `MostSpecificWins` is the default resolution mode; `Cumulative` deliberately retains repeated effective matches. This can change how many shocks a curve receives, so it is a calculation-selection policy.

## Algorithm and domain

Builder paths split on `/` and reject empty input/empty segments. Invalid path records the first error and suppresses subsequent tagging/curve assignment until another valid node is selected; build returns that error. Nodes preserve insertion order. Validation recursively rejects empty node names and any stored curve ID appearing more than once anywhere in the tree, including duplicates at the same node. It does not verify that those IDs exist in a MarketContext.

Resolve first locates the exact target path; missing path returns an empty result. With no tag filter, all curves in its subtree receive the target path's matched depth. With a filter, each visited node is tested using **its own tags**, with all predicates ANDed. Equals compares exact strings, In tests membership, Exists tests presence, and an empty predicate list matches every node. When a node matches, its entire subtree is collected with that node's depth; traversal still continues into children and may collect those descendants again when another node matches.

Cumulative returns every collected occurrence, including duplicates caused by overlapping matching ancestor/descendant subtrees. MostSpecificWins computes maximum matched depth per curve, retains only occurrences at that depth, then emits one instance per curve in traversal order. It does not prefer a particular tag value beyond the filter or apply a shock itself; the returned `matched_depth` supports caller resolution.

## Limitations, alternatives

Stored uniqueness and effective-match uniqueness are different: a valid tree can produce multiple effective matches under Cumulative. Tags are not merged into an inherited map, though a matching parent's subtree inclusion has an inheritance-like effect. A missing path is silently empty rather than a configuration error. Direct serialized node names are checked for emptiness but are not parsed as slash paths by that validation.

Strict missing-target errors, inherited-tag evaluation, or cumulative deduplication would change behavior. Both resolution modes are supported; scenario authors must choose intentionally.

## Evidence

Owning code: [builder](../../../../finstack-quant/core/src/market_data/hierarchy/builder.rs), [structure/validation](../../../../finstack-quant/core/src/market_data/hierarchy/mod.rs), [resolution kernel](../../../../finstack-quant/core/src/market_data/hierarchy/resolution.rs). Inspected [hierarchy tests](../../../../finstack-quant/core/tests/market_data/hierarchy.rs) assert a curve collected from both Credit and Credit/US appears twice under Cumulative but once under MostSpecificWins; other located cases cover parent subtree matching, predicates, duplicate stored IDs and invalid paths. This is selection-contract evidence; no external financial standard mandates this precedence. Downstream scenario composition must preserve the selected semantics.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
