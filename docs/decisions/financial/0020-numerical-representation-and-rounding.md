# ADR 0020: Model numerics and contractual monetary arithmetic

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Financial conventions

## Decision

Use `f64` where appropriate for pricing models, risk, curves, optimization,
and simulation. Monetary paths for accounting, ledgers, settlement,
regulatory reporting, and disputes preserve Decimal/Money arithmetic, with
explicit conversion boundaries and rounding rules.

Wrapping a floating-point model estimate in `Money` identifies its currency
and provides Decimal storage; it does not make the preceding calculation
Decimal-exact. An intentional model boundary must document its conversion and
rounding policy.

Preserve precision until the contract requires rounding, including any
explicitly required intermediate rounding. Display formatting does not define
the calculation's rounding policy.

## Rationale

An implementation can preserve units, signatures, and approximate tests while
changing monetary amounts through an intermediate floating-point conversion.
Model estimates and contractual amounts need different numerical guarantees;
both representation and rounding stage contribute to that guarantee.

## Examples

| Calculation or change | Treatment |
| --- | --- |
| Calculate a Monte Carlo valuation using `f64` | Appropriate model arithmetic. |
| Wrap that estimated valuation in `Money` to identify its currency | Allowed; it does not establish Decimal-exact derivation. |
| Aggregate contractual ledger balances using Decimal amounts | Required approach. |
| Convert those balances to `f64`, sum, and wrap the result in `Money` | Violates the monetary-arithmetic boundary. |
| Round a payment at its specified contractual rounding stage | Appropriate. |
| Feed display-rounded values back into settlement calculations | Prohibited unless that rounding is explicitly required. |

## Enforcement

- Test monetary conversions, required intermediate rounding, and finalization
  boundaries, including amounts that expose floating-point loss.
- Preserve the Decimal path through monetary steps; review conversions to and
  from model numerics as explicit boundaries.
- Use justified numerical tolerances for model results under
  [ADR 0014](../project/0014-financial-reference-evidence.md).
- Keep units and sign conventions consistent with
  [ADR 0007](0007-units-and-sign-conventions.md).

## Implementation notes and references

[INVARIANTS.md](../../../INVARIANTS.md), section 1, already distinguishes
model `f64` arithmetic from Decimal monetary paths. It documents
`Money::from_decimal` and `Money::amount_decimal` as the lossless amount path,
and identifies existing monetary aggregation and statement-evaluation paths
using `f64` as migration targets.

Each such path requires correction or an explicitly justified model boundary
and rounding policy. Accepting this decision does not certify those paths as
compliant, and this documentation change does not modify them.
