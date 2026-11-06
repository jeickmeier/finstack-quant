# ADR 0016: Independently protected contracts and checks

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Project policy

## Decision

Checks protecting settled decisions use a trusted, approved baseline. Changes
to expectations, exceptions, limits, or enforcement require independently
verifiable authorization. An agent cannot authorize its own exception by
editing a file or label in the change under evaluation.

The checker, its workflow, and its exceptions are themselves protected.
Authorization identifies the affected decision and permitted change, following
[ADR 0015](0015-authorized-change-scope.md). Ordinary implementation work that
preserves the contracts proceeds through ordinary checks.

For this boundary to be enforceable, agent credentials must not be able to
fabricate the owner's approval, change repository protections, or bypass the
checks. Use of the owner's unrestricted administrative identity weakens that
boundary.

## Rationale

A candidate patch can otherwise change both a requirement and the check that
judges it. Passing self-modified checks establishes consistency, not
authorization to change a settled decision.

## Examples

| Change | Treatment |
| --- | --- |
| Refactor while all protected contracts hold | Ordinary checks apply. |
| Increase both parity backlog and its permitted ceiling | Block without authorization to change the requirement. |
| Update CAGR expectations within the approved Actual/365 Fixed change | Permitted within that authorization. |
| Remove the CI job that detects a contract violation | Cannot bypass the protection. |
| Add `approved = true` to the candidate patch | Does not establish authorization. |
| Correct a faulty protection check | Allowed through an independently authorized change. |

## Enforcement

- Required checks compare against trusted baseline policies and expectations,
  not solely against editable candidate-tree values.
- Protect checker selection, workflow execution, exceptions, and limits as
  well as the tests and fixtures they govern.
- Verify approval through a mechanism outside the agent's write authority,
  scoped to the permitted change. A self-authored declaration is insufficient.
- Prove the protection with deliberate violations, including changes to both
  implementation and its expected result or threshold.

## Implementation notes and references

This is an accepted enforcement requirement, not a claim that repository
permissions or a trusted-baseline check have been configured.

The [parity tests](../../../finstack-quant-py/tests/parity/test_parity_ledger.py)
and generation checks in [mise.toml](../../../mise.toml) are evidence locations;
passing candidate-tree consistency checks does not establish independent
authorization. Implementation gaps and repository-protection verification are
tracked in [FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
