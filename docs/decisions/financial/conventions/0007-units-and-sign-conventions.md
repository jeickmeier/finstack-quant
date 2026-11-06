# ADR 0007: Financial units and sign conventions

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Financial conventions

## Decision

Every public financial input and output has explicit units and conventions.
Changing its numerical meaning requires an explicit decision, even when the
function name and parameter type remain unchanged.

Contracts specify decimal rates versus percentages, currency and monetary
scale, absolute versus relative bumps, and signed versus unsigned risk measures
where applicable. These conventions belong to each operation; there is no
implicit universal unit or sign convention for all financial quantities.

Host presentation may convert units explicitly. It must not guess an input's
representation or silently reinterpret a value.

## Rationale

A calculation can compile and return plausible numbers while being wrong by a
factor of 100, a currency scale, or a sign. Units and signs therefore form part
of the behavior that refactoring must preserve.

## Examples

| Situation | Required treatment |
|---|---|
| A rate argument is documented as a decimal | Interpret `0.05` as 5%. |
| A UI accepts a displayed 5% rate | Explicitly convert it to `0.05` before calling the decimal-rate API. |
| A wrapper assumes values above `1` are percentages | Reject that heuristic unless the operation's contract explicitly authorizes it. |
| A documented absolute 1 bp bump is replaced by a 1% relative bump | Require an explicit contract change; the absolute decimal bump is `0.0001`. |
| A signed sensitivity is replaced by its absolute magnitude | Require an explicit contract change. |

## Enforcement

- Use existing domain types where they express the required distinctions.
- Include numerical cases that expose scaling and sign mistakes.
- Verify that Rust, Python, WASM, documentation, and serialization agree on
  the interpretation of financial values.
- Protect the relevant expected values and tolerance rationale when these
  checks become settled-contract tests.

## Implementation notes and references

The public API documentation requirements already require financial units and
conventions. This ADR records the policy; it does not certify that every API
has complete coverage or introduce a new universal bump/sign choice.

- [Public API and metric conventions](../../../../AGENTS.md).
- [Existing numerical and financial invariants](../../../../INVARIANTS.md).
- [Core unit conversions and basis-point constant](../../../../finstack-quant/core/src/math/units.rs).
- [Metric unit definitions](../../../../finstack-quant/valuations/core/src/metrics/ids/unit.rs).
- [Canonical financial references](../../../REFERENCES.md).
