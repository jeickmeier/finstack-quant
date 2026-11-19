# FACTOR-019: Credit artifact semantic boundary, provenance, and schemas

**Decision status:** Accepted 2026-11-17 for financial state authority, semantic checks, and replay provenance. [ADR 0047](../../architecture/0047-credit-factor-wire-and-validation-contract.md) owns the accepted wire and loader boundary.

**Implementation status:** Wire marker and validating loaders are implemented for the inspected scope. Issuer state authority, full financial validation, and replay provenance require reconciliation.

**Validation status:** Duplicate issuer lookups were traced in source, and validator and schema fixtures were inspected. Malformed-artifact rejection and cross-host replay still require end-to-end qualification.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Use the calibrated issuer table as the authoritative source of issuer rows and derive the matching view from it. Use stored covariance for systematic risk and validated issuer residual variance for residual risk. Reject contradictory state at financial handoff. [Covariance authority](credit-covariance.md) · [Forecast horizon](credit-horizons.md).
2. Validate the current state before financial use and after relevant mutation under ADR 0047. Check the calibration window and anchor, ordered factor axes, issuer tags and loadings, operative folds, residual state, and any supplied histories. Apply the relevant checks before matching, decomposition, and forecasting. [Bucket folds](credit-buckets.md) · [Snapshot basis](credit-snapshots.md).
3. Preserve effective calibration settings, method version, and immutable identities for input data and the observation calendar so replay can resolve the same inputs and choices. Verify deterministic schema and canonical bytes separately from independent replay and financial qualification. [Replay evidence guidance](../../project/0048-reference-provenance-and-version-pinning.md).

## Observed choice and rationale

Accepted [ADR 0047](../../architecture/0047-credit-factor-wire-and-validation-contract.md) requires the exact namespaced v1 wire marker and explicit semantic validation. Ordinary `CreditFactorModel` serde decodes structure and the marker but does not invoke `CreditFactorModel::validate()`. Consumers must validate current state, or use a validating loader, before financial use and after relevant mutation. Inspected JSON loaders and calibration call validation; the inspected calls do not establish complete Rust, Python and WASM entry-point and consumer coverage.

## Input domain and conventions

The credit root is closed and requires typed `schema=finstack_quant.credit_factor_model/1`, identified by `CreditFactorModelSchema::CURRENT`; missing, malformed and unsupported markers fail during deserialization, without a first-error-ordering promise. No `schema_version` alias is retained. `CreditCalibrationDiagnostics` is explicitly open: unknown keys are accepted and discarded, while known fields retain their requiredness and meaning. In particular, `fold_ups` is computational state; a diagnostic addition that changes computation requires a coordinated semantic contract change. Model metadata includes dates/window, policy, generic series identity, hierarchy, frequency, panel space, weighting, config, issuer rows, anchor, correlation, vol state, optional histories and diagnostics.

## Algorithm stages

1. Model validation rejects duplicate issuer IDs, duplicate hierarchy keys (including built-in/custom collisions), dotted known tag values, and nonconventional BucketOnly betas (pc near 1; levels near 0 or 1 within 1e-12).
2. Check static-correlation unique axes, square finite shape, diagonal within 1e-9 of 1, off-diagonals within 1+1e-10 and symmetry within 1e-9. This check does not prove PSD.
3. Validate embedded FactorModelConfig, then require correlation ID set and vol_state.factors key set to equal declared factors. Correlation order may differ because it is compared as a set.
4. Check each supplied factor-history vector length equals its date-vector length.
5. Calibration builds sorted issuer rows and generic-first/lexical factor order, stores panel-space histories, and forms mode counts, all-member bucket counts, folds, taxonomy, and a generic-fit R² histogram. Negative/>1 R² is clamped into [0,1] for the histogram, while raw quality remains in rows.
6. Schema registration exposes calibration/config/credit artifacts and risk/decomposition contracts under existing factor_model/models namespaces. Credit examples run the actual calibrator, then iterate canonical serialization/reload up to eight times until JSON bytes reach a floating-point fixed point; failure errors.

## Selection and controls

`CreditFactorModel::from_json` structurally decodes and then validates semantics. `from_slice_strict` additionally applies explicit core resource limits and structured contract diagnostics, returning a validation report on success. These are distinct contracts: semantic validation alone does not establish resource-limited loading. `to_json` on `CreditFactorModel` serializes without calling `validate`; the envelope constructor and content hash likewise do not establish semantic validity. Schema ownership remains tied to the published models path and URI despite the factor Rust crate. Authorized development changes replace v1 in place and propagate to all in-workspace consumers and generated artifacts; no compatibility branch is retained. No schema regeneration occurred in this audit.

## Limitations, failures, and fallbacks

The current semantic validator does not comprehensively check beta lengths/finiteness, issuer/anchor/fold consistency, numeric vol-state domains, history date ordering/finiteness/universe, calibration date range, or equality between top-level issuer rows and those duplicated inside the matching config. It does not inspect `diagnostics.fold_ups`; decomposition rejects an out-of-range fold-up level later. Consequently even a validating load establishes only the implemented checks, not every downstream precondition. Consumer validation and use after relevant mutation remain coverage gaps. Unknown diagnostic keys are discarded during typed decoding. Schema shape and canonical hashes do not prove model correctness.

## Alternatives and tradeoffs

Stronger validated constructors/loaders could establish a fuller immutable invariant; consumption-time checks expose later failures and require consistent coverage. Keeping duplicate issuer state enables self-contained generic config use but requires a clear authority and cross-copy checks. These are boundary/reconciliation questions under accepted policy, not permission to retain invalid states.

## Implementation and evidence

- [src/credit/hierarchy.rs](../../../../finstack-quant/factor/src/credit/hierarchy.rs) — inspected implementation.
- [src/envelope.rs](../../../../finstack-quant/factor/src/envelope.rs) — inspected implementation.
- [src/schema.rs](../../../../finstack-quant/factor/src/schema.rs) — inspected implementation.
- [src/credit/calibration/assemble.rs](../../../../finstack-quant/factor/src/credit/calibration/assemble.rs) — inspected implementation.
- [src/credit/calibration/calibrator.rs](../../../../finstack-quant/factor/src/credit/calibration/calibrator.rs) — inspected implementation.

[src/credit/hierarchy.rs](../../../../finstack-quant/factor/src/credit/hierarchy.rs) tests locate strict/from_json validation, wrong-marker/unknown-root rejection during decoding, open diagnostics, factor-universe checks and BucketOnly conventions. [tests/schema_contract.rs](../../../../finstack-quant/factor/tests/schema_contract.rs) and [tests/canonical_contract.rs](../../../../finstack-quant/factor/tests/canonical_contract.rs) locate schema parity and exact-byte/hash fixtures. Generated examples and same-source golden artifacts are contract evidence; they are not independent financial validation.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models) and [central credit artifact validation](../../FOLLOW_UPS.md#credit-artifact-validation).
