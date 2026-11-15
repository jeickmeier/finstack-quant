# STATEMENTS-ANALYTICS-CREDIT-02: Capex, PPE, dividends and working-capital identities

- Decision status: **Accepted for the full CapEx, PPE, dividends and working-capital contract (2026-11-15)**.
- Implementation status: **Reconciliation pending**.
- Validation status: **Existing evidence mapped. Independent qualification pending**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for stock-flow reconciliations

1. Compare cash CapEx to cash-paid PPE and intangible additions in the same scope. When additions are on an accrual basis, map cash-timing and noncash adjustments explicitly. Convert the cash-flow statement outflow to a spend magnitude once using the declared sign convention. [Financial perspectives and boundary conversions](../conventions/0044-financial-perspectives-and-boundary-conversions.md#decision).
2. Calculate expected net PPE as opening carrying amount plus capitalized PPE additions, less like-scope depreciation and the carrying amount of disposals, plus explicit signed adjustments such as FX or impairment. Use a dated opening for the first selected comparison and the immediately preceding close thereafter.
3. Compare equity-recognized dividends directly with cash paid only when scope and timing match. Otherwise, expected cash paid equals opening dividends payable plus recognized cash-settleable dividends and signed payable adjustments, less closing dividends payable.
4. Calculate operating working capital as operating current assets less operating current liabilities. Remove explicitly mapped noncash and scope changes from its period change, then negate the adjusted change to obtain the expected operating cash-flow contribution.
5. Use verified results with matching scope, compatible units and one currency for each monetary identity. Use checked arithmetic. A missing or masked mapped required input makes the comparison incomplete, and typed zero is a known amount. Record actual, expected, signed difference and tolerance for every evaluated comparison, including passes. Default to zero tolerance, and use the expected amount's magnitude as the reference for any explicitly elected relative allowance. An out-of-tolerance comparison fails under the suite policy. [Numerical comparisons and check suites](../statements-contracts/14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites).

For example, USD 5 of opening dividends payable plus USD 30 of recognized cash-settleable dividends less USD 12 of closing payable gives USD 23 of expected cash paid. A signed cash outflow of USD −23 reconciles. A missing required closing payable makes the comparison incomplete.

## Audited choice and rationale

Cross-statement checks compare explicit accounting identities using absolute tolerances and report mismatches as warnings. The shared choice is arithmetic reconciliation on supplied nodes rather than reconstructing journal entries or normalizing sign conventions.

## Audited scope and input domain

Ordered model periods and mapped scalar/monetary values; optional component nodes and absolute tolerance overrides. Capex, depreciation and dividends use a `SignConventionPolicy`. Working-capital components and movement must have compatible units across both periods.

## Audited algorithm

- Capex: expected capex = PPE additions + intangible additions. No component mappings returns an empty pass. A missing/non-finite primary capex skips the period; a mapped optional component missing/non-finite is treated as zero. Compare CF capex minus expected additions; relative diagnostic is `100*abs(diff)/max(abs(expected),1)`.
- Depreciation: for adjacent periods, expected PPE = prior PPE + capex − depreciation − disposals. The first period and periods missing required finite inputs are skipped; absent/non-finite optional disposals become zero. Compare actual closing PPE with expected, with relative diagnostic denominator `max(abs(prior PPE),1)`.
- Dividends: compare cash-flow and equity dividends directly. Missing values skip; a non-finite operand yields a warning and skips arithmetic. Relative diagnostic uses `max(abs(equity dividends),1)`.
- Working capital: sum current assets minus current liabilities for both periods, then expected CF contribution = negative change in NWC. Missing components contribute zero. Validate each component against the current CF movement's units and its own prior/current units before looking up the movement, then reject a non-finite difference; on a mismatch also reject a non-finite relative diagnostic. The first period skips; missing/non-finite CF movement skips after unit validation.
- Capex, PPE and dividends store actual, expected and tolerance comparisons even when within tolerance; working capital returns only mismatch findings, with no stored comparison rows. A strict absolute difference above tolerance generates a Warning, so the result can remain passed.

## Audited selection and controls

Use per-check tolerance when present, otherwise the check context's absolute tolerance. `MagnitudePositive` can warn on negative capex/depreciation/dividend inputs; `InflowPositive` suppresses that warning but does not reverse signs or alter the identity. The convenience three-statement suite supplies no disposal node.

## Audited conventions

Values must already be expressed in the sign convention required by each equation. Absolute tolerance is in the input monetary/scalar unit; relative deviation is a diagnostic percentage, not the pass threshold. Period adjacency follows model order rather than a sort performed here.

## Audited limitations, failure and fallback

Capex, PPE and dividend arithmetic do not enforce the cross-node unit/finite-result checks present in working capital. Missing optional values becoming zero can mask incomplete data. Sign policy does not transform negative CFS outflows into positive capex/dividend magnitudes. No asset-sale gain/loss, FX translation, impairment, revaluation or acquisition bridge is inferred.

## Audited alternatives and tradeoffs

A signed cash-flow normalization layer would support inflow-positive statements but must change equations consistently. A richer PPE reconciliation could include disposals, FX and acquisitions at the cost of more required mappings. Missing-is-error or explicit incomplete results would avoid treating absence as zero.

## Evidence

[capex.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/reconciliation/capex.rs), [depreciation.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/reconciliation/depreciation.rs), [dividends.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/reconciliation/dividends.rs), [working_capital.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/consistency/working_capital.rs) are the owners. [reconciliation_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/reconciliation_tests.rs) locates balanced/mismatch identities, no-components pass and NaN-dividend handling; [consistency_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/consistency_tests.rs) locates WC reconciliation cases. These provide equation and boundary evidence, not accounting-standard certification.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
