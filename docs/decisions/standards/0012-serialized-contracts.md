# ADR 0012: Keep serialized contracts explicit and Rust-owned

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Code standards

## Decision

Rust defines the serialized structure: field names, types, required fields,
enum values, version identifiers, and the meaning of omitted or null values.
Python, WASM, and the UI preserve that contract. Changing its meaning requires
explicit agreement.

Round trips preserve agreed data meaning for types that support both
serialization and deserialization. This does not require adding loaders to
intentional output-only views. Exact byte stability applies only where
canonical serialization or hashing is explicitly promised.

During development, an approved format replacement removes the superseded format.
Before the first final release, the compatibility and migration policy must
be revisited for support across a couple of releases. The exact support
window and mechanisms remain open under ADR 0004.

## Rationale

A shared wire contract prevents hosts from interpreting the same payload
differently. Separating ordinary JSON meaning from promised canonical bytes
allows harmless representation changes without weakening persisted identity.

## Examples

| Change | Treatment |
|---|---|
| Generate schemas and TypeScript types from their designated Rust source | Required approach |
| Rename a serialized field only in a Python wrapper | Violates this decision |
| Replace an unavailable `null` with `0` | Violates this decision unless explicitly authorized by that field's contract |
| Reorder properties in ordinary JSON with no ordering promise | Allowed |
| Change bytes used by an explicitly canonical serialization or content hash | Requires a contract change |

## Enforcement

Required enforcement combines generated-schema checks, applicable round-trip
tests, and comparisons against approved schema expectations. Canonical
formats also need fixed byte/hash examples.

Generation checks verify agreement with current generators. Updating a
generator and regenerating its outputs does not independently authorize a
contract change. Protection of approved expectations remains subject to
ADR 0016.

## Implementation notes and references

- [Serde contract policy](../../SERDE_STABILITY.md) specifies Rust ownership,
  representation rules, and intentional serialize-only output views.
- [Contract catalog](../../CONTRACTS.md) inventories persisted contracts.
- [Generation tasks](../../../mise.toml) include `gen-check` and schema checks.
- [ADR 0004](../project/0004-development-compatibility.md) records the
  development-only replacement policy and required pre-release reconsideration.
- [ADR 0047](../architecture/0047-credit-factor-wire-and-validation-contract.md)
  applies this contract to the `CreditFactorModel` wire format, schema marker,
  closed root, open diagnostics extension, and semantic validation boundary.
