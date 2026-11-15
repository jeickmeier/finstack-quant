# VALUATIONS-STRUCTURED-CREDIT-20: Settlement quotes, yields, spreads and discount margin

- **Decision status:** Accepted 2026-11-15 — complete settlement, accrued, quote authority, fixed-path yield and spread package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent entitlement, signed accrued, future-settlement, yield and spread calculations checked. Production deal terms, price inversion, risk and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for structured-credit settlement quotes and inversion

1. **Prepare one signed ledger and identify the buyer claim.** Require
   explicit valuation, information and settlement cuts, including the
   phase for same-day cash. Use actual record and delivery rights and
   payable dates. Include eligible funded draws as outflows once.
   Keep redemption cash, retired par and noncash principal effects
   distinct. A tranche quote uses qualified delivered face. A deal
   quote requires an expressly selected debt basket with matching cash,
   denomination and accrued. Remove debt-plus-equity denominators and
   original-face fallback.
   [Filed ABS claim entitlement, §2.7](https://www.sec.gov/Archives/edgar/data/1133438/000119312525081396/d945122dex41.htm).

2. **Compute signed accrued from the original cash coupon.** Use the
   qualified full coupon `C` and its original accrual context. Before
   payment, cumulative accrued is `C × f` for the completed fraction
   of the parent accrual measure. Subtract the full coupon starting on
   the inclusive first-ex date. A delayed cum settlement after accrual
   end carries the full coupon. Reset the adjustment on payment
   independently of same-day cash entitlement. Resolve PIK, deferred
   claims and residual distributions under their own terms. Normalize
   a clean input with `delivered face × clean / 100 + signed accrued`.
   A whole-position dirty Money input requires its own claim and
   settlement authority.
   [Accepted original-parent accrued rules](../cashflows/13-accrued-interest.md#accepted-decision-original-parent-cash-quotation-and-inclusive-first-ex-dates).

3. **Elect one price authority and its settlement bridge.** Supply one
   observed clean or dirty target or elect a qualified labeled derived
   target. Remove the implicit model-price target for spread fitting.
   For a model-derived future-settlement price, value the buyer claim
   from the original valuation date and divide by the qualified
   financing factor. Retain independently valid holder PV. Zero-face
   claims can have whole-position value from payable cash. Prices per
   100 require positive delivered face.
   [Accepted quote and settlement carry](../valuations-bonds/02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks).

4. **Identify the contractual maturity or call projection.** The named
   maturity scenario excludes voluntary exercise only where authorized
   and preserves binding calls, mandatory acceleration and vested
   claims. An actual whole-deal call runs the full waterfall. A
   hypothetical tranche call requires exact exercise and settlement
   phases, live face, price and earned claim treatment. Remove next
   payment-date snapping and blanket truncation of co-dated and
   delayed cash.
   [Accepted terminal cash and calls](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash).

5. **Use continuous Z-spread and curve discount margin on the fixed
   buyer strip.** Use positive dated relative `D(S,T)` and native
   elapsed curve time `τ` for `D(S,T) × exp(−z × τ)`. Entitled
   settlement cash has a unit spread multiplier. Apply the same
   discount shift for curve discount margin on a qualified floating
   note. Freeze contractual margin, known fixings, projected coupons,
   caps, floors and all selected cash. Bind projection and discount
   roles separately. Matching inputs give numerically equal Z-spread
   and curve discount margin. Records 19 and 17 govern facility
   compounding and stochastic OAS, respectively.
   [Accepted spread and margin precedent](../valuations-bonds/12-spread-quotes.md) ·
   [Accepted facility compounding](19-borrowing-base-facility.md#accepted-decision-required-quote-compounding) ·
   [Accepted OAS](17-oas-scenarios.md#accepted-decision-for-structured-credit-oas-and-settlement-spread-fitting).

6. **Require a typed nominal periodic YTM convention.** Use an explicit
   positive integer frequency `m` and qualified settlement-relative
   time `t` with complete day-count context. Discount by
   `exp(−m × t × log1p(y/m))` for annual decimal `y > −m`.
   Preserve admissible negative yields and factor one on eligible
   immediate cash. Remove inferred coupon frequency, annual deal
   defaults, the separate JSON override and the 5% guess. Use the same
   law for forward and inverse quotes. Identify the projection for
   yields on projected floating or amortizing cash.
   [Accepted quote-price inversion](../valuations-bonds/10-quoted-yield.md#accepted-decision-for-checked-settlement-yield-and-price-inversion).

7. **Invert only on an identified price law.** Forward prices admit
   qualified signed cash. For inversion combine cash only under
   identical factor laws. Require positive dirty target above independent
   cash, nonnegative net exposed coefficients and at least one positive
   exposed coefficient. Use checked Brent on an explicit finite bracket
   with positive coordinate tolerance and work budget. Reprice the
   returned rate with a separate absolute native-currency residual gate.
   Remove hard-coded plausibility caps, sentinels and zero placeholders.
   Publish annual decimal yield and spread across direct, Rust registry,
   Python, WASM and stored summaries. Preserve computed zeros and valid
   PV when dependent measures are unavailable.
   [Accepted risk on the identified quote basis](21-risk-bumps-and-scenarios.md#accepted-decision-for-structured-credit-risk-and-scenarios).
   [Accepted checked Brent](../core-numerics/brent-root.md).

## Independent calculation checks

- Delivered face 80 at clean 97.50 gives cash 78. A full delayed
  coupon of 1.50 gives dirty 79.50 cum and 78 after detachment. At
  mid-accrual the same coupon contributes +0.75 cum and −0.75 ex.
  Original-origin buyer PV 76.44 divided by financing factor 0.98
  gives settlement price 78. Retained claims of 20.40 give holder PV
  96.84 at the same origin.
- Entitled settlement cash 3 and one-year discounted cash 95 fit
  target 96 at +2.1277% Z-spread. Excluding immediate cash gives
  −1.0471%. Qualified floating cash 104 at one year with discount
  factor 0.95 and dirty target 100 gives curve margin −1.2073%
  even when the contractual margin is +1%.
- Dirty target 95, settlement cash 4 and cash 100 at 1.5 quote
  years give 6.3371% quarterly or 6.4892% annual nominal YTM.
  Omitting settlement cash gives 3.4342% quarterly and a 4-unit
  residual on the entitled ledger.

## Observed implementation and scope

Structured-credit quotes use current note face and payments strictly after settlement. Cashflow generation is reused, while inverse price calculations hold the generated cashflows fixed. This is a quote convention rather than a promise that collateral behavior responds to every trial spread.

## Algorithm and conventions

Settlement defaults to valuation date and cannot precede valuation or closing. Accrued equals opening note balance × coupon rate × note-day-count fraction from accrual start to min(settlement, accrual end); zero applies outside the open accrual/payment interval. Delayed payment periods can retain accrued after accrual end until payment. Clean target cash is clean-price percent × current face/100 plus accrued. Dirty/clean outputs reverse that transformation.

Z-spread computes `sum(CF × DF(settlement,date) × exp(-s × Act365F_time))` and solves the relative-price residual `PV/target - 1`. `calculate_tranche_z_spread` returns basis points; the registry converts to decimal and rejects results outside [-0.05,0.50] after the solve. The shared kernel uses tolerance 1e-12 and initial bracket size 0.05, without those explicit post-solve plausibility bounds. Neumaier summation reduces cancellation. Discount margin is restricted to floating notes but uses the same fixed-cashflow exponential-spread objective; it does not rebuild coupon forwards at every trial margin. The standalone discount-margin function returns decimal, while summary `dm_bp` returns bp; both reject magnitudes above 5,000 bp after solving.

YTM solves `sum(CF × (1 + y/m)^(-m × t)) = target` with Neumaier summation and Brent, initial guess 5% and tolerance 1e-6. In tranche scope, `m` is note coupon frequency and `t` uses note day count; deal scope uses annual compounding and Act/365F. The `valuations.structured_credit.ytm.v1` extension can override compounding to annual, semiannual, quarterly or monthly; it does not replace the selected time basis. Nonpositive target/no positive future flow errors. Invalid compounding base returns infinity; failed date conversion signals NaN to the solver. Quote validation enforces target inputs; the tranche-summary zero-face/nonpositive-model-value branch reports zero prices/spreads/risks and no yield rather than substituting original principal. Tranche summary deliberately removes the deal call for maturity metrics and reports hypothetical call values separately.

## Limits, failures and alternatives

Failed bracketing/convergence errors; finite-result guards reject nonfinite summaries. Multiple-sign-change flows may lack a unique yield; the inspected evidence does not establish consistent future-draw sign/accounting treatment in each helper. Spread is continuous discount spread, not a hazard intensity or additive floating reset margin. Supported clean/dirty inputs and settlement dates are alternatives; forward-reset discount margin or endogenous prepayment response would require another objective.

## Implementation and validation evidence

- [structured_credit/metrics/quote.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/quote.rs)
- [structured_credit/metrics/pricing/prices.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/pricing/prices.rs)
- [structured_credit/metrics/pricing/mod.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/pricing/mod.rs)
- [structured_credit/types/results.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/results.rs)
- [structured_credit/metrics/risk/spreads.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/spreads.rs)
- [structured_credit/metrics/risk/ytm.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/ytm.rs)
- [structured_credit/types/constants.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/constants.rs)
- [structured_credit/metrics/summary.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/summary.rs)

Existing evidence: [structured_credit/unit/price_basis_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/price_basis_tests.rs); [structured_credit/unit/metrics/pricing_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/pricing_tests.rs); [structured_credit/unit/metrics/risk_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/risk_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [Brent](../../../REFERENCES.md#brent-1973), [fixed-income context](../../../REFERENCES.md#tuckman-serrat-fixed-income). These references identify the method family; they do not certify the preset parameters or every implementation branch.

Roundtrip price/spread tests validate self-consistency; independent quote and yield references were not established.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
