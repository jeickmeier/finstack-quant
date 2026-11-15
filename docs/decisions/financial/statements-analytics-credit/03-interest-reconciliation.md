# STATEMENTS-ANALYTICS-CREDIT-03: Direct and balance-implied interest checks

- Decision status: **Accepted for the full interest-reconciliation contract (2026-11-15)**.
- Implementation status: **Reconciliation pending**.
- Validation status: **Existing evidence mapped. Independent qualification pending**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for interest reconciliation and estimates

1. Elect direct interest reconciliation or an advisory balance-based estimate explicitly. Missing required direct data makes the direct comparison incomplete.
2. For the direct comparison, verify the period, instrument scope, recognition basis, currency and sign. Include earned cash-pay interest and recognized PIK once where applicable. Distinguish scheduled interest, settlements, arrears and the closing accrued-interest stock. Explicitly map effective-interest amortization, fees and hedge effects included in reported interest expense. [Contractual interest accrual](../statements-capital/03-funding-and-balances.md#accepted-decision-for-contractual-accrued-cash-interest).
3. For the advisory estimate, multiply each selected borrowing's average opening and closing outstanding principal by its decimal period rate and sum the results. Use a dated first opening and the immediately preceding close thereafter. All selected balances and rates are required. An annual quote requires an explicit quotation convention and day count. For simple annual interest, multiply the quoted decimal rate by the year fraction.
4. Use verified inputs and checked arithmetic. Compare known zeros, mark missing required inputs incomplete and treat invalid inputs or arithmetic as errors. Report typed actual, expected, signed difference and tolerance. Direct reconciliation defaults to zero tolerance and uses the expected value's magnitude for an explicitly selected relative allowance. The estimate requires an explicit advisory tolerance. [Numerical comparisons and check suites](../statements-contracts/14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites).

For example, USD 6 of earned cash-pay interest and USD 4 of PIK recognized in March give March interest expense of USD 10, even if the cash is paid in April. In an advisory estimate, outstanding principal rising from USD 100 to USD 120 at 8% simple annual interest over a quarter-year gives USD 2.20.

## Audited choice and rationale

Interest expense first compares against an explicit capital-structure interest node, otherwise against a balance-times-rate estimate. Average balances approximate accrual when a prior observation exists. The source does not implement an annual-rate-to-period conversion.

## Audited scope and input domain

Income-statement interest, optional direct capital-structure interest, or a list of debt-balance nodes with optional rate-node names. Inputs are looked up in model-period order. Relative tolerance defaults to 0.05 (5%).

## Audited algorithm

1. Skip a period when IS interest is missing or non-finite. If a direct capital-structure node is configured, require its finite value or skip the period; compare `abs(IS−CS)/max(abs(CS),1)` against tolerance and stop the period's fallback path.
2. Only when no direct node is configured, use the balance/rate path and skip near-zero IS interest. For each debt/rate pair, require available finite current balance and rate; pairs without rates or usable values do not contribute. Average prior/current balance when both are finite, otherwise use current balance.
3. Sum effective balance times the supplied rate. Compare only when at least one rate contributed and implied interest is not near zero. A strict relative deviation above tolerance generates a Warning. The relative diagnostic is multiplied by 100 in the finding.
4. Missing prior balance silently selects current balance; the comment that promises a warning for that substitution is not implemented.

## Audited selection and controls

A configured direct-interest node wins even when its missing value causes a skip; the estimate is not a fallback for missing direct data. No day count, period fraction or payment schedule enters the balance estimate. No finite/ordering validation is applied to the tolerance itself here.

## Audited conventions

To reproduce the implemented estimate, a rate must already be a decimal rate for the modeled period. An annual nominal/effective rate supplied against quarterly balances will not be scaled. Magnitudes and signs are taken directly; there is no currency conversion or explicit unit compatibility check.

## Audited limitations, failure and fallback

Pairs omitted from the sum can yield partial implied interest without an incompleteness finding. No post-sum finite guard is present. The relative denominator floor of one makes small-value comparisons depend on reporting scale. End balances approximate, rather than integrate, changing debt and rates.

## Audited alternatives and tradeoffs

Direct capital-structure cash/PIK reconciliation or annual-to-period rate conversion using explicit dates/day counts are alternatives to the balance estimate. Time-weighted outstanding balances are more accurate but require intra-period events. An incomplete estimate result would expose dropped pairs.

## Evidence

[interest.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/reconciliation/interest.rs) owns the calculation. [reconciliation_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/reconciliation_tests.rs) locates `interest_cs_matches_passes`, `interest_cs_mismatch_flags_warning`, `interest_implied_rate_reasonable_passes` and `interest_implied_rate_uses_average_balance`. These are local arithmetic/selection evidence; they do not establish annual-rate scaling.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
