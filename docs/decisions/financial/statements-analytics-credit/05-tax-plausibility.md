# STATEMENTS-ANALYTICS-CREDIT-05: Effective-tax-rate informational bands

- Decision status: **Accepted for the full effective-tax-rate screening contract (2026-11-15)**.
- Implementation status: **Reconciliation pending**.
- Validation status: **Existing evidence mapped. Independent qualification pending**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for informational effective tax rates

1. For this internal informational check, divide signed recognized statement income-tax expense by strictly positive pretax income for the same period, reporting scope and currency. Tax charges are positive and benefits are negative. Explicitly select actuals, forecasts or both. [Reported-cost earnings and tax expense](../statements-contracts/07-earnings-and-margins.md#accepted-decision-for-reported-cost-earnings-and-margins).
2. Require finite, ordered decimal bounds with a recorded source, basis and reporting cadence. Both bounds are inclusive. A rate outside the bounds produces an informational finding.
3. Treat zero pretax income and a pretax loss as distinct reasons the rate is inapplicable. Show signed tax expense when available. A scope consisting entirely of valid zero-income or loss periods is unevaluated.
4. Report the inputs, the rate or reason it is unavailable, and the bounds. Missing pretax income or missing tax expense for a period with positive pretax income makes the check incomplete. Invalid inputs, incompatible units or unrepresentable arithmetic produce errors. [Numerical comparisons and check suites](../statements-contracts/14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites).

For example, a USD 5 tax benefit on USD 100 of pretax income gives −5%, an informational finding under a band of 0% to 30%. With a USD 50 pretax loss and a USD 10 tax benefit, report pretax income of USD −50 and tax expense of USD −10, with the rate inapplicable due to the loss.

## Audited choice and rationale

Tax expense divided by positive pretax income is compared with a caller band. The suite labels this an informational mechanical screen; it is not a jurisdictional statutory-rate or tax-accounting model.

## Audited scope and input domain

Finite tax expense and pretax income for each model period, plus decimal minimum and maximum effective rates.

## Audited algorithm

Skip missing/non-finite inputs and periods with pretax income <= 0. Compute `tax/pretax`. A strict result outside the accepted band yields an Info finding. Equality at either bound passes; Info does not fail the check.

## Audited selection and controls

The three-statement suite enables the check only when both mappings exist and sets the range [0,0.50]. No annualization, loss carryforward, deferred-tax adjustment or multi-jurisdiction aggregation occurs.

## Audited conventions

Tax rates and range endpoints are decimals. Positive pretax income is the sole economic applicability test. Negative provisions on positive income can be flagged but are not automatically accounting errors.

## Audited limitations, failure and fallback

Loss/zero-income periods are excluded, so they have no coverage status. Input units, division overflow and finite/ordered bounds are not validated here. A wide default band is not evidence that rates within it are reasonable or those outside it are wrong.

## Audited alternatives and tradeoffs

Jurisdictional or blended statutory reconciliations would explain permanent/temporary differences but require tax data. Separate loss-period diagnostics avoid division instability while retaining review coverage.

## Evidence

[tax_rate.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/consistency/tax_rate.rs) and [suites.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/suites.rs) own calculation/defaults. [consistency_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/consistency_tests.rs) locates `etr_within_range_passes`, `etr_outside_range_flags_info` and `etr_skips_negative_pretax`; these are formula and selection evidence.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
