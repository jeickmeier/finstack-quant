# STATEMENTS-ANALYTICS-VALUATION-11: Statement variance ratios and additive driver bridges

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice and input domain

[Variance analysis](../../../../finstack-quant/statements-analytics/src/analysis/scenarios/variance.rs) compares requested metric/period cells from two evaluated result sets. Metric and period lists must be nonempty. Missing values error. Units are checked by equality of node-level `NodeValueType` metadata; absent metadata on both sides compares equal. This path does not use the stricter typed-money/numeric-projection reconciliation in [04](04-analysis-reports-units.md).

## Algorithm and conventions

For baseline `b` and comparison `c`, report absolute change `c−b` and relative change `(c−b)/b`; relative change is null when `abs(b)<1e−10`. The denominator keeps the baseline sign. For example, moving from a more negative loss to a smaller negative loss can produce a negative relative ratio despite economic improvement. Relative change is a decimal fraction.

Bridge decomposition calculates target change, then for each supplied driver calculates its raw comparison-minus-baseline change. Driver metadata must equal target metadata. It sums driver changes and reports `unexplained = target_change−sum(driver_changes)`. Drivers are not reevaluated one at a time, signed/weighted from a formula, differentiated or allocated by Shapley rules. Duplicate drivers are not rejected and therefore count repeatedly.

Neither ordinary variance nor bridge construction independently rejects all nonfinite numeric inputs/results. Matching metadata can coexist with inconsistent typed money or nonfinite numeric projections. This is an observed validation difference; no numerical exploit or financial impact was reproduced.

## Alternatives and evidence

The bridge is an additive arithmetic reconciliation and should not be described as causal attribution. It is exact for directly additive same-unit components selected with the correct sign convention by the caller; interaction effects, rates times volumes, reclassifications and duplicate selection can remain unexplained or distort the sum. Formula-aware decomposition, symmetric percentage change, absolute-baseline ratios and counterfactual attribution are alternative methods, not implemented variants here.

Colocated baseline-equality, negative-variance, zero-base and bridge-residual cases provide hand-sized formula/invariant evidence. No provenance or external attribution standard was established. The existing attribution crate is a separate scope; this helper's raw arithmetic must not be assumed to inherit its attribution guarantees.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
