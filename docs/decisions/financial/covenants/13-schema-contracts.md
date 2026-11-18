# COVENANTS-13: Schema registry, strictness and validation boundaries

**Decision status:** Accepted 2026-11-17 for one Rust-owned wire contract and checked host admission.

**Implementation status:** Rust schema registration and generation are implemented. Canonical metric-map admission, shared semantic restoration, and regenerated host declarations remain pending.

**Validation status:** Both focused schema contract tests passed on 2026-11-17 against current behavior. Broader root, semantic admission and host parity coverage remain pending.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Publish the Rust-owned shapes used by callers. The canonical metric-map payload is a bare object keyed by exact metric IDs. Retire the internal wrapper's wire shape. Encode dates as ISO strings and threshold schedules as `[date, threshold]` pairs. Preserve the accepted explicit terms, typed precision and missing-value rules. [Metric inputs](01-terms-and-metric-selection.md) · [Effective terms](02-effective-terms.md) · [Core wire contract](../core-numerics/numeric-wire-and-canonical.md).
2. Share Rust semantic checks across checked construction and restoration. Reject unknown fields in structured records and duplicate keys before collection. Require fields made explicit by the accepted contracts. The schema checks wire shape. Rust validation enforces domain invariants, including exact label uniqueness, nonoverlapping windows, valid breach history and consistent reports and forecasts. [Report restoration](12-reports-and-json.md).
3. Generate schemas and WASM declarations from the canonical Rust contract and update Python stubs and the parity ledger with the same changes. Verify valid round trips and invalid admissions for every published root and host surface. Host conveniences convert values and delegate business validation to Rust.

## Observed registry and generated surface

With feature `json-schema`, `schema::ARTIFACTS` registers ten roots: `ConsequenceApplication` (Component); `CovenantEngine`, `CovenantForecastConfig`, `CovenantTemplate`, `DatedMetrics`, `HashMapMetricSource` (Input); and `CovenantForecast`, `CovenantReport`, `DatedCovenantReports`, `FutureBreach` (Output). Nested specs, windows, schedules, types and consequences are emitted through those roots' definitions rather than separate registry entries. The binary delegates argument parsing and artifact emission to the core generator, writes under `schemas/covenants`, and updates `schemas/index.json`. Generation is plumbing; the registry's shape/default choices are part of the data contract.

The schema represents dates as ISO strings using `DateWire` annotations. Threshold schedules use two-element `[date, threshold]` arrays. Enum representations are snake-case externally tagged variants except `CovenantTemplate`, which is internally tagged by `kind`. `ThresholdTest::new` accepts exactly `maximum` or `minimum`, case-sensitive. Engine wire requires `specs` and defaults history, windows and waivers to empty. The current forecast config defaults omitted fields. Optional spec denominator and schedule fields omit `None`, and nullable measurements preserve absence separately from finite values.

Most specification/output structs explicitly deny unknown fields. `HashMapMetricSource` and `ConsequenceApplication` do not declare that serde policy, so strictness is not uniform. `HashMapMetricSource`'s registered representation wraps the private `metrics` map; the `from_json`/evaluation convenience parser instead accepts a bare metric object. These are distinct entry contracts, not interchangeable schemas.

## Observed validation boundaries

Generated schemas describe shapes and serde defaults, not all business invariants: label uniqueness, interval overlap, positive earnings interpretation, probability-array alignment, cure progress, and project-threshold ordering require runtime or additional semantic rules. Derived deserialization of public engine/spec/template/config/result types does not automatically execute their business checks. Engine entry points and spec canonicalizers validate configuration; template construction validates threshold inputs; forecast execution validates stochastic configuration; report canonicalization only structurally decodes. `ThresholdSchedule` is the exception with private storage and checked custom deserialization.

Consequently successful serde/schema parsing alone does not prove an engine is safe to evaluate or an imported result is internally consistent. Existing accepted invalid/missing and units policies remain constraints; this record does not grant generated types additional financial meaning.

## Rationale, alternatives, evidence

The registry describes the wire contract and feeds generated WASM declarations. Shared semantic checks keep construction and restoration aligned across entry points. Strict admission before collecting maps preserves duplicate-key evidence.

Owning [registry](../../../../finstack-quant/covenants/src/schema.rs), [generator](../../../../finstack-quant/covenants/src/bin/gen_schemas.rs), [serde terms](../../../../finstack-quant/covenants/src/engine/types.rs), [engine serde/validation](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs), [metric representation](../../../../finstack-quant/covenants/src/metric.rs) and [crate exports](../../../../finstack-quant/covenants/src/lib.rs). Inspected [schema tests](../../../../finstack-quant/covenants/tests/schema_contract.rs) render an engine with schedule, validate it with format checks, and reject object-shaped schedule entries. [Serialization tests](../../../../finstack-quant/covenants/tests/serialization.rs) cover strict engine/report/config fields, default config and ISO dated rows. These are selected fixtures, not validation of every registered root or every host declaration.

Both focused tests exercised an in-memory schema validator and passed. Generated artifact reconciliation, broader root fixtures and host admission checks remain pending.

## Follow-up tracking

Implementation reconciliation and qualification are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
