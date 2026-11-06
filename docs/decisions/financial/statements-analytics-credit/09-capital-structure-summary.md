# STATEMENTS-ANALYTICS-CREDIT-09: Seniority-grouped debt, trailing coverage and maturity wall

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

The summary groups current debt balances by payment-class rank and computes cumulative debt/earnings and earnings/interest through each entire rank. Treating pari-passu tranches as a group avoids declaration-order dependence within the same seniority level.

## Scope and input domain

A financial model with capital structure, evaluated capital-structure cashflows, selected model period, EBITDA node and optional cash node. Supported debt is bond, convertible bond, term loan and revolver; interest-rate swaps, caps/floors and swaptions are excluded, and unsupported instrument kinds error.

## Algorithm

1. Build the debt stack. With declared payment classes every included debt needs an assignment; without classes all debt has implicit rank zero. Stable-sort by rank then original declaration order. Require each instrument's selected-period row and a common balance currency; foreign currency values are rejected without FX conversion.
2. Obtain end-of-period balances and checked Money totals. Use core LTM window selection over model periods ending at the selected period. Only a full calendar year of finite EBITDA observations yields a sum; use Neumaier accumulation and reject overflow. A short or missing window leaves trailing earnings unavailable. Nonpositive available earnings remain displayed but ratios are absent.
3. Optional cash is required at the selected period if configured and must match debt currency when typed monetary. Scalar EBITDA/cash is interpreted in debt currency. Net debt = total debt − cash and can be negative.
4. For every tranche, sum cash plus PIK interest across the full LTM window. Missing any row makes that tranche's trailing interest unavailable; incompatible currencies error. Aggregate each entire rank before computing cumulative debt/positive EBITDA and positive EBITDA/positive interest. An unavailable interest component makes coverage unavailable from that rank through junior ranks. All pari-passu tranches share the through-rank ratios.
5. Tranche shares are balance/total debt only when total debt > 0. The maturity wall places each nonzero current balance in its final contractual calendar maturity year, then totals by year. Zero-balance debt remains listed but creates no wall bucket.
6. Table export flattens Money amounts and emits default result metadata; currency remains on the summary, not in the numeric table's metadata.

## Selection and controls

The selected period and declared payment-class ranks control the report; there is no future projection or priority-waterfall simulation here. Full-window availability and positive denominators control optional ratios. Currency/finite guards produce errors for contradictory typed data or unrepresentable ratios.

## Conventions

Debt is current end-of-period outstanding; interest includes cash and PIK. Leverage is gross cumulative debt unless the separately shown net-debt figure is used by the caller. Maturity-wall years are calendar years, and allocations use final maturity rather than amortization dates.

## Limitations, failure and fallback

The wall is not a schedule of future principal due: it ignores interim amortization, refinancing and conditional conversion/exercise. Scalar inputs rely on implicit currency assumptions. Incomplete LTM data suppresses ratios; partial-year annualization is deliberately absent. Exported numeric tables can lose the summary's currency context unless callers carry it separately.

## Alternatives and tradeoffs

A contractual cashflow maturity ladder answers a different question from current balances grouped by final maturity. Net/secured leverage, rank-specific EBITDA allocation or covenant adjustments require explicit definitions. Currency-bearing table metadata would preserve report interpretation without changing arithmetic.

## Evidence

[capital_structure.rs](../../../../finstack-quant/statements-analytics/src/analysis/credit/capital_structure.rs) owns all steps and inline tests. Located evidence includes `ltm_sums_four_quarters_not_the_last_one`, business-day full-year cases, `figures_through_a_class_are_shared_by_its_pari_passu_tranches`, maturity-wall grouping, missing-row propagation, mixed-currency and non-finite-ratio rejection. These are formula/window/contract tests, not economic underwriting approval.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
