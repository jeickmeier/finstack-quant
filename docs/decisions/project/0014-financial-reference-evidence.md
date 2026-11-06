# ADR 0014: Financial reference results and tolerances

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Project policy

## Decision

Tests protecting financial correctness use independently justified
expectations and explicit numerical tolerances. An agent must not change
expected results, loosen tolerances, or disable tests merely to make an
implementation pass.

Each important reference case identifies its formula or source, assumptions,
units, and tolerance rationale. A generator that calls the implementation
under test does not independently establish its financial correctness.
Generated fixtures can still establish integration consistency; their
purpose must be clear.

A mistaken reference can be corrected with evidence and explicit
authorization. Refactoring a test is ordinary work when the meaning of its
assertions is preserved.

[ADR 0048](0048-reference-provenance-and-version-pinning.md) defines the
detailed requirements for case-attributable golden metadata, shared fixture
locations, methodology and calibration version pinning, and execution evidence
for published error bands.

## Rationale

Changing implementation and expected output together can produce a passing
suite while discarding the very financial behavior the test should protect.
Independent evidence and justified tolerances make a failure informative.

## Examples

| Change | Treatment |
| --- | --- |
| Check a result against a hand-derived example or independent reference | Appropriate correctness evidence. |
| Call the same function to compute both actual and expected results | Insufficient independent correctness evidence. |
| Generate UI fixtures from Rust to verify interface consistency | Allowed for that integration purpose. |
| Widen tolerance because a refactor fails | Investigate; changing the protected tolerance requires agreement. |
| Update CAGR expectations for the approved Actual/365 Fixed default | Authorized; update the formula, tests, and documentation together. |
| Remove or skip a failing settled-contract test | Same scrutiny as changing its expectation. |

## Enforcement

- Identify reference cases that enforce settled decisions and protect their
  expected values, tolerances, and execution.
- Explain any semantic change using evidence and the affected decision ID.
- Preserve meaningful boundary cases and negative cases when restructuring
  tests; moving an assertion does not authorize weakening it.
- Apply [ADR 0016](0016-independent-contract-protection.md) to the protection
  machinery as well as to its baselines.

## Implementation notes and references

The [core golden-test guide](../../../finstack-quant/core/tests/golden/README.md)
already describes provenance and review practices. Metadata or a `certified`
label alone is not proof of independent calculation or authorization. A skip
with an explanation alone does not authorize disabling a protected test.

The approved [CAGR default change](../financial/conventions/0008-financial-defaults-and-cagr.md)
is a concrete authorized reference update. This documentation does not change
existing fixtures or certify the independence of every current golden case.
