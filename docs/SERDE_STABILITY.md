# Serde Stability Policy

This document is the contract for wire-format stability of the `finstack`
workspace. It tells a downstream consumer (data warehouse, risk database,
Python / WASM pipeline) what is safe to persist and under what conditions
those persisted bytes must be upgraded.

## Status

Pre-1.0. Breaking changes across minor versions are possible. Every breaking
change must be documented in `CHANGELOG.md` and, for types tracked below,
gated by a bumped `schema_version`.

## Scope

This policy applies to every Rust type in the workspace that:

1. Derives or hand-implements `serde::Serialize` and/or `serde::Deserialize`,
   AND
2. Is part of the public API (i.e. reachable from an item exported from the
   crate root), AND
3. Is intended to be persisted — i.e. written to Parquet / JSON / a database —
   rather than strictly for in-process inter-thread communication.

Types that are strictly intermediate (private module items, dev-only helpers,
in-process DTOs between Rust and a binding layer that always re-serializes in
one process) are outside this contract.

## The contract

For every in-scope type:

- **Additive changes are allowed** (new `Option<T>` field, new enum variant)
  as long as:
  - The new field is annotated `#[serde(default)]` or `#[serde(default =
    "…")]`, AND
  - Deserializing an older payload produces a value that is semantically
    equivalent to the pre-change behavior, AND
  - The change is recorded in `CHANGELOG.md`.

- **Non-additive changes** (rename a field, change a field type, reorder or
  remove an enum variant, tighten a validation invariant) require:
  - A `schema_version` bump on the type (see below), AND
  - A `CHANGELOG.md` entry explaining the migration path, AND
  - A migration helper (either a `serde(alias = "…")` for the simple rename
    case, or a `From<OldShape> for NewShape` helper for complex cases).

- **Field renames MUST prefer `#[serde(alias = "old_name")]`** over a hard
  rename when the old name has ever shipped. Do not silently rename.

- **Enum variant additions MUST NOT change existing variant discriminants or
  tag values.** Add new variants at the end.

- **`#[non_exhaustive]` on a public error enum or result type** is expected
  unless there is a specific reason not to — this is the workspace default.

## Schema-versioned result types

The following types carry an explicit `schema_version: u32` field so that
consumers reading persisted payloads can detect a mismatch and refuse /
upgrade / fall back rather than silently misinterpreting bytes. The
corresponding `const` lives in the same module and is the source of truth.

| Type | Module | Const | Current version |
|---|---|---|---|
| `ValuationResult` | `finstack_valuations::results` | `VALUATION_RESULT_SCHEMA_VERSION` | 1 |
| `StatementResult` | `finstack_statements::evaluator::results` | `STATEMENT_RESULT_SCHEMA_VERSION` | 1 |
| `PortfolioResult` | `finstack_portfolio::results` | `PORTFOLIO_RESULT_SCHEMA_VERSION` | 1 |
| `PortfolioOptimizationResult` | `finstack_portfolio::optimization::result` | `PORTFOLIO_OPTIMIZATION_RESULT_SCHEMA_VERSION` | 1 |
| `CreditFactorModel` | `finstack_valuations::factor_model::credit` | `"finstack.credit_factor_model/1"` (string tag, not a `u32` const) | 1 |

### When to bump `schema_version`

Bump (i.e. increment the `const`) in any of these cases:

- A required field is removed or renamed without `#[serde(alias)]`.
- A field's serialized type changes (`f64` → `Money`, `String` → `enum`).
- A field's semantic meaning changes (same name, different interpretation).
- An enum variant is removed or its tag value changes.
- A validation invariant is tightened such that older-serialized values would
  now round-trip-fail (e.g. a field gains a `deny_unknown_fields` sibling).

Do NOT bump for:

- Adding a new field with `#[serde(default)]`.
- Adding a new enum variant at the end.
- Adding a new `impl` block or deriving a new trait.
- Documentation changes.
- Internal refactors that don't touch the serialized shape.

### How to bump

