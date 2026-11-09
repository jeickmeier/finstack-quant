# SCENARIOS-003: Hierarchy expansion, namespace selection and overlap precedence

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: expand hierarchy selectors into ordinary direct operations before execution, preserving each original operation's position. Resolution keys include the market collection and curve kind, so the same textual ID in a spot and volatility collection is not one economic target.

## Domain and ordered algorithm

A scenario containing hierarchy operations requires an attached `MarketDataHierarchy`; otherwise execution errors before mutation. Each path/tag selector calls the shared hierarchy resolver with `MostSpecificWins` **within that selector**. Thus a curve reached through parent and child matches expands once at its deepest matched node, including under scenario-wide cumulative mode.

For each result, verify membership in the operation's typed collection: discount, forward, hazard, inflation, commodity price curve, price scalar, volatility surface or base correlation. Missing typed data emits `HierarchyResolvedIdSkipped`; zero resolver matches emits `HierarchyNoMatch`. The resolver's matched depth, not merely the selector path length, is retained.

For scenario `MostSpecificWins`, compute the largest matched depth per typed target across all expanded operations, then retain every expansion at that depth. Equal-depth operations both survive and compose in original order. `Cumulative` retains every expansion. Direct-target operations are always copied and are not suppressed by hierarchy specificity. There is no automatic economic netting between a direct operation and a hierarchy expansion.

## Limits and alternatives

Hierarchy expansion uses the opening market before the time roll. A later market roll may fail, drop expired data or change composites, but hierarchy resolution is not rerun. A skipped target is a warning rather than an atomic failure. The engine does not validate that a hierarchy exhaustively covers a portfolio's dependencies.

Alternatives include rejecting equal-depth conflicts, explicit precedence between direct and hierarchy targets, or requiring every selector to match. These would change how broad stresses and overrides combine. The observed rules favor stable expansion and permit intentional repeated shocks; this is inferred design rationale, not an adoption decision.

## Evidence

Owning source: [engine/hierarchy.rs](../../../../finstack-quant/scenarios/src/engine/hierarchy.rs), [spec.rs](../../../../finstack-quant/scenarios/src/spec.rs). The shared path/tag resolver is recorded in [market hierarchy selection](../core-market/hierarchy-selection.md). [Hierarchy tests](../../../../finstack-quant/scenarios/tests/hierarchy_targeting/mod.rs) locate assertions for parent/child tag deduplication, cumulative stacking, deepest matched-node precedence and namespace-qualified overlap. These establish targeted selection behavior rather than market stress severity.

## Relationship to the accepted typed hierarchy package

[CORE-MARKET-21's accepted package](../core-market/hierarchy-selection.md#accepted-decision-typed-selection-and-explicit-ordered-overlap)
was accepted on **2026-11-08**. It preserves authored equal-depth/direct action order and
per-selector deduplication, while replacing bare associations with accepted20's
exact typed targets, stabilizing placement order and normalizing empty filters.
Declared family projection precedes strict retained-active source binding;
missing paths/zero applicable matches or unavailable selected sources error
rather than warn/skip. Mixed effective composition modes error instead of
silently becoming Cumulative. Selection membership freezes once, with actual
execution-stage admission and context-owned atomicity under accepted20.
Ordered name/node arrays and shared strict loading preserve hierarchy transport;
accepted21 does not adopt broader scenario phase ordering, instrument/
statement atomicity, source dynamics, replay or financial stress calibration.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
