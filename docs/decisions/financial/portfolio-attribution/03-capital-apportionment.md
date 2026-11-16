# PORTFOLIO-ATTRIBUTION-003: Decimal largest-remainder capital apportionment

- ID: PORTFOLIO-ATTRIBUTION-003
- Decision status: Accepted 2026-11-16 for exact scalar capital apportionment.
- Implementation status: Observed for the stated owning source scope. Runtime and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact quota, rounding, order, conservation and range examples were checked. Runtime and host qualification of the accepted contract are pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for exact capital apportionment

1. Require exact signed capital in the core bounded Decimal domain and an explicit integer `capital_decimal_places` from 0 through 12. Remove the default of 10 and the `money_decimal_places` field. Capital is scalar in common caller-defined units. Currency rounding, settlement increments and trade lots have separate owners. [Core decimal conventions](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).
2. At scale `p`, round `abs(capital)*10^p` once to a whole-unit count `K`, nearest with exact ties to even. Apportion the magnitude and restore the sign afterward. Normalize signed zero. Run full validation of the selected weighting scheme even when the rounded capital is zero. At scale two, `1.005` rounds to `1.00` and `1.015` to `1.02`. [Pinned Decimal rounding rule](https://docs.rs/rust_decimal/1.43.0/rust_decimal/struct.Decimal.html#method.round).
3. Use the same effective shares for applicable risk diagnostics, reporting and apportionment. Equal uses identical integer scores. Fixed uses its exact authored decimal fractions. InverseVolatility and RiskBudget carry the admitted binary64 model scores as exact binary fractions. Align the scores to a common integer basis and use the resulting ratios throughout. Display weights are rounded views. [Accepted weighting rules](01-allocation-weights.md#accepted-decision-for-checked-strategy-allocation-weights). [Accepted risk-budget rules](02-risk-budget-solver.md#accepted-decision-for-checked-covariance-risk-budgeting).
4. For integer scores `s_i` and `S=sum_i(s_i)>0`, compute the exact quotient and remainder of `K*s_i/S` for every sleeve. Allocate the floor quotas, then award one unit to each of the highest exact remainders until the residual is exhausted. Break exact ties by original sleeve order. Exact zero scores receive zero. Check each floor/ceiling bound and exact signed closure to the rounded budget. Separate runs at different budgets may reduce an individual allocation as the total rises. [Hamilton largest-remainder method](https://www.census.gov/about/history/historical-censuses-and-surveys/census-programs-surveys/decennial-census/methods.html).
5. Allow bounded, wider integer intermediates for exact rounding, quota and remainder calculations. Unit counts may exceed `2^53` or the Decimal coefficient range. Require the rounded total and every final sleeve amount to fit the Decimal domain exactly after stripping redundant trailing zeros. Reject the entire result with an output-range error if any amount fails this check.
6. Publish the exact requested and rounded totals, precision, rounding rule and ordered sleeve amounts. Use canonical decimal strings on the wire across Rust, Python and WASM. Make any approximate float view explicit. Checked Rust restoration requires the source scores and rederives quotas, tie outcomes and conservation before admitting the result.

Independent checks found that a budget of `7e28` units divided equally across three sleeves leaves one residual unit. Rounding `1/3` before calculating quotas leaves seven. For a one-unit budget, the fixed fractions `0.4999999999999999999999999999` and `0.5000000000000000000000000001` award the unit to the second sleeve even though both display as `0.5` in binary64.

## Observed choice and domain at the audit revision

After weights are chosen, the allocator conserves a rounded total capital in caller-selected decimal units. Default precision is ten decimal places, maximum twelve. Capital is a finite scalar without a currency tag; negative capital is allowed and its sign is restored after allocation. The procedure addresses loss of integer precision when minor-unit counts exceed the exact integer range of `f64`.

## Ordered algorithm

1. Convert absolute capital and weights into `rust_decimal::Decimal`; conversion or representability failures return an input error.
2. Form `scale=10^decimal_places`; round `abs(capital)*scale` using the Decimal rounding operation to obtain the target whole-unit count. This is a Decimal nearest-rounding step, not the repository currency-specific Money rounding policy.
3. Normalize the Decimal weights by their Decimal total. For each sleeve, compute a finite-precision Decimal quota, take the floor and retain the computed fractional remainder.
4. Compute unallocated whole units. Sort eligible positive-weight sleeves by decreasing remainder, breaking equal remainders by original input index, and award residual units in that order. Zero-weight sleeves receive none.
5. Divide by scale, restore the original capital sign and convert allocations back to `f64`. Report original model weights alongside these rounded amounts.

The conservation invariant applies to the integer/Decimal target. Returning `f64` values does not promise bitwise equality of a later naive floating sum to the caller's original unrounded capital. The method does not enforce minimum tradable lots or instrument-specific settlement increments.

## Alternatives and tradeoffs

Independent rounding per sleeve can drift from the budget. Largest remainder conserves the target deterministically but creates discontinuous one-unit changes at remainder ties and depends on sleeve ordering for ties. Decimal arithmetic avoids the `2^53` minor-unit limitation of binary floating integers; its own finite range remains an explicit limit.

## Evidence

[Source and the beyond-f64-integer-precision test](../../../../finstack-quant/portfolio/src/allocation.rs) provide algorithm and invariant evidence. [Allocation integration fixtures](../../../../finstack-quant/portfolio/tests/weight_allocation.rs) cover representative budgets; none were executed in this audit. The package dependency, rather than a custom decimal implementation, supplies conversion and rounding semantics.

## Follow-up tracking

Implementation and independent validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution).
