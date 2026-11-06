# ADR 0049: Explicit matching of public error variants

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Code standards

## Decision and scope

Project library and binding code explicitly names project-owned public error
variants wherever it assigns error meaning: category, machine-readable code,
required structured diagnostic context, conversion, or specialized handling.
Explicitly named variants may share a branch when their handling is
intentionally equivalent. Both `_ =>` and an unrestricted binding such as
`other =>` are catch-all patterns; neither may silently assign an unreviewed
meaning to a new variant.

Keep exhaustive semantic classifiers and conversions in the owning Rust crate,
including nested error variants such as `InputError` when their meaning affects
the result. Adding a variant must require review at these semantic boundaries.
This applies the structured error policy in
[ADR 0006](0006-structured-errors.md) and the catalogue rule in
[INVARIANTS.md section 5.1](../../../INVARIANTS.md#51-enforced-lints).

Pure `?` propagation, unchanged forwarding, formatting that preserves the
error's meaning, and delegation to canonical Rust `kind()`/`code()` or
conversion methods do not require duplicate variant tables in every caller.
Code that independently chooses a category, code, payload contract or special
behavior remains a semantic boundary and must receive the corresponding review.

This decision does not change existing error categories, require exhaustive
matching of all non-error enums, or prescribe consumer code outside project
ownership. Recoverable failures remain subject to
[ADR 0036](0036-safe-rust-and-recoverable-failures.md). Acceptance does not
certify existing handlers or approve their current generic fallbacks.

## Non-exhaustive enums and crate boundaries

Rust's [`non_exhaustive` rules](https://doc.rust-lang.org/reference/attributes/type_system.html#the-non_exhaustive-attribute)
apply across crate boundaries, including between sibling workspace crates.
A project-owned enum imported by a binding can therefore require a catch-all
arm even though the project controls its definition. Keep exhaustive semantic
classification in the owning crate and permit required downstream catch-all
arms that delegate to the canonical semantic contract while preserving the
required diagnostic context. This does not promise that every downstream
consumer fails to compile when a variant is added.

For third-party non-exhaustive enums, define any required unknown-variant
fallback narrowly at the consuming operation's boundary. Document the
operation-appropriate failure category and preserve the original diagnostic
and source context. An unknown variant must remain a failure; do not silently
assign it the meaning of a known variant or invent a category from its message.
Existing catch-all behavior is not automatically an approved fallback.

## Rationale and alternatives

Explicit semantic matches make new variants visible where their meaning is
defined. A catch-all classification can keep compiling while giving a new
failure an unintended public category or dropping required structured fields.
Canonical delegation keeps this review boundary in Rust without repeating
classification tables across hosts.

- **Explicit variants, including grouped named patterns:** require review of
  additions while keeping equivalent handling concise.
- **Canonical delegation:** preserves the owning crate's reviewed meaning
  across consumers and required non-exhaustive boundaries.
- **Catch-all mapping to a generic error:** loses the review boundary where it
  independently assigns meaning and may discard category or diagnostic context.

## Verification criteria and mapping

Enforcement evidence must cover owning classifiers, conversions, codes and
required diagnostic payloads, including nested error enums and independently
specialized handlers. Adding a project-owned variant must require review at
its semantic boundary, with grouped patterns naming their variants. This
requirement is distinct from consumers that only propagate or delegate the
canonical representation.

Host evidence must preserve categories and required context through centralized
Python/WASM adapters, including non-exhaustive delegation and documented
third-party unknown-variant fallbacks. Enforcement claims must identify their
crate and feature coverage; a lint declaration alone does not establish
compliance.

The [invariant map](../INVARIANT_MAP.md#5-error-handling) records the exact
clause correspondence. Acceptance does not establish implementation compliance;
implementation and verification work is tracked in the
[error-classification follow-up](../FOLLOW_UPS.md#error-classification).
