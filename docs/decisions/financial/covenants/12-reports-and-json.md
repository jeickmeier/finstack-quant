# COVENANTS-12: Report semantics, metadata and canonical JSON boundaries

**Decision status:** Accepted 2026-11-17 for self-describing reports, factual provenance and checked canonical restoration.

**Implementation status:** Shared canonical JSON entry points and nullable measurements are implemented. The accepted report fields, applied provenance and semantic restoration checks remain pending.

**Validation status:** All 24 Covenants serialization tests passed on 2026-11-17 against current behavior. The accepted report and restoration rules still need qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Record the exact covenant label, test date, scope, actual or projected classification, effective terms and their source, and explicit assessment outcome. Carry typed values, units and applicable absolute and relative headroom. Report cure state independently of the assessment outcome. [Effective terms](02-effective-terms.md) · [Assessment outcomes](03-comparison-and-headroom.md) · [Cure history](05-breach-and-cure.md).
2. Preserve available source or run identity and the precision or rounding actually applied. Record the forecast method and applicable reference date, volatility basis, seed, path and sampling-unit counts, and standard error. Leave unknown facts unrecorded and preserve historical provenance on restoration. [Core metadata contract](../core-numerics/rounding-and-config.md#accepted-decision-explicit-rounding-config-local-tolerances-and-producer-metadata).
3. Check report and forecast consistency before canonicalization. Reject incompatible status and value combinations, misaligned data, probabilities outside `[0,1]` and invalid standard errors. Validate the recorded result without re-running the financial assessment. Reuse [core canonical JSON](../core-numerics/numeric-wire-and-canonical.md), preserve exact Decimal amounts and retain semantic order in arrays.

## Observed result construction and interpretation

Point-in-time results are an insertion-ordered `IndexMap` keyed by exact instance label. Each report includes a static type description, optional ID, pass flag, optional actual/threshold/headroom/detail, and `ResultsMeta`. Inactive/waived/descriptive passes have no numeric assessment; invalid ratios fail with absent actual/headroom. A numeric failure can carry `In cure period` from prior history; the invalid-ratio detail overwrites that text when present. Cure state is not an independent report field. A single report carries no evaluation date; dated wrappers add `as_of`.

`CovenantReport::passed/failed` creates metadata from `FinstackConfig::default()`, not from the supplied metric producer or an effective calculation configuration. Core provides f64 mode, default rounding context, version, no timestamp/FX policy and serial status. Evaluation does not apply that rounding context to comparisons. Public report builders can attach arbitrary values and do no finite or cross-field validation. Forecast results carry no resolved configuration, volatility/seed/reference provenance or `ResultsMeta`; vector alignment and probability ranges are established by producing code, not checked on ordinary deserialization.

## Observed JSON processing stages

`validate_covenant_spec_json` and `validate_covenant_engine_json` parse typed serde values, run their business validation, and canonicalize. Schedule deserialization separately calls its checked constructor. `validate_covenant_report_json` is only a typed structural round trip: it does not re-evaluate pass/threshold/headroom consistency. Despite its name and wording, no generated JSON Schema validator is invoked by that function.

Core `to_canonical_bytes` serializes through a checked finite-number tree, sorts object keys recursively, preserves array order and omits insignificant whitespace. Thus canonicalization preserves semantically ordered consequence/spec arrays but sorts report/map keys on the wire; typed `IndexMap` evaluation order is not promised by canonical JSON. Missing optional/default fields normalize to the serde representation. Report metadata omitted in input becomes current default metadata, including library version, so canonical output is not invariant across library-version changes for such incomplete input.

`evaluate_engine` parses/validates engine JSON, validates every metric-map value as a JSON number, parses the ISO date and evaluates applicable specs. `evaluate_engine_with_source` keeps a prebuilt Rust metric map and rejects nonfinite values only when read. The two input forms intentionally differ for unused invalid values. Neither tracks breaches or applies consequences.

## Rationale, alternatives

Shared Rust JSON entry points align host behavior. Canonical encoding preserves deterministic structure. Explicit statuses, dated terms and factual provenance let readers interpret a stored result, and semantic checks reject internally inconsistent results. Missing and invalid outcomes remain distinct under [ADR 0009](../conventions/0009-missing-and-invalid-data.md).

Owning [reports](../../../../finstack-quant/covenants/src/report.rs), [JSON entry points](../../../../finstack-quant/covenants/src/json.rs), [forecast result shapes](../../../../finstack-quant/covenants/src/forward.rs), and inspected [core canonical serializer](../../../../finstack-quant/core/src/canonical.rs) / [metadata](../../../../finstack-quant/core/src/config.rs). Inspected [serialization tests](../../../../finstack-quant/covenants/tests/serialization.rs) cover canonical key order, nullable projections, default config/metadata and unknown-field rejection; [integration tests](../../../../finstack-quant/covenants/tests/integration.rs) cover report builders without financial recomputation.

All 24 serialization tests passed against current behavior. Add restoration fixtures for invalid status and value combinations, forecast alignment and ranges, provenance preservation and exact monetary values.

## Follow-up tracking

Implementation reconciliation and qualification are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
