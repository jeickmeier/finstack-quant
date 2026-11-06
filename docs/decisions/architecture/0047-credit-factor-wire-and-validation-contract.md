# ADR 0047: Credit-factor wire format and explicit semantic validation

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Specific code decisions

## Decision and scope

The detailed `CreditFactorModel` contract in
[INVARIANTS.md section 8](../../../INVARIANTS.md#8-credit-factor-model-schema-versioning)
is adopted under accepted
[ADR 0012](../standards/0012-serialized-contracts.md),
[ADR 0004](../project/0004-development-compatibility.md), and
[ADR 0041](../standards/0041-type-invariants-and-validation-boundaries.md).
This applies to the credit-factor model artifact, its schema, validating
loaders, calibration entry points, and in-workspace consumers.

Acceptance establishes the wire and validation-boundary contract below; it
does not certify complete implementation or semantic-validation coverage.
The [invariant map](../INVARIANT_MAP.md) records the correspondence.

| Covered clause | Accepted contract |
| --- | --- |
| Marker and root | Require the serialized `schema` field, typed as `CreditFactorModelSchema`, with `CreditFactorModelSchema::CURRENT` identifying exactly `finstack_quant.credit_factor_model/1`. Reject missing, malformed or unsupported markers during deserialization. Keep the v1 root closed in Rust and JSON Schema; reject unknown root fields. |
| Stable schema identity | Keep the schema at `finstack-quant/models/schemas/factor_model/1/credit_factor_model.schema.json` and its published URI under `https://finstack_quant.dev/schemas/factor_model/1/`. Moving Rust ownership does not move that identifier. |
| Semantic validity | Ordinary Rust deserialization does not establish complete model validity or call `CreditFactorModel::validate()`. Consumers must validate the current state, or use a validating loader, before financial use and after relevant mutation. Python/WASM JSON loaders and calibration entry points retain this boundary. |
| Development replacement | Authorized root changes replace v1 in place, update all in-workspace consumers and generated artifacts in the same change, and retain no compatibility branch or alternate marker. Final-release compatibility remains the separate ADR 0004 decision. |
| Explicit nested extension | Only a declared open nested type omitting both `deny_unknown_fields` and schema-level `additionalProperties: false` may accept new keys without a root version bump. `CreditCalibrationDiagnostics` is such a type: unknown keys are accepted and discarded, not retained as arbitrary extension data. |
| Diagnostic semantics | Known fields retain their declared requiredness and meaning. Preserve `fold_ups` and its computational role in decomposition. A new diagnostic that changes computation requires a coordinated semantic contract change; openness does not authorize silent changes to financial results. |
| Canonical changes | Make field removals, type or semantic changes, required-field additions, and root-key additions at the canonical Rust serde type, then regenerate v1 deterministically and propagate the authorized change completely. |

The catalogue retains the detailed rules. This decision does not expand open
extensions to other objects or promise unknown-key round-trip preservation.

## Field and failure timing

The required `schema` field and exact typed marker replace the former
`schema_version` description. Do not retain `schema_version` as an alias or a
second format. Marker rejection occurs before a model is successfully
returned by deserialization; it does not promise that the marker error is
always reported first when other fields are also malformed.

Successful structural decoding still requires explicit semantic validation.
This field and failure contract matches the inspected
[hierarchy](../../../finstack-quant/factor/src/credit/hierarchy.rs),
[schema](../../../finstack-quant/models/schemas/factor_model/1/credit_factor_model.schema.json),
and [Serde marker policy](../../SERDE_STABILITY.md#marker-policy). The former
catalogue's `CreditFactorModel::SCHEMA_VERSION` and validation-only marker
rejection descriptions are superseded within this decision's scope.

`CreditFactorModel::from_json` performs structural decoding and then semantic
validation. `from_slice_strict` additionally uses explicit resource limits and
structured contract diagnostics, returning a validation report on success.
These are distinct loading contracts; semantic validation alone does not
establish resource-limited loading.

## Rationale and alternatives

A typed marker prevents unsupported artifact formats from becoming a
successfully decoded model. Keeping semantic validation explicit preserves the
distinction between structural decoding and model validity. A closed root
detects misspelled or unsupported structural fields, while the named open
diagnostics object permits additive diagnostic information without weakening
known fields used by computation.

The former `schema_version` representation with marker rejection deferred
until semantic validation would expose a different public wire and failure
contract. Supporting both representations conflicts with the development
replacement policy. Making the entire root open would discard the accepted
closed-root invariant.

## Verification criteria and evidence

Enforcement evidence must cover missing, malformed and wrong-marker
rejection without promising first-error ordering; unknown-root rejection;
accepted and discarded unknown diagnostic keys; preservation of known
diagnostic meaning; and semantic rejection through every validating loader,
calibration entry point and consumer boundary, including malformed fold-up
records and relevant post-load mutation. It must distinguish the ordinary and
resource-limited loading contracts and cover deterministic schema generation
and coordinated host behavior against the accepted field/failure contract.
Existing hierarchy tests and
[schema-contract tests](../../../finstack-quant/factor/tests/schema_contract.rs)
are evidence locations, not proof of complete enforcement. Implementation and
coverage work is tracked in the
[credit artifact follow-up](../FOLLOW_UPS.md#credit-artifact-validation);
guidance corrections are tracked in the
[schema reconciliation](../FOLLOW_UPS.md#credit-factor-guidance).
