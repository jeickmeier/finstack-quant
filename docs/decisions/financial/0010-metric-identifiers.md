# ADR 0010: Metric identifiers and financial attribution

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Financial conventions

## Decision

Metric keys identify the measure, the relevant curve or instrument, and any
applicable bucket. They are public data contracts. Renaming a key or changing
what it refers to requires explicit agreement.

Z-spread CS01 for a bond is attributed to the instrument ID, such as
`cs01::BOND_A`. A generic `z_spread` qualifier cannot replace that identity.
Display labels and formatting may change while the underlying key and
financial attribution remain intact.

## Rationale

Portfolios, reports, and downstream applications consume metric identifiers.
Removing a qualifier or changing its meaning can merge distinct exposures or
attribute a valid number to the wrong financial source.

## Examples

| Key or change | Meaning or treatment |
|---|---|
| `bucketed_dv01::USD-OIS::10y` | DV01 attributed to the USD-OIS curve's 10-year bucket. |
| `cs01::ACME-HZD` | CS01 attributed to the identified hazard curve. |
| `cs01::BOND_A` | Bond z-spread CS01 attributed to instrument `BOND_A`. |
| Replace `cs01::BOND_A` with `cs01::z_spread` | Violates the decision because instrument identity is lost. |
| Display "10-year USD OIS DV01" while retaining the underlying key | Allowed presentation change. |

## Enforcement

- Assert exact qualified keys and their attribution in Rust and bindings.
- Verify preservation through serialization and downstream aggregation.
- Include multiple bonds, curves, and buckets to expose collisions or
  accidentally merged exposures.
- Review key changes as public contract changes even when their numerical
  values remain unchanged.

## Implementation notes and references

The existing API conventions specify these key forms, and the metric ID
implementation supports composite identifiers. This ADR does not replace the
per-metric units and sign conventions or claim that every consumer has been
audited for attribution coverage.

- [Metric key conventions](../../../AGENTS.md).
- [Composite metric IDs](../../../finstack-quant/valuations/core/src/metrics/ids/composite.rs).
- [Metric invariants](../../../finstack-quant/valuations/tests/metrics/invariants.rs).
- [Financial units and signs](0007-units-and-sign-conventions.md).
- [Bond CS01 without a hazard curve](0011-bond-cs01-without-hazard-curve.md).
