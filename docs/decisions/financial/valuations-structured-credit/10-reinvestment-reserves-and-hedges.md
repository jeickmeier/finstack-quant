# VALUATIONS-STRUCTURED-CREDIT-10: Reinvestment, reserves and hedge cash accounts

- **Decision status:** Accepted 2026-11-15 — complete reinvestment, reserve uses and earnings, accumulation, dated hedge cash and conservation package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent purchase, account conservation, hedge timing and ranked cash calculations checked. Production pricing, contract and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for reinvestment, reserves and hedge cash

1. **Determine the purchase budget at the authorized cut.** Apply due
   funding, realized losses and applicable pre-purchase triggers in
   contractual order. Earmark amounts due to named amortizing tranches
   once. Limit purchase cash to unspent eligible scheduled principal,
   prepayments and expressly eligible carried principal. Exclude
   recoveries in this profile. Give required pro forma purchase tests
   their own dated snapshot. Preserve binding purchase obligations
   and unspent cash under their governing terms.
   [Filed CLO investment criteria](https://www.sec.gov/Archives/edgar/data/1655050/000119312525156216/d68327dex101.htm).

2. **Buy an explicit replacement cohort.** Elect no reinvestment or
   a specified replacement cohort. Retire implicit survivor scaling.
   Require price, settlement date, stated maturity, coupon terms,
   credit assumptions and required eligibility data. Keep missing
   ratings missing and apply terminal rules to the stated maturity.
   A selected current-yield test uses annual coupon divided by the
   price fraction. Record cash spent and acquired par separately.
   Keep the accepted instrument-backed restriction on generic
   reinvestment.
   [Accepted typed collateral](08-instrument-collateral.md#accepted-decision-for-instrument-collateral-and-liquidity).

3. **Bind each reserve and spread account to authorized uses.** Keep
   draw liquidity, waterfall credit support, excess spread and principal
   funding as distinct uses. An account explicitly shared across uses
   has one balance and each amount can be spent once. Define targets
   from fixed cash, a fraction of qualified dated current or original
   par, or the maximum of selected targets. Include funding-account
   cash in the base only by election. Size support against actual
   eligible fees and interest and capture genuine residual interest.
   Apply replenishment, excess release and terminal principal support
   on their authorized dates and at their named tiers.
   [Filed reserve-account example](https://www.sec.gov/Archives/edgar/data/1414620/000095014407009284/g09856hze424b3.htm).

4. **Date reserve earnings, accumulation and release.** For an elected
   simple ACT/360 reserve, use the qualified opening balance or expressly
   dated slices. Route earnings once to the reserve, eligible collections
   or a named tranche. Restore accumulation balances, capture eligible
   principal up to the contractual cap and route overflow under the
   deal terms. Release the funding balance once at the bullet or
   effective acceleration. Require explicit terminal recipients
   and priority.
   [Accepted card accumulation](07-card-master-trust.md#accepted-decision-for-card-master-trust-allocation),
   [Terminal cash](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash).

5. **Carry contractual hedge cash on its date and at its rank.** Reuse
   the single-currency IRS coupon owner. Use contractual notional or
   an expressly elected pool or named-note balance at the accrual-
   effective phase. Preserve fixed accrual slices and scale only
   qualified coupon cash. Net only under an authorized agreement at
   the same date, currency and a compatible phase. Route the receipts
   and payables at their designated accounts and ranks. Keep authorized
   unpaid and qualified termination claims on their own dates.
   [Accepted IRS coupons](../valuations-rates/03-interest-rate-swaps.md),
   [Filed CLO hedge priorities](https://www.sec.gov/Archives/edgar/data/1379785/000137978519000025/exhbiti101-cloindenture.htm).

6. **Reconcile transfers in a checked Money ledger.** Debit and credit
   the same admitted amount. Reconcile each account and the waterfall.
   Record any authorized rounding residual and destination. An
   unexplained difference fails the check. Publish active dependencies
   and the same dated balances, cash, and recipients in the existing
   Rust-backed Python and WASM interfaces.

## Independent calculations

- Ordinary principal of 140, required amortization of 55 and restricted
  principal of 15 give 70 of eligible purchase cash. Recovery of 20 is
  separate. At 80% of par and a 6% coupon, current yield is 7.5% and
  acquired par is 87.50. Opening par of 1,000 becomes 947.50 after
  repayments and purchase. A 7.6% minimum current yield leaves the 70
  unspent.
- Opening credit reserve, spread and funding balances of 12, 4 and 15,
  plus 50 gross collection and 0.12 reserve earnings, give 81.12 total.
  With reserve target 6, authorized excess release of 6 to interest,
  20 collected interest and 2 spread support cover 28 eligible interest.
  Of 30 collected principal, capture 10 to reach funding cap 25
  and route 20 to principal. Closing accounts 6, 2 and 25, waterfall
  cash 28 interest and 20 principal, and 0.12 direct reserve income
  reconcile to 81.12.
- A hedge accrual snapshot of par 100 and signed cash factor 0.05
  gives cash 5. A later 40 draw leaves that slice unchanged.
  Eligible reserve of 3 against a payable of 5 leaves 2 unpaid at
  the due phase even when a receipt of 5 arrives later in the same
  distribution period.

## Observed implementation and scope

The engine treats account movements as period cash uses or sources.
Reinvestment changes future collateral, reserves provide liquidity and
hedge cash is projected from swap contracts. The following sections
describe observed behavior. The accepted contract above governs
reconciliation.

## Algorithm and controls

Period order is external note draws, collateral flows, realized loss/tests, reinvestment/account movements, and waterfall. Ordinary collateral principal can be reinvested during the active period; recoveries are excluded. Amounts needed for named amortizing tranches are reserved first. With no new-collateral assumptions, eligible surviving rows are scaled proportionally subject to price and coupon/price current-yield criteria. With assumptions, cash/price buys par of a new first-lien cohort; maturity is capped at legal final and spread/floor assumptions determine floating terms. The new cohort is unrated unless additional metadata is provided. Instrument collateral does not reinvest.

Reserve targets can be fixed money, fractions of current/original pool, or the maximum of two targets. Opening reserve accrues simple Act/360 interest when an eligible rate is configured, routed to reserve or collections. Reserve and excess-spread accounts can support leading fees and debt interest; replenishment sits in its configured waterfall tier. Excess reserve may release, and final principal support is controlled by the reserve rule. Controlled accumulation traps ordinary principal before its bullet date and releases at the bullet, while recoveries follow their separate path.

Hedge swaps project raw non-notional IRS flows over the deal interval. The hedge notional is fixed or scaled to opening collateral/note balance. Positive hedge cash augments collections; a payable becomes a senior or junior fee recipient. Thus hedge priority affects credit support; it is not merely added to final PV.

## Conventions, limits and alternatives

Cash and par are distinct; purchase prices are percentages, spreads/floors bp, rates decimals. Conservation checks permit residual max(1 currency unit, 1e-9 relative scale). That tolerance is a diagnostic threshold, not an authorized loss budget. Hedging is contractual; no automatic hedge optimization or counterparty-default model is fitted. Proportional survivor scaling differs from purchasing a new underwriting population, and current yield differs from yield-to-maturity.

Supported alternatives are no reinvestment, explicit new-cohort assumptions, and different reserve/hedge priorities. A loan-level replenishment portfolio or forward-market purchase model would require different inputs and validation.

## Implementation and validation evidence

- [structured_credit/pricing/simulation_engine/simulate_period/cash.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/cash.rs)
- [structured_credit/pricing/simulation_engine/reserve.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/reserve.rs)
- [structured_credit/pricing/simulation_engine/hedges.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/hedges.rs)
- [structured_credit/pricing/simulation_engine/conservation.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/conservation.rs)
- [structured_credit/types/hedges.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/hedges.rs)
- [structured_credit/types/pool.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pool.rs)
- [structured_credit/types/waterfall/rules.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/waterfall/rules.rs)

Existing evidence: [structured_credit/unit/reinvestment_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/reinvestment_tests.rs); [structured_credit/unit/reserve_rules_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/reserve_rules_tests.rs); [structured_credit/unit/hedge_swap_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/hedge_swap_tests.rs); [structured_credit/unit/funding_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/funding_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Cash conservation does not prove the assumed reinvestment prices or hedge economics.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
