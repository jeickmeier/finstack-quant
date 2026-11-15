# STATEMENTS-ANALYTICS-VALUATION-10: Statement parameter grids and tornado aggregation

- **Decision status:** Accepted for the full statement sensitivity and tornado contract (2026-11-15).
- **Implementation status:** Reconciliation pending.
- **Validation status:** Existing evidence mapped. Independent qualification pending.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Accepted decision for statement sensitivity grids and tornado reports

1. Verify the evaluated, unperturbed baseline using the selected statement snapshot, cutoff and required market inputs. Select distinct existing editable forecast node and period inputs. Each input's levels must be distinct and typed in its unit and currency. Compute percentage shocks from the verified source input, with `10` meaning 10%. Reject any supplied base that disagrees with the verified input. A zero base requires absolute levels.
2. A one-at-a-time case varies one input. A full grid enumerates the Cartesian product with the last declared parameter varying fastest. Before evaluation, check work against limits of 128 parameters, 10,000 perturbation cases and 10 million node-period evaluations including the baseline.
3. Reuse the validated plan with isolated forecast and capital state for each case. Record applied inputs and run provenance. Preserve deterministic case order and first-error order. Retain unavailable target values and their reasons. [Prepared evaluation](../statements-evaluation/01-graph-precedence-and-prepared-plans.md#accepted-decision-for-dependency-graphs-cell-precedence-and-prepared-evaluation).
4. Build each tornado from one-at-a-time cases for a single target and period. Report the baseline and the minimum and maximum tested responses with their input levels, including interior levels. A ranked parameter requires complete results for at least two tested levels. Rank by observed output span and order ties by stable parameter ID. Retain results and coverage for incomplete parameters and leave them unranked.

For example, with a baseline of 90 and responses of 60, 100 and 60 at three increasing input levels, the tested span is 40. The changes from baseline are −30 at the minimum response and +10 at the maximum. The highest response occurs at the middle input level.

## Audited choice and input domain

[Sensitivity analysis](../../../../finstack-quant/statements-analytics/src/analysis/scenarios/sensitivity.rs) changes explicit forecast node/period values and evaluates the prepared statement model. Parameters have node, period, finite `base_value` and nonempty finite perturbations. One to 128 parameters are allowed, and duplicate node/period pairs reject. **The evaluated baseline is the original model**: `base_value` is metadata except when the percentage constructor computes perturbations as `base_value×(1+pct/100)`.

## Audited algorithm and numerical/resource controls

1. Validate all parameters and estimate work before preparing the model. Limit scenario count to 10,000 and total model node-period cells across baseline and scenarios to 10 million. Overflow in counts errors.
2. Prepare/evaluate the source model once. Enforce the same forecast-only and typed-currency override rules as scenarios.
3. `Diagonal` evaluates each parameter's perturbations independently with other cells at original baseline values. Parallel execution preserves input order and first-error order. `FullGrid` enumerates the Cartesian product in mixed-radix order with the last parameter varying fastest and reuses prepared evaluation state.
4. `Tornado` starts from diagonal scenarios, calculates maximum absolute delta across requested target metrics and baseline periods for each scenario, and sorts by that impact. Missing/nonfinite targets and overflowed differences error. Values in different units are not normalized before comparison.
5. Tornado-entry generation requires a true baseline and a diagonal-compatible result; full-grid results reject. For each parameter, pick outcomes at the **minimum and maximum input perturbation**, subtract baseline target, and label them downside/upside. It does not search minimum/maximum output across interior points. Sort by absolute output span; equal node names in different periods remain distinct parameter identifiers.

Outputs are raw target-unit changes, not derivative estimates or elasticities. Negative base values in percentage constructors reverse the numerical direction associated with a percentage increase; endpoint selection still follows actual perturbation values.

## Audited limits, alternatives and evidence

A supplied base value differing from the source cell does not reset that cell for the baseline. Nonmonotone response extrema can occur inside the grid and be omitted by tornado entries. Ranking a dollar metric against a ratio by raw magnitude is not dimensionally comparable. The calculation does not assign probabilities, quantify Monte Carlo error or decompose interactions from a full grid.

Colocated tests cover Cartesian ordering, true-baseline anchoring, period-specific IDs, monetary overrides, oversized work rejection, nonfinite/duplicate inputs and nonlinear overflow guards. [Sensitivity integration cases](../../../../finstack-quant/statements-analytics/tests/analysis_sensitivity.rs) provide further located evidence. These establish mechanics, not economic calibration. Alternatives include explicitly applying declared bases, normalized elasticities, output-extremum tornado ranges and factorial interaction analysis; none is silently selected.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
