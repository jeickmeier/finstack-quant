# VALUATIONS-EQUITY-AND-COMPOSITION-01: Instrument contracts, model dispatch and metric interpretation

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-01`.
- **Decision status:** Accepted 2026-11-14 — complete instrument loading, model dispatch and metric interpretation package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Source, primary specifications and existing evidence inspected. Independent duplicate-key and example-scale checks completed. Runtime and host qualification pending.

## Accepted decision for checked instrument loading, model dispatch and metrics

1. **Use one strict Rust-owned instrument contract.** Require the exact
   `finstack_quant.instrument/1` envelope and a registered canonical type
   tag and typed spec. The bare-spec constructor requires an explicit tag and
   an unwrapped spec. Typed restoration requires the same discriminator as
   the requested type. Apply the accepted strict raw JSON admission before
   materializing maps, including nested objects and patches. Reject duplicate
   keys, closed-type unknown fields, invalid numeric representations and
   unsupported versions. A prebuilt native or host object can establish only
   the current tree. Retain the 16 MiB instrument entry limit and the
   aggregate limit at batch admission. Enforce the applicable shared depth
   and actual cardinality limits. Caller controls may tighten the enclosing
   limits. Include the bond cashflow JSON helper in the same checked loading
   path. Validate its schedule through the owner and preserve explicit quote
   compounding and clean price in percent of par.
   [Strict numeric and raw JSON admission](../core-numerics/numeric-wire-and-canonical.md),
   [bounded contract loading](../core-numerics/contracts-and-diagnostics.md),
   [JSON object interoperability](https://www.rfc-editor.org/rfc/rfc8259#section-4),
   [bond cashflow JSON helper](../../../../finstack-quant/valuations/src/instruments/fixed_income/bond_json.rs).

2. **Merge metric controls before complete pricing validation.** Omission
   preserves the stored value. Replace each supplied top-level control and
   merge `bump_config` by supplied field. An explicit null clears only an
   admitted nullable field. A valid scoped patch may replace a saved metric
   field before final typed admission. Reject unknown, malformed or
   inapplicable controls. The patch authority covers metric pricing fields
   only. Apply one checked Rust path to native, serialized and host entries
   and validate the entire resolved contract and all applicable pricing
   controls before execution. Internal admission reuse is private and bound
   to the admitted request. [Accepted pricing lifecycle](../valuations-framework/pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle).

3. **Resolve one exact instrument and model pair.** Use the explicit
   registry when supplied and otherwise use the owning family. `STANDARD`
   is the built-in catalogue and owns the umbrella instrument types. The
   `default` request resolves only via documented product policy on admitted
   terms. Require a concrete model where that policy is absent. A named
   model must match its registered pair. Report supported alternatives for a
   missing pair and propagate the selected calculation's failure. Preserve
   registry, model and actual basis through dependent repricing. Discovery
   lists registered capabilities. Calibration, eligibility and particular
   equity-model choices remain with the product owners.
   [Registry composition](../../../../finstack-quant/valuations/src/pricer/mod.rs),
   [related vanilla selection review](0027-vanilla-option-model-selection.md).

4. **Interpret metric coordinates by their declared roles.** Preserve the
   exact canonical key, ordered decoded coordinates and requested display
   order. Enforce admitted encoding, a nonempty base, coordinate count and
   declared roles. A coordinate becomes numeric only on an axis declared
   numeric. In `bucketed_dv01::10y::3m`, `10y` is a curve identifier and
   `3m` is a tenor of `.25` years. Preserve admitted base and series keys.
   Mark bucket rows only on owner-qualified complete coordinates. A
   well-formed custom display key may have unknown unit. Actual calculation
   authority, currency, units, shock increment and model basis come from the
   selected producer. [Accepted metric orchestration](../valuations-framework/metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting),
   [observed catalogue](../../../../finstack-quant/valuations/src/pricer/catalog.rs).

5. **Select the applicable standard Greek menu for the prepared request.**
   Retain the ordered menu of delta, gamma, vega, theta, rho, foreign rho,
   vanna and volga. Use the selected model, lifecycle and provider's declared
   applicability. Preserve true zero and propagate a missing, failed or
   nonfinite eligible result. Explicit unsupported metric requests error.
   Use the same Rust presentation and results from Python and WASM.
   [Observed presentation](../../../../finstack-quant/valuations/src/pricer/json.rs),
   [accepted option risk basis](../valuations-framework/option-sensitivities.md).

6. **Generate schema and example artifacts from the Rust contract.** Keep
   optional Draft 2020-12 shape validation over the exact versioned embedded
   schemas, with cached compilation outcomes, bounded diagnostics and the
   actual failing phase and path. Validate each example through the schema
   and Rust owner, test round-trip meaning, and reconcile current native,
   Python, WASM and UI artefacts. Pretty-printing establishes formatting
   only. Correct the composite-history example from `1.0125` to `101.25`
   for the existing base 100 and period return `.0125`, and regenerate the
   affected outputs during implementation. Record 24 owns composite history
   economics. [Canonical schema publication](../core-numerics/schema-publication.md),
   [JSON Schema 2020-12](https://json-schema.org/draft/2020-12/json-schema-core),
   [current example](../../../../finstack-quant/valuations/src/schema/examples.rs),
   [composite history](24-composite-valuation-history.md).

## Observed behavior

### Choice and rationale

JSON construction and pricing use one closed tagged-instrument registry and the merged `STANDARD` family. Method choice is resolved from the actual instrument when the caller requests `default`; registry availability alone is not a default or acceptance decision. Strict typed contracts prevent a malformed payload from selecting unintended financial behavior.

### Scope and input domain

The umbrella's serialized instrument entry points, model/metric introspection, schema validation and schema generation; equity-family registration. This record covers dispatch and validation, not the mathematical kernels owned by other records.

### Algorithm

1. Parse the versioned envelope under `LoadLimits` and `MAX_JSON_BYTES`, deserialize its canonical type tag, then run the instrument's pricing invariants before exposing a boxed object.
2. A bare-spec constructor rejects already-tagged payloads/envelopes. Typed validation additionally checks the requested tag; unknown fields/tags and unsupported conversions fail.
3. Metric override patches are first deserialized as `MetricPricingOverrides`. Top-level patch fields replace existing fields; `bump_config` merges fieldwise. Apply the patch before final pricing validation.
4. Resolve `default` via `Instrument::default_model`; otherwise parse the explicit model key. Use a supplied registry or `STANDARD`, resolve requested metric IDs, optionally deserialize market history and dispatch `price_with_metrics` with the validated marker.
5. Metric metadata decodes composite components and units. Numeric coordinates recognize key-rate labels or finite numbers; only two nonempty components of bucketed DV01/CS01 receive the bucket flag. Standard Greek presentation requests the applicable subset and rejects nonfinite returned values.
6. Optional JSON-schema validation uses cached compiled validators and embedded resources. Schema artifacts/examples are generated from the same tagged registry, with example roundtrip checks. Schema shape validation and runtime financial invariants remain distinct.

### Selection, controls and conventions

The equity family registers Black-labelled vanilla, Heston Fourier, Heston MC, two PDE adapters, rough Heston Fourier/MC and rough Bergomi MC explicitly, plus GBM autocall/cliquet and DCF. The umbrella adds composite, basket and property types. `json-schema` controls generation; `jsonschema-validate` controls schema validators. The bond JSON adapter validates the supplied cashflow schedule before `Bond::from_cashflows`; clean price remains percent of par.

### Limitations, failures and alternatives

A registered alternative is selectable but not automatically calibrated or domain-approved. `pretty_instrument_json` only parses and pretty-prints JSON; it does not validate an instrument. Schema examples are construction evidence, not price benchmarks. Direct native APIs and serialized routes may have different construction paths; the alternative of one permissive decoder is deliberately absent in these implementations.

### Evidence

The inline tests in [json.rs](../../../../finstack-quant/valuations/src/pricer/json.rs) cover override merging, typed/envelope errors, defaults and metric units; [mod.rs](../../../../finstack-quant/valuations/src/pricer/mod.rs) checks registry disjointness, collisions and family/umbrella agreement. [schema.rs](../../../../finstack-quant/valuations/src/schema.rs) contains schema/example contract checks. These are structural/dispatch evidence, not model accuracy. No host-wrapper parity or runtime gate was executed in this audit. No separate financial canonical reference is needed for serialization; model rationale is in the linked mathematical records.

## Independent checks and qualification

An independent JSON probe confirmed that map materialization can erase a
duplicate nested `rate_bump_bp` before a typed overlay sees it. A pairs-level
probe rejected the duplicate. Decimal arithmetic reproduced `101.25` for a
1.25% return from base 100. Implement and qualify the actual Rust paths for
duplicates, caps, tagged and typed admission, scoped null and merge behavior,
exact registry routing and failure propagation, typed coordinates, sparse
Greek applicability and versioned schema/host round-trips. The probes checked
the convention. Rust runtime and host suites remain unrun.

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
