# ADR 0024: Explicit extension interfaces and persisted catalog

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Specific code decisions

## Decision

Support custom Rust instruments and pricers through explicit native
interfaces and caller-supplied registries. Persisted instrument types remain
centrally defined and schema-backed. Runtime plugins, arbitrary host-language
callbacks, or external deserialization registries require a separate
architectural decision.

Registration must not silently replace an existing pricer. A caller may
deliberately replace a pricer through the explicit replacement interface in
its own controlled registry.

Native extensibility does not automatically promise persistence or
Python/WASM exposure for every caller-defined type. A supported persisted
type needs its canonical registration, schemas, and applicable host surfaces.
Document the extension capabilities that actually cross each interface.

## Rationale

Native extension points and persisted input interpretation are different
contracts. Treating an arbitrary custom type as automatically serializable,
portable, or executable from input data would introduce new execution,
validation, and compatibility obligations without an explicit design.

## Examples

| Extension or change | Treatment |
| --- | --- |
| Supply a custom Rust pricer through a caller-owned registry | Supported extension mechanism. |
| Register a second pricer under an occupied key | Reject without silently replacing behavior. |
| Explicitly replace a pricer in a caller-controlled registry | Allowed through the deliberate replacement interface. |
| Add a supported persisted instrument type | Update its canonical registration, schemas, and applicable host surfaces. |
| Load an unknown instrument by importing executable Python or JavaScript named in its JSON | Requires an architectural decision. |
| Assume every custom Rust instrument can be reconstructed from a saved portfolio | Unsupported unless that persistence path is explicitly implemented. |

## Enforcement

- Test custom native pricing, duplicate-registration rejection, and deliberate
  replacement as separate behaviors.
- Reject unsupported persisted instrument types through the documented
  failure path.
- Keep schemas and host surfaces aligned when a new persisted type is added,
  following [ADR 0012](../standards/0012-serialized-contracts.md) and the
  exposure scope in [ADR 0005](../standards/0005-python-wasm-parity.md).
- Review changes to extension scope as architecture changes rather than
  treating them as an incidental parser or binding convenience.

## Implementation notes and references

[Family::unregistered](../../../finstack-quant/valuations/core/src/pricer/family.rs)
describes custom native pricing through the instrument or a supplied registry.
[PricerRegistry](../../../finstack-quant/valuations/core/src/pricer/registry.rs)
distinguishes registration from deliberate replacement. The
[instrument JSON catalog](../../../finstack-quant/valuations/src/instruments/json_loader.rs)
centrally defines supported persisted tags.

The [portfolio guide](../../../finstack-quant/portfolio/README.md) states that
reconstruction has no external-registry hook and cannot reconstruct positions
whose custom instruments provide no supported serialized specification.
Audit documentation and tests against these extension boundaries. This ADR
does not add plugin loading, new callback execution, or custom-type persistence.
