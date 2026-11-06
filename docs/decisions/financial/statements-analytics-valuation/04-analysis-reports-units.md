# STATEMENTS-ANALYTICS-VALUATION-04: Corporate-analysis gating, monetary reconciliation and credit reports

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice and domain

[Orchestration](../../../../finstack-quant/statements-analytics/src/analysis/valuation/orchestrator.rs) combines statement evaluation, optional DCF and optional capital-structure analysis. [Reports](../../../../finstack-quant/statements-analytics/src/analysis/reports.rs) derive P&L tables, trailing-year credit ratios and text summaries. [Unit reconciliation](../../../../finstack-quant/statements-analytics/src/analysis/units.rs) controls typed-value compatibility. Credit-specific checks/metrics are owned by the neighboring credit-analysis scope; this record covers selection, result construction and report arithmetic here.

## Algorithms and actual controls

- If DCF or capital structure is requested, a supplied `CheckSuite` must include the exact `non_finite` check ID. Evaluate statements, run the suite and block on error-severity failures. Statement-only analysis takes the simpler evaluation path and does not run even a supplied check suite. The interest-coverage numerator defaults to `ebitda` and the DCF UFCF node to `ufcf`; CFADS must be explicitly mapped when capital-structure cashflows need it.
- Builder `net_debt_override` modifies an already configured DCF; calling it before configuring DCF has no stored effect. In the convenience function, terminal-value specification without WACC does not enable a DCF. WACC without terminal value errors.
- A positive DCF EV can be broadcast as a flat value path for LTV. An explicit value node overrides this and supplies period values; missing/nonpositive values are skipped. Wrong-currency/scalar typed nodes are rejected by the strict LTV extraction path. Nonpositive EV status is exposed separately rather than manufacturing a denominator.
- Unit helpers reconcile declared `NodeValueType` with typed money. Monetary projections must match the typed amount after money rounding and use the same currency. Scalar/undeclared values remain allowed in the helper that merely checks expected currency; strict DCF/LBO extraction instead requires money. The ratio helper checks the quotient for finiteness rather than both operands independently (for example, a finite numerator over infinity can yield finite zero); surrounding report calculations add operand guards, and ratio operands must have matching type/currency metadata where checked.
- Credit reports use the core complete `ltm_window` to sum trailing EBITDA and interest with per-period unit checks. Leverage is current debt divided by positive TTM EBITDA; coverage is TTM EBITDA divided by positive TTM interest. Missing/invalid denominators or incomplete windows produce unavailable ratios, not fabricated values. A requested period must exist; series retain model order for period IDs up to that period. Current-period free cashflow is not trailing-year cashflow.
- P&L table rows preserve missing numeric entries as null and render `-` in text. The text report labels money as `$M`; it does not infer display currency/scale from each result. The checked credit assessment filters nonfinite current FCF, while raw text formatting can still print a nonfinite FCF.

## Failure boundaries, alternatives and evidence

The typed-unit checks do not establish a universal invariant for all analysis helpers: [variance](11-variance-bridges.md) and [scenario tables](09-scenario-inheritance.md) use weaker checks. Callers must not interpret a rendered table as validation of units or finiteness. Check-ID presence is not equivalent to independent validation of all financial inputs. Gate behavior and order-sensitive builder setters are observed API contracts, not approval of those contracts.

Colocated orchestration tests cover required check suite, error-severity blocking, LTV currency/scalar rejection, scalar EV broadcasting and skipped node values. Report tests cover fiscal-quarter/year boundaries, daily leap years, business-day windows, incomplete TTM and scalar-to-report reconciliation. These are internal contracts; no financial-report certification is established. Alternatives include universal evaluation checks, deferred builder settings, currency-aware formatting and an enforced typed-result boundary; they are not silently enabled.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
