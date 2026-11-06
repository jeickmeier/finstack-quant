# COVENANTS-13: Schema registry, strictness and validation boundaries

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted decisions retain their scope.

**Implementation status:** Observed in the exact source paths identified below, including stated discrepancies and limitations.

**Validation status:** Source and identified test assertions inspected; tests, builds, numerical experiments and external benchmarks not executed. Test existence is not adoption or independent qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Registry and generated surface

With feature `json-schema`, `schema::ARTIFACTS` registers ten roots: `ConsequenceApplication` (Component); `CovenantEngine`, `CovenantForecastConfig`, `CovenantTemplate`, `DatedMetrics`, `HashMapMetricSource` (Input); and `CovenantForecast`, `CovenantReport`, `DatedCovenantReports`, `FutureBreach` (Output). Nested specs, windows, schedules, types and consequences are emitted through those roots' definitions rather than separate registry entries. The binary delegates argument parsing and artifact emission to the core generator, writes under `schemas/covenants`, and updates `schemas/index.json`. Generation is plumbing; the registry's shape/default choices are part of the data contract.

Dates use ISO `DateWire` schema substitutes matching serde date strings. Threshold schedules are arrays of two-element `[date,threshold]` arrays. Enum representations are snake-case externally tagged variants except `CovenantTemplate`, which is internally tagged by `kind`; `ThresholdTest::new` accepts exactly `maximum` or `minimum`, case-sensitive. Engine wire requires `specs`; history/windows/waivers default empty. Forecast config uses complete struct defaults for omitted fields. Optional spec denominator/schedule fields omit `None`; nullable report and forecast measurements preserve absence separately from finite values.

Most specification/output structs explicitly deny unknown fields. `HashMapMetricSource` and `ConsequenceApplication` do not declare that serde policy, so strictness is not uniform. `HashMapMetricSource`'s registered representation wraps the private `metrics` map; the `from_json`/evaluation convenience parser instead accepts a bare metric object. These are distinct entry contracts, not interchangeable schemas.

## What validation establishes

Generated schemas describe shapes and serde defaults, not all business invariants: label uniqueness, interval overlap, positive earnings interpretation, probability-array alignment, cure progress, and project-threshold ordering require runtime or additional semantic rules. Derived deserialization of public engine/spec/template/config/result types does not automatically execute their business checks. Engine entry points and spec canonicalizers validate configuration; template construction validates threshold inputs; forecast execution validates stochastic configuration; report canonicalization only structurally decodes. `ThresholdSchedule` is the exception with private storage and checked custom deserialization.

Consequently successful serde/schema parsing alone does not prove an engine is safe to evaluate or an imported result is internally consistent. Existing accepted invalid/missing and units policies remain constraints; this record does not grant generated types additional financial meaning.

## Rationale, alternatives, evidence

The registry's documented purpose is one Rust-owned host contract, including generated WASM declarations. Adopting validated wrappers for more inputs or explicit semantic result validators would strengthen restoration guarantees, at a cost of different API boundaries. No blanket strictness or constructor equivalence is inferred.

Owning [registry](../../../../finstack-quant/covenants/src/schema.rs), [generator](../../../../finstack-quant/covenants/src/bin/gen_schemas.rs), [serde terms](../../../../finstack-quant/covenants/src/engine/types.rs), [engine serde/validation](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs), [metric representation](../../../../finstack-quant/covenants/src/metric.rs) and [crate exports](../../../../finstack-quant/covenants/src/lib.rs). Inspected [schema tests](../../../../finstack-quant/covenants/tests/schema_contract.rs) render an engine with schedule, validate it with format checks, and reject object-shaped schedule entries. [Serialization tests](../../../../finstack-quant/covenants/tests/serialization.rs) cover strict engine/report/config fields, default config and ISO dated rows. These are selected fixtures, not validation of every registered root or every host declaration.

No schema generator, build, host tests or schema validator was run.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
