# ADR 0015: Autonomous work within authorized scope

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Project policy

## Decision

A task authorizes the implementation work necessary to achieve its stated
outcome. It does not implicitly reopen unrelated settled decisions. An
explicit instruction to change a decision supplies authorization; do not
repeatedly ask for approval of the same change.

Agents make routine implementation choices autonomously within agreed
behavior and architecture. Fixing code that contradicts an agreed contract
restores that contract and does not require a new decision. Evidence that the
contract itself may be wrong calls for a concrete proposed correction and a
separate decision.

Authorization is specific to the requested outcome and its necessary
consequences. Continue independent work while a separate decision conflict is
being resolved.

## Rationale

Settled decisions must survive broad requests such as cleanup or optimization,
while routine work must remain practical. The boundary is the intended change
in behavior or architecture, not every implementation detail or touched file.

## Examples

| Request or situation | Treatment |
| --- | --- |
| Make a calculation faster | Preserve agreed financial behavior and accuracy. |
| Fix an implementation that contradicts an agreed formula | Ordinary bug fix restoring the contract. |
| Clean up the analytics module | Does not authorize public renames or changed defaults. |
| Use Actual/365 Fixed as the dated-window CAGR default | Authorizes that change and necessary binding, test, and doc updates. |
| Discover another convention may be wrong | Present evidence and the proposed correction for a separate decision. |
| Encounter a decision conflict with unrelated work remaining | Continue the unaffected work. |

## Enforcement

- Change descriptions identify affected ADR IDs and distinguish preserved
  contracts from intentionally changed ones.
- Contract checks flag changes beyond the authorized scope.
- Do not turn protection into repeated permission requests for work already
  authorized in the task or conversation.
- Authorization to change one decision does not authorize adjacent changes.

## Implementation notes and references

The [contribution workflow](../../../CONTRIBUTING.md) already calls for focused
changes and validation evidence. This ADR defines the authorization boundary;
it does not imply that a machine-verifiable authorization mechanism exists.

Related: [development compatibility](0004-development-compatibility.md),
[financial defaults](../financial/0008-financial-defaults-and-cagr.md),
[independent contract protection](0016-independent-contract-protection.md).
