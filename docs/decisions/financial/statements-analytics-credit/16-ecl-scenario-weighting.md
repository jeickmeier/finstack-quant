# STATEMENTS-ANALYTICS-CREDIT-16: Scenario weights, LGD precedence and schedule request normalization

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Scenario ECL is the weighted sum of separately measured allowances under one caller-assigned stage. Each scenario retains its own PD source and optional base LGD, while the configured LGD model determines whether that override is permitted and how it is transformed.

## Scope and input domain

Nonempty scenario/source pairs; each weight finite and >=0; sum within absolute 1e−6 of one; optional LGD override finite in [0,1]. Config and adjusted exposure must satisfy measurement validation. Scenario identifiers are labels rather than lookup keys in this path.

## Algorithm

1. Validate all weights and LGD overrides. Weights within tolerance are accepted as supplied, without renormalization; duplicate IDs are not rejected.
2. For each pair in input order, reject an LGD override with ThroughTheCycle mode. Otherwise use scenario override or exposure base LGD, clone the exposure with that LGD, and call the full stage-specific engine. Downturn adjustment therefore applies after the scenario override; TTC uses the pinned TTC input.
3. Add `weight*scenario_allowance` and retain `(scenario ID, weight, full EclResult)` in the breakdown. Even a zero-weight scenario is fully priced and can fail validation/source lookup. Reject a non-finite final weighted sum.
4. The canonical schedule request builds IDs `scenario_0`, etc., creates the default config with optional bucket-width and recovery-delay overrides, clones the exposure with both rating labels set to `scenario`, inserts zero anchors where needed and constructs validated RawPdCurves. It delegates to the same weighted engine. Single-schedule `compute_ecl` is exactly a one-scenario request of weight one.
5. Direct `classify_stage` accepts supplied matched-window PDs independently; schedule measurement does not classify or recompute a stage for each macro scenario.

## Selection and controls

Stage, EAD path and EIR are shared across scenarios; this API varies PD and optionally LGD. Schedule convenience does not expose TTC/downturn settings and uses defaults for remaining fields. Full curve/config measurement offers those choices.

## Conventions

Weights are probabilities; no percent conversion or normalization occurs. Scenarios need not have matching PD knots. Monetary results remain scalar base-currency allowances; default metadata does not identify source curves or policy versions. Returned breakdown order is input order.

## Limitations, failure and fallback

The generic full-engine path validates adjusted rather than original exposure LGD when all scenarios override it; unused original values are not a separate prerequisite there. Zero-weight sources still cause errors. Duplicate IDs weaken audit labeling. A single common stage and constant shared EAD/EIR omit scenario-dependent staging, exposure and rates.

## Alternatives and tradeoffs

A tighter weight-sum tolerance or normalization with a disclosed correction would have different probability semantics. Separate staging per scenario would need an explicit accounting policy. Rich scenario exposure/LGD/cashflow inputs broaden the model but should not be inferred from this lightweight weighting wrapper.

## Evidence

[engine.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/engine.rs) and [request.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/request.rs) own weighting and normalization. Inspected single/two-scenario tests compare weighted production outputs (internal consistency); located tests cover invalid weights, source-specific PD, TTC/override conflict, downturn-over-override ordering and undrawn scaling in the convenience request. No independent multi-scenario allowance benchmark is established by those same-kernel comparisons.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
