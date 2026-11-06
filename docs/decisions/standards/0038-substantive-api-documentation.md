# ADR 0038: Substantive, executable public API documentation

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Code standards

## Decision

Documentation is part of implementing a public API. It must explain how
callers use the API correctly, and its examples must remain consistent with
the supported public interface.

Document the API's purpose and every input by its exact public parameter name.
Explain units, conventions, accepted shapes and ranges, defaults, missing-data
behavior, and relevant ownership or state effects. Descriptions must convey
the contract rather than merely repeat a parameter's name or type.

Describe returned values and the public errors callers should handle,
including the conditions that trigger them. Financial representations and
conventions must be explicit where they affect interpretation or results.

Provide meaningful, executable examples of public use. Routine accessors can
be covered by their enclosing type's example. Preserve the existing example
coverage requirements for each language surface.

Rust, Python, and WASM documentation preserve equivalent semantics while
explaining host-specific differences. Behavior and signature changes include
corresponding documentation and example updates in the same change.

Documentation checks and doctests support review; passing them alone does
not establish semantic correctness. Source ownership remains governed by
[ADR 0013](../project/0013-documentation-sources.md). This decision establishes
the content and verification standard without changing where each document
is maintained.

## Rationale

Types and signatures alone do not explain financial conventions, default
behavior, missing inputs, or failures. Substantive documentation makes these
contracts usable. Executable examples help detect drift in public usage,
while review and behavioral evidence establish whether the explanation is
accurate.

## Examples

| Documentation choice | Treatment |
| --- | --- |
| Describe a rate only as "the rate value" | Insufficient; state its representation and applicable convention. |
| Explain an optional curve identifier's lookup and fallback behavior | Required where it changes the result. |
| Name the mapped Python exception and the condition that raises it | Describes a failure callers can handle. |
| Document a non-void result only by repeating its type | Insufficient; explain what the returned value represents. |
| Cover ordinary getters in a working example of their public type | Allowed within the applicable example-coverage requirements. |
| Change a default or parameter name while leaving examples unchanged | Update the affected documentation and examples in the same change. |
| Pass a documentation checker with plausible but incorrect convention prose | Still violates the semantic documentation requirement. |

## Enforcement

- Apply the public input-documentation checks and the applicable summary,
  return, error, and example checks across Rust, Python, and WASM.
- Compile or type-check examples and run the executable examples through the
  corresponding documentation and doctest tasks against public interfaces.
- Review documentation alongside the affected implementation and behavioral
  tests, including financial units, defaults, and missing-data semantics.
- Verify error documentation against the centralized mappings under
  [ADR 0006](0006-structured-errors.md) and preserve host semantic parity under
  [ADR 0005](0005-python-wasm-parity.md).
- Update designated sources and regenerate derived documentation under
  ADR 0013. Use the shared checks under
  [ADR 0034](0034-code-standards-authority.md).

## Implementation notes and references

- The public API documentation contract in [AGENTS.md](../../../AGENTS.md)
  specifies exact-parameter guidance and host-specific example coverage.
- [Python documentation style](../../../finstack-quant-py/DOCS_STYLE.md)
  describes the authoritative stub documentation and its conventions.
- [Canonical tasks](../../../mise.toml) define `rust-doc`, `python-lint`
  (which runs the Python documentation checkers), `wasm-doc`, and the
  corresponding doctest checks.
- The [Rust input-documentation checker](../../../scripts/check_public_api_input_docs.py),
  [Python API checker](../../../scripts/check_python_api_input_docs.py), and
  [WASM declaration checker](../../../finstack-quant-wasm/scripts/check-typescript-docs.mjs)
  (over the declarations wasm-bindgen generates from the Rustdoc) provide
  structural and substantive-description checks.
- [Runtime documentation tests](../../../finstack-quant-py/tests/test_runtime_docs.py)
  check agreement between Python runtime docstrings and their stub sources.

These checks do not certify every existing explanation or example as correct.
Coverage and semantic review follow-ups are tracked in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