1. Increment the corresponding `*_SCHEMA_VERSION` const in the owning module.
2. If the change is non-trivial, add a `pub fn upgrade_v{N}_to_v{N+1}(old:
   serde_json::Value) -> crate::Result<serde_json::Value>` helper next to the
   type so downstream tools can migrate persisted payloads.
3. Record the bump in `CHANGELOG.md` under `### Changed`, referencing:
   - The old and new version numbers.
   - The semantic change.
   - The migration path (or that old payloads now fail to deserialize, with an
     error type the consumer can match).

### How consumers should read versioned payloads

```rust
use finstack_valuations::results::{
    ValuationResult, VALUATION_RESULT_SCHEMA_VERSION,
};

let payload: ValuationResult = serde_json::from_str(&bytes)?;
if payload.schema_version > VALUATION_RESULT_SCHEMA_VERSION {
    // Refuse: binary is older than data. Upgrade finstack, don't plow through.
    return Err(/* forward-incompatible error */);
}
// payload.schema_version < CURRENT is handled by `#[serde(default)]` and any
// `alias`es / migration helpers the type provides.
```

## Types outside the schema-versioned set

Everything else under `pub` serde types in the workspace follows the
"additive changes only between minor versions" rule, but does not (yet) carry
a version tag. If you persist them, pin a specific workspace version in your
consumer or be prepared to handle deserialization errors on upgrade.

Notable in this category:

- `finstack_valuations::results::ValuationDetails`
  (enum of structured pricing details; variants may be added)
- `finstack_portfolio::valuation::PortfolioValuation`
  (sub-envelope of `PortfolioResult`)
- `finstack_portfolio::factor_model::whatif::{WhatIfResult, StressResult}`
  (no versioning yet — track upstream)
- All `*Spec`, `*Config`, `*Envelope` types used as inputs to pricing,
  calibration, scenarios, and statements. These are user-authored payloads;
  the workspace treats added fields as additive only.

### Credit factor hierarchy types (additive, no schema-version constant)

The following types were introduced with the credit factor hierarchy feature.
They follow the additive-only rule; new `Option<T>` fields may be added between
minor versions without a schema-version bump.

- `CreditFactorAttribution` (`finstack_attribution::credit_factor`) —
  additive, opt-in field on `PnlAttribution`; deserializing an older payload
  (missing field) produces `None`.
- `CreditCarryDecomposition` (`finstack_attribution::credit_factor`) —
  additive, opt-in field on `PnlAttribution`; same rule as above.
- `SourceLine` (`finstack_attribution::credit_factor`) — custom
  `Deserialize`: accepts both legacy `Money` shape and new tagged shape for
  backward compatibility.
- `PositionResidualContribution` (`finstack_valuations::factor_model::credit`) —
  additive, opt-in field on `RiskDecomposition`.
- `CreditCalibrationInputs`, `CreditCalibrationConfig`
  (`finstack_valuations::factor_model::credit_calibration`) — versioned serde
  (round-trippable) but carry no `schema_version` constant yet; safe to
  persist when the workspace version is pinned.

Bumping any of these to a versioned shape is a planned follow-up.

## MSRV and toolchain

- Workspace `rust-version = "1.90"` (see root `Cargo.toml`).
- MSRV bumps are allowed in a minor release and must be recorded in
  `CHANGELOG.md`. A consumer pinning an older toolchain should pin the
  workspace version accordingly.

## What is NOT covered

- **Python wheel ABI** across Python minor versions. PyO3 and the Python
  ABI handle that.
- **WASM binary layout** across `wasm-bindgen` versions. Regenerate the `pkg/`
  output whenever the Rust side changes.
- **In-memory layout of Rust structs.** `#[repr(Rust)]` is the default and
  layout is not a stability contract. Do not `mem::transmute` finstack types.
- **Benchmarks / criterion baselines.** Those are throwaway artifacts.

## Getting clarity

If a change you're about to make seems ambiguous under this policy, the
default is: treat it as breaking, bump the schema version, document it in
`CHANGELOG.md`, and add a migration note. Consumers can't un-persist bad
assumptions.
