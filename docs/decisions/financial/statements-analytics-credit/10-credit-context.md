# STATEMENTS-ANALYTICS-CREDIT-10: Instrument DSCR, interest coverage and reference-value LTV

- Decision status: **Accepted for the full instrument credit context contract (2026-11-15)**.
- Implementation status: **Reconciliation pending**.
- Validation status: **Existing evidence mapped. Independent qualification pending**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for instrument credit ratios and path coverage

1. Explicitly select one eligible borrowing, the requested measures and unique canonical periods. Use statement and capital results from the same verified run, with matched scope and currency. Reject an unknown instrument. When a known instrument's required data is missing, retain the period row and give the reason.
2. Use a named CFADS series in each selected DSCR. Cash DSCR uses scheduled cash interest and scheduled principal due in the period. Cash-plus-PIK DSCR adds recognized PIK once. Fee-inclusive cash DSCR adds explicitly selected cash fees to scheduled cash debt service. Interest coverage separately uses named earnings divided by matching recognized interest, including recognized PIK once. [Dated principal claims](../statements-capital/04-dated-principal.md#accepted-decision-for-dated-principal-claims) · [Interest recognition](03-interest-reconciliation.md#accepted-decision-for-interest-reconciliation-and-estimates).
3. Calculate LTV as period-end economic face divided by positive collateral value matched by date, debt scope and currency. Reject duplicate collateral references. An explicitly selected DCF enterprise value supports a separately labeled debt-to-EV measure only on its valuation date. Evaluate collateral availability independently of debt-service and interest coverage. [Corporate analysis and typed reporting](../statements-analytics-valuation/04-analysis-reports-units.md#accepted-decision-for-corporate-analysis-and-typed-reporting).
4. Require positive denominators and preserve signed numerators. Confirmed zero debt service makes the corresponding ratio inapplicable. Report the typed operands or reason for unavailability, and coverage counts for each measure. Label an observed minimum partial when required periods are missing. A full-path minimum requires complete applicable coverage.

For example, USD 30 of CFADS and USD 20 of scheduled cash debt service give cash DSCR of 1.5×. If the next quarter has scheduled service but missing CFADS, 1.5× is the observed minimum over one of two applicable quarters. The full-path minimum is unavailable.

## Audited choice and rationale

Instrument credit context combines statement numerator nodes with one instrument's modeled cashflows, retaining missing coverage as unavailable rather than zero. It reports available paths and minima, not a complete-period solvency certificate.

## Audited scope and input domain

Statement results, capital-structure cashflows, instrument identifier, explicit reporting currency, requested periods, CFADS and earnings nodes, and optional `(PeriodId, reference value)` observations. Instrument cashflows and typed monetary nodes must match reporting currency; untyped scalars are interpreted in that currency.

## Audited algorithm

1. If the instrument is absent, return an empty default result. Build a lookup of reference values; duplicate periods use the last supplied value. Process requested periods in their supplied order.
2. Require compatible numerator-node currencies and finite inputs. Missing instrument cashflow rows mark the period skipped. Cash interest, PIK interest, principal, fees and debt balances must be finite and currency-consistent.
3. Compute cash DSCR = CFADS/(cash interest + principal); total DSCR = CFADS/(cash interest + PIK + principal); fee-inclusive DSCR = CFADS/(cash interest + principal + fees). Separately compute interest coverage = earnings/(cash interest + PIK). A denominator <= 0 omits that measure, and non-finite computed ratios error. Negative finite numerators are permitted.
4. If a finite positive reference value is provided for a period, LTV = ending debt balance/reference value. Missing/nonpositive reference values omit LTV. LTV can still be computed when a coverage numerator is unavailable.
5. Missing any coverage measure marks that period skipped and produces a warning, while retaining other available measures. Minima are taken over available ratios only.

## Audited selection and controls

Node names explicitly select CFADS versus earnings concepts. No ratio uses automatic EBITDA, EBITDA adjustments or consolidation. No FX conversion, period sorting or reference-path interpolation is performed.

## Audited conventions

DSCR/coverage/LTV are dimensionless multiples. Fee-inclusive DSCR adds fees to cash service, not to the total cash-plus-PIK denominator. The reference-value path is in the instrument currency and periods map by exact identifier.

## Audited limitations, failure and fallback

An absent instrument returns empty output without marking every requested period skipped. Available-only minima can omit the weakest period when its inputs are missing. Nonpositive denominators are omitted rather than treated as infinite coverage. Duplicate reference-value periods overwrite silently.

## Audited alternatives and tradeoffs

Complete-path minima or minima paired with a coverage fraction are alternative reporting contracts. Erroring on duplicate references would improve provenance. A consolidated DSCR would need multiple-instrument aggregation and explicit numerator/debt-service alignment.

## Evidence

[credit_context.rs](../../../../finstack-quant/statements-analytics/src/analysis/credit/credit_context.rs) owns the calculation and inline tests. Located cases cover hand-computed DSCR/coverage/LTV, missing coverage versus zero, LTV surviving absent coverage, mixed-currency rejection and finite-input overflow. These support formulas and availability semantics rather than default-risk calibration.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
