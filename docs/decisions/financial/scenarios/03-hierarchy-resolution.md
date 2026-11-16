# SCENARIOS-003: Hierarchy expansion, namespace selection and overlap precedence

- Decision status: **Accepted 2026-11-15 — complete scenario hierarchy selection, overlap, active binding and evidence package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent overlap arithmetic and JavaScript order checks completed.** Production selection, rollback and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for typed scenario hierarchy expansion and overlap

1. **Resolve each selector once against the opening hierarchy.** Use the
   shared checked Rust resolver and exact paths. Apply all filter predicates
   to each node's own tags. A matching node selects its entire subtree,
   and a deeper matching node can refine specificity. An empty filter is
   equivalent to no filter. Emit each exact typed source once per selector
   in stable placement order. Use the selected path depth for an unfiltered
   selector and the deepest matching ancestor's depth for a filtered one.
   [Accepted core hierarchy rules](../core-market/hierarchy-selection.md#accepted-decision-typed-selection-and-explicit-ordered-overlap).

2. **Project to the declared family before applying precedence.** Validate
   every authored selector and its controls. Missing hierarchy, missing path
   and zero compatible structural matches fail, including for a zero-sized
   shock. A valid selector may have all its compatible candidates suppressed
   by deeper matches. Preserve suppression evidence. Look up live sources
   only for retained active targets.
   [Accepted specification](01-specification-and-composition.md#accepted-decision-for-checked-scenario-specifications-and-ordered-composition).

3. **Apply overlap per exact typed source.** Keep `MostSpecificWins` as
   the persisted default and retain every maximum-depth tie. `Cumulative`
   retains one action per supplied selector and source. Replace each
   selector's original slot with retained actions in stable placement
   order. Direct actions keep their original slots and compose with
   retained hierarchy actions. Shared ID text in different namespaces
   identifies distinct sources.
   [Accepted typed market identity](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).

4. **Bind retained targets at their execution point.** Carry exact kinds
   through dispatch, including price and unitless scalar kinds. Bind to the
   actual candidate market after conditioning or earlier actions. A missing,
   wrong-kind or unsupported retained source fails the transaction without
   expanding the hierarchy again. Delegate effect calculation to the
   source owner. Use dedicated commodity percentages and distinct native
   hazard and CDS replay operations.
   [Accepted scenario execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution).

5. **Preserve ordered selection evidence across hosts.** Carry the authored
   operation position, selector path and filter, exact target, matched
   path and depth, and retained or suppressed outcome. Preserve hierarchy
   order in arrays through the checked Rust loader and the existing Python
   and WASM routes. Record 017 owns the report and replay representation.
   [JSON array ordering](https://datatracker.ietf.org/doc/html/rfc8259#section-1) ·
   [Reporting record 017](17-reports-and-replay.md).

## Independent selection checks

- At level `100`, apply a broad `+10%`, a direct `+5%`, and tied deeper
  shocks of `+20%` and `−10%` in that order. `MostSpecificWins` gives
  `113.4` and `Cumulative` gives `124.74`. The direct action and both
  deeper actions run under either policy.
- Ordered pairs for hierarchy names `2`, `1` retain that order in
  JavaScript. Enumerating the corresponding numeric-looking object keys
  yields `1`, `2`.

## Observed choice and rationale at the audit revision

Observed choice: expand hierarchy selectors into ordinary direct operations before execution, preserving each original operation's position. Resolution keys include the market collection and curve kind, so the same textual ID in a spot and volatility collection is not one economic target.

## Domain and ordered algorithm

A scenario containing hierarchy operations requires an attached `MarketDataHierarchy`; otherwise execution errors before mutation. Each path/tag selector calls the shared hierarchy resolver with `MostSpecificWins` **within that selector**. Thus a curve reached through parent and child matches expands once at its deepest matched node, including under scenario-wide cumulative mode.

For each result, verify membership in the operation's typed collection: discount, forward, hazard, inflation, commodity price curve, price scalar, volatility surface or base correlation. Missing typed data emits `HierarchyResolvedIdSkipped`; zero resolver matches emits `HierarchyNoMatch`. The resolver's matched depth, not merely the selector path length, is retained.

For scenario `MostSpecificWins`, compute the largest matched depth per typed target across all expanded operations, then retain every expansion at that depth. Equal-depth operations both survive and compose in original order. `Cumulative` retains every expansion. Direct-target operations are always copied and are not suppressed by hierarchy specificity. There is no automatic economic netting between a direct operation and a hierarchy expansion.

Live expansion projects the declared family before binding and rejects
absent hierarchy, no applicable structural matches and missing or
wrong-kind retained targets. It preserves typed targets for active checks,
while some downstream actions lose their scalar subkind and the expansion
output omits matched paths, authored slots and suppression evidence.

## Audit-revision limits and alternatives

Hierarchy expansion uses the opening market before the time roll. A later market roll may fail, drop expired data or change composites, but hierarchy resolution is not rerun. A skipped target is a warning rather than an atomic failure. The engine does not validate that a hierarchy exhaustively covers a portfolio's dependencies.

Alternatives include rejecting equal-depth conflicts, explicit precedence between direct and hierarchy targets, or requiring every selector to match. These would change how broad stresses and overrides combine. The observed rules favor stable expansion and permit intentional repeated shocks; this is inferred design rationale, not an adoption decision.

## Evidence

Owning source: [engine/hierarchy.rs](../../../../finstack-quant/scenarios/src/engine/hierarchy.rs), [spec.rs](../../../../finstack-quant/scenarios/src/spec.rs). The shared path/tag resolver is recorded in [market hierarchy selection](../core-market/hierarchy-selection.md). [Hierarchy tests](../../../../finstack-quant/scenarios/tests/hierarchy_targeting/mod.rs) locate assertions for parent/child tag deduplication, cumulative stacking, deepest matched-node precedence and namespace-qualified overlap. These establish targeted selection behavior rather than market stress severity.

## Relationship to the accepted typed hierarchy package

[CORE-MARKET-21](../core-market/hierarchy-selection.md#accepted-decision-typed-selection-and-explicit-ordered-overlap)
owns the shared hierarchy tree, resolver and transport. The scenario
adapter uses them inside the phase order and owned-state transaction of
[SCENARIOS-002](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution).

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios).
