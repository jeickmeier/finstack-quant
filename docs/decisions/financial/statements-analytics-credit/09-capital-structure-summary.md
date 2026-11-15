# STATEMENTS-ANALYTICS-CREDIT-09: Seniority-grouped debt, trailing coverage and maturity wall

- Decision status: **Accepted for the full capital structure summary contract (2026-11-15)**.
- Implementation status: **Reconciliation pending**.
- Validation status: **Existing evidence mapped. Independent qualification pending**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for capital structure summaries and maturity walls

1. Use verified period-end economic principal for bonds, convertible bonds, term loans and drawn revolvers. Report hedges and pending settlements separately. When payment classes are declared, require exactly one valid assignment per borrowing. Otherwise use one implicit group. Order by class rank and then instrument declaration order within the rank. Retain known zero balances.
2. Report native balances and currency totals. An elected consolidation uses approved period-close FX with quote provenance for each contributing period. A missing required quote makes the affected consolidated result unavailable while native results remain visible. Report signed net debt when the caller explicitly selects eligible cash and it is available on the same reporting basis. [Capital reporting and FX](../statements-capital/09-reporting-and-fx.md#accepted-decision-for-capital-reporting-and-fx).
3. Sum each entire rank and all senior ranks before calculating through-rank ratios. Pari-passu borrowings share those ratios. Leverage requires positive EBITDA over a complete TTM window. Coverage divides signed TTM EBITDA by positive TTM in-scope borrowing interest. Standard coverage uses recognized statement interest, including recognized PIK once. A separately elected schedule-based ratio is distinctly labeled. Missing required interest affects coverage for its own rank and junior ranks. [Bundled leverage and coverage](../statements-contracts/08-leverage-and-coverage.md#accepted-decision-for-bundled-leverage-and-coverage).
4. Group current positive face by final contractual calendar maturity year in the maturity wall. Use dated principal claims for any schedule of amounts due. Calculate debt shares only when the covered gross balance is complete and positive. Publish exact currency-tagged `Money` and run provenance. Give reasons for unavailable values and keep unaffected figures visible. [Dated principal claims](../statements-capital/04-dated-principal.md#accepted-decision-for-dated-principal-claims).

For example, two pari-passu borrowings with balances of USD 40 and USD 60 and TTM EBITDA of USD 50 share leverage of 2×. If both mature in 2030, the maturity wall reports USD 100 in 2030. A USD 20 installment due in 2028 belongs in the dated schedule.

## Audited choice and rationale

The summary groups current debt balances by payment-class rank and computes cumulative debt/earnings and earnings/interest through each entire rank. Treating pari-passu tranches as a group avoids declaration-order dependence within the same seniority level.

## Audited scope and input domain

A financial model with capital structure, evaluated capital-structure cashflows, selected model period, EBITDA node and optional cash node. Supported debt is bond, convertible bond, term loan and revolver; interest-rate swaps, caps/floors and swaptions are excluded, and unsupported instrument kinds error.

## Audited algorithm

1. Build the debt stack. With declared payment classes every included debt needs an assignment; without classes all debt has implicit rank zero. Stable-sort by rank then original declaration order. Require each instrument's selected-period row and a common balance currency; foreign currency values are rejected without FX conversion.
2. Obtain end-of-period balances and checked Money totals. Use core LTM window selection over model periods ending at the selected period. Only a full calendar year of finite EBITDA observations yields a sum; use Neumaier accumulation and reject overflow. A short or missing window leaves trailing earnings unavailable. Nonpositive available earnings remain displayed but ratios are absent.
3. Optional cash is required at the selected period if configured and must match debt currency when typed monetary. Scalar EBITDA/cash is interpreted in debt currency. Net debt = total debt − cash and can be negative.
4. For every tranche, sum cash plus PIK interest across the full LTM window. Missing any row makes that tranche's trailing interest unavailable; incompatible currencies error. Aggregate each entire rank before computing cumulative debt/positive EBITDA and positive EBITDA/positive interest. An unavailable interest component makes coverage unavailable from that rank through junior ranks. All pari-passu tranches share the through-rank ratios.
5. Tranche shares are balance/total debt only when total debt > 0. The maturity wall places each nonzero current balance in its final contractual calendar maturity year, then totals by year. Zero-balance debt remains listed but creates no wall bucket.
6. Table export flattens Money amounts and emits default result metadata; currency remains on the summary, not in the numeric table's metadata.

## Audited selection and controls

The selected period and declared payment-class ranks control the report; there is no future projection or priority-waterfall simulation here. Full-window availability and positive denominators control optional ratios. Currency/finite guards produce errors for contradictory typed data or unrepresentable ratios.

## Audited conventions

Debt is current end-of-period outstanding; interest includes cash and PIK. Leverage is gross cumulative debt unless the separately shown net-debt figure is used by the caller. Maturity-wall years are calendar years, and allocations use final maturity rather than amortization dates.

## Audited limitations, failure and fallback

The wall is not a schedule of future principal due: it ignores interim amortization, refinancing and conditional conversion/exercise. Scalar inputs rely on implicit currency assumptions. Incomplete LTM data suppresses ratios; partial-year annualization is deliberately absent. Exported numeric tables can lose the summary's currency context unless callers carry it separately.

## Audited alternatives and tradeoffs

A contractual cashflow maturity ladder answers a different question from current balances grouped by final maturity. Net/secured leverage, rank-specific EBITDA allocation or covenant adjustments require explicit definitions. Currency-bearing table metadata would preserve report interpretation without changing arithmetic.

## Evidence

[capital_structure.rs](../../../../finstack-quant/statements-analytics/src/analysis/credit/capital_structure.rs) owns all steps and inline tests. Located evidence includes `ltm_sums_four_quarters_not_the_last_one`, business-day full-year cases, `figures_through_a_class_are_shared_by_its_pari_passu_tranches`, maturity-wall grouping, missing-row propagation, mixed-currency and non-finite-ratio rejection. These are formula/window/contract tests, not economic underwriting approval.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
