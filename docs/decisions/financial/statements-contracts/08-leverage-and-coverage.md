# STATEMENTS-CONTRACTS-008: Bundled balance leverage and trailing-flow coverage

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice README](README.md)).

## Choice and input domain

Six embedded fin metrics measure balance leverage or EBITDA-based debt coverage. They combine current model balances with either current balances or trailing-twelve-month flows. The catalogue selects these formulas explicitly; its prose does not establish an approved credit policy or covenant definition.

## Algorithm and controls

| Metric | Executed formula |
|---|---|
| debt_to_equity | total_debt / total_equity |
| debt_to_assets | total_debt / total_assets |
| equity_multiplier | total_assets / total_equity |
| debt_to_ebitda | total_debt / ttm(ebitda) |
| interest_coverage | ttm(ebitda) / ttm(interest_expense) |
| debt_service_coverage | ttm(ebitda) / ttm(interest_expense + principal_payment) |

All results are dimensionless multiples. Debts, interest and principal use the caller's positive-liability/expense/payment magnitudes. Same-currency amount ratios are permitted by model typing; no currency conversion or debt netting is included. TTM dispatch delegates to the statements evaluator's period-aware algorithm; the registry does not independently select the period window.

Selection is through loading the embedded catalogue and adding the metric or all built-ins to the model. The inherited [EBITDA formula](07-earnings-and-margins.md) and any user override of its qualified ID change all three trailing coverage measures. No tax adjustment, cash availability, mandatory-only principal filter, covenant add-back cap or PIK exclusion is performed here.

## Limitations and alternatives

The previously agreed direction rejects ambiguous partial-period TTM windows; the concrete implementation of that rule belongs to the temporal evaluator audit. A registry formula string alone cannot establish that every supplied timeline satisfies it.

The same formulas can produce negative or undefined ratios for negative/zero equity, EBITDA or interest. No industry, rating or legal agreement threshold is applied at runtime. Text claims about 3x leverage, 1.5x interest coverage or 1.25x debt-service requirements are descriptive catalogue prose with no identified quantitative qualification here; they are not accepted universal standards.

EBIT-based interest cover, cash-only debt service, net-debt ratios and agreement-specific EBITDA are meaningful alternatives. A custom registry formula can express them, but the bundled names do not automatically switch.

## Evidence

[Leverage catalogue](../../../../finstack-quant/statements/data/metrics/fin_leverage.json), [registry dispatch](../../../../finstack-quant/statements/src/registry/builtins.rs) and [registry tests](../../../../finstack-quant/statements/tests/spec/registry_tests.rs) provide selection evidence. The inspected builtin_credit_ratios_use_ttm_flows assertions pin exact TTM formula strings; they do not independently validate the temporal kernel or an agreement's covenant definition. [Corporate valuation reference](../../../REFERENCES.md#koller-valuation) is contextual, not evidence for the catalogue's threshold prose.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
