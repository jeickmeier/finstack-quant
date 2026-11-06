# VALUATIONS-EQUITY-AND-COMPOSITION-21: Levered property equity, debt payoff, coverage and financing risk

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-21`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

Levered property PV is property value less the value of explicit financing instruments. Levered performance instead assembles equity cashflows from acquisition, operations, financing cashflows and outstanding principal at sale. This separation allows debt market value and contractual payoff to differ.

## Scope and input domain

Property plus zero or more Bond, TermLoan, RevolvingCredit or Repo financings. Asset/financing PVs must match equity currency; underlying instruments validate their own conventions. Levered return/coverage metrics require supported financing cashflow schedules and property purchase/sale inputs.

## Algorithm

PV subtracts the sum of each financing instrument's native value from asset PV. Equity return flows start with negative purchase plus acquisition costs, add net property operations and negate financing cashflows within [as-of,exit], excluding PIK. At exit, scheduled Notional/Amortization flows are omitted and replaced by the absolute outstanding balance immediately before exit, then net asset sale proceeds are added. This preserves debt draws as positive equity funding and avoids counting scheduled exit principal twice.

Levered IRR uses property day count; equity multiple is positive dated net flows/absolute negative dated net flows. LTV is absolute financing PV/property PV, requiring positive property value. Origination LTV sums each financing's first negative Notional/RevolvingDraw cashflows on/after as-of and divides by purchase price.

DSCR uses each NOI interval `(previous_NOI_date,current_NOI_date]`, sums positive scheduled debt service and returns the minimum NOI/debt-service ratio. Exclude Notional, PIK, PrePayment, RevolvingDraw/Repayment; interest-only DSCR also excludes Amortization. Intervals without service are skipped; no qualifying interval errors. DebtPayoffAtExit reports the same pre-exit outstanding total.

## Selection, controls and conventions

Parallel/key-rate DV01 reprice financing dependencies. The custom property DiscountRate01 changes the asset scalar discount component while holding financing value fixed; CapRate01 changes only the embedded property capitalization. Native debt valuation dispatch is inherited from each financing instrument; no separate portfolio model override is supplied by this adapter.

## Limitations, failures and alternatives

Exit payoff is absolute outstanding principal, not an exercise/settlement quote with fees, accrued interest, breakage or prepayment penalty. LTV is debt market value based; OriginationLTV is based on remaining events on/after current as-of, not guaranteed historical origination. DSCR depends on NOI schedule granularity and cashflow-kind tagging. A marked debt buyback, contractual early repayment calculator or normalized annual coverage would be different methods.

## Evidence

The [property tests](../../../../finstack-quant/valuations/tests/instruments/equity/real_estate/pricing.rs) locate asset-minus-debt, metrics, exit sale date, balloon exclusion and equality of asset/levered custom DiscountRate01. These are accounting/behavior identities; no lender covenant, early-payoff or fee reconciliation has been established.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [levered.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/levered.rs)
- [levered_pricer.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/levered_pricer.rs)
- [levered.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/levered.rs)
- [mod.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/mod.rs)
- [sensitivities.rs](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/metrics/sensitivities.rs)
