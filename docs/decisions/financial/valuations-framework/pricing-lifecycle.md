# VALUATIONS-FRAMEWORK-01 — Selected-model dispatch, valuation lifecycle, and metric enrichment

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and selection

A price is selected by `(InstrumentType, ModelKey)`. `Instrument::price_with_metrics` uses an explicit request model or the instrument's default (trait default Discounting), and an explicit registry or the instrument family's registry. Unknown pairs fail with available alternatives; no next-best model fallback occurs. Registration rejects duplicates, while replacement is an explicit operation. The generic instrument adapter calls `base_value`, not the fully wrapped `value`, so lifecycle adjustments are applied once.

## Ordered algorithm

1. Validate instrument invariants and instrument, metric and scenario override blocks. An internal `instrument_validated` flag can skip this; the public instrument convenience entry resets it to false. Resolve the effective valuation date through the instrument's hook; its default is the requested date.
2. Invoke the selected base pricer at the effective date. Stamp the result date and request metadata. Apply scenario price shock as `V×(1+shock)` once; finite shocks below −1 are not forbidden by generic validation. Spread shock is only stored here and must be consumed by a supporting product; it is not a generic post-price transformation.
3. Record model, requested date, declared market dependencies and scenario price shock; when metrics are requested, record resolved configured bump sizes. These are request controls, not a complete executable model/calibration snapshot, and can differ from effective boundary-shrunk Greek stencils.
4. Preserve pricer-provided explanation or construct a bounded cashflow trace. An unavailable decomposition becomes an explanatory trace entry rather than a price failure. A composite instrument can supply its own leg metrics/details; leg requests intentionally clear the parent model override so child products choose their own model.
5. For non-Discounting models, requested spread-equivalent metrics may run on the instrument's `metrics_equivalent` representation, while remaining risk runs on the original instrument. Both use the same selected dispatch, but only original-instrument metrics receive the pricer seed. The product owns which equivalent transformation is valid.
6. Build metric context with selected dispatch, config, market history/provider, instrument overrides, pricer seed, instrument seed and selected-pricer seed hook. Standard pricer measures enter the computed cache. Compute requested metrics, include custom/bucketed and declared diagnostic side outputs, then merge original pricer measures last, so original values win duplicate keys.

Raw repricing follows validation/date resolution/scenario multiplication too, while using a raw floating-point PV path where implemented. The default raw adapter extracts a Money price; it is not automatically a separate high-precision numerical kernel. Family registration and typed downcasts constrain model ownership; model-key names alone do not establish algorithm or supported features.

Dependency merging deduplicates each curve role, scalar, series and FX pair while preserving first insertion order. A volatility descriptor is equal only when surface, optional spot and strike **bit pattern** agree; distinct strikes on one surface remain distinct, while unique-surface extraction preserves first-descriptor order. Present-surface filtering probes ordinary surfaces only, ignores missing candidates and does not discover cubes or FX-delta surfaces. This matters because shared pricing source resolution supports those other artifacts. Consumers must explicitly provide compatible risk resolution; dependency declaration alone is not proof that all volatility source types receive equivalent bumps.

The `validate_on_deserialize` macro invokes instrument invariants after serde parsing; ordinary constructor/builder validation and explicit override validation remain separate boundaries. The empty-cashflow-provider macro deliberately emits an empty classified schedule, defaulting to no notional and ACT/365F unless supplied; it does not synthesize an option payoff schedule.

## Limitations, alternatives and evidence

[registry.rs](../../../../finstack-quant/valuations/core/src/pricer/registry.rs), [enrichment.rs](../../../../finstack-quant/valuations/core/src/pricer/enrichment.rs), [instrument trait](../../../../finstack-quant/valuations/core/src/instruments/common_impl/traits/instrument.rs), [helpers.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs), [generic adapter](../../../../finstack-quant/valuations/core/src/instruments/pricing/generic.rs), and [family.rs](../../../../finstack-quant/valuations/core/src/pricer/family.rs) are the inspected execution owners. Inline tests provide dispatch/lifecycle behavior evidence, not model accuracy; none was run. This orchestration has no external quantitative standard to claim.

A complete replay artifact would additionally need market/model/calibration state; present provenance should not be described as that artifact. Alternate registries and explicit models are supported choices; automatic failover is not.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
