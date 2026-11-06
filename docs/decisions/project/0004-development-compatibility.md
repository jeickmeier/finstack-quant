# ADR 0004: Remove obsolete paths during development

**Status:** Accepted

**Lifecycle condition:** Revisit before the first final release

**Decision date:** 2026-11-04

**Category:** Project policy

## Decision

During development, an explicitly approved replacement becomes the supported
path. Remove the superseded API, format, or implementation and update its
consumers in the same change. Do not retain compatibility aliases, legacy
fallbacks, or migration layers solely to preserve that superseded path.

Before the first final release, revisit this ADR and adopt a sensible backward
compatibility strategy covering a couple of releases. The exact support
window, deprecation process, migration mechanisms, and exceptions remain
open. The development policy must not automatically become the permanent
release policy.

An agent cannot declare a capability obsolete merely because a replacement
would simplify its task. The replacement must be within the explicitly
authorized scope, as defined by [ADR 0015](0015-authorized-change-scope.md).

## Rationale

Maintaining obsolete paths during development creates duplicate behavior and
contracts that can diverge. Released consumers introduce different obligations;
those obligations require an explicit release policy before final release.

## Examples

| Change | Treatment |
| --- | --- |
| Approve a rename and update Rust, bindings, UI, tests, docs, and notebooks together | Required. |
| Keep the old name as an alias just in case | Prohibited during development. |
| Replace a wire format but silently accept the obsolete form | Prohibited during development. |
| Reject an obsolete format with a clear error | Appropriate. |
| Retain two calculation methods serving distinct current requirements | Allowed; neither is obsolete. |
| Promise indefinite breaking changes after final release | Not authorized by this ADR. |

## Enforcement

- API and schema checks verify the approved replacement.
- Targeted rejection tests confirm retired names or formats are not accepted.
- Search all consumers for obsolete references when replacing a public path.
- Before final release, approve a revision or superseding decision that defines
  the compatibility strategy. Do not infer an exact two-release guarantee or
  a particular migration mechanism from the present direction.

## Implementation notes and references

The development rule is already expressed in [AGENTS.md](../../../AGENTS.md)
and the [Serde Contract Policy](../../SERDE_STABILITY.md). This ADR adds the
explicit final-release reconsideration requirement; it does not establish a
released-version support matrix.

Related: [canonical names](../standards/0003-canonical-public-names.md),
[serialized contracts](../standards/0012-serialized-contracts.md).
