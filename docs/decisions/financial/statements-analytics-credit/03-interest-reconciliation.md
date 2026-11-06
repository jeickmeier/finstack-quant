# STATEMENTS-ANALYTICS-CREDIT-03: Direct and balance-implied interest checks

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Interest expense first compares against an explicit capital-structure interest node, otherwise against a balance-times-rate estimate. Average balances approximate accrual when a prior observation exists. The source does not implement an annual-rate-to-period conversion.

## Scope and input domain

Income-statement interest, optional direct capital-structure interest, or a list of debt-balance nodes with optional rate-node names. Inputs are looked up in model-period order. Relative tolerance defaults to 0.05 (5%).

## Algorithm

1. Skip a period when IS interest is missing or non-finite. If a direct capital-structure node is configured, require its finite value or skip the period; compare `abs(IS−CS)/max(abs(CS),1)` against tolerance and stop the period's fallback path.
2. Only when no direct node is configured, use the balance/rate path and skip near-zero IS interest. For each debt/rate pair, require available finite current balance and rate; pairs without rates or usable values do not contribute. Average prior/current balance when both are finite, otherwise use current balance.
3. Sum effective balance times the supplied rate. Compare only when at least one rate contributed and implied interest is not near zero. A strict relative deviation above tolerance generates a Warning. The relative diagnostic is multiplied by 100 in the finding.
4. Missing prior balance silently selects current balance; the comment that promises a warning for that substitution is not implemented.

## Selection and controls

A configured direct-interest node wins even when its missing value causes a skip; the estimate is not a fallback for missing direct data. No day count, period fraction or payment schedule enters the balance estimate. No finite/ordering validation is applied to the tolerance itself here.

## Conventions

To reproduce the implemented estimate, a rate must already be a decimal rate for the modeled period. An annual nominal/effective rate supplied against quarterly balances will not be scaled. Magnitudes and signs are taken directly; there is no currency conversion or explicit unit compatibility check.

## Limitations, failure and fallback

Pairs omitted from the sum can yield partial implied interest without an incompleteness finding. No post-sum finite guard is present. The relative denominator floor of one makes small-value comparisons depend on reporting scale. End balances approximate, rather than integrate, changing debt and rates.

## Alternatives and tradeoffs

Direct capital-structure cash/PIK reconciliation or annual-to-period rate conversion using explicit dates/day counts are alternatives to the balance estimate. Time-weighted outstanding balances are more accurate but require intra-period events. An incomplete estimate result would expose dropped pairs.

## Evidence

[interest.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/reconciliation/interest.rs) owns the calculation. [reconciliation_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/reconciliation_tests.rs) locates `interest_cs_matches_passes`, `interest_cs_mismatch_flags_warning`, `interest_implied_rate_reasonable_passes` and `interest_implied_rate_uses_average_balance`. These are local arithmetic/selection evidence; they do not establish annual-rate scaling.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
