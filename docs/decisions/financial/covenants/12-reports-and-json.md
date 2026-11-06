# COVENANTS-12: Report semantics, metadata and canonical JSON boundaries

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted decisions retain their scope.

**Implementation status:** Observed in the exact source paths identified below, including stated discrepancies and limitations.

**Validation status:** Source and identified test assertions inspected; tests, builds, numerical experiments and external benchmarks not executed. Test existence is not adoption or independent qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Result construction and interpretation

Point-in-time results are an insertion-ordered `IndexMap` keyed by exact instance label. Each report includes a static type description, optional ID, pass flag, optional actual/threshold/headroom/detail, and `ResultsMeta`. Inactive/waived/descriptive passes have no numeric assessment; invalid ratios fail with absent actual/headroom. A numeric failure can carry `In cure period` from prior history; the invalid-ratio detail overwrites that text when present. Cure state is not an independent report field. A single report carries no evaluation date; dated wrappers add `as_of`.

`CovenantReport::passed/failed` creates metadata from `FinstackConfig::default()`, not from the supplied metric producer or an effective calculation configuration. Core provides f64 mode, default rounding context, version, no timestamp/FX policy and serial status. Evaluation does not apply that rounding context to comparisons. Public report builders can attach arbitrary values and do no finite or cross-field validation. Forecast results carry no resolved configuration, volatility/seed/reference provenance or `ResultsMeta`; vector alignment and probability ranges are established by producing code, not checked on ordinary deserialization.

## JSON processing stages

`validate_covenant_spec_json` and `validate_covenant_engine_json` parse typed serde values, run their business validation, and canonicalize. Schedule deserialization separately calls its checked constructor. `validate_covenant_report_json` is only a typed structural round trip: it does not re-evaluate pass/threshold/headroom consistency. Despite its name and wording, no generated JSON Schema validator is invoked by that function.

Core `to_canonical_bytes` serializes through a checked finite-number tree, sorts object keys recursively, preserves array order and omits insignificant whitespace. Thus canonicalization preserves semantically ordered consequence/spec arrays but sorts report/map keys on the wire; typed `IndexMap` evaluation order is not promised by canonical JSON. Missing optional/default fields normalize to the serde representation. Report metadata omitted in input becomes current default metadata, including library version, so canonical output is not invariant across library-version changes for such incomplete input.

`evaluate_engine` parses/validates engine JSON, validates every metric-map value as a JSON number, parses the ISO date and evaluates applicable specs. `evaluate_engine_with_source` keeps a prebuilt Rust metric map and rejects nonfinite values only when read. The two input forms intentionally differ for unused invalid values. Neither tracks breaches or applies consequences.

## Rationale, alternatives

Shared Rust JSON entry points keep host parsing/selection consistent; canonical bytes provide deterministic structure, not provenance or compliance certification. Full result verification, an explicit assessed-status enum and resolved input/configuration stamps are alternatives. Financially meaningful nonfinite outputs must preserve missing-versus-invalid semantics under [ADR 0009](../conventions/0009-missing-and-invalid-data.md); direct ordinary serde/report mutation is a different boundary from checked canonicalization.

Owning [reports](../../../../finstack-quant/covenants/src/report.rs), [JSON entry points](../../../../finstack-quant/covenants/src/json.rs), [forecast result shapes](../../../../finstack-quant/covenants/src/forward.rs), and inspected [core canonical serializer](../../../../finstack-quant/core/src/canonical.rs) / [metadata](../../../../finstack-quant/core/src/config.rs). Inspected [serialization tests](../../../../finstack-quant/covenants/tests/serialization.rs) cover canonical key order, nullable projections, default config/metadata and unknown-field rejection; [integration tests](../../../../finstack-quant/covenants/tests/integration.rs) cover report builders without financial recomputation.

No wire round trips were executed in this audit.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
