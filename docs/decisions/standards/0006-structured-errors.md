# ADR 0006: Preserve structured error meaning across interfaces

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Code standards

## Decision

Rust determines an error's category. Bindings use centralized adapters to
preserve that category and useful diagnostic context. Callers can handle
failures through exception types or structured error fields without parsing
message text.

WASM error adaptation uses `crate::utils::to_js_err`. Bare string errors such
as `JsValue::from_str` are not the agreed binding error path. Python uses the
centralized, kind-aware adapters in `errors.rs`.

This applies where an operation's contract requires an error. Permitted
missing observations and undefined numerical results follow their separately
agreed operation-specific policies. Improving prose is allowed while error
categories and required structured fields retain their meaning.

[ADR 0049](0049-explicit-public-error-matching.md) defines the detailed
requirements for explicit error-variant matching at semantic boundaries,
canonical delegation, and non-exhaustive cross-crate handling.

## Rationale

Structured errors allow reliable handling across hosts and preserve the
distinction between invalid input, missing required data, and failed
computation. A plausible numeric value must not conceal a failure.

## Examples

| Situation | Treatment |
|---|---|
| Rust reports a missing required lookup | Python raises `KeyError`; WASM preserves the corresponding kind |
| Rust rejects invalid input | Python raises `ValueError` or an appropriate domain subclass; WASM preserves the validation category |
| A computation fails to converge | Preserve its computation-failure category and relevant context |
| Catch a pricing failure and return zero | Violates this decision |
| Improve an error message while preserving category and required fields | Ordinary implementation work |

## Enforcement

Boundary checks enforce centralized adapters. Cross-language tests must
verify error categories and required diagnostic fields. Exact message prose
is generally flexible unless an operation explicitly makes it contractual.

The existing WASM boundary checker already rejects bare error construction
under the API tree. Acceptance of this ADR does not establish full coverage
of every diagnostic field or binding.

## Implementation notes and references

- [Python error adapters](../../../finstack-quant-py/src/errors.rs) map
  `NotFound`, `Validation`, and `Computation` to the corresponding public
  exception families.
- [WASM adapters](../../../finstack-quant-wasm/src/utils/mod.rs) implement
  `to_js_err`; the [boundary checker](../../../scripts/check_wasm_boundary.py)
  enforces that path in binding API code.

Guidance reconciliation is tracked in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#reconciliation-items).
