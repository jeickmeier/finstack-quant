# ADR 0017: Reusable computation library with optional presentation

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Project policy

## Decision

Financial computation remains usable independently of the component registry,
documentation site, or a particular application. UI components are optional
consumers of the engine. Introducing a mandatory hosted service, account, or
application dependency requires an explicit product decision.

Preserve display-only components that render supplied financial results
without loading WASM or requiring a computation provider. Components that
perform calculations may require WASM and the corresponding computation
infrastructure for that capability.

A hosted application may be built around Finstack within an authorized
project; the computation library remains independently usable.

## Rationale

Rust services, Python notebooks, and browser or Node consumers use the engine
in different environments. A presentation-layer convenience must not become a
mandatory dependency of financial computation or force consumers into the
repository's demonstration application.

## Examples

| Change or use | Treatment |
| --- | --- |
| Use Rust or Python valuation functions without installing the UI | Required supported use. |
| Render supplied results in a display-only component without WASM | Preserve this capability. |
| Load WASM for a component that performs financial calculations | Appropriate for that computational capability. |
| Make a pricing API depend on demo-application assets | Violates the boundary. |
| Build a hosted application around the library | Allowed within an authorized project; retain independent library use. |
| Require an online account before local calculations work | Requires an explicit product decision. |

## Enforcement

- Use headless library checks and clean consumer-package tests to detect
  dependencies on UI or documentation application infrastructure.
- Use isolated component installs to protect display-only consumption and
  the actual dependency requirements of computational components.
- Keep presentation and application conveniences out of mandatory engine
  dependencies, consistent with [ADR 0001](../architecture/0001-rust-owned-financial-behavior.md)
  and [ADR 0002](../architecture/0002-crate-boundaries.md).

## Implementation notes and references

The [root README](../../../README.md) describes library and binding consumers
and optional UI components. The [UI removal guide](../../REMOVING_THE_UI.md)
documents the existing test and generation seams that must be adjusted when
removing the registry; deleting its directory alone is not a verified removal
procedure. The [UI contracts](../../../finstack-quant-ui/README.md) describe
display-only components and installed-consumer checks.

This record protects the product boundary. It does not claim that every
consumer configuration or removal path has been tested by this documentation
change.
