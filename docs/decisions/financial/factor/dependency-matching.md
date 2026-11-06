# FACTOR-003: Ordered mapping, hierarchy, and cascade dispatch

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

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

Priority/specificity scoring could remove order dependence but adds another policy; rejecting overlapping rules would strengthen control but require formal overlap semantics. The current ordered rule system is simple and explainable if rule order is preserved in persisted configs.

## Implementation and evidence

- [src/matching/filter.rs](../../../../finstack-quant/factor/src/matching/filter.rs) — inspected implementation.
- [src/matching/config.rs](../../../../finstack-quant/factor/src/matching/config.rs) — inspected implementation.
- [src/matching/matchers.rs](../../../../finstack-quant/factor/src/matching/matchers.rs) — inspected implementation.
- [src/primitives/dependency.rs](../../../../finstack-quant/factor/src/primitives/dependency.rs) — inspected implementation.

[src/matching/matchers.rs](../../../../finstack-quant/factor/src/matching/matchers.rs) contains tests for wildcard/exact precedence, deepest hierarchy selection, sibling ties, cascade order and error behavior. [src/matching/filter.rs](../../../../finstack-quant/factor/src/matching/filter.rs) tests exact typed filters. These are dispatch-contract checks; independent financial evidence must establish that a given rulebook maps the intended risk.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
