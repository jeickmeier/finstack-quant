# CORE-CONVENTIONS-016: Continuous flat discounting and valuation-date-normalized cashflow NPV

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `cashflow/discounting.rs`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

`flat_discount_factor` computes continuous-compounding discount factors. General NPV delegates time basis and discount factors to a supplied `Discounting` curve, normalizes to the valuation date and includes strictly future payments only. No curve construction, projection or FX conversion occurs in this helper. Continuous discounting and valuation-date normalization are explicit implementation choices rather than product-model selection.

## Algorithm and conventions

Flat DF is `exp(-rate * years)`, with decimal continuous rate and finite nonnegative time. Rate may be negative; zero rate/time gives 1. Reject nonfinite rate/time, negative time and nonfinite output; underflow to zero is accepted by this flat helper.

For NPV, reject empty flows. Read curve `day_count`, `day_count_context` and `base_date`; compute signed curve time to valuation base, `t0`, and `DF0=disc.df(t0)`. Reject DF0 unless finite positive. For each flow with date **strictly after base**, compute curve time to payment using the same day-count/context and factor `DF(t)/DF0`, requiring finite positive. Sum discounted amounts. Flows on base and earlier are excluded without reinvestment or accrued interest.

Money NPV first checks every input currency, including excluded flows, against the first flow's currency; each DF is converted to Decimal and multiplied using Money checked arithmetic, then exact-as-representable Decimal addition. Amount-only NPV validates all `f64` inputs as finite, including excluded flows, then accumulates discounted values with Neumaier compensation; each product and final total must be finite. All-past input returns zero after validating inputs and DF0.

## Selection, controls and limitations

Callers select curve and valuation date; the curve supplies context, including Bus252 calendar. Currency is inferred from the first Money flow rather than a curve-currency check. There is no `include_today` option, cashflow-kind filter, accrued value, settlement adjustment or past-flow discounting mode. A flat DF underflow accepted by the low-level helper is rejected if supplied as a curve factor in NPV. Compensated float and Decimal paths have different precision/error behavior and need not be bit-identical.

## Alternatives and tradeoffs

The Money and float-amount paths are supported alternatives. A base-date-inclusive convention or discount-to-curve-origin convention would alter observed behavior. Different compounding conventions belong in supplied curves rather than this flat formula; no nominal/simple flat override is provided.

## Evidence

[Implementation](../../../../finstack-quant/core/src/cashflow/discounting.rs). Inspected inline tests include `npv_discounts_to_valuation_date_when_base_differs_from_curve_base`, `npv_rejects_invalid_valuation_date_discount_factor`, `npv_counts_business_days_on_the_curve_calendar`, `flat_discount_factor_matches_the_closed_form`, and `npv_precision_many_cashflows`. The flat formula and valuation normalization tests provide direct expected-value evidence; curve tests also rely on the production curve builder. [Hull](../../../REFERENCES.md#hull-options-futures) and [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) support financial/numerical background.

No independent vendor valuation reconciliation was executed or inferred.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
