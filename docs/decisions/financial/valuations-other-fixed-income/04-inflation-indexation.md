# VALUATIONS-OTHER-FIXED-INCOME-04: Inflation-linked cashflows, publication state and deflation floors

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

[Indexation](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/indexation.rs) resolves lagged reference CPI using published observations plus projection and prices unpublished deflation protection as an option. [Types](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/types.rs) generate nominal indexed cashflows discounted on a nominal curve. Using publication state avoids treating future stored observations as known; floor time value distinguishes projected protection from a deterministic maximum. Adoption of this particular option approximation remains open.

## Scope and algorithm

1. TIPS, Canadian, French and Japanese indexation use daily linear interpolation of lagged monthly anchors. Modern UK uses three-month lag with linear interpolation; legacy UK uses eight-month lag with step interpolation. Other UK lags reject. A supplied index must itself have `lag=None` to prevent double lag and must match the expected interpolation.
2. For month lag `L`, anchors are the first day of valuation/payment month minus L months and the next month; weight is `(day−1)/days_in_current_month`. Reference CPI is the anchor interpolation, divided by positive base CPI. Step indexation selects the first-of-month lagged anchor; day lag subtracts calendar days.
3. The market resolver assesses each anchor's publication state independently. Published fixing data takes precedence; unknown anchors project from the inflation curve. Direct index-only and curve-only functions remain explicit alternatives without the market resolver's hybrid selection.
4. Unprotected payment ratio is `F=CPI_forward/base_CPI`. Protected known CPI gives `max(F,1)`. Protected unknown CPI requires `floor_vol_surface_id` with Black-lognormal quote type and strike axis; use Actual/365 Fixed time to publication/fixing date, volatility at strike one, and `ratio=F+BlackPut(F,K=1,DF=1,sigma,T)`.
5. Build real coupons from unadjusted contractual accrual periods with adjusted payment dates, no payment lag/end-of-month roll. Nominal coupon is `face × real_coupon × accrual_fraction × payment_ratio`. `AllPayments` floors coupons and principal; `MaturityOnly` floors only principal; `None` floors neither. Principal pays on adjusted maturity. Only payments strictly after `as_of` enter projected value.

## Controls and conventions

Real coupon is annual decimal Decimal; CPI/base CPI are index levels and ratios dimensionless. The bond owns lag; the market index must not duplicate it. Nominal discount DFs value the inflation-projected money, with no real-discount-curve inference. Floor volatility is explicit; the example with no floor surface cannot price unpublished protected cashflows without supplying one.

## Limitations, failure and alternatives

The floor treats an interpolated CPI ratio as one Black-lognormal forward and uses one expiry; it does not separately model joint uncertainty of two CPI anchors or inflation/rate covariance. No automatic volatility fallback substitutes intrinsic value for unknown protection. Cashflow export may log and omit an index-ratio diagnostic lookup failure even after schedule generation. Observed omission is a reporting fallback, not permission to omit pricing inputs. Deterministic inflation-only valuation and a joint nominal/real-rate model are distinct alternatives with different data needs.

## Evidence

Colocated `stored_cpi_is_not_known_before_publication`, `hybrid_ref_cpi_resolves_published_and_projected_anchors_independently`, `protected_unknown_cpi_includes_option_time_value`, adjusted-maturity and protection-scope tests address these branches. `protected_cpi_ratio_matches_independent_stochastic_density_grid` compares the floor formula with a separate numerical density grid; it does not validate an empirical CPI model. [Deacon–Derry–Mirfendereski](../../../REFERENCES.md#deacon-derry-mirfendereski-2004) and [Kerkhof](../../../REFERENCES.md#kerkhof-2005) are reference context.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
