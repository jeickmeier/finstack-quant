# Simplicity Auditor — Examples

These examples show the expected shape and level of specificity in a simplicity audit. Replace symbols/paths with real ones from the codebase under review.

## Example 1: Small redundancy cluster

```markdown
## Summary
- The pricing API has multiple overlapping entry points and wrapper layers; users don’t have a single obvious “happy path”.
- Risk level: Major (API churn), Low (behavior) if we lock behavior with golden tests first.

## Simplicity principles applied
- One obvious way per capability
- Bias to deletion; wrappers must add semantics

## Surface area inventory
pricing → `price()`, `price_with_config()`, `try_price()`, `price_v2()`

## Redundancy clusters
### Cluster: pricing
**Canonical API**: `price_with_config()` (`src/pricing/mod.rs`)

**Redundant pathways**
- `price()` (`src/pricing/mod.rs`) — forwards default config; recommend keep temporarily but deprecate if it causes ambiguity.
- `try_price()` (`src/pricing/mod.rs`) — duplicates validation; recommend merge validation into the canonical API (return `Result` if needed) and delete `try_*`.
- `price_v2()` (`src/pricing/v2.rs`) — divergent naming; recommend migrate into canonical API or rename module as internal implementation detail.

**Migration notes**
- Before: `price(trade, market)`
- After: `price_with_config(trade, market, PricingConfig::default())`

## Consolidation plan (PR-sized)
- PR 1: add golden tests for pricing results + error cases; route all functions through one internal implementation
- PR 2: deprecate `try_price` + `price_v2`; update call-sites and docs to point to the canonical API
- PR 3: delete deprecated functions; remove `v2` module export if no longer needed

## Scorecard (1–5)
- API simplicity: 2
- Redundancy level: 2
- Consistency: 3
- Maintainability: 2
- Ergonomics: 3

## Action items
- [ ] Make `price_with_config()` the only documented entry point for pricing
- [ ] Add golden tests before deleting `try_price()` / `price_v2()`
```

## Example 2: Duplication in validation/parsing

```markdown
## Summary
- Input validation is duplicated across multiple constructors and builders.
- Risk level: Minor behavior risk if we preserve error messages and edge-case handling.

## Surface area inventory
construction → `new()`, `try_new()`, `from_str()`, `builder().build()`

## Redundancy clusters
### Cluster: construction
**Canonical API**: `new()` (`src/types/foo.rs`) — canonical construction pathway (return `Result` if construction is fallible)

**Redundant pathways**
- `try_new()` (`src/types/foo.rs`) — naming-only duplicate; recommend collapse into `new()` and delete `try_new()` unless you truly need both infallible+fallible entry points.
- `from_str()` (`src/types/foo_parse.rs`) — duplicates parsing/validation; recommend route into `new()`.
- `builder().build()` (`src/types/foo_builder.rs`) — duplicates defaults + validation; recommend reduce builder to a thin config aggregator that calls `new()`.

## Consolidation plan (PR-sized)
- PR 1: extract `validate_foo_inputs(...)` as a private helper used by all constructors
- PR 2: make `builder().build()` call `new()`; remove duplicated validation blocks
- PR 3: document `new()` as the canonical construction pathway; remove `try_new()` if it doesn’t add semantic clarity

## Action items
- [ ] One validation helper, used everywhere
- [ ] One canonical constructor documented as “the way”
```
