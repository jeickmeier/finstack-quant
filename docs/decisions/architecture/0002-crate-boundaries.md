# ADR 0002: Protect crate responsibilities and dependency direction

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Specific code decisions

## Decision

Each crate has a defined responsibility and permitted dependencies. Routine
implementation work respects those boundaries. Changing ownership or adding
a dependency across a prohibited boundary requires an explicit architectural
decision.

In particular, `models` and the valuations crates must not depend on
`factor`. Moving domain-specific behavior into a lower-level crate merely to
evade a dependency restriction is also an architectural change.

## Rationale

Dependency direction preserves modularity and keeps lower-level components
usable without higher-level workflows. It permits substantial refactoring
within a boundary without allowing a convenient import to redefine the
architecture.

## Examples

| Change | Treatment |
|---|---|
| Split a large pricing module into smaller internal modules | Ordinary implementation work |
| Reuse a utility through an already permitted dependency | Ordinary implementation work |
| Add a `factor` dependency to `models` for convenience | Requires changing the architectural decision |
| Move instrument-specific pricing into `core` to avoid a restriction | Requires changing the architectural decision |
| Propose a new crate split with explicit responsibilities | The proposed boundaries need agreement |

## Enforcement

Required enforcement is a checked-in dependency policy validated against
Cargo's dependency graph, including optional features. Test-only dependencies
need explicit allowances. The policy receives the same protection as the
architecture it describes.

Creating a complete policy and wiring its validation into required checks is
follow-up enforcement work. This ADR does not claim that such a gate is
already installed or that the current graph has been exhaustively audited.

## Implementation notes and references

- [Project structure and architecture instructions](../../../AGENTS.md)
  specify crate ownership and the `factor` restriction.
- [Workspace manifest](../../../Cargo.toml) inventories the workspace.
- [Models manifest](../../../finstack-quant/models/Cargo.toml) and
  [factor manifest](../../../finstack-quant/factor/Cargo.toml) are concrete
  dependency sources.
- [ADR 0001](0001-rust-owned-financial-behavior.md) assigns financial behavior
  to Rust while this ADR governs its placement within Rust.
