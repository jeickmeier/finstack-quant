# VALUATIONS-OTHER-FIXED-INCOME-07: Term-loan funding, amortization, covenants and fee compilation

- **Decision status:** Accepted 2026-11-14 — full checked term-loan cashflow compilation package.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Reconciliation pending. The observed entry points and kernels are described below.
- **Validation status:** Live sources and primary contractual examples inspected. Independent funding, balance, coupon and fee arithmetic checked. Contract, runtime and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked term-loan cashflow compilation

1. **Bind the term-loan terms, schedule and covered state explicitly.**
   Use one Rust term-loan compiler over the shared cashflow
   programs. Bind lender currency, actual issue and maturity,
   dated opening balance and complete relevant history, and
   recognized actual or explicitly projected events at the stated
   information cut. Track original loan terms independently of
   the opening snapshot. Select the coupon grid, frequency,
   reference periods, day count, stub, roll/EOM and actual payment
   convention with covered calendar and explicit payment lag,
   including zero. This supported profile uses unadjusted accrual
   boundaries. Payment adjustment does not shift exposure,
   covenant, observation or principal dates. Support weekend-only
   only when explicitly selected. An incompatible required date or
   missing original context errors.

   Carry the actual accrual and cash dates, contractual parent
   identity, observed/projected status, and signed exact
   same-currency Money across the resolved schedule. Report
   funding as a lender outflow and repayment as a receipt. Each
   explicit principal event has its requested economic date,
   payment date, signed par change and separately signed cash.
   Cash-only and exposure changes retain their own phases.
   [Contractual periods and context](../cashflows/01-schedule-periods.md),
   [atomic principal and cash phases](../cashflows/03-principal-and-emission-order.md),
   [accepted loan quote and origination basis](06-loan-quotes-eir.md#accepted-decision-for-checked-loan-quotes-cash-cost-and-effective-interest).

2. **Admit ordinary funding or a nonrevolving delayed-draw path.**
   The ordinary profile has one authorized origination draw at
   the stated gross funded par, with its cash and effective date
   independently bound. For a DDTL, use one opening gross
   authorization. An explicitly distinct cap must have its own
   meaning and be enforced. Do not let a separate limit silently
   change or weaken the elected commitment. Require positive,
   identified draws and an explicit availability start and
   expiration phase. Commitment steps are nonincreasing,
   unambiguous and nonnegative, with no reduction fee in this
   profile. Resolve notices, draw-stop and expiration fact/cut
   before testing eligible prospective draws against current
   capacity. A qualified stop removes funding eligibility
   from its stated phase. Exclude expressly canceled prospective
   events from cash and limit consumption after validating their
   structural data. Required active events outside the permitted
   window, unresolved conflicts with settled history, and
   over-limit live draws error. A later decision does not
   retroactively cancel a settled draw.

   Count cumulative gross admitted funding once, independent
   of repayments and PIK. On this nonrevolving profile
   repayments do not restore legal draw capacity. Reductions
   apply prospectively and can cancel remaining availability
   when the terms expressly provide for it. They do not rewrite
   past consumed amounts or create automatic principal
   repayments. If a reduction purports to revoke already
   consumed authorization, it needs separately qualified terms.
   Require an authoritative phase where a same-day funding and
   cap change affect admission.
   [Shared commitment authority](06-loan-quotes-eir.md),
   [filed delayed-draw commitment and loan terms](https://www.sec.gov/Archives/edgar/data/318833/000119312525053470/d935596dex101.htm).

3. **Apply OID once, without changing gross funded principal.**
   Require an explicit no-OID, withheld or separately paid
   policy. A basis-point policy uses the authorized nonnegative
   decimal bp fraction of each eligible gross draw. A fixed
   facility amount is allocated only across the complete
   expressly selected eligible draw scenario after cancellations,
   proportional to gross draw par. Require the applicable amount
   to be no greater than the eligible gross face and preserve
   exact aggregate cash under the elected Money rounding. Assign
   any last-unit remainder under a deterministic retained draw
   order with checked per-draw bounds, or report unavailable.
   Keep settled per-draw OID fixed. A changed future draw set
   can allocate only an expressly authorized remaining amount
   across the still-eligible draws. The absence of eligible
   draws cannot absorb nonzero OID.

   Withheld OID means funding cash `−(gross_draw−OID)` and
   principal delta `+gross_draw`. Separately paid OID means
   gross lender funding plus one separately identified fee
   receipt on its authorized payment date. A paid fee and
   withholding cannot both charge the same amount. Preserve
   exact lender cash, rate/amount provenance and product rounding.
   EIR classification follows record 06.
   [Checked Money](../core-conventions/012-money-decimal-precision.md),
   [filed alternatives for paying OID](https://www.sec.gov/Archives/edgar/data/1458891/000119312514275170/d761397dex42.htm).

4. **Use literal approved principal changes and elected installments.**
   Compile exact custom principal and authorized cash sweeps on
   the correct economic and settlement dates. Resolve the
   sweep amount and prepayment waterfall from the governing
   terms before compilation. Keep repaid par, actual settlement
   amount and any premium or fee identifiable. Exact principal
   instructions must fit the admitted live balance and phase,
   with errors for over-repayment. Remove the preliminary
   funded-only cap and the tolerance used to rewrite repayments.
   Percentage-only sweep instructions need a separately
   established calculation and amount.

   Support explicit None and the qualified product amortization
   rules. Original-base installments on ordinary term loans
   use the authored original funded base without recasting it
   after PIK or repayment. For the admitted DDTL cumulative
   funding profile, compute a Decimal fraction of gross
   admitted draws strictly before the installment accrual-end
   boundary. This cutoff is a choice for that contractual
   profile. A different cutoff or an earlier prepayment recast
   needs its own terms. Apply the named original-base
   installment as `min(requested_amount, live_balance)` at
   the amortization phase. Use that cap only for the
   original-base formula. Literal cash instructions stay exact. Remaining-base
   installments and LinearBetween use the accepted live
   post-event balance laws. LinearBetween divides live balance
   by remaining selected installments including the current
   date and clears it at the last selected boundary.
   CustomPrincipal uses literal dated par reductions. Keep
   LinearTo and StepRemaining outside this product profile.

   Close ending coupon first, apply admitted same-date PIK,
   post the authorized atomic principal batch, then scheduled
   amortization, fee evaluation and terminal redemption under
   the shared phase order. Retain actual authorized installment
   and settlement dates. Use the jointly resolved terminal
   economic and payment date pair and retire exact residual
   at par once for the ordinary maturity profile. Nonpar,
   deferred or waived claims use the actual qualified cash and
   stock terms. No extra automatic funding for an already
   funded opening and no hidden write-off.
   [Shared principal order and terminal pair](../cashflows/03-principal-and-emission-order.md),
   [shared amortization](../cashflows/04-amortization.md),
   [filed cumulative draw installments and prepayment terms](https://www.sec.gov/Archives/edgar/data/318833/000119312525053470/d935596dex101.htm),
   [filed sweep waterfall](https://www.sec.gov/Archives/edgar/data/1312206/000114420408045487/v122537_ex10-25.htm).

5. **Apply covenant rate changes to their qualified coupon parents.**
   Bind each change to a recognized dated effective event and
   declared margin/rate authority. Preserve checked Decimal
   fractional bp. Under the supported whole-parent profile,
   use the original coupon parent start for both fixed and
   floating rate selection. Require a supplied effective parent
   start or an explicitly elected rule to resolve it from the
   trigger, such as the next eligible original parent. Combine
   simultaneous additive changes only under a qualified
   commutative law, retaining their identities. Reject
   contradictory events, undisclosed grid snapping, and an
   unmatched required rate change. A contract that changes a
   rate inside a parent must supply its authorized independently
   valid coupon windows or other separately qualified payoff.
   Payment-split and principal timestamps do not elect a new
   rate-effective date. An actual true-up is dated cash on
   its own authority and never rewrites settled history.

   Retain existing fixed or term/overnight floating choices
   with their exact observation, fixing, fallback, calendar,
   rate controls, spread, reset and compounded-growth laws
   from the shared owners. Resolve required historical fixings
   before preparing dependent coupon, PIK, principal or fee
   amounts. Use the selected active source and preserve the
   original parent grid. Intra-parent options require their own
   qualified law, including the original fixing where applicable.
   Nominal model
   fixing restatement and valuation timing remain with record 08.
   [Coupon program windows](../cashflows/02-program-compilation.md),
   [term rate projection](../cashflows/06-floating-rate-projection.md),
   [overnight compounding](../cashflows/07-overnight-compounding.md),
   [filed next-interest-period timing](https://www.sec.gov/Archives/edgar/data/1101215/000110121514000111/exhibit_10-5.htm),
   [filed rate reset and true-up terms](https://www.sec.gov/Archives/edgar/data/1995807/000119312524151957/d752700dex107.htm).

6. **Fix cash/PIK composition on authorized coupon parents.**
   Require explicit base Cash, Pik or exact Decimal Split,
   and each effective election on the authorized original
   parent. Compile a disjoint payment-mode program through
   terminal coverage. A later return to cash requires an
   actual election or prequalified contractual reversion.
   An off-parent election needs independently valid authored
   payment parents. Resolve any premium as a separately
   authorized rate event. Under the supported profile, price
   one original parent amount and apply the one cash share,
   allocating the remaining exact amount to PIK. Cash and PIK
   sum to the admitted whole coupon. The PIK row settles zero
   cash and capitalizes at the actual accrual end before that
   date's principal and amortization phases. Distinct
   simultaneous cash and PIK rates, or capitalization at
   a different date, require their own qualified profile.
   [Cash allocation and PIK capitalization](../cashflows/05-coupon-cash-and-pik.md),
   [filed PIK election and associated rate premium](https://www.sec.gov/Archives/edgar/data/1760173/000119312519139774/d632434dex102.htm).

7. **Calculate each dated fee on its own complete parent and exposure.**
   Bind the explicit lender payee, annual Decimal bp, basis,
   eligibility window, parent grid, complete day-count context,
   due date and phase. The upfront fee uses an explicit
   same-currency amount or a checked fraction of original opening
   commitment under record 06. Commitment-fee accrual and
   payment require their own authorized cutoffs. A draw stop
   or commitment expiry ends fees only where the governing
   terms say so. An authored same-coupon payment rule is
   eligible. Keep fees payable on other contractual dates.

   Split each running fee parent at every relevant actual fee
   rate, eligibility, commitment and economic principal
   boundary. Use the established post-phase state on the
   following half-open interval, with original-parent prefix
   differences to measure each slice. Usage fees apply to
   the expressly eligible funded exposure. Elect commitment
   less cumulative admitted gross funding or commitment less
   admitted outstanding, with the selected contract's
   nonnegative fee base `max(commitment−base,0)`. Define
   whether PIK enters eligible outstanding. A sweep or
   capitalization can change this fee even off the coupon
   grid. An outstanding-based fee cannot replenish legal
   draw capacity. Compute `rate × eligible_base × fraction`
   over all slices, restore the checked amount once on the
   fee parent, and preserve parent identity even when fees
   share cash dates. No implicit zero for absent required
   balance or parent coverage.
   [Accepted fee calculation](../cashflows/09-fees.md),
   [filed daily ticking fee and independent payment cutoff](https://www.sec.gov/Archives/edgar/data/1177394/000119312518241316/d593721dex101.htm).

8. **Keep loan consequences, publication and callers auditable.**
   An executable covenant consequence needs a qualified event,
   authority and effective phase. A percentage-only sweep or
   collateral demand cannot create cash or exposure. A
   distribution block may mark state without itself changing
   cash. Realized default requires typed principal, recovery,
   entitlement and dated payment economics from the defaults
   owner. A qualified maturity change revalidates the complete
   affected schedule atomically. Reject failed or ambiguous
   mutations. Keep covenant episode selection and engine-wide
   consequences with their owner. A timeless change to the
   base rate must not stand in for a dated loan consequence.

   Share this checked schedule across native/registry pricing,
   raw and entitled cash views, carrying and quote preparation,
   balance reporting, and typed Python/WASM and UI exports.
   Preserve cash versus principal, recognized and projected state,
   fee purpose and event identities. Bind only active inputs and
   propagate unresolved data and failed qualification. Reconcile
   wire and generated schemas, statements consumers, parity
   and affected fixtures in the implementation slice.
   [Default and recovery assumptions](../cashflows/11-default-and-recovery-assumptions.md),
   [covenant consequence execution](../covenants/06-consequence-execution.md),
   [schedule state and cash views](../cashflows/12-schedule-lifecycle.md),
   [term-loan pricing](08-term-loan-pricing.md).

## Observed behavior

### Choice and rationale

The loan compiles typed events into the shared cashflow builder rather than implementing a second coupon engine. [cashflows.rs](../../../../finstack-quant/valuations/fixed-income/src/term_loan/cashflows.rs), [spec.rs](../../../../finstack-quant/valuations/fixed-income/src/term_loan/spec.rs) and [types.rs](../../../../finstack-quant/valuations/fixed-income/src/term_loan/types.rs) own the product-specific event ordering and domain restrictions. Economic treatment of these events awaits adoption.

### Scope and algorithm

1. Non-DDTL loans fund the limit at issue. A delayed-draw loan emits face increases only for draws within availability and strictly before the earliest draw-stop date. Funding cash can differ from principal: basis-point OID is applied per draw; absolute OID is allocated by each valid draw's share of total valid draws. Withheld OID reduces lender funding; separate OID produces positive fee receipts.
2. Upfront fees are based on DDTL commitment when present, otherwise the limit. Cash sweeps and custom principal create amortization events. Percent-of-original amortization uses cumulative draws strictly before each coupon accrual end; percent-of-remaining and linear-between pass to the shared builder. `LinearTo` and `StepRemaining` are rejected.
3. Sort principal events by date and cap each repayment to the running nonnegative funded balance. Excess requested amortization is reduced, not rejected at emission. The schedule starts from zero and replays these funding/principal events; maturity redemption comes from the builder.
4. Fixed-rate covenant margin changes accumulate in Decimal and use the step-up coupon path. Floating spread changes are snapped forward to the first accrual start on/after the event, combined on that boundary, and applied through the floating-margin program. The floating index constraints, gearing, reset lag/tenor and compounding remain contractual inputs.
5. PIK toggles produce a payment-split program. Cash and PIK composition affects outstanding principal through the common builder; PIK is not a cash coupon paid twice.
6. Usage fees use time-weighted drawn balance. Commitment fees stop at availability end or the earlier draw-stop. The integration grid includes coupon accrual boundaries, commitment changes and draw dates. Each interval charges `fee_rate × base × accrual_fraction`; the base is either commitment minus cumulative funding, or commitment minus outstanding. That grid omits some economic outstanding changes, including off-grid repayment and capitalization events. Amounts aggregate to the enclosing adjusted coupon payment date.

### Selection and conventions

The direct loan cashflow/export path calls this compiler; pricing adds the fixing restatement in [record 08](08-term-loan-pricing.md). Dates use unadjusted accruals, contractual frequency/day count/stub, adjusted payment dates, no payment lag and no end-of-month roll. Calendar defaults to weekends-only. All funding and repayments use loan currency; rates are decimals, margin/fee steps basis points. DDTL commitment reductions cannot increase the limit and cannot carry reduction fees in this product.

### Limitations and failure behavior

Validation requires positive finite limit, ordered dates, in-life events, matching currencies and valid splits. Cumulative draws are checked against commitment in force. Cash sweep and amortization amounts can be capped by available funded balance. Fixed rates are selected at the actual period start and floating steps are pre-snapped to the base accrual grid. They generally defer off-cycle changes on that grid. A payment-program cut can change the fixed period starts without changing the floating snap grid. `set_default_status(true)` rejects without explicit recovery economics; percentage cash sweeps and collateral requirements reject without typed economics. The live covenant `increase_rate` preserves fractional basis points through checked Decimal arithmetic, but the generic action mutates the rate without recording a dated coupon consequence. Those are concrete semantics, not blanket support for arbitrary covenant consequences.

### Alternatives and tradeoffs

Typed dated events make changes auditable and composable. Modeling draw stops as probabilities, optimizing borrower funding, or inferring cash sweeps from financial statements would introduce different economic models and are not performed here. The two commitment-fee bases intentionally distinguish nonrevolving availability from reusable paid-down principal.

### Evidence

[Cashflow tests](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/cashflows.rs), [finance regressions](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/finance_regressions.rs), [overnight-floor tests](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/overnight_floor.rs) and colocated validation tests cover event and convention examples. No external loan-servicing tape was reconciled. [ISDA day-count reference](../../../REFERENCES.md#isda-2006-definitions) supplies context, not adoption of these covenant semantics.

## Independent arithmetic and qualification

- Gross draws `30/60/30` with fixed OID `6` allocate
  `1.5/3/1.5`. Withheld funding is `−28.5/−57/−28.5`
  and gross par is unchanged. A cancellation of the third
  draw before any settlement on a newly authorized `30/60`
  scenario gives `2/4`. If first-draw OID `1.5` was
  already settled, it remains fixed and the remaining
  `4.5` can be assigned to draw two only with authority.
  A commitment of `100` with cumulative draws `70`
  still admits only `30` of further gross draws after a
  repayment of `50`.
- With opening par `100`, same-day PIK `5` and
  authorized repayment `104`, the principal closes at
  `1`. A funded-only cap would reduce the cash receipt
  to `100`. A whole coupon `40` at cash share `3/8`
  pays cash `15` and capitalizes `25`.
- On `1000` at `4%` ACT/360, an explicitly elected
  next-parent change of `12.5 bp` triggered February 1,
  2025 leaves Jan–Apr cash coupon `10`. The Apr–Jul
  coupon is `1001/96` (about `10.42708333`). If the
  contract changed the rate immediately on
  February 1 and qualified that split, the Jan–Apr
  amount would be `2939/288` (about `10.20486111`).
  An eligible observed `2.5%` annual coupon on `1000`
  paid entirely in PIK gives principal `1025` and the
  subsequent annual `4%` coupon is `41.00`. Using a
  stale `3%` projection yields `1030` and `41.20`.
- For a 90-day ACT/360 fee parent, commitment `1000`,
  opening drawn `600`, day-15 draw `100`, day-30
  commitment reduction to `950`, day-45 sweep `200`
  and day-65 fee-eligible PIK `50`, 100 bp on undrawn
  gross capacity pays `17/24`. The outstanding-based
  alternative pays `133/144`. A 50 bp usage fee on
  that funded exposure pays `215/288`.

Standalone exact arithmetic checks cover these cases.
Qualification still requires independent loan-servicing and
agreement fixtures, effective phases and exact fee dates,
settled OID and seasoned state, nominal period/day-count
cases, coupon and fee source coverage, and runtime and host
agreement. Repository pricing and host suites were not run.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
