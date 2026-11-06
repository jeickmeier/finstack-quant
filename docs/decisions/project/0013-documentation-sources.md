# ADR 0013: Authoritative documentation sources

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Project policy

## Decision

Each documentation surface and generated artifact has a designated source.
Update that source and regenerate its derived outputs. Public contract changes
include corresponding documentation changes; a documentation edit cannot
independently redefine agreed behavior.

Python `.pyi` stubs own Python binding docstrings. Rust owns financial and wire
semantics; host documentation must describe those semantics accurately. This
does not classify every declaration or documentation file as generated:
hand-maintained surfaces are edited at their designated source.

Agent guidance links to authoritative decisions and avoids competing
descriptions of the same rule. Where guidance conflicts with an explicitly
settled decision, reconcile it to that decision instead of silently changing
the behavior. Wording, formatting, and examples remain flexible when their
meaning is preserved.

## Rationale

Duplicated guidance can direct otherwise valid implementation work toward
contradictory behavior. A known source for each surface makes corrections
traceable and prevents generated output from becoming a second authority.

## Examples

| Change | Treatment |
| --- | --- |
| Edit a Python docstring in its `.pyi` source and rebuild runtime docs | Required approach. |
| Patch a generated TypeScript declaration without updating its source | Prohibited. |
| Improve wording or add an example without changing its contract | Ordinary documentation work. |
| Describe an unauthorized new default in the docs | Does not authorize the default change. |
| Correct bare-WASM-error guidance after accepting structured errors | Required reconciliation with ADR 0006. |

## Enforcement

- Retain runtime/stub documentation equality checks, public API documentation
  checks, and generated-artifact drift checks.
- Review semantic documentation changes alongside the affected contract.
- Update input units, defaults, returns, errors, and examples in the same
  change as the relevant public behavior or signature.
- Do not claim that mechanical documentation checks establish semantic
  agreement; source review and contract tests also matter.

## Implementation notes and references

Existing mechanisms include the [Python documentation style](../../../finstack-quant-py/DOCS_STYLE.md),
[runtime documentation tests](../../../finstack-quant-py/tests/test_runtime_docs.py),
and documentation/generation tasks in [mise.toml](../../../mise.toml).

Guidance reconciliation is tracked in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#reconciliation-items). Acceptance establishes
documentation ownership; it does not certify complete guidance or check coverage.
