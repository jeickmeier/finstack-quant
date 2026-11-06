# STATEMENTS-ANALYTICS-CREDIT-05: Effective-tax-rate informational bands

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Tax expense divided by positive pretax income is compared with a caller band. The suite labels this an informational mechanical screen; it is not a jurisdictional statutory-rate or tax-accounting model.

## Scope and input domain

Finite tax expense and pretax income for each model period, plus decimal minimum and maximum effective rates.

## Algorithm

Skip missing/non-finite inputs and periods with pretax income <= 0. Compute `tax/pretax`. A strict result outside the accepted band yields an Info finding. Equality at either bound passes; Info does not fail the check.

## Selection and controls

The three-statement suite enables the check only when both mappings exist and sets the range [0,0.50]. No annualization, loss carryforward, deferred-tax adjustment or multi-jurisdiction aggregation occurs.

## Conventions

Tax rates and range endpoints are decimals. Positive pretax income is the sole economic applicability test. Negative provisions on positive income can be flagged but are not automatically accounting errors.

## Limitations, failure and fallback

Loss/zero-income periods are excluded, so they have no coverage status. Input units, division overflow and finite/ordered bounds are not validated here. A wide default band is not evidence that rates within it are reasonable or those outside it are wrong.

## Alternatives and tradeoffs

Jurisdictional or blended statutory reconciliations would explain permanent/temporary differences but require tax data. Separate loss-period diagnostics avoid division instability while retaining review coverage.

## Evidence

[tax_rate.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/consistency/tax_rate.rs) and [suites.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/suites.rs) own calculation/defaults. [consistency_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/consistency_tests.rs) locates `etr_within_range_passes`, `etr_outside_range_flags_info` and `etr_skips_negative_pretax`; these are formula and selection evidence.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
