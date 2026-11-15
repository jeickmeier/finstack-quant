# STATEMENTS-ANALYTICS-VALUATION-01: Statement cashflows, terminal metric and corporate DCF bridge

- **Decision status:** Accepted for the full Statements corporate DCF contract (2026-11-15).
- **Implementation status:** Reconciliation pending.
- **Validation status:** Existing evidence mapped. Independent qualification pending.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Accepted decision for Statements corporate DCF

1. Author the effective valuation date and identify the statement snapshot and valuation currency explicitly. Apply the point-in-time cutoff even when evaluation needs no market data, and require the cutoff to be on or before the effective valuation date. Use market inputs where statement evaluation requires them. Discount the DCF using WACC.
2. Select a named unlevered FCFF node with complete signed `Money` in the valuation currency for all selected forecast periods. Elect literal period-close or midpoint timing from each period's actual bounds. A period that straddles the valuation date requires an explicitly modeled forward remainder. Specify the terminal date at or after the end of the explicit forecast.
3. For Gordon or H-model, supply normalized annual FCFF at the terminal horizon or expressly elect a complete 12-month additive forecast aggregate. Stable growth must be finite, greater than −100% and below WACC. A ceiling applies only if the caller explicitly selects one. An exit metric and multiple must be finite and nonnegative on a matched enterprise-value basis. A trailing aggregate is available only for an additive metric. The Valuations owner defines the terminal formulas, including the H-model. [Valuations corporate DCF](../valuations-equity-and-composition/17-corporate-dcf.md#accepted-decision-for-checked-corporate-dcf-and-dilution).
4. Author a finite annual effective WACC above −100%, or calculate it using explicit nonnegative weights that sum exactly to one, finite annual effective component costs above −100%, and a tax rate in `[0,1]`. Use authored bridge amounts or named statement balances reconciled to the effective valuation date and currency. A net-debt-only bridge requires an explicit election. Report debt less cash separately from other claims and valuation discounts.
5. Use the Valuations owner's results for explicit flow PV, terminal PV, enterprise value, the equity bridge, discounts and dilution. Retain the statement run, timing election, assumptions and input references for replay. The cashflow export contains only explicit forecast flows. Terminal and bridge amounts remain valuation components.

For example, take an effective valuation date of December 31, 2024, USD 100 of forecast cash on December 31, 2025, and a terminal date of January 1, 2026. Normalized annual FCFF of USD 100, WACC of 10% and stable growth of 2% give Gordon terminal value of USD 1,275 and enterprise value of USD 1,249.78 on ACT/365.25. Debt of USD 70 and cash of USD 20 give equity value of USD 1,199.78 before valuation discounts.

## Audited choice, domain and selection

[Corporate valuation](../../../../finstack-quant/statements-analytics/src/analysis/valuation/corporate.rs) converts evaluated statements into a `DiscountedCashFlow` instrument. UFCF and balance-sheet inputs must be finite monetary values in the explicit ISO currency in `model.meta["currency"]`; no implicit currency, relabeling or FX conversion is provided. Optional market data affects statement evaluation only and requires an explicit as-of date. WACC discounts the DCF; market curves do not replace it.

`wacc` blends `wE × costE + wD × costD × (1−tax)`. Inputs are finite decimal rates, nonnegative weights summing to one within `1e−6`, and tax in `[0,1]`. Negative costs are not separately prohibited here. This is the observed capital-cost convention; empirical weight/cost estimation and tax-shield realizability are caller responsibilities.

## Audited algorithm and controls

1. Evaluate the statement model, with market/as-of if both are supplied. Default valuation date is the first forecast period's start, otherwise the first period's start.
2. Select forecast UFCF, date each flow at `period.end−1 day`, and retain only dates strictly after valuation date. The monetary type and currency are checked at extraction.
3. For Gordon or H-model terminal value, sum a complete contiguous calendar year ending at the last forecast end. Walk whole periods backward, requiring each end to match the cursor and no period to cross the one-year start; a leap-day prior-year start falls back to February 28. Missing values, gaps, stubs crossing the boundary and nonfinite sums error. There is no fractional-period annualization.
4. Stable growth must be below WACC and at most the configured stable-growth ceiling, default `.05`. H-model high growth must be at least stable growth and half-life positive. For exit multiple, an optional metric node is resolved on the same full-year basis; otherwise the literal terminal metric must be positive and finite. A metric-node option is irrelevant to other terminal models.
5. Resolve the EV-to-equity bridge: explicit `equity_bridge` wins, then `net_debt_override` becomes a debt-only bridge, otherwise select the latest model balance period whose final included day is on/before valuation date. Debt aliases are `total_debt`, then `debt`; cash aliases are `cash`, then `cash_and_equivalents`. A future forecast balance cannot substitute for an opening balance.
6. Build the DCF with WACC, terminal specification, optional shares/valuation discounts and mid-year convention. Delegate explicit-flow discounting, terminal formulas and equity adjustments to the [owning DCF methodology](../valuations-equity-and-composition/17-corporate-dcf.md). Explicit rows report `CF/(1+WACC)^t` using the instrument's own discount time; EV equals explicit-flow PV plus terminal PV. Equity is the instrument result; reported valuation discount reconciles `EV−effective_net_debt−equity` when discounts exist.

The result includes money-valued EV, equity, net debt, terminal PV, cashflow detail and optional per-share/dilution outputs. The embedded executable DCF instrument is not serialized; reloading a result is not reconstructing its pricing inputs.

## Audited limits, alternatives and evidence

Full-year terminal extraction rejects ambiguity rather than scaling a stub; arbitrary stub annualization would be a distinct, unimplemented policy. WACC is externally specified, not fitted to traded equity/debt, and the simple tax term is not a tax-capacity model. The strict cashflow date rule can exclude valuation-date flows even when statements include them; that exclusion is part of the observed date boundary.

Colocated `wacc_blends_equity_and_after_tax_debt` is a formula check; `dcf_detail_reconciles_to_enterprise_and_equity_value` and `valuation_discount_reconciles_the_equity_bridge` are internal reconciliations, not independent prices. `exit_multiple_metric_requires_complete_trailing_year` and currency/balance-sheet tests cover failure contracts. [Production valuation audit cases](../../../../finstack-quant/statements-analytics/tests/production_valuation_audit.rs) provide additional located regression evidence. [Koller](../../../REFERENCES.md#koller-valuation) supplies general valuation context, not validation of each convention.

No model adoption or tests-passed assertion follows from this inspection.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
