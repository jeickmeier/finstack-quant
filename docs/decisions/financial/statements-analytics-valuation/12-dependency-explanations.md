# STATEMENTS-ANALYTICS-VALUATION-12: Dependency traversal and provenance-aware formula explanations

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice and selection

[Introspection](../../../../finstack-quant/statements-analytics/src/analysis/introspection.rs) inspects the statement dependency graph and already evaluated values. It neither reprices the model nor validates its numerical outputs. Direct dependencies and dependents come from the graph; transitive dependencies use depth-first traversal with visited tracking and insertion-ordered unique results, placing deeper dependencies before their parent references.

## Algorithms and result semantics

- Dependency-tree construction recursively expands references with path-cycle markers rather than recursing indefinitely. Shared dependencies can appear on multiple tree branches; tree node count counts those appearances and marker nodes, not unique economic drivers. Text rendering preserves branch rails and optionally formats period values to two decimals.
- Explaining a cell first requires the node and evaluated period value. The result retains node type, final value and available source provenance. Forecast cells expose their forecast specification; formula breakdown is built only for formula source, or where provenance is absent and formula text is available. Thus an absent provenance record can cause a formula explanation to be assumed.
- Parse formula syntax. If the entire expression consists of node/capital-structure references joined by addition/subtraction, recursively preserve signed reference order and report each component with its operation only if all component values are available. If any is missing, the entire expression falls back to the unsigned reference-list path. A complete signed trace can reconcile an additive expression, including repeated terms.
- For any nonlinear/unsupported expression, fall back to distinct referenced nodes and capital-structure identifiers with available **current-period** values. No signed attribution is claimed. A lag/function dependency list is not an explanation of its time-shifted contribution. Missing component values are omitted; capital-structure lookup errors are converted to unavailable components.
- Final values and component values use explicit nonfinite wire representation so an explanation can preserve an anomalous evaluated result. Formatting an explanation is not a successful finite-value check.

## Limits, alternatives and evidence

Dependency traversal gives structural reachability, not economic materiality or sensitivity. Nonlinear component lists cannot explain product/ratio interactions, and current-period components can differ from the observations actually used by lagged formulas. Missing provenance and omitted component errors weaken replay auditability. These are observed output semantics, not unperformed algorithm inspection.

Colocated direct/transitive/tree/dependent cases and ASCII rendering tests check structural mechanics. `test_explain_operations_reconcile_for_additive_formulas` is an internal additive reconciliation; forecast-cell, capital-structure and nonfinite-wire tests cover provenance/serialization. No independent attribution benchmark exists in this scope. Alternatives include evaluation traces with time-indexed operands and formula-level counterfactual attribution.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
