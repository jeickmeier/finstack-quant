# ADR 0023: Shared host-neutral tabular exports

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Specific code decisions

## Decision

`core::table::TableEnvelope` remains the canonical serializable columnar
surface. pandas, Polars, and Arrow integration happens through boundary
adapters. Introducing a competing public table contract or making a DataFrame
engine part of the core architecture requires an explicit decision.

This governs tabular interchange. Internal algorithms can continue using
suitable typed structs, slices, matrices, and specialized numerical
representations. The export contract does not require internal calculations
to materialize tables or pass through serialization.

Supported conversions preserve the agreed column names and ordering, types,
missing values, column roles, and required metadata. A conversion must not
silently discard information required by its contract.

## Rationale

A shared host-neutral surface keeps tabular meaning independent of a
particular language or DataFrame engine. Boundary adapters provide useful
host integration without giving a reporting dependency ownership of the
core data model or introducing duplicate export contracts.

## Examples

| Change or use | Treatment |
| --- | --- |
| Export portfolio results through `TableEnvelope`, then convert them to pandas | Appropriate. |
| Convert an envelope to an Arrow `RecordBatch` in the supporting adapter | Appropriate. |
| Add Polars to `core` to simplify one reporting workflow | Requires an architectural decision. |
| Introduce another public table container for the same export purpose | Requires an architectural decision. |
| Silently lose nulls, column roles, or required metadata during conversion | Violates the conversion contract. |

## Enforcement

- Verify column names and ordering, types, missing values, and required
  metadata across supported conversions.
- Use dependency checks to protect the separation between the core contract
  and host adapters.
- Keep serialized meaning aligned with
  [ADR 0012](../standards/0012-serialized-contracts.md) and missing-data
  semantics aligned with [ADR 0009](../financial/conventions/0009-missing-and-invalid-data.md).
- Test the conversion directions that are actually supported; do not infer a
  bidirectional or zero-copy promise from the existence of an export adapter.

## Implementation notes and references

The [core guide](../../../finstack-quant/core/README.md) names
`TableEnvelope` as its canonical serializable columnar surface and excludes
DataFrame engines from core. The [table implementation](../../../finstack-quant/core/src/table.rs)
defines ordered columns, column roles, and metadata.

The [Arrow adapter](../../../finstack-quant/arrow-interchange/README.md) is
currently export-only. Arrow import, IPC support, and zero-copy guarantees
require separate scope and evidence. Acceptance does not certify every
conversion as lossless or add unsupported import capabilities. Export audits
and conversion coverage are tracked in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
