# STATEMENTS-ANALYTICS-VALUATION-09: Statement scenario inheritance, overrides and comparison tables

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice, domain and selection

[Scenario sets](../../../../finstack-quant/statements-analytics/src/analysis/scenarios/scenario_set.rs) apply named changes to a source statement model. [Scenario types](../../../../finstack-quant/statements-analytics/src/analysis/scenarios/types.rs) distinguish forecast-method changes, model-wide value overrides and specific node/period overrides. They are separate maps with separate precedence, not one chronological stream of mutations. Scenario IDs and parent chains select the calculation.

## Algorithm and controls

1. Traverse a requested scenario's parent chain to the root; missing parents and cycles error. Merge ancestors before descendants within each override map, so the child replaces the same key.
2. Clone the original model. Apply forecast-method changes first and select mixed-source behavior. Apply model-wide value overrides to forecast periods only. Apply explicit node/period values last, requiring the period to exist and be forecast. Consequently an ancestor's explicit cell override can override a descendant's model-wide value: explicit cells win across ancestry levels.
3. Preserve typed scalar/money distinctions. Monetary overrides must match declared currency; there is no FX conversion. Invalid node, period or amount conversion propagates an error.
4. Evaluate each completed scenario with a fresh evaluator, optionally using supplied market/as-of. Results retain each scenario's name, label and evaluated data. There is no probability weighting, stochastic joint distribution or automatic calibration of override sizes.
5. A scenario diff uses the [variance methodology](11-variance-bridges.md). The comparison table chooses named `base` when present, otherwise the first insertion-ordered scenario; it iterates that baseline's periods. Missing values in other scenarios are null. Relative change is `(scenario−base)/base`, unavailable when `abs(base)<1e−10`.

The comparison helper does not perform the strict typed-money/currency reconciliation of [financial reports](04-analysis-reports-units.md), nor does it independently reject all nonfinite values. Its numeric relative-change cells are fractions, not percentage points. Scenario-only periods absent from the chosen baseline do not appear.

## Limits, alternatives and evidence

Hierarchy merges are deterministic but specific-cell dominance can be surprising when a child broad override is intended to replace all inherited values. Scenario values are forecasts, not realized history. Additive deltas, multiplicative shocks, probability-weighted expectations and covariance-calibrated shocks are unimplemented alternatives to literal replacement here.

Colocated `resolve_overrides_respects_parent_chain` and [scenario integration cases](../../../../finstack-quant/statements-analytics/tests/analysis_scenario_set.rs) are located evidence for inheritance, evaluation and typed override behavior. They are not empirical scenario calibration. No numerical scenarios or tests were run during this audit.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
