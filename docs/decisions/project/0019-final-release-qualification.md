# ADR 0019: Qualify final releases and their exact artifacts

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Project policy

## Decision

A final release declares its supported capabilities, platforms, and
limitations. It passes the agreed release checks and financial validation,
and approval applies to the actual artifacts being shipped. A passing
development build or nightly does not establish final-release readiness.

Record the qualified source revision and the identities of the exact approved
artifacts. If final-version stamping or any other change requires rebuilding
a tested candidate, validate the resulting final packages before publication;
approval of the candidate alone does not qualify those rebuilt artifacts.

Preserve the upstream-first process: fixes land on `master` and reach release
branches through backports. A release directly from `master` requires the
same qualification as one promoted through a candidate branch.

Before the first final release, settle the compatibility strategy required by
[ADR 0004](0004-development-compatibility.md).

## Rationale

Source tests, package installation, financial validation, and consumer runtime
behavior establish different parts of readiness. Packaging or rebuilding can
change the delivered product even when the source commit stays the same.
Release approval must identify what consumers will actually receive.

## Examples

| Situation | Treatment |
| --- | --- |
| Publish a nightly after development checks pass | Allowed under the nightly channel. |
| Present that nightly as a qualified final release | Requires full release qualification. |
| Fix something only on a release branch | Violates upstream-first development; the fix must land on `master`. |
| Test a candidate and release a different commit | Qualify the changed revision before release. |
| Rebuild the tested commit with final-version metadata | Validate the resulting final artifacts. |
| Include a capability with documented limitations | State its qualified scope; availability alone does not establish production suitability. |

## Enforcement

- Release gates cover the agreed support matrix, financial validation,
  installed-package checks, and blocking findings.
- Define and preserve the qualification evidence for each shipped artifact;
  record its identity alongside the approved revision.
- Apply the same qualification to direct-from-`master` releases and candidate
  promotion. The branch route does not waive the requirements.
- Keep nightly, candidate, and final-release claims distinct.
- Use [ADR 0018](0018-platform-and-runtime-support.md) for platform evidence
  levels and [ADR 0014](0014-financial-reference-evidence.md) for reference
  results and numerical tolerances.

## Implementation notes and references

[CONTRIBUTING.md](../../../CONTRIBUTING.md) describes nightly/candidate/final
channels, upstream-first fixes, promotion of the QA-tested commit, and direct
releases from `master`. The [release workflow](../../../.github/workflows/release.yml)
calls the [artifact workflow](../../../.github/workflows/build-artifacts.yml)
to rebuild packages for the selected version.

Exact final-artifact qualification is the accepted requirement. This record
does not establish that the current workflow captures independent approval of
those artifacts or that any particular release has been qualified. The
concrete qualification criteria and their enforcement still need to be made
explicit and verified.
