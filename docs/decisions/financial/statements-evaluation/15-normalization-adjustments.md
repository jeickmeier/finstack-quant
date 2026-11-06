# STATEMENTS-EVALUATION-015 — Ordered normalization bridges, caps and result merging

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice, scope and input domain

Normalization constructs an ordered adjustment bridge for every reported target period. Fixed signed amounts or percentage-of-reference amounts retain the target's Scalar/Monetary type and currency; mismatches fail rather than triggering FX. Missing type metadata defaults to Scalar for comparison. Adjustment categories affect presentation, not numerical order or arithmetic. This is a post-evaluation operation, not a rerun of the statement DAG or an automatic D&A adjustment.

## Algorithm and cap conventions

1. Validate unique adjustment IDs, require the target result node and eagerly compare every referenced node's unit/currency metadata with the target (missing metadata defaults Scalar). For each target period require a finite reported base and process adjustments in declaration order. A fixed adjustment absent for the date is zero; percentage adjustment requires a same-period finite reference and multiplies it by the supplied decimal percentage.
2. With no cap, keep the finite raw amount. An absolute cap requires finite nonnegative amount in the same units. A reference cap requires finite nonnegative fraction and a finite same-period reference value; no upper bound of one is imposed on the fraction.
3. A cap referencing the target uses the original reported base by default. Explicit Progressive uses `reported + total_prior_adjustments`; a different reference node always uses its reported value. Effective cap is `max(reference,0)*fraction`, so a negative base supplies zero cap room.
4. Cap each adjustment independently using `sign(raw)*min(abs(raw),cap)`, retaining raw/applied/capped metadata. This is not a shared cumulative basket: two independently capped adjustments can together exceed one cap amount. Sum applied adjustments with Neumaier compensation and require the final `base+total` to be finite. Each intermediate `AppliedAdjustment.adjusted_value=base+running_total` is stored without a separate finite check; it can be nonfinite even when later offsetting adjustments leave a finite final result. Emit chronological per-period bridges and final values.
5. After eager duplicate-ID/reference-unit checks, numeric and referenced-value validation is use-driven: inputs are checked when used for a target date. Unused fixed entries outside target periods need not have their numeric domain visited. Constructor chaining does not fully validate every future evaluation input.

## Merge behavior

`merge_into_results` returns immediately on empty input. Otherwise it prebuilds all numeric and Money maps, rejecting mixed types, duplicate periods, nonfinite final values or Money conversion failures before writing. It replaces the entire destination node's values/type, removes or replaces its monetary map, and clears the check report.

It does **not** recompute dependent nodes or update/remove existing `node_sources`. Replacing an existing node can therefore leave old source provenance attached to new normalized values and derived results referring to the previous values. This is confirmed static behavior; no new execution was performed. An earlier append-only normalization proposal is not treated as an accepted user requirement here. The overwrite/provenance contract remains a reconciliation/adoption question.

## Rationale and alternatives

An ordered bridge makes each add-back and cap explainable; Reported versus Progressive explicitly controls whether prior adjustments enlarge a self-referential cap. It does not resolve which contract permits those add-backs. A shared basket, post-adjustment fixed-point cap, separate output node or full dependent re-evaluation would have materially different semantics.

Previously agreed reported COGS/OPEX include reported D&A. Normalization does not authorize changing that input convention or adding D&A twice. Preserve the distinction between source financial statements and an explicitly named adjusted metric.

## Evidence

[Normalization engine](../../../../finstack-quant/statements/src/adjustments/engine.rs), [types/validation](../../../../finstack-quant/statements/src/adjustments/types.rs), [result structure](../../../../finstack-quant/statements/src/evaluator/results.rs). Inspected inline self-referential cap test: reported 100 with prior +20 and 30% cap gives 30 under Reported versus 36 under Progressive; the negative-base test gives zero cap room. The unguarded intermediate-bridge finiteness is a source-derived concern; no extreme-value execution was performed. Located fixed/percentage/reference-missing, duplicate-ID and merge tests in the same source. These are direct bridge arithmetic/contract evidence; they do not certify a lender's permitted adjustments or GAAP/IFRS presentation. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
