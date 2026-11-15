# VALUATIONS-EQUITY-AND-COMPOSITION-21: Levered property equity, debt payoff, coverage and financing risk

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-21`.
- **Decision status:** Accepted 2026-11-14 — complete levered property value, payoff, returns, coverage and financing risk package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent examples cover payoff settlement, investor returns, LTV, coverage and isolated value risk. Production pricing, contractual payoff and host qualification remain pending.

## Accepted decision for checked levered property value and financing

1. **Use matched property and financing values.** Start with the
   selected [record 20 property basis](20-property-valuation.md#accepted-decision-for-checked-property-valuation-and-unlevered-returns).
   Subtract the sum of signed lender-perspective financing PVs
   using checked Money arithmetic. Align the native currency,
   valuation date, knowledge and entitlement cut, property
   interest and financing position quantities. Admit each bond,
   term loan, revolver or repo only on its qualified,
   distinctly identified property-borrower obligation and
   matching lender value. Use the financing owner's qualified
   native pricing route. Repo uses the cash lender's value of
   the matched borrower obligation. A negative lender mark
   increases equity. Negative equity is valid. With zero
   financing, equity value equals the property value.
   [Repo conventions](../valuations-rates/15-repo.md).

2. **Calculate returns from dated borrower cash and contractual
   payoff.** Include the actual acquisition and buyer costs,
   owner-entitled net operating cash and dated net sale cash
   from record 20. Add borrower advances and subtract borrower
   interest, fees and principal settlements on their actual
   or qualified projected dates. PIK and write-downs change
   principal state without creating settlement cash. Distinguish
   a principal reduction and its effective date from the cash
   obligation and its payment date. At sale, require complete
   dated contractual payoff from the financing owner or an
   authorized payoff quote for the exact termination and payment
   phase. Include unpaid or accrued interest, premium, breakage
   and fees where applicable. Admit par alone when the terms
   establish it as full settlement. Replace scheduled amounts
   only where the payoff covers those identified obligations.
   Retain earlier, separately settled and surviving obligations
   with their own dates. Use these same qualified payoff amounts
   for `DebtPayoffAtExit`. A missing payoff prevents
   payoff-dependent metrics and can coexist with a qualified PV.
   Use canonical ACT/365F XIRR with record 20's optional
   equal-growth ACT/360 quote. Divide positive dated net equity
   cash by absolute negative dated net equity cash for the equity
   multiple, including later equity injections.
   [Cashflow semantics](../core-conventions/015-cashflow-semantics.md).

3. **Define the two LTV bases explicitly.** Market LTV divides
   the sum of separately qualified nonnegative financing
   liability marks by positive matched property value.
   Undrawn commitments and financing assets have no offset
   role. Proven absence of liabilities gives zero. An
   inseparable net mark makes the ratio unavailable.
   Origination LTV divides documented original acquisition
   advances, including qualified acquisition-related delayed
   draws, by positive original purchase consideration.
   Exclude later redraws and refinancings from the historical
   calculation.

4. **Measure operating coverage on complete aligned periods.**
   For each explicit period `(a,b]`, match its signed NOI to
   cash interest, expressly included recurring service fees
   and mandatory regular principal due in that period.
   Apply the declared boundary phase and retain each period's
   own units. Interest-only DSCR removes regular principal
   and retains the included fees. Exclude draws, PIK,
   discretionary revolver repayments and voluntary
   prepayments. Identify maturity balloons and sale payoff
   separately, with an owner-qualified split for combined
   payments. Skip periods with confirmed zero service and
   return the minimum ratio over positive-service periods.
   Incomplete coverage or the absence of an eligible period
   makes the result unavailable. The financing owner retains
   covenant-specific calculation rules.
   [Fannie Mae multifamily DSCR guidance](https://mfguide.fanniemae.com/node/1766).

5. **Reprice each active risk on its own basis.** Financing
   parallel and bucketed DV01 hold the property mark fixed.
   Use exact eligible financing source identities, bump each
   once across its participating components and use matched
   two-sided reprices with the actual represented bump width.
   Report signed native-currency equity value change per
   positive basis point. Record 20 cap-rate or discount-rate
   risk holds financing marks fixed and moves the active
   property scalar. Proven zero exposure admits zero.
   An unsupported risk or failed required reprice is
   unavailable under its owning error contract.
   [Rate risk rules](../valuations-framework/finite-difference-rates.md).

6. **Preserve the export basis and canonical owner.** Investor
   cash exports preserve component and obligation identity,
   borrower signs, original cash dates and realized or projected
   status before same-date return netting. A valuation value
   bridge can report the standalone property and financing
   marks. A complete valuation cash export requires each
   selected component to reconcile to its PV. Rust owns the
   computation and errors. Existing Python and WASM routes
   carry the qualified result.
   [Valuation cashflow export](../valuations-framework/cashflow-export.md).

## Observed behavior

### Choice and rationale

Levered property PV is property value less the value of explicit financing instruments. Levered performance instead assembles equity cashflows from acquisition, operations, financing cashflows and outstanding principal at sale. This separation allows debt market value and contractual payoff to differ.

### Scope and input domain

Property plus zero or more Bond, TermLoan, RevolvingCredit or Repo financings. Asset/financing PVs must match equity currency; underlying instruments validate their own conventions. Levered return/coverage metrics require supported financing cashflow schedules and property purchase/sale inputs.

### Algorithm

PV subtracts the sum of each financing instrument's native value from asset PV. Equity return flows start with negative purchase plus acquisition costs, add net property operations and negate financing cashflows within [as-of,exit], excluding PIK. At exit, scheduled Notional/Amortization flows are omitted and replaced by the absolute outstanding balance immediately before exit, then net asset sale proceeds are added. This preserves debt draws as positive equity funding and avoids counting scheduled exit principal twice.

Levered IRR uses property day count; equity multiple is positive dated net flows/absolute negative dated net flows. LTV is absolute financing PV/property PV, requiring positive property value. Origination LTV sums each financing's first negative Notional/RevolvingDraw cashflows on/after as-of and divides by purchase price.

DSCR uses each NOI interval `(previous_NOI_date,current_NOI_date]`, sums positive scheduled debt service and returns the minimum NOI/debt-service ratio. Exclude Notional, PIK, PrePayment, RevolvingDraw/Repayment; interest-only DSCR also excludes Amortization. Intervals without service are skipped; no qualifying interval errors. DebtPayoffAtExit reports the same pre-exit outstanding total.

### Selection, controls and conventions

Parallel/key-rate DV01 reprice financing dependencies. The custom property DiscountRate01 changes the asset scalar discount component while holding financing value fixed; CapRate01 changes only the embedded property capitalization. Native debt valuation dispatch is inherited from each financing instrument; no separate portfolio model override is supplied by this adapter.

### Limitations, failures and alternatives

Exit payoff is absolute outstanding principal, not an exercise/settlement quote with fees, accrued interest, breakage or prepayment penalty. LTV is debt market value based; OriginationLTV is based on remaining events on/after current as-of, not guaranteed historical origination. DSCR depends on NOI schedule granularity and cashflow-kind tagging. A marked debt buyback, contractual early repayment calculator or normalized annual coverage would be different methods.

### Evidence

The [property tests](../../../../finstack-quant/valuations/tests/instruments/equity/real_estate/pricing.rs) locate asset-minus-debt, metrics, exit sale date, balloon exclusion and equality of asset/levered custom DiscountRate01. These are accounting/behavior identities; no lender covenant, early-payoff or fee reconciliation has been established.

## Independent numerical checks

- On 2025-01-01, purchase 1,000 plus buyer costs 20 and a
  funded advance of 600 require equity outlay 420. On
  2026-01-01, operating cash 80 plus net sale proceeds
  1,120 less a separately paid coupon of 30 give 1,170
  before remaining debt settlement. A separately paid
  principal installment of 50 plus residual payoff 566
  (principal 550, incremental accrued interest 4 and
  premium 12) gives the same 554 equity exit cash as
  payoff 616 covering the unsettled 50 installment.
  Both give ACT/365F XIRR .319047619048 and equity
  multiple 1.319047619048.
- Matched liabilities valued at 560 against a property
  value of 1,200 give market LTV 46.6666666667%.
  Original acquisition advances of 450 and 50 against
  purchase consideration 1,000 give origination LTV 50%.
  A later redraw of 100 is excluded.
- Period NOI of 30 and 24, interest of 8 and 10,
  included recurring fees of 1 and 1, and regular
  principal of 4 and 5 give minimum DSCR 1.5 and
  minimum interest-only DSCR `24/11`. A separate
  maturity balloon of 100 is excluded from both.
- Property flows 100 and 1,000 at times one and two
  discounted at annual effective .10 give property
  PV 917.355371900826. Financing flows 100 and 200 at
  the same times discounted on one .05 continuously
  compounded source give financing PV 276.090426057263.
  Equity PV is 641.264945843563. Centered one-bp bumps
  produce financing DV01 +.045705791224 and property
  discount-rate sensitivity −.158527425542 in native
  currency per positive bp with the other side held fixed.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [levered.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/levered.rs)
- [levered_pricer.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/levered_pricer.rs)
- [levered.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/levered.rs)
- [mod.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/mod.rs)
- [sensitivities.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/sensitivities.rs)
