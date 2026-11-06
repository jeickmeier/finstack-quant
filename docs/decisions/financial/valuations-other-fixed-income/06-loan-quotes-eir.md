# VALUATIONS-OTHER-FIXED-INCOME-06: Loan discount margins, yields, cost and effective-interest amortization

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

Term loans and revolvers share quote arithmetic in [loan_quotes.rs](../../../../finstack-quant/valuations/fixed-income/src/loan_quotes.rs). The implemented choice is periodic zero-rate spread discounting, annual-effective XIRR, an undiscounted annualized cash-cost measure, and carrying-value roll-forward at that XIRR. Sharing arithmetic prevents the two products from defining the same quote differently; this is an implementation rationale, not an approved accounting policy.

## Scope and input domain

Dated lender cashflows are positive receipts and negative funding. Callers decide the purchased balance, accrued interest, settlement date and which flows belong to the quote. Loan quote rates are decimals; fees and contractual spread inputs are basis points. `OidEirSpec.include_fees` defaults true in [loan_terms.rs](../../../../finstack-quant/valuations/fixed-income/src/loan_terms.rs). No impairment, expected-credit-loss staging, or accounting derecognition is calculated here.

## Algorithm

1. **Discount margin:** for each cash settlement after settlement, obtain `t` on the discount curve's day count and `DF(settlement,payment)`. Convert that DF to its periodic zero-rate equivalent at the caller's coupon frequency, add decimal `dm`, and discount at the shifted periodic rate using the shared `z_spread_discount_factor`. Sum signed flows with Neumaier compensated summation. Zero margin preserves the original curve DF.
2. **Inverse margin:** solve `PV(dm) − target = 0` with fallible Brent, tolerance `1e-10`, initial bracket size `.05`, and the caller's initial guess. Reject a solved absolute margin greater than `.50`. This is a post-solve sanity check, not a ±50% search bracket.
3. **Yield:** prepend the negative purchased dirty value at settlement, retain later `is_cash_settlement()` flows, and call dated `xirr` with the contractual loan day count. Returned yield uses XIRR's annual-effective convention, unlike periodic DM.
4. **All-in rate:** sum future cash interest and fee kinds, excluding PIK and principal; divide by the integral of outstanding balance from `as_of` to maturity, using the stepwise outstanding path and contractual day count. Return zero when that balance-time denominator is nonpositive. This is neither IRR nor PV-weighted cost.
5. **OID/EIR:** aggregate same-date interest, permitted fees, amortization and notional funding/repayment; exclude PIK as a cash receipt. Add optional extra fees and opening funding. Solve XIRR for these signed date buckets. Set opening carrying value to minus the first bucket's net amount. For each later interval, `income = opening × ((1+eir)^year_fraction − 1)`, `closing = opening + income − net_cash`, and `OID_amortization = income − bucket_cash_interest_and_included_fees`.

## Selection, controls and conventions

Term-loan and revolver metrics choose their own input schedules and call these helpers. Optional fees alter both XIRR and amortization. Settlement advances a nonnegative business-day count using an explicit calendar, or weekdays when absent; zero lag returns `as_of` unchanged. Commitment steps use the last ordered step effective on/before the date. Absolute fees are used directly; proportional upfront fees multiply commitment.

## Limitations, failure and fallback

Fallible solver objectives propagate pricing errors. Cashflow sign patterns may have no or multiple IRR roots; helper use does not establish economic uniqueness. Fee inclusion is broad: commitment, usage, facility, LC and fronting fees are treated as interest for the roll-forward when enabled. The carrying-value series is an arithmetic output, not evidence that every fee belongs in EIR under an accounting standard. Full origination cashflows are required; a seasoned balance alone is insufficient to reconstruct historical OID.

## Alternatives and tradeoffs

Continuous z-spread and periodic DM are distinct conventions; only the latter is selected here. A cash-cost ratio is cheaper and interpretable over changing balances, while XIRR recognizes funding timing. Accounting-specific fee classification or credit-adjusted EIR would require explicit additional contracts; neither is inferred from these helpers.

## Evidence

The colocated `zero_margin_reproduces_curve_discounting_and_solve_recovers_it` test checks DF identity and inverse round-trip; `discount_margin_solve_propagates_the_pricing_error` checks failure preservation. These are internal consistency evidence, not independent market-price validation. [Brent](../../../REFERENCES.md#brent-1973) supplies the numerical reference.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
