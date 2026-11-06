# ADR 0035: A deliberate, minimal public API

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Code standards

## Decision

Expose the smallest coherent public surface that supports intended use. Keep
implementation details private and provide consistent, unambiguous paths and
construction patterns.

Start with minimum visibility. Helpers stay private or `pub(crate)` unless
consumers need them. Each public item has one canonical public path within its
defining crate. Avoid duplicate exports, renamed aliases, glob re-exports, and
preludes. Intentional umbrella module mounts remain supported.

Builder-based types use `Type::builder(...)` as their entry point, without
redundant `Builder::new` or equivalent constructor aliases. Simple value types
can use direct constructors. Separate entry points represent distinct
operations or input forms rather than alternative names for the same
operation.

Group related settings into meaningful parameter structures when positional
signatures become unwieldy. Keep the grouping aligned with the operation's
inputs and responsibilities.

Changes to established public names and paths still follow
[ADR 0003](0003-canonical-public-names.md) and
[ADR 0015](../project/0015-authorized-change-scope.md). This decision defines the
design standard; recording it does not authorize a broad rename, removal, or
signature change to existing APIs. Approved replacements follow
[ADR 0004](../project/0004-development-compatibility.md).

## Rationale

A focused public surface makes the intended operation easier to discover and
reduces duplicate contracts to document, bind, and maintain. Consistent
construction patterns and meaningful input grouping make APIs easier to read
without exposing their implementation structure.

## Examples

| Change or design | Treatment |
| --- | --- |
| Keep a pricing helper `pub(crate)` when only the crate uses it | Appropriate minimum visibility. |
| Expose a type in its public namespace and re-export it again at the crate root | Creates duplicate public paths; use one intentional path. |
| Mount a defining crate's namespace through the umbrella | Supported intentional module mount. |
| Offer `Curve::builder(...)` and `CurveBuilder::new(...)` for the same construction | Redundant entry points. |
| Give a simple value type a direct constructor | Allowed; a builder is not required for every type. |
| Provide a separately named constructor for a genuinely different input representation | Allowed purposeful variant, following canonical naming. |
| Group related solver settings into a parameter structure | Appropriate when it clarifies an unwieldy signature. |
| Remove an established public path during an unrelated cleanup | Requires authorization for that public contract change. |

## Enforcement

- Review new public items and visibility changes against intended consumer use.
- Inspect public paths, re-exports, and construction entry points for redundant
  ways to access the same item or operation.
- Compare public surfaces before and after module moves or visibility changes
  so an item cannot become unnameable without detection.
- Apply the naming, host-parity, and documentation requirements to approved
  public API changes in the same change set.
- Use the shared checks and exception policy in
  [ADR 0034](0034-code-standards-authority.md), preserving independently
  authorized contract protection under
  [ADR 0016](../project/0016-independent-contract-protection.md).

## Implementation notes and references

- [Rust standards guidance](../../../.agents/rules/rust/code-standards.md)
  describes single public paths, minimum visibility, and builder entry points.
- [Agent instructions](../../../AGENTS.md) specify the argument-count rule
  and coordinated changes across Rust, Python, and WASM.
- [Workspace configuration](../../../Cargo.toml) defines the applicable API
  design lints.
- The [public-path checker](../../../scripts/check_single_path.py) reports
  duplicate paths, re-exports, and umbrella reachability. Its comparison mode
  detects lost public items; the presence of the report does not establish
  complete blocking enforcement of this decision.

Implementation audits and check coverage remain tracked in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up). This
record does not change public APIs or claim that every existing surface already
complies.
