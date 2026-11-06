# STATEMENTS-ANALYTICS-VALUATION-10: Statement parameter grids and tornado aggregation

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice and input domain

[Sensitivity analysis](../../../../finstack-quant/statements-analytics/src/analysis/scenarios/sensitivity.rs) changes explicit forecast node/period values and evaluates the prepared statement model. Parameters have node, period, finite `base_value` and nonempty finite perturbations. One to 128 parameters are allowed, and duplicate node/period pairs reject. **The evaluated baseline is the original model**: `base_value` is metadata except when the percentage constructor computes perturbations as `base_value×(1+pct/100)`.

## Algorithm and numerical/resource controls

1. Validate all parameters and estimate work before preparing the model. Limit scenario count to 10,000 and total model node-period cells across baseline and scenarios to 10 million. Overflow in counts errors.
2. Prepare/evaluate the source model once. Enforce the same forecast-only and typed-currency override rules as scenarios.
3. `Diagonal` evaluates each parameter's perturbations independently with other cells at original baseline values. Parallel execution preserves input order and first-error order. `FullGrid` enumerates the Cartesian product in mixed-radix order with the last parameter varying fastest and reuses prepared evaluation state.
4. `Tornado` starts from diagonal scenarios, calculates maximum absolute delta across requested target metrics and baseline periods for each scenario, and sorts by that impact. Missing/nonfinite targets and overflowed differences error. Values in different units are not normalized before comparison.
5. Tornado-entry generation requires a true baseline and a diagonal-compatible result; full-grid results reject. For each parameter, pick outcomes at the **minimum and maximum input perturbation**, subtract baseline target, and label them downside/upside. It does not search minimum/maximum output across interior points. Sort by absolute output span; equal node names in different periods remain distinct parameter identifiers.

Outputs are raw target-unit changes, not derivative estimates or elasticities. Negative base values in percentage constructors reverse the numerical direction associated with a percentage increase; endpoint selection still follows actual perturbation values.

## Limits, alternatives and evidence

A supplied base value differing from the source cell does not reset that cell for the baseline. Nonmonotone response extrema can occur inside the grid and be omitted by tornado entries. Ranking a dollar metric against a ratio by raw magnitude is not dimensionally comparable. The calculation does not assign probabilities, quantify Monte Carlo error or decompose interactions from a full grid.

Colocated tests cover Cartesian ordering, true-baseline anchoring, period-specific IDs, monetary overrides, oversized work rejection, nonfinite/duplicate inputs and nonlinear overflow guards. [Sensitivity integration cases](../../../../finstack-quant/statements-analytics/tests/analysis_sensitivity.rs) provide further located evidence. These establish mechanics, not economic calibration. Alternatives include explicitly applying declared bases, normalized elasticities, output-extremum tornado ranges and factorial interaction analysis; none is silently selected.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
