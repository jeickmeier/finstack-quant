# STATEMENTS-CONTRACTS-008: Bundled balance leverage and trailing-flow coverage

**Decision status:** Accepted for the full bundled leverage and coverage contract (2026-11-15).

**Implementation status:** Reconciled (2026-11-17). All six bundled metrics use `ratio`, so a missing input and a zero or negative denominator are unavailable with a reason and never a division error; zero gross debt is a valid zero and a negative EBITDA gives signed coverage over a positive denominator. The balance ratios read period-end values; debt to EBITDA, interest coverage and debt-service coverage read `ttm(...)`, which is unavailable for an incomplete or gapped window. Interest coverage divides by statement `interest_expense`, including recognized PIK. Debt-service coverage divides by `scheduled_cash_interest_due + scheduled_principal_due`: explicit inputs for the scheduled cash interest and scheduled principal recognized once on their contractual due dates. A capital structure reports both as components (`cs.scheduled_cash_interest_due.<scope>` and `cs.scheduled_principal_due.<scope>`), taken from each borrowing's contractual schedule before the waterfall allocates cash, so a missed installment is recognized on its due date and arrears settlements, advance payments, prepayments, sweeps, PIK, fees and hedge flows are excluded. The `principal_payment` input is no longer read. The descriptions make no threshold claim. The sections below the accepted decision describe the audited catalogue.

**Validation status:** Qualified by hand-summed values in the [registry tests](../../../../finstack-quant/statements/tests/spec/registry_tests.rs): balance ratios at each close, a complete trailing year (EBITDA 160, interest 40, scheduled service 80) and the three incomplete quarters before it, a gapped window, zero gross debt, negative EBITDA with positive and negative denominators, and a zero denominator. Due-date service is qualified through the evaluator in the [residual regressions](../../../../finstack-quant/statements/src/capital_structure/tests/finance_residual_regressions.rs): an installment and coupon missed for lack of cash are due once in their own quarter, the quarter that settles them reports only its own due amounts, and a sweep, a capitalized coupon share and revolver fees are excluded.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for bundled leverage and coverage

Use period-end balance-sheet values and gross interest-bearing debt before cash offsets. Use consistent reporting scope and currency across all inputs. EBITDA follows the reported-cost earnings contract. [Reported-cost earnings](07-earnings-and-margins.md#accepted-decision-for-reported-cost-earnings-and-margins).

| Metric | Accepted formula |
|---|---|
| Debt to equity | Period-end gross debt / period-end equity |
| Debt to assets | Period-end gross debt / period-end assets |
| Equity multiplier | Period-end assets / period-end equity |
| Debt to EBITDA | Period-end gross debt / TTM EBITDA |
| Interest coverage | TTM EBITDA / TTM statement interest expense including recognized PIK |
| Debt-service coverage | TTM EBITDA / (TTM scheduled cash interest due + TTM scheduled principal due on borrowings) |

Recognize each scheduled borrowing service amount once on its contractual due date, including missed amortization and maturity payments. Exclude arrears settlements, advance payments, prepayments and sweeps, PIK, fees, hedge flows and distress cash from scheduled cash debt service. [Dated claims and settlements](../statements-capital/04-dated-principal.md#accepted-decision-for-dated-principal-claims).

Require positive denominators and complete supported TTM windows. Report unavailable with a reason for missing or invalid inputs or incomplete windows. Zero gross debt is valid. Negative EBITDA yields signed coverage when the other inputs and denominator are valid. Remove generic threshold claims from the bundled ratios. Covenant thresholds and definitions require explicit agreement-specific terms. [Reporting periods and TTM windows](../core-conventions/014-fiscal-periods-ltm.md#accepted-decision-reporting-periods-fiscal-context-and-ltm-windows).

## Audited input domain

Six embedded fin metrics measure balance leverage or EBITDA-based debt coverage. They combine current model balances with either current balances or trailing-twelve-month flows. The catalogue selects these formulas explicitly; its prose does not establish an approved credit policy or covenant definition.

## Audited algorithm and controls

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
