# FACTOR-003: Ordered mapping, hierarchy, and cascade dispatch

**Decision status:** Accepted 2026-11-17 for ordered matching, hierarchy and cascade dispatch.

**Implementation status:** Authored rule order, deepest-node selection and cascade error propagation are implemented. Checked rulebook admission, traversal bounds and assignment provenance remain pending.

**Validation status:** All 45 focused matching tests passed on 2026-11-17 against current behavior. The accepted provenance, rulebook bounds and qualified dependency inventory coverage still need verification.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Select the first authored matching table rule. Combine supplied dependency and instrument-attribute predicates with AND. Match exact identifiers and preserve FX pair direction. Preserve authored priority for exact and wildcard rules. Record the selected rule and flag overlaps observed when qualifying the rulebook against its intended dependency inventory.
2. Prune a hierarchy branch when a parent predicate fails. Select the deepest eligible factor-bearing node, with authored sibling order breaking ties. Retain the selected node's path in assignment evidence.
3. Evaluate cascades in authored order. Continue after no match or an empty result; return the first nonempty result. Propagate errors immediately. Keep no-match distinct from structural zero. Apply Strict or Warn under the [accepted assignment coverage contract](../portfolio-attribution/12-factor-assignment-and-credit-exposures.md#accepted-decision-for-checked-factor-assignment-and-credit-exposures).
4. Validate the declared factor references and enforce depth and size limits before recursive compilation or traversal of typed or restored rulebooks. [FACTOR-004](credit-matching.md) owns credit issuer selection and proxy policy.

## Observed choice and rationale

Select the first matching mapping rule, the deepest matching hierarchy node, or the first nonempty cascade result. This is observed deterministic dispatch, not an optimization or automatic economic classification.

## Input domain and conventions

Dependency filters combine optional dependency type, curve type, and exact ID with AND. Attribute filters require every tag and exact metadata key/value; empty filters match all. FX dependency IDs are directional `BASE/QUOTE` strings. A selected simple mapping emits one factor with beta 1.

## Algorithm stages

1. Mapping tables index exact IDs and wildcard rules, but retain original rule indices. Choose the earliest matching exact-ID candidate, then allow an earlier matching wildcard to replace it. Exact IDs do not automatically have precedence over earlier wildcards.
2. A hierarchical matcher first checks its dependency filter, recursively prunes a node whose attributes fail, and selects the deepest matching factor-bearing descendant. Equal depths retain the first child traversal result.
3. A cascade evaluates configured matchers in order. `None` and empty results continue; the first nonempty result wins. Any matcher error immediately propagates and prevents later fallback.
4. Factor-ID enumeration traverses all rules/nodes/cascade members and deduplicates into sorted order for configuration validation.

## Selection and controls

`MatchingConfig` chooses one of MappingTable, Hierarchical, Cascade, or the credit hierarchy record. Construction itself is infallible and does not semantically validate the enclosing factor universe. The test-only helper at matchers.rs:105 precedes production code; production rules and dispatch below that marker were included in the audit.

## Limitations, failures, and fallbacks

Deep recursive hierarchy construction/traversal has no local explicit depth budget. Rule ambiguity is resolved by order rather than rejected. An empty result is operationally unmatched; the portfolio unmatched policy is separate. A more-specific filter in a later rule can be masked by an earlier broad filter.

## Alternatives and tradeoffs

Authored order makes precedence explicit, including intentional overlaps. Observed overlap diagnostics and selected-path evidence let rulebook owners verify coverage against the actual dependency inventory.

## Implementation and evidence

- [src/matching/filter.rs](../../../../finstack-quant/factor/src/matching/filter.rs) — inspected implementation.
- [src/matching/config.rs](../../../../finstack-quant/factor/src/matching/config.rs) — inspected implementation.
- [src/matching/matchers.rs](../../../../finstack-quant/factor/src/matching/matchers.rs) — inspected implementation.
- [src/primitives/dependency.rs](../../../../finstack-quant/factor/src/primitives/dependency.rs) — inspected implementation.

[Matcher](../../../../finstack-quant/factor/src/matching/matchers.rs) and [filter](../../../../finstack-quant/factor/src/matching/filter.rs) tests passed, including wildcard and exact precedence, hierarchy depth and ties, cascade order, and exact filters. Qualify bounded construction, preserved order on restore, evidence of the winning path and intended rulebook coverage.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
