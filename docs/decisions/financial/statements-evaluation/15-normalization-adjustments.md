# STATEMENTS-EVALUATION-015 — Ordered normalization bridges, caps and result merging

Decision status: **Accepted for the full normalization bridge contract (2026-11-15)**. Implementation status: **Reconciled (2026-11-17)**. Validation status: **Qualified by the USD 150 and USD 156 cap example, a capped deduction, a negative base, a fraction above one, an absolute cap, each unavailable input, an infinite intermediate with a finite total, compensated scalar cancellation, collisions and republication, off-plan keys, unit and currency mismatches, a single D&A add-back, a check on the adjusted node and normalization of a rolled result**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

A configuration names the reported target, a new output node and the ordered adjustments. Fixed amounts and absolute caps are typed (a scalar, or money in the target's currency) and a cap is either an absolute amount or a non-negative fraction of a base node. The whole configuration is validated against the result before any period is calculated: unique identifiers, an output node that differs from the target, every referenced node with recorded and matching units, fixed-amount keys within the result's period plan, finite numbers and non-negative caps. Each period reads typed cells; a target, percentage reference or cap base that is missing, masked or unavailable leaves the period unavailable with its reason, and a non-finite value is an error. Money is added and scaled with checked exact arithmetic and scalars with a compensated sum; every row records its raw amount, cap base, limit, applied amount and running value, and any arithmetic failure returns an error with nothing published. Publication calculates the bridge from the configuration, writes the output node under a new identifier (an unavailable period as an unavailable cell), rejects any identifier the result already holds, requires a source run and records the ordered configuration against the output node. The reported target, the other nodes and the check report are unchanged, and a ratio or covenant on the adjusted basis references the output node. A result that holds a published node cannot be rolled up; a rolled result is normalized after the rollup. The sections below the accepted decision describe the audited implementation.

## Accepted decision for normalization bridges

Build an ordered bridge from a reported metric to a separately named adjusted metric.

1. Validate unique adjustment IDs, period keys, references, units and currencies across the canonical source period plan before calculation. Adjustments are signed fixed amounts or signed decimal percentages of a same-period reported reference. An omitted fixed amount is zero for that adjustment. If a required value is missing, masked or otherwise unavailable, mark the period's adjusted value unavailable with a reason.
2. Apply each cap individually. A cap limits magnitude to a typed absolute amount or to the positive part of its base multiplied by a nonnegative reference fraction. The fraction may exceed 100%. Preserve the adjustment's sign. A cap on the target defaults to `Reported`. Explicit `Progressive` uses the reported value plus all previously applied adjustments in declaration order. Cap bases drawn from other nodes use their reported values. A shared basket requires its own rule.
3. Use checked `Money` arithmetic or checked compensated scalar summation. Record each adjustment in declaration order with its raw amount, applicable cap base and limit, applied amount and running adjusted value. Arithmetic errors prevent publication of the bridge. Categories are presentation labels. Follow the approved D&A treatment and add back included D&A once. [Reported costs and earnings](../statements-contracts/07-earnings-and-margins.md#accepted-decision-for-reported-cost-earnings-and-margins).
4. Publish under a new node ID, reject collisions and link the adjusted result to the source run and ordered configuration. Source checks apply to reported figures. A dependent ratio or covenant calculation must explicitly reference the adjusted line and be evaluated on that basis.

For example, reported EBITDA is USD 100. The first adjustment adds USD 20, and the second requests USD 40 with a 30% cap. `Reported` applies USD 30 to the second adjustment and gives adjusted EBITDA of USD 150. `Progressive` uses USD 120 as the cap base, applies USD 36 and gives adjusted EBITDA of USD 156.

## Audited choice, scope and input domain

Normalization constructs an ordered adjustment bridge for every reported target period. Fixed signed amounts or percentage-of-reference amounts retain the target's Scalar/Monetary type and currency; mismatches fail rather than triggering FX. Missing type metadata defaults to Scalar for comparison. Adjustment categories affect presentation, not numerical order or arithmetic. This is a post-evaluation operation, not a rerun of the statement DAG or an automatic D&A adjustment.

## Audited algorithm and cap conventions

1. Validate unique adjustment IDs, require the target result node and eagerly compare every referenced node's unit/currency metadata with the target (missing metadata defaults Scalar). For each target period require a finite reported base and process adjustments in declaration order. A fixed adjustment absent for the date is zero; percentage adjustment requires a same-period finite reference and multiplies it by the supplied decimal percentage.
2. With no cap, keep the finite raw amount. An absolute cap requires finite nonnegative amount in the same units. A reference cap requires finite nonnegative fraction and a finite same-period reference value; no upper bound of one is imposed on the fraction.
3. A cap referencing the target uses the original reported base by default. Explicit Progressive uses `reported + total_prior_adjustments`; a different reference node always uses its reported value. Effective cap is `max(reference,0)*fraction`, so a negative base supplies zero cap room.
4. Cap each adjustment independently using `sign(raw)*min(abs(raw),cap)`, retaining raw/applied/capped metadata. This is not a shared cumulative basket: two independently capped adjustments can together exceed one cap amount. Sum applied adjustments with Neumaier compensation and require the final `base+total` to be finite. Each intermediate `AppliedAdjustment.adjusted_value=base+running_total` is stored without a separate finite check; it can be nonfinite even when later offsetting adjustments leave a finite final result. Emit chronological per-period bridges and final values.
5. After eager duplicate-ID/reference-unit checks, numeric and referenced-value validation is use-driven: inputs are checked when used for a target date. Unused fixed entries outside target periods need not have their numeric domain visited. Constructor chaining does not fully validate every future evaluation input.

## Audited merge behavior

`merge_into_results` returns immediately on empty input. Otherwise it prebuilds all numeric and Money maps, rejecting mixed types, duplicate periods, nonfinite final values or Money conversion failures before writing. It replaces the entire destination node's values/type, removes or replaces its monetary map, and clears the check report.

It does **not** recompute dependent nodes or update/remove existing `node_sources`. Replacing an existing node can therefore leave old source provenance attached to new normalized values and derived results referring to the previous values. This is confirmed static behavior; no new execution was performed. An earlier append-only normalization proposal is not treated as an accepted user requirement here. The overwrite/provenance contract remains a reconciliation/adoption question.

## Audited rationale and alternatives

An ordered bridge makes each add-back and cap explainable; Reported versus Progressive explicitly controls whether prior adjustments enlarge a self-referential cap. It does not resolve which contract permits those add-backs. A shared basket, post-adjustment fixed-point cap, separate output node or full dependent re-evaluation would have materially different semantics.

Previously agreed reported COGS/OPEX include reported D&A. Normalization does not authorize changing that input convention or adding D&A twice. Preserve the distinction between source financial statements and an explicitly named adjusted metric.

## Evidence

[Normalization engine](../../../../finstack-quant/statements/src/adjustments/engine.rs), [types/validation](../../../../finstack-quant/statements/src/adjustments/types.rs), [result structure](../../../../finstack-quant/statements/src/evaluator/results.rs). Inspected inline self-referential cap test: reported 100 with prior +20 and 30% cap gives 30 under Reported versus 36 under Progressive; the negative-base test gives zero cap room. The unguarded intermediate-bridge finiteness is a source-derived concern; no extreme-value execution was performed. Located fixed/percentage/reference-missing, duplicate-ID and merge tests in the same source. These are direct bridge arithmetic/contract evidence; they do not certify a lender's permitted adjustments or GAAP/IFRS presentation. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
