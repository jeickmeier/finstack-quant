# SCENARIOS-017: Application manifests, shock units and versioned replay envelopes

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: return enough typed change information for consumers to invalidate/reprice affected instruments, alongside a serializable applied context and ordered shock descriptions. A scenario content hash identifies its input specification; it is not a hash of all data needed to reproduce financial results.

## Ordered reporting semantics

`user_operations` is the original specification length; `expanded_operations` is the count after hierarchy expansion. `operations_applied` counts successful adapter effects or changed top-level instruments, not a universal one-per-request measure. A multi-ID equity request, split node shock or instrument filter can contribute several; an empty/zero bucket or unchanged structure can still count. Warning-only skips often count zero, while a rate binding with no values can count as success. Read these fields together with warnings and changes.

Market changes are keyed by typed target: curve kind+ID, volatility-index ID, base-correlation ID, vol-surface ID, price-scalar ID or ordered FX pair. Keep first-seen target order and deduplicate. Changed inventory indices are inserted in sorted unique order. A changed date sets `as_of_changed` and `all_dirty`; unexpected unclassified effects also set `all_dirty`. No operation currently changes portfolio shape. Statement-input changes do not populate a separate statement dirty-node manifest.

`AppliedShock` records each emitted shock in its own quote space: basis points, percent or absolute units. Discount/inflation key-rate effects report resolved knot time and emitted pillar amplitude; rebuilt forward/commodity/ParCDS/index operations report requested node tenors/amplitudes. Surface buckets include snapped expiries when emitted through their dedicated bump. Commodity curve fields are reported as Percent despite their `bp` input name. Correlations remain decimal absolute changes. **Only price-scalar effects record numeric before/after levels.** Clipping, quote rebootstrap and no-match effects can therefore differ from the requested shock shown in the report.

Report metadata stamps engine configuration's results metadata/rounding mode. `effective_as_of` uses `time_roll.new_date` when present, otherwise the caller's original date. A returned report exists only after successful engine completion; an `Err` after partial mutation carries no completed application report.

## Persistence and validation boundaries

`ScenarioEnvelope` uses exactly `finstack_quant.scenario/1`, rejects unknown fields, and delegates canonical content hashing to core. `from_slice_strict` decodes with caller `LoadLimits`, then performs semantic scenario validation and returns a validation report. `new` and hashing do not themselves validate market dependencies or scenario semantics. Bare `ScenarioSpec::from_json` validates but is not the versioned byte-loader path.

`ApplicationEnvelope::from_contexts` serializes market, optional model and optional canonical instrument envelopes plus report. Unsupported instrument canonical serialization errors rather than dropping the instrument. The envelope does not include an explicit rate-binding map, selected calendar object, scenario specification, engine/provider cache or complete source/provenance hash; a full replay bundle requires those inputs elsewhere. Market snapshot semantics, including quote-only FX restore, are shared core contracts.

The `json-schema` feature publishes scenario envelope/spec, template metadata, application envelope, horizon result and horizon summary schemas/examples. The schema binary only delegates generation to the core tool; it does not implement an alternate pricing or validation engine. Error kind mapping preserves lookup versus validation versus computation distinctions; structured warning variants describe skip/clamp/approximation/evaluation failures, but no unified severity or result-completeness gate is computed.

## Evidence and alternatives

Owning source: [engine/types.rs](../../../../finstack-quant/scenarios/src/engine/types.rs), [effects.rs](../../../../finstack-quant/scenarios/src/engine/effects.rs), [envelope.rs](../../../../finstack-quant/scenarios/src/envelope.rs), [schema.rs](../../../../finstack-quant/scenarios/src/schema.rs), [error.rs](../../../../finstack-quant/scenarios/src/error.rs), [warning.rs](../../../../finstack-quant/scenarios/src/warning.rs). Inspected [canonical contract test](../../../../finstack-quant/scenarios/tests/canonical_contract.rs) compares golden bytes/hash and attribute insertion-order invariance; inspected engine tests assert changed targets, shock units and correlation clamping. [Report serialization](../../../../finstack-quant/scenarios/tests/report_serialization.rs) locates output shape evidence. These validate transport/report contracts, not numerical equivalence across missing replay inputs.

A complete reproducibility bundle, effective-shock report, separate mutation and price-effect counts, and explicit incomplete-result status are alternatives.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
