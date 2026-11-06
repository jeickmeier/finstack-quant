# CALIBRATION-FRAMEWORK-001: Strict envelopes and authoring defaults

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

The request is a typed, versioned envelope rather than an executable untyped map. `CalibrationStep::from_wire_fields` fills missing or null output IDs from the step ID for six curve and three surface kinds, defaults `quote_set` to the step ID, chooses senior hazard debt and the NS parametric variant, then deserializes the typed step. It overwrites caller map fields `kind`, `id`, and `quote_set`. This centralizes host-language authoring; historical adoption rationale is not established.

## Input domain and ordered behavior

1. Parse under `LoadLimits`, require the current schema marker, reject malformed/version-incompatible shapes and unknown fields at typed structures. Flattened steps explicitly deserialize their remaining fields as `StepParams`.
2. `parse_envelope` performs shape loading; `validate` separately assembles semantic errors. Request `from_slice_strict` returns `(envelope, report)` even when that semantic report contains errors. Execution explicitly rejects the first semantic error. `dry_run` is a static dependency/quote check and does not solve, build the context, or run full numeric configuration/preflight checks.
3. Quote constructors build and validate typed rate/CDS/inflation/volatility/XCCY/tranche quotes. Futures convexity defaults to zero; option type defaults to call, swaption convention to USD, cap/floor side to cap. Rates and volatility are decimal inputs, CDS coupons/spreads are bp, and CDS/tranche `upfront_pct` is a fraction despite its name.
4. Step defaults include discount bootstrap/log-linear/flat-forward; forward global solve with finite differences and cubic Hermite; hazard bootstrap/linear par interpolation; inflation bootstrap/linear; SABR beta 0.5; Student-t initial df 5, bounds [2.1,50], correlation 0.3; normal/lognormal conventions remain explicit where required. Surface extrapolation defaults to Error, missing SABR bucket fallback false, HW cap volatility scalar. Detailed target methods are in [calibration targets](../calibration-targets/README.md).
5. Result loading checks the nested final market using the market-context strict loader, prefixes its diagnostics, and applies depth limits separately to outer and nested structures. Content hashes use canonical serialization. Hand-written schema catalogues derive artifacts and examples; they are not generated runtime source.

## Limits and alternatives

A well-shaped result can carry inconsistent report numbers: result loading restores market state but does not rerun calibration or independently validate the report's financial acceptance. A successful request parse does not imply semantic validity; consumers must honor the returned report. Quote constructors do not fill missing financial conventions beyond the named defaults.

Alternatives are eager rejection of semantic errors at every entry point or a distinct validated-envelope type. Both would narrow the opportunity to ignore a report but change the authoring contract. No compatibility behavior is proposed.

## Source and validation evidence

- Implementation: [api/schema.rs](../../../../finstack-quant/calibration/src/api/schema.rs) — shape/version checks, semantic-report handoff, step defaults and nested-market restoration.
- Implementation: [api/authoring.rs](../../../../finstack-quant/calibration/src/api/authoring.rs) — validated quote construction and map-default precedence.
- Implementation: [api/validate.rs](../../../../finstack-quant/calibration/src/api/validate.rs) — parse/dry-run distinction.
- Implementation: [api/market_datum.rs](../../../../finstack-quant/calibration/src/api/market_datum.rs) — quote versus context datum classification.
- Implementation: [api/prior_market.rs](../../../../finstack-quant/calibration/src/api/prior_market.rs) — typed prior-object variants.
- Implementation: [json_schema.rs](../../../../finstack-quant/calibration/src/json_schema.rs) — hand-written artifact catalogue and examples.
- Test evidence: [src/api/authoring.rs](../../../../finstack-quant/calibration/src/api/authoring.rs) — constructor/default/unknown-field regressions; inspected or located, not executed.
- Test evidence: [tests/calibration/schema_parity.rs](../../../../finstack-quant/calibration/tests/calibration/schema_parity.rs) — schema-surface evidence located; inspected or located, not executed.
- Test evidence: [tests/calibration/serialization.rs](../../../../finstack-quant/calibration/tests/calibration/serialization.rs) — wire roundtrip evidence located; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
