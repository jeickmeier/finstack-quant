# ADR 0039: Tests protect observable behavior and invariants

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Code standards

## Decision

Tests establish the library's intended behavior, detect meaningful regressions,
and protect financial and numerical invariants while allowing implementation
changes that preserve those contracts.

Prioritize public API coverage. Integration tests exercise realistic caller
workflows, and focused unit tests protect internal invariants. Assertions cover
normal behavior, invalid inputs, boundaries, and relevant mathematical
properties. Avoid tests that merely reproduce implementation steps.

Bug fixes include a regression test that fails without the fix and clearly
identifies the failure being prevented.

Use explicit dates, fixtures, and seeds. Tests remain independent of network
services, incidental machine configuration, and current time. Reproducibility
assertions follow the supported guarantees under
[ADR 0021](0021-reproducibility-guarantees.md).

Financial references and tolerances follow
[ADR 0014](../project/0014-financial-reference-evidence.md). Changing
expectations simply to make tests pass is prohibited. Corrections to mistaken
references require the evidence and authorization specified there.

Coverage metrics help locate gaps, but test counts and percentages do not
establish correctness. Choose cases and assertions for the behavior and
invariants they protect. Keep performance timing assertions in benchmarks
rather than ordinary correctness tests.

## Rationale

Tests are useful when they distinguish incorrect behavior from the intended
contract. Realistic public workflows and focused invariant checks detect
different classes of failure. Independent inputs and justified expectations
make regressions reproducible, while assertions tied to behavior permit
internal refactoring without discarding protection.

## Examples

| Test or change | Treatment |
| --- | --- |
| Construct and evaluate an instrument through public APIs, checking its documented outputs | Appropriate integration coverage. |
| Exercise an internal numerical kernel's valid shape and boundary cases | Appropriate focused unit coverage. |
| Add a regression assertion that also passes with the original defect | Insufficient regression evidence. |
| Use the current date to choose a valuation fixture | Replace the incidental clock dependency with an explicit date. |
| Test a conservation property within the domain where the contract promises it | Meaningful invariant coverage. |
| Calculate the expected result by repeating the implementation under test | Insufficient independent correctness evidence. |
| Preserve an assertion's meaning while changing its setup after an internal refactor | Ordinary test maintenance. |
| Increase coverage by adding cases with no meaningful outcome assertions | Does not establish correctness. |
| Assert that pricing finishes within a wall-clock threshold in a unit test | Put the performance measurement in a benchmark. |

## Enforcement

- Review new or changed behavior for public workflow coverage, relevant invalid
  inputs, boundaries, and internal invariants.
- Verify that a bug-fix regression distinguishes the defect from correct
  behavior and names the protected failure mode.
- Keep test setup deterministic and independent of external services and
  incidental host state. Exercise the documented execution conditions where
  reproducibility is promised.
- Protect expectations, tolerances, and meaningful negative and boundary cases
  under ADR 0014 and the independent enforcement in
  [ADR 0016](../project/0016-independent-contract-protection.md).
- Use coverage reports to identify untested behavior and review assertion
  quality; a passing coverage gate is not a substitute for that review.
- Use the shared scoped and full checks under
  [ADR 0034](0034-code-standards-authority.md). Keep benchmark measurements
  separate from ordinary correctness assertions.

## Implementation notes and references

- [INVARIANTS.md](../../../INVARIANTS.md), section 6, links to the governing
  test policies and retains detailed golden-reference metadata and shared
  Rust/Python fixture conventions.
- [Rust testing guidance](../../../.agents/rules/rust/testing-standards.md)
  describes public API scenarios, focused unit and property tests, explicit
  inputs, coverage goals, and separate performance benchmarks.
- [Canonical tasks](../../../mise.toml) provide scoped and full test runs,
  coverage measurement, and benchmark execution.
- [ADR 0036](0036-safe-rust-and-recoverable-failures.md) specifies the library
  failure behavior that invalid-input tests must protect.

This record does not certify current test coverage or the independence of
existing expectations. Guidance and coverage follow-ups are tracked in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
