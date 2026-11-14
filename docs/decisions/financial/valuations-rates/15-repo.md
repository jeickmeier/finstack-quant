# VALUATIONS-RATES-15-REPO: Repo lending cashflows, collateral and margin proxies

- ID: VALUATIONS-RATES-15-REPO
- Decision status: **Accepted — complete checked repo cashflows, collateral and risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, market, margin and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked repo cashflows, collateral and risk

1. **Price one fixed-rate repurchase agreement from the cash lender's
   perspective.** The collateral buyer lends cash at purchase and
   receives the repurchase cash at close. Require positive exact
   Money purchase cash `P` in one currency and a signed Decimal
   annual simple rate `r` equal to the final agreed rate. General
   collateral and special identify the collateral terms. They supply
   no rate adjustment in the product. If a qualified quote adapter
   accepts a reference rate and an explicit signed special basis
   in bp, resolve them once to the final contractual quote before
   construction. Remove the instrument-owned special-rate overlay.
   A consumer requesting a signed position applies its quantity
   once to the lender basis. The collateral seller is the cash
   borrower. Price the scheduled cash under contractual performance.
   [ICMA repo definition](https://www.icmagroup.org/market-practice-and-regulatory-policy/repo-and-collateral-markets/icma-ercc-publications/frequently-asked-questions-on-repo/1-what-is-a-repo)
   and [special collateral](https://www.icmagroup.org/market-practice-and-regulatory-policy/repo-and-collateral-markets/icma-ercc-publications/frequently-asked-questions-on-repo/9-what-is-a-special-in-the-repo-market/).

2. **Resolve actual purchase and repurchase dates once.** Require
   complete authored dates or materialization from an explicitly
   selected dated convention. Use the exact settlement calendar
   and declared adjustment policy. A date already agreed as actual
   is validated on that basis. A nominal date requires the elected
   adjustment. Reject date collapse, missing or incompatible
   calendar and convention data, and nonpositive or unavailable
   day-count fractions. Retain the original intent, effective
   `S<E`, and contractual day-count context. Fixed-term funding
   can start in the future. Overnight means the next eligible
   business day from the resolved purchase date. Apply the elected
   accrual clock across the actual interval, including intervening
   nonbusiness days where it counts them. Remove the currency-inferred
   USD defaults and the implicit business-day rule. Remove the
   open constructor that treats an initial or illustrative date as
   maturity. A formerly open contract can be prepared as fixed only
   after confirmed termination fixes its actual dates and complete
   cash terms within this single-rate law. Unresolved open, evergreen,
   floating and optional terms require a separate owner.
   [Dated convention selection](../valuations-framework/convention-selection.md)
   and [ICMA ERCC Guide, sections 2.18 and 2.20–2.25](https://www.icmagroup.org/assets/ERCC-Guide-to-Best-Practice-March-2025.pdf).

3. **Prepare the exact signed contractual cash.** Use the represented
   positive full accrual fraction `A` and require `1+r A>0`.
   Compute `I=P r A` with the accepted bounded fixed-coupon Money
   arithmetic and expressly elected contractual rounding, where
   applicable. Check the actual payable total `P+I>0`. Signed
   interest can be negative or zero. Emit `−P` at purchase,
   `+P` principal and `I` fixed interest at close. Preserve
   the literal zero interest row and the accrual and rate basis.
   The outstanding-principal ledger opens at `P`, records zero
   added principal on the funding event and `−P` at redemption.
   Record security delivery or return only in a qualified settlement
   view with its actual terms. Its collateral mark supplies no cash
   row in this schedule.
   [Cashflow and principal semantics](../core-conventions/015-cashflow-semantics.md),
   [fixed coupon arithmetic](../cashflows/05-coupon-cash-and-pik.md)
   and [ICMA on negative repo rates](https://www.icmagroup.org/market-practice-and-regulatory-policy/repo-and-collateral-markets/icma-ercc-publications/frequently-asked-questions-on-repo/28-what-happens-to-repo-transactions-when-interest-rates-go-negative/).

4. **Agree one entitlement cut for price and cash exports.** Keep
   the whole contractual schedule separate from eligible valuation
   cash. Before purchase both outstanding dates contribute. The
   ordinary holder cut excludes same-day cash. An explicit
   entitled pre-settlement cut includes it at factor one, at either
   purchase or repurchase. After funded purchase only the unpaid
   close contributes. At a proved completed repurchase the value
   is zero. Validate the contract before applying settled fast paths.
   Known fails or overdue unsettled cash require a qualified
   outstanding-claim treatment. Require the exact native-currency
   nominal discount binding and its positive dated relative factor
   for eligible future cash. Skip inactive source lookups. Share
   the admitted cash rows, actual dates and factors across checked
   raw PV, final Money PV, metrics and exports. Raw PV avoids final
   PV quantization and respects the actual contractual cash amounts.
   [Dated NPV](../core-conventions/016-discounting.md)
   and [shared cash valuation](../valuations-framework/cashflow-export.md).

5. **Keep collateral quotation and haircut on a named basis.** For
   collateral diagnostics require the exact security and positive
   quantity, and an eligible dated positive dirty Money quote in
   the cash currency per declared unit or face lot. Normalize the
   lot explicitly. A clean quote first needs a qualified conversion
   to dirty on the required date and entitlement basis. Reject
   missing, unitless, stale, mismatched and cross-currency quotes
   on this profile. For authored haircut fraction `0≤h<1` and
   checked collateral value `M`, report the initial requirement
   `P/(1−h)` and coverage `M/[P/(1−h)]`. Any surplus diagnostic
   uses the same principal basis. Principal remains the agreed
   purchase cash. Require checked positive division with no
   fallback or floor. A haircut differs from an initial margin
   ratio. The collateral diagnostics use purchase principal, while
   a qualified margin method may use accrued repurchase cash,
   agreed collateral marks and actual balances. Retire
   `implied_collateral_return`, which annualizes static coverage.
   Gross contractual PV needs no collateral mark.
   [ICMA haircut definition](https://www.icmagroup.org/market-practice-and-regulatory-policy/repo-and-collateral-markets/icma-ercc-publications/frequently-asked-questions-on-repo/21-what-is-a-haircut/)
   and [ICMA ERCC Guide, sections 3.12–3.16](https://www.icmagroup.org/assets/ERCC-Guide-to-Best-Practice-March-2025.pdf).

6. **Define what interest and remaining time report.** Report the
   final contractual rate, actual period interest and cumulative
   contractual accrual in native currency. Accrual is zero through
   purchase and uses the admitted partial fraction on the actual
   interval to the earlier of valuation or repurchase. At and after
   repurchase the cumulative contractual diagnostic returns the
   actual total interest. An outstanding or unsettled accrued
   balance needs its own settlement state and measure. Remaining
   time uses the stated contractual clock and actual close, with
   zero at or after close. A clock unable to supply the requested
   subinterval without its context returns unavailable. Preserve
   actual dates also in lifecycle and expiry reporting.
   [Accrual context](../core-conventions/002-coupon-context-day-counts.md)
   and [cashflow lifecycle](../cashflows/12-schedule-lifecycle.md).

7. **Separate contractual funding risk from market discount risk.**
   `funding_risk` reports the signed raw PV move for exact
   `r+.0001` versus `r`. Hold contractual dates, accrual,
   quantity, discount, collateral and eligibility fixed and apply
   actual cash rounding on both sides. On the unquantized simple
   law the equivalent eligible close change is
   `P A .0001 D(V,E)`. Use this shortcut only when it gives the
   same admitted result. A proved absence of eligible close cash
   produces structural zero.
   Discount `dv01`, typed `pv01` and `bucketed_dv01` reprice
   the fixed cash on the exact active source under the checked
   parallel and triangular rate experiments, in native currency
   per positive bp. Keep original source clock, held inputs and
   represented shock widths. A contract bump, a discount-source
   bump and an authorized market-quote replay are distinct.
   Repo-specific calibration, par quotes and quote replay require
   their own qualified materialization and price basis.
   [Direct rate risk](../valuations-framework/finite-difference-rates.md).

8. **Require an actual agreement for margin and close-out.** Remove
   the synthetic trade-based netting identity and the duration
   sensitivity placed into 3M or 6M. An applicable margin request
   supplies the actual method and edition, counterparty and
   agreement or exact clearing/account scope, and the method's
   authorized prepared mark or fully qualified risk factors and
   reprices. Return unsupported or an input error when required
   evidence is absent. Tri-party identifies an operational agent
   arrangement. Clearing and method eligibility require their
   own authority. A gross financing PV serves as a margin input
   only when that method expressly selects it. Margin thresholds,
   calls, transfer settlement, interest, balances and substitution
   stay with the repo margin owner. Collateral income and
   manufactured payments, buy/sell-back adjustments, failed
   settlement charges, default valuation, liquidation recoveries,
   close-out netting and XVA require their own qualified contracts
   and methods. Haircut and coverage supply no recovery law.
   [Accepted margin bridge](../valuations-framework/margin-bridge.md),
   [repo margin cashflows](../margin-collateral-im/20-repo-margin-cashflows.md),
   [ICMA tri-party repo](https://www.icmagroup.org/market-practice-and-regulatory-policy/repo-and-collateral-markets/icma-ercc-publications/frequently-asked-questions-on-repo/24-what-is-tri-party-repo/)
   and [ICMA default process](https://www.icmagroup.org/market-practice-and-regulatory-policy/repo-and-collateral-markets/icma-ercc-publications/frequently-asked-questions-on-repo/26-what-happens-to-repo-in-a-default/).

9. **Use the same prepared repo across valuation and consumers.**
   Reconcile Rust builders, restore, pricer, metrics and exports,
   the existing tagged repo envelope and generic Python and WASM
   routes, schemas, portfolio margin and real-estate financing.
   Preserve effective dates, quote identity, lender direction,
   collateral units and settlement basis. Require any financing
   caller using the borrower side to apply its sign once. Distinguish
   active pricing and requested collateral dependencies in the
   same dispatch. Qualify dates and cutoff boundaries, negative
   and zero interest, cash rounding, foreign and per-face quotes,
   role-separated risk and absent margin authority independently.
   [Repo wire](../../../../finstack-quant/valuations/schemas/instruments/1/rates/repo.schema.json)
   and [real-estate financing consumer](../../../../finstack-quant/valuations/src/instruments/equity/real_estate/levered.rs).

## Observed behavior

### Choice, scope and rationale

A repo is valued from the cash lender's contractual cashflows, with simple interest and an optional special-collateral rate adjustment. Collateral affects coverage and margin reporting, not the contractual cashflow PV. Term, overnight and open contracts use the supplied effective maturity; open-repo termination optionality is not modeled.

### Algorithm and controls

1. Require finite positive cash and collateral quantity, haircut in `[0,1)`, finite special adjustment, and ordered raw/adjusted dates. A resolvable calendar is mandatory. Adjust start and maturity with the selected business-day convention.
2. Effective decimal rate is `repo_rate + special_adjustment_bp/10000` when present. Interest is `cash*rate*year_fraction(adjusted_start,adjusted_end)`; the notional repayment is cash plus interest.
3. Export negative initial cash, positive terminal principal and signed interest. Discount remaining dated flows with the discount curve relative to valuation. PV returns zero on and after adjusted maturity.
4. Required collateral is `cash/(1-haircut)`. Actual collateral value is quantity times a currency-bearing price; unitless prices error. Coverage is actual/required value. The named implied collateral return is `(actual/required-1)/remaining_tau`, returning zero when tau or required value is zero/nonpositive; it is a static collateral excess annualization, not a forecast of collateral total return.
5. Accrued interest clamps the accrual endpoint to the adjusted contract interval. FundingRisk is the one-sided PV difference after +1 bp contractual repo rate; DV01 instead bumps market rate curves.
6. Margin uses a bilateral synthetic netting-set identifier and an IR delta proxy `-cash*remaining_calendar_years*1bp`, placed entirely in 3M or 6M according to remaining tenor. This proxy ignores the market and is not the repriced DV01. VM uses actual repo PV.

### Conventions, selection and failures

Convenience constructors obtain USD general-collateral defaults from the embedded contract registry; other currencies require explicit conventions. Overnight starts Following-adjusted and matures one business day later. Negative finite repo rates are allowed. Haircut is a fraction. The signed rate adjustment is in bp. Day count governs interest; margin uses calendar-year scaling and unadjusted maturity.

### Alternatives and tradeoffs

A cashflow-derived curve sensitivity could replace the margin duration proxy; contractual funding-rate sensitivity and discount-curve DV01 should remain distinguishable. Open termination optionality and collateral income/price dynamics are unimplemented, economically distinct models.

## Evidence

- [Repo implementation](../../../../finstack-quant/valuations/rates/src/repo/types.rs), [metrics](../../../../finstack-quant/valuations/rates/src/repo/metrics/mod.rs), [accrued interest](../../../../finstack-quant/valuations/rates/src/repo/metrics/accrued_interest.rs), [collateral return](../../../../finstack-quant/valuations/rates/src/repo/metrics/implied_collateral_return.rs), [margin proxy](../../../../finstack-quant/valuations/rates/src/repo/marginable.rs).
- [Repo pricing](../../../../finstack-quant/valuations/tests/instruments/repo/pricing.rs), [collateral tests](../../../../finstack-quant/valuations/tests/instruments/repo/collateral.rs), [margin tests](../../../../finstack-quant/valuations/rates/tests/repo/margin.rs): formula, convention and proxy behavior evidence.
- [Hull](../../../REFERENCES.md#hull-options-futures) provides general financing context. A specific adopted repo margin-proxy validation reference was not established; the existence of [SIMM](../../../REFERENCES.md#isda-simm) does not validate this approximation.

## Evidence limits

Collateral and required collateral metrics discard currency before division; no conversion or same-currency check is present in the local coverage/return calculators.

### Independent review evidence

- A Friday January 9 to Monday January 12, 2026 overnight on
  an eligible business-day calendar spans three ACT/360 accrual
  days. With USD 1 million at `−.012`, interest is `−100`
  and terminal cash `999,900`. With purchase-date factor
  `.9998` to close, the ordinary holder mark is `999,700.02`
  and the explicitly entitled pre-settlement trade mark
  `−299.98`. On the close date, entitled cash is `999,900`
  at factor one and the ordinary same-day exclusion is zero.
- With one million face and dirty price 97 per 100, collateral
  value is `970,000`. Purchase cash `980,000` and haircut
  `.02` give a principal-based requirement of `1,000,000`,
  coverage `.97`, and a collateral-value shortfall of
  `30,000`. The haircut-adjusted value is `950,600`.
  A hypothetical agreement using the accrued repurchase amount
  after 30/360 at `.05` gives exposure `33,483⅓` before
  transfers. This last computation illustrates a distinct
  agreement basis using a rule from the ICMA ERCC Guide.
- Before purchase, with one million cash, quote `.035`,
  accrual `30/360`, and dated factors `.999,.99`, the
  unquantized contract has trade PV `−6,112.50` and funding
  risk `+8.25`. A centered continuous-zero shock of one bp
  on the same two factors with source clock distances
  `.01,.1` gives discount DV01 about `−8.929875`.
  Under an explicit cent-rounding election interest becomes
  `2,916.67` versus `2,925.00` after the contractual bump.
  The actual funding-risk reprice is then `+8.2467`.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
