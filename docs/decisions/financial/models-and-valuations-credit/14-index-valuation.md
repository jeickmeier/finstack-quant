# MODELS-AND-VALUATIONS-CREDIT-14: Index constituent aggregation and informational cashflow projection

- Decision status: **Accepted — complete checked CDS index valuation and cashflows package, 2026-11-13**.
- Implementation status: Current behavior observed below. The accepted contract requires Rust, reporting, market-state and host reconciliation.
- Validation status: Source, selected tests and primary references inspected. Independent allocation, quote and cashflow examples evaluated. Production and host qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked CDS index valuation and cashflows

1. **Require explicit series, trade terms and pricing mode.** Retain
   `HazardRate` for an untranched fixed-coupon index. Require the exact
   index family, series and version, positive original trade notional
   in one currency, side, signed fixed coupon in bp and resolved
   [CREDIT-12 CDS terms](12-cds-legs.md#accepted-decision-for-checked-cds-legs-and-quote-basis).
   Carry the authored schedule, first covered day, unadjusted maturity,
   documentation clause, calendars and explicit default settlement.
   Value at the close using the same final-day coverage and same-day cash
   choice as single-name CDS. An explicit contemporaneous settlement
   keeps that meaning. A regional preset can supply selected contract
   data through the existing materializer. Retain its resolved inputs
   and provenance. Family names alone supply no count, factor, mode or
   calendar fallback. Select `SingleCurve` or `Constituents` explicitly.

2. **Resolve current protection notional once, from the selected mode.**
   In `SingleCurve`, require a dated, authoritative surviving factor
   `f∈[0,1]` with applicable series/version provenance, one index
   hazard source and one explicit future recovery. Live notional is
   `N_orig f`. An authoritative original count, if supplied, serves
   only the explicit equal-weight gross default proxy in
   [CREDIT-13](13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk).
   In `Constituents`, require the complete original named roster with
   exact IDs, original weights, explicit future recoveries and member
   curve bindings. Reuse [CORE-MARKET-19](../core-market/credit-index-composition.md)
   validation, canonical member order and compensated original weight
   mass within `2^-52` of one. The complete as-of state identifies
   whether each name is live or has exited through a resolved credit event.
   A live name with positive weight receives `N_i=N_orig w_i`.
   Require a resolved, valuation-close snapshot. Reject a dated state
   that does not establish exposure at the requested close.
   Derived surviving weight is the sum of the original live weights
   at that close. Keep it as a diagnostic within the original
   weight representation budget. The named route takes no independent
   factor or count. Keep zero-weight identities without assigning them
   exposure. Remove survivor renormalization and the permissive named
   factor adjustment. Distinct members can share a qualified hazard
   binding. A shared source is still separate from member identity
   and contractual recovery. [White, sections 5.1–5.4](https://quant.opengamma.io/CDS-Options-OpenGamma.pdf)

3. **Use resolved as-of exposure and cash state for credit events.**
   Freeze the relevant series/version, effective exposure as of
   valuation close and the status of confirmed event cash. A published
   version or factor can lag the state that applies to a given trade.
   In named mode an exited member is outside future contingent pricing,
   with exact member and event identity and effective exit no later than
   valuation close. In single-curve mode an authoritative aggregate
   exposure snapshot can stand without a named roster. In either mode
   require complete eligible unpaid event cash, or explicit confirmation
   that none remains. Use the confirmed due date and actual signed
   buyer-axis amount for each event, with protection and accrued components
   when supplied or an explicitly identified confirmed net amount.
   Include components or net once for the same obligation. Validate
   event identity, currency, dates, status, duplicate cash and exact
   member binding where named. Preserve settled cash as history only.
   An unresolved amount or due date, an overdue unsettled claim, or a
   pending event with unresolved exposure or entitlement fails this
   valuation. Use the resolved contractual state supplied by the
   event owner. Model recovery, the current factor and the modeled
   default lag supply no actual event cash. Legal event determination,
   auction, reversion and backstop adjudication stay outside this
   valuation. [S&P DJI, index credit-event example](https://www.spglobal.com/spdji/en/documents/campaigns/cds-indices-primer.pdf#page=18)

4. **Keep already-earned coupons and other eligible cash after
   protection ends.** A coupon is earned at the close of its final
   accrual day under the accepted CDS clock. Retain unpaid earned
   cash through its contractual payment date, on its actual entitlement
   notional, even if a member has exited, the live factor is zero or
   valuation has reached maturity. Resolve each eligible completed
   coupon once from authoritative entitlement state. Where unchanged
   exposure throughout the relevant entitlement interval is established,
   use the contract coupon and that known notional. Otherwise require the
   resolved earned amount or entitlement notional and payment status.
   Missing entitlement information is an error when exposure may have
   changed. Known earned cash uses survival one and only needs its
   dated discount factor. Apply the same explicit valuation-date
   eligibility to earned coupons, actual trade upfront and confirmed
   event cash. Earlier settled payments are excluded. Include eligible
   same-day amounts at factor one. Validate authored cash before
   filtering. An overdue unpaid coupon or upfront requires a separate
   resolved claim and is outside the forward cash cutoff. Apply the
   actual upfront once at index level, in its
   original signed amount and currency, regardless of current factor.
   Coupon and event dates determine the remaining cash independently of
   protection expiry.

5. **Aggregate raw analytic legs on their actual exposures.** Use
   CREDIT-12's checked conditional survival, covered AoD, default
   settlement, relative discount and window moments. A common index
   curve supplies the single-curve leg. Named mode sums only resolved
   live member legs. Require exact contract/source recovery consistency
   for those priced CDS slices under CREDIT-12. With `N_i` the effective
   live notionals, sum `P=Σ N_i P_i`, scheduled future unearned
   annuity `A_s=Σ N_i A_si` and `A_d=Σ N_i A_di` for AoD.
   Define `A_dirty=A_s+A_d`. On the buyer axis, the residual dirty
   mark is `P−c A_dirty`, with `c=c_bp/10000`. Economic PV then
   includes eligible earned coupon cash, actual signed upfront and
   confirmed event cash, each once with its buyer cash sign. Premium
   paid by the buyer is negative cash. Reverse the final buyer sign
   once for the seller. Preserve individual signed amounts with a
   signed coupon. Use scaled checked products and deterministic
   compensated sums. Finalize Money at the reporting boundary.
   Exact zero live notional gives no contingent legs, while separately
   eligible fixed cash still values normally. At or after maturity
   close there is no covered default mass. No maturity-only early
   return on the complete PV.

6. **Quote the remaining index on a consistent clean or dirty basis.**
   Quote operations are for the residual running-coupon trade. Completed
   coupon entitlements are fixed existing-trade cash, alongside booked
   upfront and confirmed event settlement. They stay outside that quote
   and its coupon coefficient. For clean quotes require the selected
   settlement date and its relative factor `B` under CREDIT-12.
   Use the accepted accrued fraction `α_i` through valuation close
   in each current unearned period and compute
   `A_clean=A_dirty−B Σ N_i α_i`. Clean and dirty residual marks
   are `P−c A_basis`. Aggregate par is `10000 P/A_basis` in bp
   and coupon risky PV01 is `A_basis/10000` in currency per bp,
   with the selected basis recorded. A positive coupon move has
   the negative of that coefficient on the buyer axis. For par
   require remaining covered days, positive live exposure and a
   strictly positive selected annuity, without a small-value floor.
   A live zero-hazard case with a positive annuity can give zero par.
   The clean upfront amount at the quote settlement date is
   `(P−c A_clean)/B`; quote its fraction of current live notional
   and retain the normalization basis and original notional. The
   break-even actual settlement amount is `(P−c A_dirty)/B`.
   Require a live residual trade for the upfront quote. Historical
   cash, trade upfront and mark-to-market stay available through
   economic PV. Label the selected output as an index-curve
   model-implied quote or a constituent intrinsic quote. An
   observed index basis can remain between them. No mean of name
   spreads or automatic market-basis adjustment.
   [White, sections 5.1–5.5](https://quant.opengamma.io/CDS-Options-OpenGamma.pdf)

7. **Build one prospective cash report from the pricing geometry.**
   Retain the prospective cashflow view for cash ladders. Scheduled
   expected coupon cash uses the same accrual-end survival and actual
   payment dates as the analytic legs. Calculate expected protection
   and AoD from the same covered windows with discount set to one.
   Use the selected settlement date for a positive business-day lag.
   With contemporaneous settlement, label the expected amount by its
   named default day and retain that it represents cash spread within
   the day. Re-discounting that date-only cash is generally approximate.
   Keep eligible known earned, upfront and event cash identifiable
   as actual dated cash. Preserve separate protection, AoD and premium
   kinds, identity and timing provenance. Under
   [CORE-CONVENTIONS-015](../core-conventions/015-cashflow-semantics.md),
   protection uses `ProtectionPayment`; noncash notional reductions
   do not appear as settlement cash. Assemble the projected output
   once in the index owner under
   [CASHFLOWS-012](../cashflows/12-schedule-lifecycle.md).
   Remove the midpoint projection, half-period AoD proxy,
   payment-date survival, survival floor and implicit weekday lag.
   The priced leg breakdown comes directly from the analytic PV.
   Reject the generic hazard cashflow PV export for this projected
   route, which would apply credit weighting a second time.
   Forecasts are eligible for prospective reports only. Realized
   attribution consumes authenticated dated receipts or payments.
   [Cashflow export owner](../valuations-framework/cashflow-export.md)

8. **Share checked index state and outputs across the actual callers.**
   Freeze resolved contract, roster, effective exposures, cash state
   and the applicable clock and source bindings. Validate through
   one construction and restore path, with required choices and
   strict fields. Only the selected pricing mode supplies live
   credit dependencies. A common index hazard in named mode can
   be auxiliary metadata if explicitly retained. It supplies no
   missing member curve. Deduplicate identical sources without
   merging positions or repricing a zero-exposure member. Distinguish
   structural dependency declarations from actual date-eligible reads.
   An earned-only or depleted result can need discounting without
   requiring an active hazard query. Share the same dated allocations
   and raw marks with CREDIT-13, including through registry pricing.
   Quote, rate and recovery risk retain their accepted distinct
   replay bases. Reconcile Python, WASM, wire schemas, report
   and cashflow consumers and examples to the same Rust result.
   Expose actual mode and term provenance. The option, tranche,
   calibration and margin owners retain their model and fit decisions.
   Independent replay, cashflow and reference qualification remain
   implementation work.

## Observed behavior

### Choice and rationale

Index pricing delegates analytic CDS legs to one synthetic curve or active constituents. The public projected schedule deliberately uses a coarser midpoint approximation.

### Scope and input domain

SingleCurve or Constituents mode; constituent weights sum to one within 1e−6 and declared defaults constrain index factor to surviving original weight. No active names contributes zero.

### Algorithm

Synthetic notional=N×indexFactor. Constituent notional=N×factor×weight/activeWeight when defaults exist; curves and recovery are per name. Aggregate analytic CDS NPVs/legs; apply index upfront once. Par spread divides summed protection by summed notional annuities, not average spreads.
The informational schedule weights contractual coupons by conditional payment-date survival plus half period default mass; protection is LGD×conditional default mass at midpoint hazard-time default date, then weekday settlement delay. It uses payment dates as survival endpoints, not analytic accrual endpoints; schedules merge constituents and append upfront.
visit_positions exits at as-of≥premium maturity, suppressing unpaid rolled coupons unlike single-name pricing. Projection separately retains dates≥as-of, so boundary behavior differs from analytic coupon pricing.

### Selection and controls

The explicit pricing mode is stored. Preset and constituent helpers can select a mode implicitly. The synthetic CDS route fills some regional defaults, replaces an explicit zero default lag, and selects the default valuation bundle. Aggregate upfront is applied outside that synthetic CDS.

### Conventions

Original weights and index factor must not double-count defaults. Projected flow amounts have premium side sign and opposite protection sign.

### Limitations, failure and fallback

Projected schedule is informational and will not generally discount back to analytic PV; half-AoD, payment survival and weekday settlement differ from CDS kernels. Maturity early-exit loses live payment receivables.

### Alternatives and tradeoffs

Unified analytic expected cashflow rows would improve reconciliation but require timing semantics; constituent pricing is already supported instead of single index hazard.

## Evidence

Current sources include the [index pricer](../../../../finstack-quant/valuations/credit/src/cds_index/pricer.rs),
[contract and mode](../../../../finstack-quant/valuations/credit/src/cds_index/types.rs),
[series parameters](../../../../finstack-quant/valuations/credit/src/cds_index/parameters.rs),
[generic cashflow PV export](../../../../finstack-quant/valuations/core/src/instruments/cashflow_export.rs),
[portfolio cashflow ladders](../../../../finstack-quant/portfolio/src/cashflows.rs)
and [period-cash collection](../../../../finstack-quant/valuations/core/src/metrics/period_cash.rs).
The projected route currently classifies expected protection as
`DefaultedNotional`. This conflicts with its cash role and with
generic readers that exclude this kind or reject it when recovery is
also supplied. The generic hazard export uses the first credit curve
and can apply credit weighting again. Reconcile both at the existing
caller boundaries.

[Python](../../../../finstack-quant-py/src/bindings/valuations/typed_credit/cds_index.rs),
[WASM](../../../../finstack-quant-wasm/src/api/valuations/typed/credit.rs),
[valuation schemas](../../../../finstack-quant/valuations/src/schema.rs)
and the [index lesson](../../../../docs-site/content/learn/C/C4-index-credit-cdx-tranches-and-credit-index-options.mdx)
expose the current choices. The [index tests](../../../../finstack-quant/valuations/credit/tests/cds_index)
and inline pricer tests cover live weights, upfront signs and comparison
with production single-name valuation. The existing loose comparison of
projected cash to PV does not qualify either cashflow timing or exact
PV reconciliation. Runtime tests and host suites were not run.

### Independent review evidence

- Original notional of 10 million and original weights .50/.30/.20
  with the last name exited give surviving notionals of 5 million
  and 3 million and surviving weight .80. Applying .80 again
  would reduce the slices to 4 million and 2.4 million.
- On those live notionals, take per-unit protection PVs .06/.12,
  dirty annuities 4.5/3, quote accrued .10 and settlement factor .98.
  Dirty and clean par are 209.52381 bp and 214.87173 bp.
  The notional-weighted mean of the member dirty spreads is 233.33333 bp.
  At a 150 bp coupon, dirty and clean residual PV are 187,500 and
  199,260. An existing buyer upfront payment of 100,000 at that
  factor gives economic PV 89,500. The clean quote is 203,326.53
  at settlement, about 2.54158% of 8 million live notional.
  The break-even actual payment is 191,326.53 after the
  12,000 accrued rebate.
- A coupon earned on 10 million before a 20% exit, at 100 bp
  and 90/360 accrual, is 25,000. Using the current 8 million
  would reduce it to 20,000. The dated entitlement state
  determines which cash remains unpaid.
- In the [published Whiting event example](https://www.spglobal.com/spdji/en/documents/campaigns/cds-indices-primer.pdf#page=18),
  10 million original notional, a 1% member weight and 7% auction
  recovery yield 93,000 gross protection cash. The example's
  stated 11/360 accrual and 500 bp coupon yield 152.78 of
  premium rebate and 92,847.22 net after rounding. The surviving
  notional is 9.9 million. The example's accrued fraction is a
  supplied input here.
- With 10 million notional, .60 LGD, one covered day with .02
  integrated hazard and a .01 log discount drop across that day,
  expected protection cash is 118,807.96. Contemporaneous analytic
  PV is 118,217.87, while discounting the whole cash bucket at
  day close gives 117,625.80. If the day instead settles at one
  fixed lagged factor .98, the same expected cash gives 116,431.80.
- Exact rational and high-precision exponential checks establish these
  examples. Production aggregation, cash entitlement, host parity,
  dated curve behavior and external index fixtures remain open.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
