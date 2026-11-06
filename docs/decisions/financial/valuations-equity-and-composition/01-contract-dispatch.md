# VALUATIONS-EQUITY-AND-COMPOSITION-01: Instrument contracts, model dispatch and metric interpretation

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-01`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

JSON construction and pricing use one closed tagged-instrument registry and the merged `STANDARD` family. Method choice is resolved from the actual instrument when the caller requests `default`; registry availability alone is not a default or acceptance decision. Strict typed contracts prevent a malformed payload from selecting unintended financial behavior.

## Scope and input domain

The umbrella's serialized instrument entry points, model/metric introspection, schema validation and schema generation; equity-family registration. This record covers dispatch and validation, not the mathematical kernels owned by other records.

## Algorithm

1. Parse the versioned envelope under `LoadLimits` and `MAX_JSON_BYTES`, deserialize its canonical type tag, then run the instrument's pricing invariants before exposing a boxed object.
2. A bare-spec constructor rejects already-tagged payloads/envelopes. Typed validation additionally checks the requested tag; unknown fields/tags and unsupported conversions fail.
3. Metric override patches are first deserialized as `MetricPricingOverrides`. Top-level patch fields replace existing fields; `bump_config` merges fieldwise. Apply the patch before final pricing validation.
4. Resolve `default` via `Instrument::default_model`; otherwise parse the explicit model key. Use a supplied registry or `STANDARD`, resolve requested metric IDs, optionally deserialize market history and dispatch `price_with_metrics` with the validated marker.
5. Metric metadata decodes composite components and units. Numeric coordinates recognize key-rate labels or finite numbers; only two nonempty components of bucketed DV01/CS01 receive the bucket flag. Standard Greek presentation requests the applicable subset and rejects nonfinite returned values.
6. Optional JSON-schema validation uses cached compiled validators and embedded resources. Schema artifacts/examples are generated from the same tagged registry, with example roundtrip checks. Schema shape validation and runtime financial invariants remain distinct.

## Selection, controls and conventions

The equity family registers Black-labelled vanilla, Heston Fourier, Heston MC, two PDE adapters, rough Heston Fourier/MC and rough Bergomi MC explicitly, plus GBM autocall/cliquet and DCF. The umbrella adds composite, basket and property types. `json-schema` controls generation; `jsonschema-validate` controls schema validators. The bond JSON adapter validates the supplied cashflow schedule before `Bond::from_cashflows`; clean price remains percent of par.

## Limitations, failures and alternatives

A registered alternative is selectable but not automatically calibrated or domain-approved. `pretty_instrument_json` only parses and pretty-prints JSON; it does not validate an instrument. Schema examples are construction evidence, not price benchmarks. Direct native APIs and serialized routes may have different construction paths; the alternative of one permissive decoder is deliberately absent in these implementations.

## Evidence

The inline tests in [json.rs](../../../../finstack-quant/valuations/src/pricer/json.rs) cover override merging, typed/envelope errors, defaults and metric units; [mod.rs](../../../../finstack-quant/valuations/src/pricer/mod.rs) checks registry disjointness, collisions and family/umbrella agreement. [schema.rs](../../../../finstack-quant/valuations/src/schema.rs) contains schema/example contract checks. These are structural/dispatch evidence, not model accuracy. No host-wrapper parity or runtime gate was executed in this audit. No separate financial canonical reference is needed for serialization; model rationale is in the linked mathematical records.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [family.rs](../../../../finstack-quant/valuations/equity/src/family.rs)
- [lib.rs](../../../../finstack-quant/valuations/equity/src/lib.rs)
- [json.rs](../../../../finstack-quant/valuations/src/pricer/json.rs)
- [mod.rs](../../../../finstack-quant/valuations/src/pricer/mod.rs)
- [json_loader.rs](../../../../finstack-quant/valuations/src/instruments/json_loader.rs)
- [bond_json.rs](../../../../finstack-quant/valuations/src/instruments/fixed_income/bond_json.rs)
- [schema.rs](../../../../finstack-quant/valuations/src/schema.rs)
