# PORTFOLIO-ATTRIBUTION-003: Decimal largest-remainder capital apportionment

- ID: PORTFOLIO-ATTRIBUTION-003
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

After weights are chosen, the allocator conserves a rounded total capital in caller-selected decimal units. Default precision is ten decimal places, maximum twelve. Capital is a finite scalar without a currency tag; negative capital is allowed and its sign is restored after allocation. The procedure addresses loss of integer precision when minor-unit counts exceed the exact integer range of `f64`.

## Ordered algorithm

1. Convert absolute capital and weights into `rust_decimal::Decimal`; conversion or representability failures return an input error.
2. Form `scale=10^decimal_places`; round `abs(capital)*scale` using the Decimal rounding operation to obtain the target whole-unit count. This is a Decimal nearest-rounding step, not the repository currency-specific Money rounding policy.
3. Normalize the Decimal weights by their Decimal total. For each sleeve, compute exact Decimal units, take the floor and retain the fractional remainder.
4. Compute unallocated whole units. Sort eligible positive-weight sleeves by decreasing remainder, breaking equal remainders by original input index, and award residual units in that order. Zero-weight sleeves receive none.
5. Divide by scale, restore the original capital sign and convert allocations back to `f64`. Report original model weights alongside these rounded amounts.

The conservation invariant applies to the integer/Decimal target. Returning `f64` values does not promise bitwise equality of a later naive floating sum to the caller's original unrounded capital. The method does not enforce minimum tradable lots or instrument-specific settlement increments.

## Alternatives and tradeoffs

Independent rounding per sleeve can drift from the budget. Largest remainder conserves the target deterministically but creates discontinuous one-unit changes at remainder ties and depends on sleeve ordering for ties. Decimal arithmetic avoids the `2^53` minor-unit limitation of binary floating integers; its own finite range remains an explicit limit.

## Evidence

[Source and the beyond-f64-integer-precision test](../../../../finstack-quant/portfolio/src/allocation.rs) provide algorithm and invariant evidence. [Allocation integration fixtures](../../../../finstack-quant/portfolio/tests/weight_allocation.rs) cover representative budgets; none were executed in this audit. The package dependency, rather than a custom decimal implementation, supplies conversion and rounding semantics.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
