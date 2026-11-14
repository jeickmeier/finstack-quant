# ADR 0011: Bond CS01 without a hazard curve

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Financial conventions

## Decision

For a supported bond without a hazard curve, calculate CS01 using the agreed
z-spread bump-and-reprice method. The absence of a hazard curve alone must not
cause the calculation to return zero, become unavailable, or substitute a
different sensitivity.

Attribute the result to the instrument, such as `cs01::BOND_A`, as required by
[ADR 0010](../valuations-framework/0010-metric-identifiers.md). Bump size, units, and sign follow the
documented metric convention under
[ADR 0007](../conventions/0007-units-and-sign-conventions.md). Genuine invalid or insufficient
pricing inputs still produce their specified errors.

## Rationale

A missing hazard curve does not remove a supported bond's spread exposure.
The z-spread method provides the agreed calculation for that case without
inventing a credit curve or substituting interest-rate risk.

## Examples

| Situation | Required treatment |
|---|---|
| Price a supported bond without a hazard curve and request CS01 | Use its agreed z-spread bump-and-reprice method. |
| Return zero solely because the hazard-curve lookup is empty | Violates the decision. |
| Substitute interest-rate DV01 because it is available | Violates the decision. |
| Invent a hazard curve to reuse another calculation path | Requires an explicit change to the financial model. |
| Required pricing inputs are invalid or insufficient | Return the specified error. |

## Enforcement

- Include regression cases that explicitly omit the hazard curve.
- Compare CS01 with independently specified z-spread repricing using the
  documented settlement, compounding, bump, and sign conventions.
- Verify the instrument-qualified result key and required-input errors.
- Protect the financial reference expectations; regenerating fixtures does
  not authorize changing the method.

## Implementation notes and references

The vanilla-bond implementation and settlement-anchored regression use
z-spread repricing. The [accepted BONDS-16 scope](16-spread-credit-risk.md#accepted-decision-for-checked-bond-spread-and-credit-risk)
requires a qualified fixed Z cash path for a supported option-bearing bond
and removes the implicit OAS and parallel-as-bucket substitutes. Runtime
reconciliation and independent qualification are tracked in the
[central follow-up register](../../FOLLOW_UPS.md#reconciliation-items).

- [Existing project convention](../../../../AGENTS.md).
- [Bond CS01 calculation and model selection](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/cs01.rs).
- [Settlement-anchored z-spread regression](../../../../finstack-quant/valuations/tests/instruments/bond/metrics/cs01.rs).
