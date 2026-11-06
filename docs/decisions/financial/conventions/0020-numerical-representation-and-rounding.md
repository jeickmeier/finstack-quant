# ADR 0020: Model numerics and contractual monetary arithmetic

**Status:** Accepted

**Decision date:** 2026-11-04

**Amended:** 2026-11-05 — monetary construction precision and input validity.

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

`Money::new` preserves sub-minor-unit precision and does not implicitly
quantize to ISO-4217 currency scale. Request currency-scale rounding through
explicit configuration or apply it when finalizing an amount at its required
rounding boundary.

Reject non-finite monetary inputs. Validate floating-point inputs at the
monetary construction or conversion boundary without introducing an
intermediate `f64` conversion into a lossless Decimal/Money path.

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
| Construct a USD amount of `0.001` with `Money::new` | Preserve the sub-cent amount; construction does not implicitly round it to cents. |
| Supply `NaN` or infinity to a monetary constructor | Reject the input. |
| Construct Money from a Decimal amount | Preserve the lossless path; do not convert through `f64` for validation. |
| Round a payment at its specified contractual rounding stage | Appropriate. |
| Feed display-rounded values back into settlement calculations | Prohibited unless that rounding is explicitly required. |

## Enforcement

- Test monetary conversions, required intermediate rounding, and finalization
  boundaries, including amounts that expose floating-point loss.
- Preserve the Decimal path through monetary steps; review conversions to and
  from model numerics as explicit boundaries.
- Check that Money construction preserves a representable sub-minor-unit
  amount and that explicitly configured or finalized currency-scale rounding
  follows the required boundary, independently of display formatting.
- Test non-finite monetary-input rejection at the supported construction and
  conversion boundaries without converting lossless Decimal inputs through
  `f64`.
- Use justified numerical tolerances for model results under
  [ADR 0014](../../project/0014-financial-reference-evidence.md).
- Keep units and sign conventions consistent with
  [ADR 0007](0007-units-and-sign-conventions.md).

## Implementation notes and references

[INVARIANTS.md](../../../../INVARIANTS.md), section 1, already distinguishes
model `f64` arithmetic from Decimal monetary paths. It documents
`Money::from_decimal` and `Money::amount_decimal` as the lossless amount path
and retains the monetary construction, input-validity, and rounding rules.
The [Money constructors](../../../../finstack-quant/core/src/money/types.rs)
provide the relevant construction and validation paths; their existence does
not establish complete caller compliance.

Floating-point accumulation and comparison rules belong to
[ADR 0042](../../standards/0042-numerical-accumulation-and-comparisons.md).
Monetary-path reconciliation is tracked in
[the monetary-boundary follow-up](../../FOLLOW_UPS.md#monetary-boundaries).
Acceptance does not certify implementation compliance.

No numerical tests were run for the 2026-11-05 amendment.
