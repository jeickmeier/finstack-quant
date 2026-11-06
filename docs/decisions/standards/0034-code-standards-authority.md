# ADR 0034: One authoritative definition of code standards

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Code standards

## Decision

Accepted decision records define the library's coding policy. Shared workspace
configuration and canonical development tasks implement that policy.
Contributor and agent instructions reference those sources and remain
consistent with them.

Every crate inherits the applicable workspace standards. Production code and
tests may have deliberately different rules. Local development and CI use the
same underlying definitions, with scoped checks during implementation; this
does not require every local invocation to run the full CI suite.

Fix the underlying issue before using a lint exception. A permitted exception
names the specific lint, uses the smallest practical scope, and explains why
it is appropriate. A local exception already permitted by the accepted policy
does not redefine that policy. Narrow scope and a rationale do not authorize
weakening a protected standard or its enforcement.

Changing configuration does not itself authorize weakening an accepted
standard. Policy changes require explicit agreement under
[ADR 0015](../project/0015-authorized-change-scope.md), with independent
protection of contracts and checks under
[ADR 0016](../project/0016-independent-contract-protection.md).

Reconcile conflicting guidance against the accepted decision. Decision records
retain the policy; configuration retains individual lint settings and
formatting details. Routine maintenance that preserves policy remains ordinary
implementation work. This decision does not ratify every existing setting or
legacy instruction as accepted policy.

## Rationale

Competing copies of a standard can make code pass one check while violating
the intended rule. Shared executable definitions reduce that drift, while
explicit policy keeps a configuration edit from silently changing the
requirement it is meant to enforce.

## Examples

| Change | Treatment |
| --- | --- |
| Run a crate-scoped lint task using the workspace lint definitions | Appropriate during implementation. |
| Permit panic-based assertions in tests while library input failures return errors | A deliberate target distinction, subject to the applicable failure policy. |
| Add a narrowly scoped lint exception already permitted by policy, with its reason | Allowed within that policy. |
| Disable a protected lint across a crate to clear failures | Requires authorization to change the protected requirement; passing the changed check is insufficient. |
| Update contributor guidance to link to the accepted standard and canonical task | Ordinary maintenance when the meaning is preserved. |
| Find a configuration setting that conflicts with an accepted decision | Reconcile the implementation to the decision; the setting does not override it. |

## Enforcement

- Verify crate inheritance and coverage of the applicable targets and features.
  A target omitted from a check is a coverage gap, not an exemption.
- Keep local and CI checks connected to shared definitions; scoped and full
  checks may select different workloads without redefining the standard.
- Review lint exceptions for permitted scope, specific lint, and rationale.
- Use ADR 0016 for changes to protected enforcement rather than treating a
  self-authored exception as authorization.
- Reconcile guidance and configuration under
  [ADR 0013](../project/0013-documentation-sources.md). Track incomplete
  reconciliation and verification in
  [FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

## Implementation notes and references

- [Workspace configuration](../../../Cargo.toml) defines shared lint settings
  and explains deliberate differences between library and test targets.
- [Canonical tasks](../../../mise.toml) define scoped and full checks. The
  workspace lint task records why bindings must not be omitted through the
  default-member selection.
- [Workflow documentation](../../../.github/workflows/README.md) describes
  local and CI check tiers.
- [Contributor guidance](../../../CONTRIBUTING.md),
  [agent instructions](../../../AGENTS.md), and
  [Rust standards guidance](../../../.agents/rules/rust/code-standards.md)
  are maintained against the accepted policies and executable definitions.

These sources provide existing mechanisms, not proof of complete coverage or
independent protection. Recording this decision does not change lint settings,
CI workflows, or repository permissions.
