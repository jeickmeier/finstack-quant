# VALUATIONS-OTHER-FIXED-INCOME-06: Loan discount margins, yields, cost and effective-interest amortization

- **Decision status:** Accepted 2026-11-14 — full checked loan quotes, cash cost and effective-interest package.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Reconciliation pending. The observed entry points and kernels are described below.
- **Validation status:** Live source and primary references inspected. Independent quote, cash-cost and effective-interest arithmetic checked. Runtime, market and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked loan quotes, cash cost and effective interest

1. **Bind actual settlement, funded par and one quote authority.**
   Require the exact purchaser and transferred position, eligible
   settlement date and phase, separate information cut and one
   authorized quote as of that cut. Use an authorized actual
   settlement date or derive it from an elected trade date,
   nonnegative business lag, covered calendar and contractual
   adjustment. An explicitly elected zero lag requires a valid
   same-day settlement. Weekdays-only requires an explicit
   calendar choice. Trading or delayed-compensation terms cannot
   supply a missing actual settlement or entitlement.

   For a per-100 quote require positive purchased funded par on
   the agreed denomination basis. Include a pending principal
   receivable only when its amount and right actually transfer on
   that basis. Keep undrawn commitment and prospective funding
   obligations separate. Convert clean points to lender dirty
   Money by `points × purchased_par/100 + signed_cash_accrued`.
   Compute accrued under the qualified original-parent and
   assignment rules with the actual eligible elapsed economics.
   A full-period coupon prorated across changing funding does not
   establish the elapsed amount. A direct dirty amount needs no
   unused accrued calculation. Preserve separately billed delayed
   compensation, trade expenses, third-party fees and settlement
   adjustments on their own bases. An actual trade invoice
   enters the analytical quote only if the authorized quote
   definition explicitly includes the same components. For this
   quote require a funded transferred claim and positive dirty
   purchase. Quoted rates are annual decimals with checked bp
   conversions, and clean price points are per 100.
   [Settlement claim and authority](../valuations-bonds/02-settlement-and-clocks.md),
   [original-parent cash accrued](../cashflows/13-accrued-interest.md),
   [LSTA on settlement and delayed compensation](https://events.lsta.org/app/uploads/2025/04/Mastering-Complicated-Loan-Market-Intricacies-09Apr25.pdf),
   [filed loan assignment](https://www.sec.gov/Archives/edgar/data/1487918/000114420415034989/v412237_ex10-1.htm).

2. **Prepare the complete buyer cash strip on the same basis.**
   Use one admitted scheduled maturity path, or an expressly
   elected fixed workout under its product rules. Hold known
   fixings, authorized projections, contractual margins and fee
   economics, schedules, draw assumptions and the ownership of
   each entitled cash item fixed through a DM fit or yield
   calculation. Use signed lender settlement cash, including only
   cash and obligations that transfer, with actual dates and phases.
   PIK is a noncash principal change. Its eventual cash repayment
   belongs in the elected strip when entitled. Same-day cash
   payable to the buyer is immediate at factor one. Cash already
   paid or retained by a seller has no buyer value on that phase.

   A quote driver may be a qualified observation or an expressly
   authorized derived buyer value. For a derived settlement value,
   value the actual buyer claim `J` at the original valuation
   origin and divide by the independently qualified positive
   financing factor `F(V,S)`. The inverse uses the same claim
   bridge. An additive retained-cash bridge is admitted only when
   it fully resolves the difference. A credit, contingent exercise
   or simulated base PV needs qualified buyer ownership and
   valuation on that basis before it can supply the price. A
   separately qualified, frozen expected cash strip can supply a
   labeled quote on expected cash when the buyer's full obligations
   and entitlement are represented on that strip. This does not
   estimate the expected path yield or adjust the DM for option or
   default risk. An unsupported quote leaves independent raw
   holder pricing available.
   [Buyer claim bridge](../valuations-bonds/02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks),
   [recognized cash and PIK](../core-conventions/015-cashflow-semantics.md).

3. **Use a checked curve DM with explicit quote compounding.**
   Admit the curve DM experiment for the qualified floating-rate
   buyer cash strip. Keep projection and cash discounting as
   separately bound roles. For each future cash date obtain
   positive finite native relative `D(S,T)` and matching
   source-coordinate interval `t=c(T)−c(S)`. For Continuous apply
   `log D_s=log D−s t`. For nominal Periodic(`m`), with required
   positive integer quote frequency, set `b=exp(−log D/(m t))`
   and `D_s=(b+s/m)^(-m t)` for `t>0). Require the full shifted
   base positive and checked. At zero spread preserve the qualified
   original factor. An entitled immediate claim or a separately
   admitted native-time plateau with exact factor one is
   spread-independent. Reject negative or unresolved source time.
   Use the native source clock and its complete context. Coupon
   tenor cannot select`m` or replace that clock. Simple requires
   its own qualified spread law. This curve margin can differ from
   the contractual spread and uses a different basis from the
   Treasury FRN auction margin.

   Sum qualified signed cash on a common native position basis.
   For inversion, net spread-sensitive amounts only when their
   complete factor functions coincide. Require a positive dirty
   target strictly above net spread-independent cash, nonnegative
   net amounts at sensitive coordinates and at least one positive
   sensitive amount. An admitted signed path can support forward
   pricing even when its inverse fails this monotonicity test.
   Use checked fallible Brent on an explicit finite ordered
   in-domain bracket with positive coordinate tolerance and a work
   limit. Reprice the returned annual decimal spread against the
   original dirty amount with an explicit positive absolute
   native-currency residual tolerance. Remove the contractual
   guess, hidden expansion, post-solve ±50% cap and input or
   pricing error substitution. Forward clean and price per 100
   use the same admitted signed accrued and denomination.
   [Accepted spread/curve and inverse laws](../valuations-bonds/12-spread-quotes.md),
   [checked compounding](../core-numerics/compounding.md),
   [checked Brent](../core-numerics/brent-root.md),
   [Treasury FRN auction rules](https://treasurydirect.gov/files/laws-and-regulations/auction-regulations-uoc/31-cfr-part-356.pdf).

4. **Use canonical dated XIRR for the admitted loan maturity yield.**
   Add the negative authorized dirty purchase at its actual
   settlement date to the matched buyer cash strip, including
   eligible same-day cash. Use accepted canonical date netting
   before testing eligibility. Require a strictly negative net
   investment at settlement after entitled immediate receipts
   and exactly one sign reversal over distinct effective dates.
   Propagate core solver failures. The maturity yield follows the
   named maturity scenario. Early exercise and workout selection
   stay with the separate loan records. Retain ACT/365F or use the explicitly selected
   ACT/360 annual-effective quote, with equal-growth conversion
   `q_B=expm1((B/365) log1p(r_365))`, `B∈{365,360}`.
   No alternate coupon-clock XIRR or automatic guess.
   Check the actual represented quote on its stated day basis
   using the same prepared net stream and accepted scaled
   residual rule. Annual yield can be negative above the
   admissible growth boundary. An unavailable or insensitive
   path has no numerical yield substitute.
   [Accepted dated XIRR](../core-conventions/017-irr.md#accepted-decision-irr-and-xirr-timing-eligibility-and-numerical-acceptance),
   [current annual yield basis adapter](../../../../finstack-quant/valuations/core/src/metrics/returns.rs).

5. **Measure running cash cost over covered funded balance-time.**
   Define `all_in_rate = eligible_running_cash / Σ(B_i Δt_i)`
   on explicit opening and closing holder cuts of a positive
   interval. Use the actual pending/paid cutoff for each cash
   boundary. Select signed lender cash coupon and expressly
   eligible running fees owned by the same facility position
   within those cuts. Exclude funding, repayment, noncash PIK,
   origination OID/upfront cost and nonrecurring or third-party
   fees. A dated fee's category alone cannot establish inclusion.
   Use an explicit covered opening funded balance, then replay
   ordered real principal effects on their economic dates and
   phases, including capitalized PIK. On the authorized interval
   integrate the piecewise balance `B_i≥0` on ACT/365F. Keep
   contractual coupon and fee day counts in cash generation.
   Include admitted lagged cash at its actual date only within
   the selected cuts, with covered zero balance after retirement.

   Use checked amounts and a strictly positive funded exposure
   denominator. Zero cash on positive exposure gives zero.
   An undrawn fee-only facility, missing coverage, mixed
   currency and unrepresentable results cannot yield the ratio.
   Require a qualified deterministic, realized or fixed prepared
   path with time-matched running cash and exposure. A simple
   average of simulation cash rows does not establish the required
   exposure. This annual cash-date statistic is separate from
   effective yield and the commitment-normalized approximation
   in record 12. Remove synthetic issue funding from current
   balance and date-only unconstrained running fee selection.
   [Balance replay and covered schedules](../cashflows/12-schedule-lifecycle.md),
   [revolver cost and quote owner](12-revolver-quotes-risk.md).

6. **Apply shared commitment and fee terms on their legal bases.**
   Validate an authorized opening commitment and strictly ordered,
   unambiguous effective steps in one currency. Use the last
   effective authorized amount through the next phase. Remove
   implied sorting and unchecked assumptions of coverage.
   An upfront amount uses the authored Money in facility
   currency, or a checked explicit fraction of the originally
   bound opening commitment. Date it and identify its recipient
   and cash purpose. A separately authorized pro rata reduction
   fee uses the positive eligible reduction times the stated
   bp rate divided by 10,000. Commitment increases, disallowed
   fee types and other fee bases require their owning rules.
   Keep DDTL and revolving limits, funding and fee schedules
   with records 07 and 10. General fee calculation does not
   elect either cash-cost inclusion or EIR classification.
   [Term-loan cashflow rules](07-term-loan-cashflows.md),
   [revolver cashflow rules](10-revolver-cashflows.md),
   [filed agreement illustrating fee bases and payees](https://www.sec.gov/Archives/edgar/data/1418076/000119312526445328/ck0001418076-ex10_2.htm).

7. **Require a complete authorized origination effective-interest
   path.** Elect the diagnostic explicitly. Bind the original
   origination/recognition cut and knowledge state, actual
   opening net funding, contractual principal, original cash
   estimates or an authoritative original ledger, and complete
   coverage through discharge. Distinguish original known cash
   and original projected cash. Replace blanket `include_fees`
   and default EIR with explicit eligibility of each included
   fee, discount or premium and directly assigned cost under the
   chosen lender perspective. Qualified later draw, repayment and
   prepayment cash enter on their actual or originally elected
   dates by identity. PIK and balance changes without settlement
   enter the noncash ledger. Recover capitalized principal through
   the later settlement cash once. Withheld OID and a separately
   paid OID receipt are alternative representations of initial net
   funding. Do not use both for the same amount or change legal par.
   Service, LC, fronting and commitment fees require their own
   economic classification. A fee's contract type or its running
   cash-cost eligibility does not make it integral.

   Open at the first authorized net funding bucket and require
   it to be the first retained XIRR date and strictly negative.
   An earlier fee or separate recognition treatment needs
   its own qualified basis. Do not backdate current drawn
   principal, substitute the current projection market, merge
   revised estimates into an original snapshot or infer a
   complete history from a seasoned facility. Repeat draws are
   admissible only if the complete dated cash stream meets the
   one-sign-change contract. No stochastic-average carry,
   modified credit-adjusted EIR, ECL, impairment, derecognition
   or subsequent accounting re-estimation is selected here.
   The output is a scoped contractual-path effective-interest
   calculation with explicit fee and funding inputs.
   [AASB 9 text on fee and interest-method scope, Appendix A and B5.4](https://standards.aasb.gov.au/aasb-9-dec-2022),
   [FASB on origination fees and direct costs](https://storage.fasb.org/ASU%202022-02.pdf).

8. **Roll carrying value on the same dated return.** Solve the
   authorized origination stream with the same accepted one-sign
   XIRR and equal-growth reporting basis. Use checked same-date
   Money preparation for contractual components and the same
   admitted net scalar amounts for solving, roll and checks.
   Set opening carry to the opposite of first net funding.
   Retain economic component dates for output even where net
   cash on a date is zero and the solver omits that date.
   In each later elapsed interval, `I=B_open ×
   expm1(log1p(eir) × actual_days/B)`. Set
   `B_close=B_open+I−net_cash` using the signed lender cash
   ledger, including new funding after the opening where admitted.
   Preserve economic PIK on its effective date and exclude it
   from net cash. Compute with unrounded model carry and validate
   the full roll and native currency amounts. Verify the terminal
   unrounded carrying residual against an explicit positive
   absolute Money tolerance at legal discharge. Never plug the
   carry to zero or feed rounded presentation Money back into
   subsequent periods.

   Report the rate, dated opening and closing carry, effective
   interest and cash components. Replace the misleading
   `oid_eir_amortization` result with
   `effective_interest_adjustment` in native currency. Its
   period value is effective interest less ordinary contractual
   cash interest, contractual PIK and admitted later integral fee
   or cost cash, using their signed authorized amounts. Opening
   net fees are already in initial carry. The stated date-level
   allocation can be signed. A standalone OID-only allocation
   requires an independently qualified attribution. Retain
   `oid_eir_carrying_value` as a clearly labeled modeled carry
   series and `oid_eir_rate` as the original modeled return in
   its explicit basis. Validate requested values before reporting.
   [Checked monetary boundary](../core-conventions/012-money-decimal-precision.md),
   [effective-interest definition](https://www.ifrs.org/content/dam/ifrs/publications/pdf-standards/english/2024/issued/part-a/glossary.pdf).

9. **Share canonical preparation and preserve result bases.**
   Reconcile term and revolver direct APIs, discount-margin
   and price inverse, maturity and fixed-workout consumers,
   cost, rate and carrying schedule reporting, runtime wire
   state, Python/WASM and UI. Reuse the same prepared strip
   where the claim, cuts and quote law match. Record quote and
   market authority, compounding/clock, cash and denomination,
   original projection or realization basis, fit residual and
   currency. Do not publish stale prepared data across different
   cuts or metrics. Preserve verified zero, unavailable and failed
   input/valuation results. Records 07–12 own the product-specific
   schedule, model, rights, quote preparation and risk details.
   [Metric basis and availability](../valuations-framework/metric-orchestration.md),
   [term-loan risk and yields](09-term-loan-risk-yields.md),
   [revolver risk and quotes](12-revolver-quotes-risk.md).

## Observed behavior

### Choice and rationale

Term loans and revolvers share quote arithmetic in [loan_quotes.rs](../../../../finstack-quant/valuations/fixed-income/src/loan_quotes.rs). The implemented choices include zero-rate spread discounting on the configured quote compounding, annual-effective XIRR, an undiscounted annualized cash-cost measure, and carrying-value roll-forward at that XIRR. Sharing arithmetic prevents the two products from defining the same quote differently; this is an implementation rationale, not an approved accounting policy.

### Scope and input domain

Dated lender cashflows are positive receipts and negative funding. Callers decide the purchased balance, accrued interest, settlement date and which flows belong to the quote. Loan quote rates are decimals; fees and contractual spread inputs are basis points. `OidEirSpec.include_fees` defaults true in [loan_terms.rs](../../../../finstack-quant/valuations/fixed-income/src/loan_terms.rs). No impairment, expected-credit-loss staging, or accounting derecognition is calculated here.

### Algorithm

1. **Discount margin:** for each cash settlement after settlement, obtain `t` on the discount curve's day count and `DF(settlement,payment)`. The shared `z_spread_discount_factor` shifts the zero rate on the explicit quote compounding. The live helper also admits Continuous and Simple. Sum signed flows with Neumaier compensated summation. Zero margin preserves the original curve DF.
2. **Inverse margin:** solve `PV(dm) − target = 0` with fallible Brent, tolerance `1e-10`, initial bracket size `.05`, and the caller's initial guess. Reject a solved absolute margin greater than `.50`. This is a post-solve sanity check, not a ±50% search bracket.
3. **Yield:** prepend the negative purchased dirty value at settlement, retain later `is_cash_settlement()` flows, and call `xirr_on_basis` with the explicit yield basis. It solves on canonical ACT/365F and can restate the annual-effective rate on ACT/360.
4. **All-in rate:** sum future cash interest and fee kinds, excluding PIK and principal; divide by the integral of outstanding balance from `as_of` to maturity, using the stepwise outstanding path and contractual day count. Return zero when that balance-time denominator is nonpositive. This is neither IRR nor PV-weighted cost.
5. **OID/EIR:** aggregate same-date interest, permitted fees, amortization and notional funding/repayment; exclude PIK as a cash receipt. Add optional extra fees and opening funding. Solve XIRR for these signed date buckets. Set opening carrying value to minus the first bucket's net amount. For each later interval, `income = opening × ((1+eir)^year_fraction − 1)`, `closing = opening + income − net_cash`, and `OID_amortization = income − bucket_cash_interest_and_included_fees`.

### Selection, controls and conventions

Term-loan and revolver metrics choose their own input schedules and call these helpers. Optional fees alter both XIRR and amortization. Settlement advances a nonnegative business-day count using an explicit calendar, or weekdays when absent; zero lag returns `as_of` unchanged. Commitment steps use the last ordered step effective on/before the date. Absolute fees are used directly; proportional upfront fees multiply commitment.

### Limitations, failure and fallback

Fallible solver objectives propagate pricing errors. Core XIRR now requires exactly one sign reversal. The discount-margin helper still claims monotonicity without establishing that property for its signed input strip. Fee inclusion is broad: commitment, usage, facility, LC and fronting fees are treated as interest for the roll-forward when enabled. The carrying-value series is an arithmetic output, not evidence that every fee belongs in EIR under an accounting standard. Full origination cashflows are required; a seasoned balance alone is insufficient to reconstruct historical OID.

### Alternatives and tradeoffs

The shared discount helper currently admits continuous, periodic and simple factor transforms. Product quote adoption remains open. A cash-cost ratio is cheaper and interpretable over changing balances, while XIRR recognizes funding timing. Accounting-specific fee classification or credit-adjusted EIR would require explicit additional contracts; neither is inferred from these helpers.

### Evidence

The colocated `zero_margin_reproduces_curve_discounting_and_solve_recovers_it` test checks DF identity and inverse round-trip; `discount_margin_solve_propagates_the_pricing_error` checks failure preservation. These are internal consistency evidence, not independent market-price validation. [Brent](../../../REFERENCES.md#brent-1973) supplies the numerical reference.

## Independent arithmetic and qualification

- One receipt of `120`, with two source years, relative factor
  `.96` and quarterly compounding, gives `115.20` at zero DM
  and `109.62640436158165` at `250 bp`. Independent inversion
  recovers `.025`. For annual bases shifted from zero, the
  signed stream `+120,−180,+80` at one, two and three years
  prices at `20` with gross discount bases both `1` and `4`.
  For a `100` investment and `110` receipt 366 days later,
  ACT/365F yield is `9.9713586%` and ACT/360 is `9.8282634%`.
- On ACT/365F, funded balance `100` for 180 days and `110`
  for 185 days following a `10` PIK event gives exposure
  `7670/73` currency-years. Cash interest `7` and eligible
  fee `1.50` give running cost `8.089960887%`. An explicitly
  elected upfront fraction of `70 bp` on opening commitment
  `200` pays `1.40`.
- At par `100` with initial net cash funded `98` and receipt
  `106` after 365 days, ACT/365F EIR is `8.1632653061%`,
  effective income `8`, adjustment `2` relative to the
  cash coupon `6`, and terminal carry zero. A separate
  nonintegral service fee does not change this result. At
  par funding `100` with contractual PIK `10` and later
  receipt `110`, the EIR is `10%`, with total adjustment
  zero once PIK is included in the contractual-interest
  comparison. A stream `−100,+5,−20,+132` has three
  switches and fails the accepted ordinary XIRR admission.

Standalone exact and high-precision arithmetic checks cover
these cases. Qualification still requires loan settlement and fee
terms, buyer-right and accrued fixtures, delayed draws, source
context and quote-market vectors, origination snapshot and payoff
reconciliation, and runtime and consumer agreement. Repository
pricing and host suites were not run for this review.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
