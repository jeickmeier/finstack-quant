# VALUATIONS-STRUCTURED-CREDIT-19: Borrowing base and asset-backed facility synthesis

- **Decision status:** Accepted 2026-11-15 — complete facility, borrowing-base, draws, fees, reporting and risk package, including the previously accepted O6 quote compounding.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent concentration, draw, cure, fee and lender-return calculations checked. Production facility terms, pricing, risk and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for borrowing-base facilities and lender cashflows

1. **Use the shared waterfall with explicit lender and residual rights.**
   Represent a senior lender claim and a named residual cash entitlement.
   Separate closing amounts from current debt, commitment, accounts, funded
   history and pending actions at the authorized valuation cut. Admit zero
   drawn principal against a live commitment. Require qualified schedules,
   accrual, funding, coupon priority, loss treatment and residual terms.
   Remove compulsory positive synthetic equity inferred from collateral
   less drawn and collateral-type reinvestment defaults.
   [Accepted deal dispatch and terms](01-dispatch-and-assumptions.md#accepted-decision-for-structured-credit-dispatch-and-assumptions).

2. **Evaluate lender eligibility on dated live collateral.** Use the exact
   asset-class advance rule, then an expressly authored wildcard. Other
   assets are ineligible. Reject duplicate selectors. Apply each rule
   to the qualified current balance and valuation basis, maturity,
   default and workout status, delinquency and required group identities
   at the selected assessment phase. Missing required data errors unless
   the terms expressly allow exclusion before fixing the denominator.
   Preserve lender-rule and advance-rate provenance.
   [Filed borrowing values and advance rates](https://www.sec.gov/Archives/edgar/data/1377936/000121390020027101/exhibit_a.pdf).

3. **Elect the concentration and base composition.** For the supported
   ordered profile freeze total eligible value before concentration
   limits. Calculate each elected group cap on that denominator and
   reduce its remaining eligible amounts proportionally in the authored
   order. Record each stage's exclusions and apply advance rates to
   surviving amounts. Add only eligible cash and account credits and
   deduct prescribed reserves and exposures once. Report the denominator,
   order, adjustments and cut. Other concentration methods require
   qualified calculation and contract terms.
   [Filed base and concentration provisions](https://www.sec.gov/Archives/edgar/data/1983514/000119312525264711/d45172dex101.htm).

4. **Apply a common dated assessment to draws, cures and reports.**
   For borrowing base `B`, commitment `C` and covered exposure `D`
   on the same qualified basis, report drawable capacity
   `max(0,min(B,C)−D)`, base deficiency `max(0,D−B)` and commitment
   excess `max(0,D−C)` separately. Contractual funding conditions still
   apply. Post scheduled and discretionary advances on actual authorized
   dates, phases and priority. Each funded draw creates one lender
   outflow, one debt increase and its authorized borrower destination.
   Recalculate capacity before further draws. Exclude the draw's own
   proceeds as a source of its capacity. Overadvance requires an express
   bounded contractual exception. Recompute eligible assets and debt
   after an authorized cure and report remaining deficiency. Remove the
   absolute one-unit draw threshold.
   [Accepted coverage and cures](12-coverage-and-cures.md#accepted-decision-for-coverage-tests-and-cure-diversion) ·
   [Filed overadvance provisions](https://www.sec.gov/Archives/edgar/data/1549922/000154992221000010/smlp-ablcreditagreement202.htm).

5. **Elect reinvestment, acceleration and term-out separately.** Select
   no reinvestment or authorized replacement collateral and funding.
   Apply the stated deficiency cure and restrictions. Permanent
   acceleration requires its own trigger, effective phase and reason.
   Use qualified original par for cumulative loss and completed calendar
   months for the elected three-month excess-spread test. Reject
   silently ignored duplicate trigger roles. At the authorized cutoff
   stop discretionary draws and recycling and preserve binding actions
   and earned claims. Set term-out from its authored date or expressly
   elected permanent-acceleration anchor. Bind roll, repayment, actual
   sale proceeds and retained recoveries to their qualified terms.
   Remove automatic liquidation at par.
   [Accepted early amortization](13-principal-regimes-and-amortization.md#accepted-decision-for-principal-regimes-and-early-amortization) ·
   [Accepted terminal cash](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash).

6. **Accrue fees on actual eligible unused capacity.** Specify fee rate,
   usage definition, accrual clock, start, stop and payments. Split
   accrual at changes in drawn par, commitment, included LC usage or rate.
   A zero-drawn live commitment can incur fees. Recognize actual fee
   cash through the authorized waterfall source and priority or a
   qualified external payer. Track due, paid and unpaid amounts.
   [Filed daily unused-commitment fee, §2.07](https://www.sec.gov/Archives/edgar/data/1812554/000119312525127606/d908189dex101.htm).

7. **Use one signed lender ledger and qualified capital-return bases.**
   Carry entitled interest, fees, repayment and recovery cash, and
   funded draws and readvances as outflows. Keep redemption cash,
   retired par and noncash loss separate. Use one entitlement cut,
   including same-day treatment, for PV, exports and each matching metric.
   Lender IRR uses ACT/365F XIRR on positive current drawn capital as
   its synthetic opening investment. Residual IRR requires independent
   positive retained capital and matching signed residual cash. Apply
   the canonical unique-return gate. A purchase-price yield requires
   its own price basis.
   [Accepted XIRR](../core-conventions/017-irr.md).

8. **Keep independent compounding and qualified risk bases.** Use the
   required O6 `Compounding` field for fixed-strip spread pricing,
   supported inversion and spread risk. Apply the selected checked
   finite-horizon law to the entitled signed settlement strip. An
   inverse needs a qualified monotone bracket and separate native
   currency residual gate. Live rate risk reprices matching lender
   PV from actual active sources, including nested collateral roles.
   Named credit-quote CS01 requires qualified replayable inputs.
   Fixed-strip Z sensitivity requires its own election. Report live
   base amount, percent cushion and decimal utilization on the same
   assessment. Zero base is valid, with base-denominated ratios
   unavailable. Reconcile amounts, units and failures in Rust, the
   registry, Python, WASM and stored results.
   [Accepted independent quote compounding](#accepted-decision-required-quote-compounding).

## Independent calculation checks

- Test assets have eligible amounts 60, 30 and 10 and advance rates
  90%, 80% and 50%. The first two share an obligor and the first and
  third share an industry. With a 60% obligor limit and 50% industry
  limit, obligor-first leaves 40, 20 and 10 for an asset base of 57.
  Industry-first leaves `600/17`, `420/17` and `50/7` for a base
  of approximately 55.1008.
- Debt 65, commitment 90 and base 85 allow a scheduled draw of 10
  and only 10 of further readvance. In a separate cure, base 58 and
  debt 61 have a deficiency of 3. Repaying 3 from already counted
  cash leaves base 55 and debt 58, still deficient by 3. Qualified
  outside cash of 3 cures without reducing the base.
- A commitment of 100, debt 60 for 15 days and debt 80 for 15 days,
  at 120 bp ACT/360, gives fees of 0.03. Using only opening debt
  would give 0.04.
- Lender capital 60, an entitled same-day fee of 2, a draw of 20
  one year later and receipts of 88 at year two give XIRR 7.1358%.
  Dropping the same-day fee gives 5.5808%.

## Observed implementation and scope

An asset-backed facility is represented as a two-class structured-credit deal: a senior facility note and residual collateral equity. It therefore reuses the same funding, coverage and loss engine. Borrowing-base eligibility is distinct from market valuation and requires lender-specific adoption.

## Algorithm and conventions

Each asset receives an exact asset-class advance rule, else wildcard `*`, else ineligible. Eligibility filters include default state, maturity, delinquency days, unresolved NPL and other rule fields. Live evaluation uses current balances/status rather than unchanged original input rows. Eligible balances are grouped by obligor, industry or asset class for concentration caps measured against total eligible collateral before exclusions. Limits are applied sequentially; over-cap groups are proportionally reduced. Missing grouping metadata escapes that dimension's cap. Advance rates weight adjusted collateral; cash/account additions and debt determine borrowing-base availability/deficiency.

The synthetic facility note begins at drawn amount; residual is collateral balance minus drawn. A borrowing-base test at 1.0 after note interest directs deficiency cash to senior paydown and suspends reinvestment. Notes preserve par and permit principal to support senior interest. Statistical collateral reinvests until effective revolving end; instrument collateral amortizes from closing. Scheduled draws become note-funded cash, and optional readvance uses available borrowing base capped by commitment. Term-out creates a deal call at repayment date, optionally relative to early-amortization occurrence, using the configured liquidation price.

Commitment fee is `(commitment - opening note balance)+ × fee_bp/10000 × accrual` until commitment end, prorated for a partial last accrual and paid on note payment date. Lender cashflows sum interest, principal and fees, subtract future funded draws, and aggregate by date. Facility IRR adds `-drawn` at valuation date and solves Act/365F XIRR. It is a return on current drawn capital, not a mark-to-market purchase-price yield.

Facility reporting uses initial collateral borrowing base, percent cushion `(base - drawn)/base × 100`, and decimal utilization `drawn/base`; nonpositive base errors for the ratios. Residual IRR is the synthetic equity IRR at par. Registered DV01 uses the shared combined discount/forward bump, bucketed DV01 triangular key rates, and CS01 the shared hazard-when-credit-curve / otherwise z-spread machinery. Its z-spread cashflow adapter includes lender draws and commitment fees, uses payment-frequency compounding and includes dates on or after valuation. These delegated conventions differ from the continuous-spread structured-note helper.

## Domain, limitations and alternatives

Facility amounts, commitment, drawn/current collateral, currency, dates and event units validate; cumulative-loss events are fractions, concentration caps percentages and advance rates fractions. Statistical synthetic residual must be positive under construction. Commitment fees are appended to lender receipts rather than competing as a collateral waterfall claim; this assumes an external payer or requires a separate source-of-cash justification. Overlapping concentration dimensions are sequential haircuts and can be order-sensitive; missing metadata can overstate availability.

A direct standalone loan model would be a different methodology. Supported controls include rule inputs, no readvance, scheduled draws, term-out and liquidation price; no automatic lender credit policy is inferred.

## Implementation and validation evidence

- [asset_backed_facility/types.rs](../../../../finstack-quant/valuations/structured-credit/src/asset_backed_facility/types.rs)
- [asset_backed_facility/pricing.rs](../../../../finstack-quant/valuations/structured-credit/src/asset_backed_facility/pricing.rs)
- [asset_backed_facility/metrics.rs](../../../../finstack-quant/valuations/structured-credit/src/asset_backed_facility/metrics.rs)
- [structured_credit/types/borrowing_base.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/borrowing_base.rs)
- [structured_credit/types/draws.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/draws.rs)

Existing evidence: [asset_backed_facility/event_unit_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/asset_backed_facility/event_unit_tests.rs); [structured_credit/unit/funding_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/funding_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

The inspected event tests verify units and rejected retired keys, not an independent borrowing-base/legal facility reference.

## Accepted decision: required quote compounding

Accepted on **2026-11-12** as owner resolution O6 of the core-conventions
reconciliation, within the accepted
[CORE-CONVENTIONS-007 #9](../core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions)
separation of payment cadence from quote compounding. Runtime
implementation is tracked in the register.

The asset-backed (borrowing-base) facility carries a required, independent `Compounding` quote
convention. Spread pricing, quote inversion and spread sensitivities (CS01)
all read that one field. It is never derived from the payment frequency or
any other tenor, and it has no default: a convention preset may state it
explicitly, but no value is computed from the preset's frequency.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
