# STATEMENTS-ANALYTICS-VALUATION-14: Configurable credit-scorecard thresholds and partial-evidence aggregation

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice, domain and selection

[The scorecard extension](../../../../finstack-quant/statements-analytics/src/extensions/scorecards/mod.rs) evaluates user formulas, assigns rating-bucket points and weights them. Default target is the latest actual period, otherwise the final model period; explicit target must exist. Default scale is `sp`. Actual registry-backed choices come from [rating-scale data](../../../../finstack-quant/core/data/rating_scales/rating_scales.v2.json), including S&P and Moody's; a module-introduction mention of Fitch does not add a supported registry.

Weights must be finite and nonnegative. A positive configured total must lie in `[.01,100]`; all-zero weight configuration survives this range check but cannot yield usable weighted evidence. Metric threshold ranges are not comprehensively validated for ordering, finiteness, overlap or unknown rating names during configuration.

## Algorithm and controls

1. Compile/evaluate each metric formula with current values and capital-structure context; history includes only period IDs before the target. No monetary-unit metadata enters this formula-scoring context.
2. Nonfinite or unmatched metric values are excluded with warnings; formula failures, including unresolved inputs, are collected as errors and also contribute no score. Match thresholds in registry best-to-worst rating order, using inclusive lower and upper bounds. A shared boundary belongs to the better rating, regardless of input threshold ordering.
3. Convert the matched rating to registry points. S&P ranges from AAA=100 to D=0; Moody's from Aaa=100 to C=2. These are software data values, not empirically calibrated default probabilities.
4. Compute `sum(weight×score)/sum(usable_weight)`, excluding unavailable metrics and renormalizing the remaining weights. A denominator strictly below floating-point epsilon is unavailable. Partial-evidence and weight-coverage information is reported.
5. Map aggregate score to the first best-to-worst rating whose registry minimum is met; otherwise use the worst rating. The inspected data uses S&P AAA minimum 95 and Moody's C minimum `.01`, among its full thresholds. A configured minimum-rating comparison uses the same order; unknown minimum rating produces a warning/nonpassing minimum check rather than validating that rating up front.
6. Extension diagnostics may mark the result Failed while a partial aggregate rating remains populated. A caller reading only the rating string can miss formula errors and evidence loss.

## Limits, alternatives and evidence

This is a configurable ordinal scoring scheme, not an implementation of an agency's published issuer methodology, ratings opinion, probability-of-default model or regulatory mapping. Renormalization allows remaining metrics to determine a full-range score after exclusions. Threshold overlaps/gaps and units are caller policy. Registry points/thresholds are material data dependencies beyond the 32-file Rust manifest.

Colocated boundary tests confirm best-bucket tie handling, unmatched exclusion and sub-epsilon weight behavior; [extension integration cases](../../../../finstack-quant/statements-analytics/tests/extensions_scorecards.rs) are additional located evidence. No external agency calibration or out-of-sample validation was established. Missing-evidence floors, calibrated logistic/PD models and explicit agency methods are unimplemented alternatives.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
