# ADR 0001: Rust owns financial behavior

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Specific code decisions

## Decision

Financial calculations, business validation, and financial defaults live in
Rust. Python, WASM, and the UI call that implementation. They may handle input
conversion, presentation, and workflow orchestration.

Host interfaces may provide immediate input feedback, but Rust remains
responsible for enforcing the underlying business constraint. Presentation
conversion does not authorize a host to introduce its own financial defaults
or calculation rules.

## Rationale

A shared implementation keeps financial meaning consistent across languages
and ensures a correction reaches every interface. A capability requested for
the UI still starts in Rust when it introduces financial behavior.

## Examples

| Change | Treatment |
|---|---|
| Optimize a Rust bond-pricing calculation while preserving agreed results | Ordinary implementation work |
| Convert a NumPy array into Rust inputs | Allowed in Python |
| Format a decimal return as `5.25%` | Allowed presentation conversion |
| Implement a separate financial formula in a Python wrapper | Violates this decision |
| Substitute a missing discount rate with zero in JavaScript | Violates this decision; defaulting belongs in Rust |
| Show an invalid-date message in the UI | Allowed, with Rust enforcing the business constraint |

## Enforcement

Required enforcement combines shared behavioral cases across Rust, Python,
and WASM with checks for known prohibited calculation or defaulting paths in
bindings. Review is still needed for semantic duplication that cannot be
reliably detected automatically.

Acceptance of this ADR does not establish complete binding compliance.
Implementation and coverage work is tracked in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

## Implementation notes and references

- [Binding architecture instructions](../../../AGENTS.md) establish Rust as
  the canonical implementation owner.
- [Contribution principles](../../../CONTRIBUTING.md) describe thin bindings.
- [ADR 0005](../standards/0005-python-wasm-parity.md) defines equivalent host behavior.
- [ADR 0006](../standards/0006-structured-errors.md) defines centralized error adaptation.
