# MARGIN-COLLATERAL-IM-001 — Registry composition and contractual defaults

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: five embedded JSON assets supply schedules, collateral, defaults, CCP proxies and historical SIMM parameters. Configuration is composed before typed parsing. Proposed rationale: one explicit parameter source makes financial defaults inspectable; it does not establish that a named regulatory profile applies to a user or transaction.

## Input domain and conventions

The overlay is a JSON value in `FinstackConfig.extensions["margin.registry.v1"]`. Schedule and haircut rates are decimal fractions; SIMM weights have class-specific bp/percentage-point units, despite the registry module's broad “rates are decimal” comment. Raw monetary defaults become the same numeric amount in the caller's chosen currency; no EUR-equivalent conversion occurs.

## Ordered algorithm

1. Parse the five `include_str!` files into one root. Recursively merge objects; replace arrays, scalars and null values wholesale. Supplying an `entries` array replaces that section's entire array, not just named entries.
2. Parse every section even when the requested calculator uses only one. Missing required sections or unknown fields in typed records fail. `schema` and `version` are optional wire fields and are not checked for expected values.
3. Expand each record's `ids` into map entries. Duplicate schedule/collateral/CCP/SIMM IDs fail; schedule rate duplicates, asset-class default duplicates and canonical correlation duplicates overwrite earlier entries. CCP requires exactly one flagged default with a nonempty IDs list.
4. Embedded data are cached with `OnceLock`; the fallible accessor returns errors. The infallible accessor used by selected collateral/CCP conveniences panics if the embedded asset cannot load. Configured builds are separately resolved and do not mutate the cache.
5. Defaults are copied, not calibrated: VM threshold 0, MTA 500,000, rounding 10,000, IA 0, daily calls and T+1; SIMM/schedule IM 10 days, threshold 50,000,000, MTA 0 and segregation; cleared IM 5 days, zero threshold, no segregation; haircut IM 2 days. Cleared VM overrides rounding to 1 and lag to 0.

## Selection, controls and limitations

`ScheduleImCalculator::from_finstack_config`, `SimmCalculator::from_finstack_config`, configured collateral/CSA/OTC constructors consume the overlay. `CsaSpec::with_im` resolves embedded methodology defaults again; `ClearingHouseImCalculator` uses embedded data or a direct `CcpParams` override. The standard/regulatory/CCP notification-response-dispute-grace timing values are metadata: respectively (13h,2h,2d,1d), (13h,2h,1d,0d), (10h,1h,1d,0d); VM generation does not implement intraday deadlines or disputes.

Registry parsing and direct constructors have different validation boundaries; parsed configuration does not establish uniform consumer validation.

## Alternatives and tradeoffs

A complete immutable parameter snapshot with explicit source/version identity would avoid overlay ambiguity but require more authoring. ID-aware array merging would be a different configuration contract. Caller-specified contractual amounts avoid treating generic major-currency defaults as legal maxima; this audit adopts neither alternative.

## Source and validation evidence

Embedded assets and all parser/merge paths were inspected. Existing registry tests exercise loading and rejected overlays. The local data README is a provenance lead, not independent proof of parameter identity or regulatory applicability.

Implementation and data: [src/registry/mod.rs](../../../../finstack-quant/margin/src/registry/mod.rs), [src/registry/embedded.rs](../../../../finstack-quant/margin/src/registry/embedded.rs), [src/registry/merge.rs](../../../../finstack-quant/margin/src/registry/merge.rs), [src/registry/wire.rs](../../../../finstack-quant/margin/src/registry/wire.rs), [src/registry/validation.rs](../../../../finstack-quant/margin/src/registry/validation.rs), [data/margin/defaults.v1.json](../../../../finstack-quant/margin/data/margin/defaults.v1.json), [data/margin/README.md](../../../../finstack-quant/margin/data/margin/README.md).

Existing evidence: [src/registry/mod.rs](../../../../finstack-quant/margin/src/registry/mod.rs), [tests/simplification_contracts.rs](../../../../finstack-quant/margin/tests/simplification_contracts.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
