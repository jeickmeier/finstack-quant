# VALUATIONS-OTHER-FIXED-INCOME-13: Agency MBS amortization, prepayment and delayed cashflow ownership

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

[The pool kernel](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/pricer.rs) models a homogeneous level-pay mortgage pool using a supplied prepayment specification and agency payment rule. Scheduled mortgage amortization uses monthly WAC, while investor interest uses the explicit accrual day count. Separating these clocks is documented in the source; pool homogeneity and prepayment assumptions still require adoption.

## Scope and algorithm

1. Start from `current_face`. Find the earliest unpaid accrual month from `last_paid_accrual_end`, or infer it by comparing delayed payment dates with `as_of`. The holder path retains a prior month's P&I still in flight.
2. Project monthly while balance is at least `.01`, the period cap is not reached and the legal-maturity stopping condition permits. WAM is remaining maturity at valuation; it shrinks once per projected month and is not reduced again for seasoning. PSA/other seasoning instead uses `floor(days_since_issue/30.4375)`.
3. For balance `B`, remaining payments `n`, monthly mortgage rate `r=WAC/12`, scheduled principal is `B` for `n≤1`; otherwise `max(0,min(B, B r(1+r)^n/((1+r)^n−1) − Br))`. If `r≤1e-12`, use `B/n`.
4. Apply `prepayment = max(B−scheduled,0) × SMM` after scheduled amortization. Interest is `B × net_coupon × accrual_fraction`; ending balance is floored at zero. `SMM` must be finite and in `[0,1]`.
5. Emit separate interest, amortization and prepayment rows. Investor coupon must match `WAC − servicing_bp/10000 − guarantee_bp/10000` within `1e-6`. Discount payments strictly after `as_of` using the nominal curve's dated DF ratio; native value requests WAM+12 as a projection cap.
6. Quote-basis purchasers receive settlement-month accrual onward. `quote_basis_pool` marks prior accrual months paid, excluding the seller's in-flight P&I. Dirty quote adds settlement-month interest accrued from the first day of that month. This differs intentionally from holder NPV.

## Selection and conventions

[Types and payment rules](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/types.rs) supply the sole native Discounting path. FNMA/FHLMC pay on the following month's 25th, GNMA I on the 15th and GNMA II on the 20th, rolled Following on `usny`. Custom stated delay `D` maps to month offset `(D−1)/30` and day `D−30k`; it is not literal calendar-day addition. Default/custom prepayment is supplied explicitly to the pool. Original face affects pool factor; current face drives cashflows.

## Limitations, failure and fallback

The implementation does not model loan-level heterogeneity, defaults/guarantee timing, delinquency advances, burnout, seasoning-adjusted WAC or stochastic house prices. Positive WAM is required; a balance below one cent is treated as retired. A legal maturity earlier than remaining WAM can stop projection before full scheduled retirement, so full retirement is not guaranteed for misaligned inputs. Missing paid-period state is inferred from dates, not from an external servicing ledger. The negative/tiny monthly-rate branch uses straight-line principal rather than a negative-rate annuity.

## Alternatives and tradeoffs

A homogeneous monthly pool is inexpensive and reusable by TBA and MC-OAS. Loan-level simulation and empirical refinancing/burnout models offer different behavior but require borrower data and calibration. Holder receivable valuation and clean-quote spread fitting require distinct cash ownership bases; silently sharing one basis would misprice delayed P&I.

## Evidence

Colocated projection tests and `agency_convention_vector_is_stable` in [types](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/types.rs) check decomposition, amortization and dated agency cases. The [SIFMA formula reference](../../../REFERENCES.md#sifma-mbs-standard-formulas) supplies methodology context. No independent servicing tape was reconciled.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
