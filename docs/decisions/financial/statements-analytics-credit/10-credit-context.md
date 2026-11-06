# STATEMENTS-ANALYTICS-CREDIT-10: Instrument DSCR, interest coverage and reference-value LTV

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Instrument credit context combines statement numerator nodes with one instrument's modeled cashflows, retaining missing coverage as unavailable rather than zero. It reports available paths and minima, not a complete-period solvency certificate.

## Scope and input domain

Statement results, capital-structure cashflows, instrument identifier, explicit reporting currency, requested periods, CFADS and earnings nodes, and optional `(PeriodId, reference value)` observations. Instrument cashflows and typed monetary nodes must match reporting currency; untyped scalars are interpreted in that currency.

## Algorithm

1. If the instrument is absent, return an empty default result. Build a lookup of reference values; duplicate periods use the last supplied value. Process requested periods in their supplied order.
2. Require compatible numerator-node currencies and finite inputs. Missing instrument cashflow rows mark the period skipped. Cash interest, PIK interest, principal, fees and debt balances must be finite and currency-consistent.
3. Compute cash DSCR = CFADS/(cash interest + principal); total DSCR = CFADS/(cash interest + PIK + principal); fee-inclusive DSCR = CFADS/(cash interest + principal + fees). Separately compute interest coverage = earnings/(cash interest + PIK). A denominator <= 0 omits that measure, and non-finite computed ratios error. Negative finite numerators are permitted.
4. If a finite positive reference value is provided for a period, LTV = ending debt balance/reference value. Missing/nonpositive reference values omit LTV. LTV can still be computed when a coverage numerator is unavailable.
5. Missing any coverage measure marks that period skipped and produces a warning, while retaining other available measures. Minima are taken over available ratios only.

## Selection and controls

Node names explicitly select CFADS versus earnings concepts. No ratio uses automatic EBITDA, EBITDA adjustments or consolidation. No FX conversion, period sorting or reference-path interpolation is performed.

## Conventions

DSCR/coverage/LTV are dimensionless multiples. Fee-inclusive DSCR adds fees to cash service, not to the total cash-plus-PIK denominator. The reference-value path is in the instrument currency and periods map by exact identifier.

## Limitations, failure and fallback

An absent instrument returns empty output without marking every requested period skipped. Available-only minima can omit the weakest period when its inputs are missing. Nonpositive denominators are omitted rather than treated as infinite coverage. Duplicate reference-value periods overwrite silently.

## Alternatives and tradeoffs

Complete-path minima or minima paired with a coverage fraction are alternative reporting contracts. Erroring on duplicate references would improve provenance. A consolidated DSCR would need multiple-instrument aggregation and explicit numerator/debt-service alignment.

## Evidence

[credit_context.rs](../../../../finstack-quant/statements-analytics/src/analysis/credit/credit_context.rs) owns the calculation and inline tests. Located cases cover hand-computed DSCR/coverage/LTV, missing coverage versus zero, LTV surviving absent coverage, mixed-currency rejection and finite-input overflow. These support formulas and availability semantics rather than default-risk calibration.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
