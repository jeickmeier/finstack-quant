# ADR 0037: Simple, modular implementation and dependency reuse

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Code standards

## Decision

Choose the simplest implementation that fully meets current requirements.
Keep responsibilities separate, reuse established capabilities, and deliver
working increments that support the intended long-term design.

Introduce abstractions, configuration, and extension mechanisms when current
requirements justify them. Place behavior in its responsible module and share
genuinely common functionality. Preserve the ownership and dependency direction
defined by [ADR 0001](../architecture/0001-rust-owned-financial-behavior.md) and
[ADR 0002](../architecture/0002-crate-boundaries.md).

Inspect existing project utilities and dependency documentation and types
before adding packages or implementing replacements. Prefer maintained
libraries when they reduce total complexity or improve reliability, considering
supported targets and existing dependency policies.

For custom implementations of capabilities available elsewhere, identify the
concrete capability, correctness, performance, or integration requirement that
existing options cannot adequately meet. Reuse is a design choice evaluated
against those requirements, not a requirement to add a dependency for every
small operation.

Build in working layers. Each increment remains usable and contributes to the
intended long-term design. Avoid temporary architecture intended for
replacement, and do not trade a working product for unfinished complexity.

Routine implementation and dependency choices within agreed boundaries remain
autonomous under [ADR 0015](../project/0015-authorized-change-scope.md). This
standard does not independently authorize changes to financial behavior,
public contracts, or architectural boundaries.

## Rationale

Complexity includes the code, configuration, integration, and maintenance
needed to keep a capability correct. Using an existing capability can reduce
that burden, while a clear reason for custom code makes its cost and purpose
reviewable. Cohesive modules and working increments keep the system usable as
its capabilities grow.

## Examples

| Change or choice | Treatment |
| --- | --- |
| Reuse an existing numerical utility through a permitted dependency | Preferred when it meets the operation's requirements. |
| Add a second package without inspecting the equivalent capability already available | Does not satisfy the reuse-first requirement. |
| Add a general plugin framework for hypothetical future pricing modes | Unjustified speculative complexity. |
| Add a maintained library after identifying an unmet requirement and checking target and policy fit | Appropriate when it reduces total complexity or improves reliability. |
| Implement a specialized numerical routine because available options cannot meet its required semantics | Allowed with the concrete requirement and reason documented. |
| Share a domain-specific calculation by moving it into a prohibited lower-level owner | Requires an architectural decision; reuse does not override ownership. |
| Deliver successive complete capabilities through the intended architecture | Appropriate incremental development. |
| Introduce a temporary architecture whose planned next step is replacement | Violates the long-term design requirement. |

## Enforcement

- Review added abstractions, configuration, and extension points against
  current requirements and the responsibility of their owning module.
- For added dependencies or custom replacements, identify the existing
  capabilities inspected and the concrete reason for the chosen approach.
- Check dependency choices against supported targets, permitted crate
  boundaries, and the existing supply-chain policies.
- Validate each completed increment through the applicable scoped checks;
  preserve working end-to-end behavior as subsequent capabilities are added.
- Apply the canonical check definitions and exception policy under
  [ADR 0034](0034-code-standards-authority.md). Mechanical checks support these
  reviews but cannot alone establish that a design is the simplest adequate
  implementation.

## Implementation notes and references

- [Agent instructions](../../../AGENTS.md), under "Always", already require
  simplicity, modularity, reuse, working increments, and long-term design.
- [Workspace dependencies](../../../Cargo.toml) and individual crate manifests
  identify the libraries already available to an implementation.
- [Supply-chain configuration](../../../deny.toml) and the canonical audit
  tasks in [mise.toml](../../../mise.toml) implement existing dependency checks.
  Passing those checks does not establish capability fit or maintainability.
- [ADR 0035](0035-deliberate-public-api.md) governs the public surface produced
  by implementation choices.

Review and guidance follow-ups are tracked in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
Recording this decision does not certify current dependencies or authorize a
repository-wide refactor.
