# VALUATIONS-OTHER-FIXED-INCOME-08: Term-loan settlement value and callable rates-credit lattices

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

[Default dispatch](../../../../finstack-quant/valuations/fixed-income/src/term_loan/types.rs) selects Tree whenever a credit curve is supplied or a nonempty call schedule exists; otherwise Discounting. Explicit registry alternatives remain available. A supplied clean-price quote bypasses model valuation and gives a marked settlement value. The inferred rationale is that credit and borrower optionality must influence model value, while a quote is a distinct price driver; approval of these choices remains open.

## Scope and algorithm

The [discounting pricer](../../../../finstack-quant/valuations/fixed-income/src/term_loan/pricing/discounting.rs) first compiles the schedule. For seasoned simple floating coupons, historical resets require exact fixings. It applies covenant spread deltas effective at the accrual start and rescales the coupon by new/old rate; a near-zero old rate uses outstanding-before-payment times rate times accrual. Seasoned floating PIK is rejected because historic capitalization cannot be restated. Non-simple compounding follows the builder path instead.

At settlement, discard cashflows paid on/before settlement and PIK entries. A clean quote becomes dirty value on outstanding plus pending principal, with cash accrued added. Otherwise use quoted periodic z-spread/DM discounting or the unshifted curve. As-of value is `settlement_value × DF(as_of,settlement) + PV(cashflows in (as_of,settlement])`.

The [tree engine](../../../../finstack-quant/valuations/fixed-income/src/term_loan/pricing/tree_engine.rs) performs these stages:

1. Start the uniform grid at settlement and end at legal maturity; default 100 steps. Map tree times to nearest calendar dates on the curve day count. Ordinary cashflows split across floor/ceil nodes; exercise-date cashflows snap to the ceil node. Each piece receives a DF timing correction and an OAS offset so it retains its dated discounting.
2. A supplied hazard curve selects the two-factor rates-credit lattice, calibrated to discount and conditional-survival targets. Its volatilities and correlations come from `resolve_rates_credit_config`; unset factor volatilities are deterministic. No credit curve selects `ShortRateTree`; fixed-rate default sigma is `.01`, while floating-rate default sigma is zero.
3. Book coupon/fee and signed principal cashflows separately. Track outstanding before and after each event. Survival discounts continuation; add fractional recovery of post-payment outstanding using `continuous_frp_weight` over the interval.
4. Calls are standing provisions from each start date until replaced. Redemption is clean call price plus accrued cash; exercise claims account for accrued but not yet capitalized PIK. Make-whole prices use the named reference curve and contractual spread, floored by the call percentage. `Soft` and `Hard` have the same price/exercise rule.
5. Issuer exercise replaces hold value only when `hold > dirty_call + outstanding × call_friction_cents/10000`. Coupon and principal booked at the node are added outside this continuation decision. Future stochastic floating resets are rebuilt from rate nodes; calls inside a reset-to-payment period are disabled.
6. OAS freezes the prepared tree and solves price minus dirty quote in basis points with Brent: tolerance `1e-6 bp`, 50 iterations, initial bracket size 1000 bp. Underlying pricing errors are retained for a failed solve.

## Controls and unsupported combinations

Rates-only trees reject floating coupons with positive sigma and reject unused hazard parameters. Rates-credit stochastic floating PIK is rejected because its outstanding balance is path dependent. Live make-whole calls reject positive rate volatility because exercise-date reference curves are not modeled. OAS is decimal in quote overrides and converted to basis points internally. Tree returns zero if settlement is at/after maturity; as-of return is zero at/after legal maturity in the discounting wrapper.

## Limitations and alternatives

Call-grid exercise and the reset-boundary restriction approximate continuous borrower behavior. Friction is a threshold, not a cash fee retained by the lender. A zero-volatility lattice is available; there is no automatic accuracy-based grid refinement. The rates-only calibration and cashflow timing adjustment call `disc.df(t)` with settlement-relative times. Correct re-anchoring for a nonflat curve whose base differs from settlement is not established by the inspected evidence. Models that explicitly simulate borrower refinancing or stochastic make-whole curves are unimplemented alternatives.

## Evidence

[Tree-pricing tests](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/tree_pricing.rs), [pricing tests](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/pricing.rs), and colocated tree tests exercise dispatch, timing and call behavior; they are not a universal convergence certificate. [Hull–White numerical procedures](../../../REFERENCES.md#hull-white-1994-numerical-procedures) and [Duffie–Singleton](../../../REFERENCES.md#duffie-singleton-1999) are reference context.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
