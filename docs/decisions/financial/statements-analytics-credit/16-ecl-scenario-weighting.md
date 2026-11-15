# STATEMENTS-ANALYTICS-CREDIT-16: Impairment scenario weights, LGD and provenance

- Decision status: **Accepted for the full impairment scenario weighting contract (2026-11-15)**.
- Implementation status: **Reconciliation pending**.
- Validation status: **Existing evidence mapped. Independent qualification pending**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for weighted impairment scenarios

1. Require unique scenario IDs. Share the verified exposure, reporting date, currency, stage, effective interest rate and resolved policy across scenarios. Performing scenarios use qualified PD curves or explicit anchored PD schedules. Stage 3 uses applicable dated recovery evidence. Use the same measurement path for a single scenario with weight one. [Exposure and cumulative default inputs](12-ecl-exposure-and-pd.md#accepted-decision-for-exposure-and-cumulative-default-inputs) · [Stage-specific measurement](15-ecl-bucket-measurement.md#accepted-decision-for-stage-specific-impairment-measurement).
2. Require explicit decimal probabilities in `[0,1]` that sum to exactly one as supplied. Retain zero-weight rows as metadata and measure only positive-weight scenarios. Missing required evidence for a positive-weight scenario makes the aggregate unavailable. Retain completed scenarios and give the reason for the gap.
3. For point-in-time and downturn modes, select a sourced scenario LGD or the exposure LGD as the base. Point-in-time mode uses that base directly. Downturn mode applies the selected adjustment once to that base. Through-the-cycle LGD uses the fixed policy value and rejects scenario overrides.
4. Use checked products and compensated summation. Report each weight, source, effective LGD or recovery basis, standalone allowance and weighted contribution where applicable. Retain policy and input provenance. [Resolved impairment policy snapshots](14-ecl-policy-defaults.md#accepted-decision-for-resolved-impairment-policy-snapshots) · [Numerical accumulation](../../standards/0042-numerical-accumulation-and-comparisons.md).

For example, a USD 20 base allowance at 70% contributes USD 14 and a USD 80 downside allowance at 30% contributes USD 24, giving an aggregate of USD 38. If downside evidence is missing, the completed base scenario and its USD 14 contribution stay visible and the aggregate is unavailable.

## Audited choice and rationale

Scenario ECL is the weighted sum of separately measured allowances under one caller-assigned stage. Each scenario retains its own PD source and optional base LGD, while the configured LGD model determines whether that override is permitted and how it is transformed.

## Audited scope and input domain

Nonempty scenario/source pairs; each weight finite and >=0; sum within absolute 1e−6 of one; optional LGD override finite in [0,1]. Config and adjusted exposure must satisfy measurement validation. Scenario identifiers are labels rather than lookup keys in this path.

## Audited algorithm

1. Validate all weights and LGD overrides. Weights within tolerance are accepted as supplied, without renormalization; duplicate IDs are not rejected.
2. For each pair in input order, reject an LGD override with ThroughTheCycle mode. Otherwise use scenario override or exposure base LGD, clone the exposure with that LGD, and call the full stage-specific engine. Downturn adjustment therefore applies after the scenario override; TTC uses the pinned TTC input.
3. Add `weight*scenario_allowance` and retain `(scenario ID, weight, full EclResult)` in the breakdown. Even a zero-weight scenario is fully priced and can fail validation/source lookup. Reject a non-finite final weighted sum.
4. The canonical schedule request builds IDs `scenario_0`, etc., creates the default config with optional bucket-width and recovery-delay overrides, clones the exposure with both rating labels set to `scenario`, inserts zero anchors where needed and constructs validated RawPdCurves. It delegates to the same weighted engine. Single-schedule `compute_ecl` is exactly a one-scenario request of weight one.
5. Direct `classify_stage` accepts supplied matched-window PDs independently; schedule measurement does not classify or recompute a stage for each macro scenario.

## Audited selection and controls

Stage, EAD path and EIR are shared across scenarios; this API varies PD and optionally LGD. Schedule convenience does not expose TTC/downturn settings and uses defaults for remaining fields. Full curve/config measurement offers those choices.

## Audited conventions

Weights are probabilities; no percent conversion or normalization occurs. Scenarios need not have matching PD knots. Monetary results remain scalar base-currency allowances; default metadata does not identify source curves or policy versions. Returned breakdown order is input order.

## Audited limitations, failure and fallback

The generic full-engine path validates adjusted rather than original exposure LGD when all scenarios override it; unused original values are not a separate prerequisite there. Zero-weight sources still cause errors. Duplicate IDs weaken audit labeling. A single common stage and constant shared EAD/EIR omit scenario-dependent staging, exposure and rates.

## Audited alternatives and tradeoffs

A tighter weight-sum tolerance or normalization with a disclosed correction would have different probability semantics. Separate staging per scenario would need an explicit accounting policy. Rich scenario exposure/LGD/cashflow inputs broaden the model but should not be inferred from this lightweight weighting wrapper.

## Evidence

[engine.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/engine.rs) and [request.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/request.rs) own weighting and normalization. Inspected single/two-scenario tests compare weighted production outputs (internal consistency); located tests cover invalid weights, source-specific PD, TTC/override conflict, downturn-over-override ordering and undrawn scaling in the convenience request. No independent multi-scenario allowance benchmark is established by those same-kernel comparisons.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
