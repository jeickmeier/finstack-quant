# VALUATIONS-OTHER-FIXED-INCOME-15: TBA assumed collateral, settlement ownership and dollar-roll carry

- **Decision status:** Accepted 2026-11-14 — full TBA settlement, delivery and dollar-roll package.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below. Reconciliation to the accepted contract is pending.
- **Validation status:** Live source and primary references inspected. Independent invoice, forward, roll lifecycle and financing arithmetic checked. Delivery operations, market, runtime and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked TBA settlement and dollar rolls

1. **Bind one settlement-date authority and a separate delivery state.**
   Require the trade identity and date, buyer/seller direction, currency,
   contract product, coupon, tenor, delivery month, clean quote, quantity
   basis and applicable stipulations. Bind valuation and information cuts.
   Elect the exact published year/month/class settlement date from the frozen
   CORE-CONVENTIONS-010 data, or the expressly agreed contractual date and
   phase. Require consistency with the governing terms, including for an
   off-calendar agreement. Missing selected published coverage fails the
   lookup. Resolve the terms and lifecycle before shortcuts. Notification
   dates and cutoffs, allocated identities, any actual rescheduling and
   delivery-versus-payment (DVP) completion have their own authority. A
   pool notice or scheduled date alone establishes no completed transfer.
   [Published settlement contract](../core-conventions/010-sifma-settlement.md),
   [SIFMA notification and settlement calendar](https://www.sifma.org/resources/general/mbs-notification-and-settlement-dates),
   [SIFMA trade confirmations](https://www.sifma.org/wp-content/uploads/2017/08/ch06.pdf).

   An admitted open exchange stays live at the explicit same-day pre-DVP
   phase with a unit invoice factor. Authenticated DVP or termination
   closes the forward when all its remaining obligations are resolved.
   Delivered MBS and their owned distributions pass to the separate holder
   position. For a fail, partial delivery or correction, require authoritative
   settled and open quantities, remaining dates and phases, transferred
   rights, and any due bills or compensation to value or report the affected
   claims. Absent a fully qualified valuation route, the affected result is
   unavailable. Preserve settled evidence and the separate dated adjustment.
   [Fedwire securities settlement](https://www.frbservices.org/financial-services/fedwire-securities-service/),
   [SIFMA remittance and due bills](https://www.sifma.org/wp-content/uploads/2024/04/TBA_Market_Governance-Uniform_Practices_Manual-Chapter_10_Remittance_Reporting_Payments_Due_Bills_and_Compensation.pdf).

2. **Elect fixed assumed collateral or a complete authorized allocation.**
   A fixed proxy can use a representative pool or an expressly authored
   generic assumption snapshot. Bind dated original and current stock and
   factor basis, seasoning, remaining installments, mortgage WAC, security
   coupon, prepayment law, remittance profile and calendar, coverage,
   entitlements and active curve roles under record 13. When an analytical
   homogeneous position is scaled, use the expressly elected trade current
   face and one checked proportional preparation of stock and claims.
   Remove automatic factor-one, servicing and guarantee fees and unit PSA,
   and settlement-month issue and contract-term remaining-maturity guesses.
   Label any projected state and hypothetical face. Keep actual allocations
   on their own original and current face with identity, authoritative factor
   vintage and dated eligibility evidence. [Agency MBS state and ownership](13-mbs-projection.md).

   Resolve exact security, product, coupon, tenor rules, applicable
   eligibility edition, stipulations and the complete allocation into named
   lots or authorized pieces. Establish covered eligibility from effective
   rules and qualified evidence, including where a security is reconstituted.
   Issuer alone is insufficient for UMBS eligibility. Apply the Chapter 8
   ±0.01% current-face variance to each applicable lot, including the
   applicable balance of a trade. A specified pool with expressly specified
   original face requires exact original face and identity. Keep actual
   delivered face distinct from the agreed trade target. This admits
   deterministic delivery with documented eligibility. Delivery-option
   optimization, specified-pool payup and unsupported clearing rules remain
   outside the route. [SIFMA delivery requirements](https://www.sifma.org/wp-content/uploads/2025/08/UniformPractices-Chap8-revise2025.pdf).

3. **Prepare the invoice and P&I rights once from the eligible delivery.**
   Retain contractual quantity, delivered original face `O_i`, applicable
   factor `f_i`, calculated `O_i f_i` and authorized settlement current face
   `B_i` with their exact bases and applicable rounding. Reconcile supplied
   values under the authority's law. The ordinary bilateral dirty invoice is
   the sum of `B_i × clean_points / 100` plus each piece's record-13 accrued
   `B_i × coupon_i × N_i / 360` under the qualified month-start profile.
   Use the stated money precision, rounding granularity and currency. A
   cleared invoice or external adjustment requires its own authority and
   recorded cash. Preserve confirmed invoice cash through later factor
   corrections. Resolve buyer and seller P&I by record-date and transfer
   terms, including pending claims with later remittance dates. Do not
   recreate seller claims by restarting the pool or deduct them a second
   time from accrued. [SIFMA standard settlement formulas](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf#page=46),
   [FICC MBSD clearing rules](https://www.dtcc.com/legal/rules-and-procedures),
   [MBS clean, dirty and entitlement basis](13-mbs-projection.md).

4. **Value each live TBA from the original valuation date.** For the
   admitted deterministic exchange, let `J(V)` be the original-origin PV
   of the exact transferred MBS claims and `K_S` the dirty invoice at the
   admitted settlement `S`. The long raw value is `J(V) − D(V,S) × K_S`.
   Bind the collateral discount and trade invoice discount roles explicitly.
   The settlement-price identity `D(V,S) × [J(S) − K_S]` requires exactly the
   same claims and a composition of dated factors that reproduces `J(V)`.
   Qualify each source and positive relative factor. Apply signed direction
   and quantity once to raw native value. No date-only expiry, collateral
   rebasing, implicit funding source or observed-quote fallback.

   A conventional roll sells the front TBA and purchases the back TBA at
   a strictly later settlement on matching contractual product, tenor,
   coupon, currency and agreed current face. Store each leg's own dates,
   collateral selection, invoice and lifecycle. Actual deliverables and
   delivered amounts follow each leg's own terms. For the front seller and
   back buyer, PV is `long_back_raw − long_front_raw`, with each closed leg
   contributing zero and checked native conversion after netting. Between
   completed front and pending back settlements, value the back obligation.
   Drop, carry and repo specialness are separate results. [New York Fed
   dollar-roll mechanics](https://www.newyorkfed.org/medialibrary/media/research/epr/2013/1212vick.pdf),
   [New York Fed comparable contract terms](https://www.newyorkfed.org/markets/ambs-treasury-faq-10142022).

5. **Use a matched surviving-face financing comparison.** Require a
   comparable clean quote pair from one information cut, and identify the
   contract or market quote basis and collateral snapshot. Advance one
   explicit reference pool through the front and back ownership cuts. Let
   `F` be dirty front proceeds on the reference opening position. Let `B*`
   be the qualified back dirty price on the reference's surviving current
   face. Use the covered principal effects exactly once. The actual back
   invoice continues to use its own delivered quantity.

   Let `W` be the checked back-date value of foregone P&I identified by the
   difference in entitlement across the two ownership cuts. Include eligible
   pending later payments and exclude claims both alternatives own. For
   eligible cash `C` due before `B`, use `C/D_carry(pay,B)`. At `B` use `C`
   after classifying the phase, and after `B` use `C×D_carry(B,pay)`. Require
   an explicit qualified carry source for active off-date flows. No separate
   interval coupon or paydown adjustment may duplicate dirty accrued or the
   owned strip. With `τ=actual_civil_days(front,back)/360`, solve the simple
   balance `F(1+rτ)=B*+W` as `r=(B*+W−F)/(Fτ)`. Preserve the signed numerator
   with checked stable arithmetic. Require positive finite `F`, surviving
   face, `τ` and represented gross `1+rτ`. Allow finite negative rates and
   check balance reproduction on the declared amount precision. The result is
   a model-dependent annual decimal financing diagnostic.
   [Federal Reserve Board financing equation](https://www.federalreserve.gov/econresdata/feds/2016/files/2016052pap.pdf#page=12),
   [Song and Zhu timing and surviving-face example](https://www.mit.edu/~zhuh/SongZhu_MortgageDollarRoll_RFS.pdf#page=41).

6. **Keep the clean drop, specialness and live rate risk explicit.** Drop
   is `front_clean−back_clean`, in points per 100. Require an independently
   selected repo benchmark matched to collateral, currency and exact roll
   dates for specialness. Compare yields on the same simple ACT/360 basis.
   If a qualified repo factor supplies the benchmark, use
   `r_repo=(1/D_repo(front,back)−1)/τ` with a stable log/expm1 evaluation.
   Report `10,000 × (r_repo−r)` bp. A missing benchmark makes specialness
   unavailable and leaves independently supported outputs available. Carry
   and repo roles may share an explicitly qualified source. Remove the
   financing-discount fallback.

   For DV01, elect each live leg's frozen or responsive prepayment under
   record 14. Shock the selected typed discount or refinancing sources
   coherently with the explicit rate direction and key coordinates. Hold
   executed invoices, actual faces, rights and lifecycle fixed. Reprice
   each leg from its prepared state. A separately elected projected-face
   comparison moves both invoice and claims together and identifies the
   scenario basis. Combine signed raw leg values for each shock side, then
   calculate monetary DV01 and publish exact active source and shock
   identities. A carry-only source contributes no PV sensitivity. Use each
   leg's own response and exclude admitted closed legs from market demands.
   [MBS effective risk](14-mbs-risk-oas.md),
   [shared finite differences](../valuations-framework/finite-difference-rates.md),
   [repo contract](../valuations-rates/15-repo.md).

7. **Report the physical exchange through one Rust-owned contract.**
   Carry leg identity, signed invoice cash, the corresponding pool identities
   and exact original/current security faces, factor vintage, transferred
   claims, source and phase, with projected or confirmed status. Maintain
   invoice cash and security stock effects separately. Remove the roll's
   ordinary schedule that treats invoice cash as principal. Both instruments
   reject complete money-only export for the physical exchange. Horizon risk
   crossing delivery requires a matched cash, transferred security and P&I
   ownership scenario. Reconcile native pricing, registered results,
   existing JSON-first Python and WASM entry points, schemas, UI, market
   dependencies and portfolio consumers with the same prepared leg state.
   [Physical bond-future delivery precedent](01-bond-futures.md),
   [complete cashflow export convention](../valuations-framework/cashflow-export.md),
   [cash and principal ownership](../core-conventions/015-cashflow-semantics.md).

## Observed behavior

### Choice and rationale

[TBA pricing](../../../../finstack-quant/valuations/fixed-income/src/tba/pricer.rs) values delivery of a deterministic assumed MBS pool. [Dollar-roll pricing](../../../../finstack-quant/valuations/fixed-income/src/dollar_roll/pricer.rs) takes long back TBA minus long front TBA. Carry diagnostics share the projected pool's coupon and prepayment assumptions. This enforces internal cashflow consistency but does not price delivery choice or forecast market specialness.

### TBA algorithm and selection

1. Use the explicit settlement-date override when supplied. The live `AgencyTba` type otherwise requires the explicit SIFMA year, month and class. Pricing propagates a missing date, while expiry can suppress the date error and validation can skip the chronology check on failed lookup. [TBA type](../../../../finstack-quant/valuations/fixed-income/src/tba/types.rs).
2. Without explicit pool, create one at the first of settlement month with current face equal to trade notional, original face `notional/factor`, maturity after contract term and WAC equal pass-through coupon plus servicing and guarantee fees. [Embedded defaults](../../../../finstack-quant/valuations/fixed-income/data/assumptions/tba_assumptions.v1.json) are factor 1, servicing 25 bp, agency guarantee 25 bp or GNMA 6 bp, PSA multiplier 1. An explicit prepayment model overrides PSA. Generic accrual is 30/360.
3. Explicit pool must match trade currency, coupon, optional supplied factor and delivery agency. FNMA/FHLMC are interchangeable in the implemented agency predicate; GNMA I/II remain separate. Rebase pool to settlement-month ownership and scale to the purchased current face. Earlier month's accrued P&I stays with the seller even if its agency payment is later.
4. Before settlement, value is `DF(as_of,settlement) × [MBS_PV_at_settlement − notional×trade_clean_points/100 − settlement_accrued]`. MBS cashflows follow [record 13](13-mbs-projection.md). At and after settlement, forward PV is zero; delivered collateral must be a separate position.
5. Parallel/bucketed DV01 reprice with the MBS refinancing adjustment from [record 14](14-mbs-risk-oas.md). Explicit pools are rebuilt directly; generic pools get the adjusted prepayment specification. TBA's cashflow exporter rejects physical delivery as lacking a standalone money-only schedule.

### Dollar-roll carry algorithm

[Carry](../../../../finstack-quant/valuations/fixed-income/src/dollar_roll/carry.rs) uses the front generic pool for the interval between actual settlement dates. Project through the interval, accrue coupon on declining balances with pool day count, and recognize principal reductions once at next month's accrual boundary. Both quantities are points per 100 initial current face. With `drop=front_clean−back_clean`,

`implied_financing = [coupon + paydown×(100−back_clean)/100 − drop] / front_dirty × 360/calendar_days`.

Front dirty includes coupon accrued from month start. This correctly makes a wider drop lower implied financing under the implemented convention. [Specialness](../../../../finstack-quant/valuations/fixed-income/src/dollar_roll/metrics.rs) is `(repo_rate−implied_financing)×10,000`. Repo is the simple Actual/360 rate `(1/DF(front,back)−1)/tau` from explicit repo curve or fallback discount curve. Roll days must be positive. Forward legs preserve explicit settlement overrides; risk rebuild applies the front pool's refinancing speed adjustment to both legs.

### Limitations and alternatives

Generic collateral assumptions are embedded values, not calibrated delivery-pool observations. TBA delivery tests are partial eligibility checks, not the complete SIFMA good-delivery rulebook. Neither engine models cheapest-to-deliver choice, specified-pool payups, stochastic drop or balance-sheet scarcity. The roll's back pool is a new generic pool for its settlement month; carry references the front pool, so carry is a diagnostic and is not separately added to PV.

[Dollar-roll cashflow export](../../../../finstack-quant/valuations/fixed-income/src/dollar_roll/types.rs) emits positive dirty front sale proceeds and negative dirty back purchase payment as contractual Notional cashflows. It does not export delivered collateral or its income. Thus discounting that cash schedule does not generally reproduce roll PV, unlike a cash-settled derivative schedule. This differs deliberately in observed behavior from TBA's rejection, but a common physical-delivery reporting contract remains unagreed. Explicit delivery-pool pricing is supported for standalone TBA; portfolio-of-pools optional delivery valuation is unimplemented.

### Evidence

Colocated tests in [TBA pricer](../../../../finstack-quant/valuations/fixed-income/src/tba/pricer.rs) cover settlement-month entitlement and accrued. Dollar-roll carry contains `implied_financing_uses_pool_prepayment_and_dirty_front_price`, a hand-derived monthly amortization/carry case; metrics contain `specialness_resolves_the_supplied_repo_curve` and deterministic prepayment-risk rebuild comparisons. These are narrower than external trade-price validation. [SIFMA formulas](../../../REFERENCES.md#sifma-mbs-standard-formulas) and [good delivery](../../../REFERENCES.md#sifma-tba-good-delivery) are canonical reference pointers.

## Independent arithmetic and qualification

- An agreed current-face target of 1,000,000 and allocated original face
  1,250,000 at factor `.80004000` yield 1,000,050 current face. At 101.25
  clean points and 5.5% coupon over `10/360`, the half-up aggregate invoice
  is `1,014,078.48`. Invoicing the trade target understates cash by `50.70`.
  With original-origin `J(V)=1,020,000` and `D(V,S)=.997`, raw long PV is
  `8,963.75544` using that rounded invoice. The July 2025 SIFMA example
  shows that a two-million total near target can still fail the separate
  million-lot limits. [SIFMA allocation examples](https://www.sifma.org/wp-content/uploads/2025/08/UniformPractices-Chap8-revise2025.pdf).
- At one cut, the two raw long values `2.604` and `2.735` give front-selling
  roll PV `.131`. With a proved closed front and unchanged back mark in this
  arithmetic comparison, only `2.735` contributes.
- With 30 civil days, front dirty proceeds `102.2`, surviving balance 95,
  back dirty points `101.25`, and owned `5.5` payable after back settlement
  with transport factor `.9995`, `B*=96.1875` and `W=5.49725`. The simple
  rate is `−.0604990215264` and the 4% repo comparison gives
  `+1,004.9902153` bp. Substituting a purchase at the opening current face at the back
  quote changes the exposure and fails the surviving-face comparison.

Qualify actual calendar coverage, original versus current lot basis,
multi-lot evidence, clearing cash, revised factors, phase and failure
handling, P&I that crosses the back settlement, matched market quotes and
benchmarks, rate and horizon repricing, and complete host reconciliation.
The arithmetic and primary references establish the selected convention.
Repository pricing and host suites and independent market reconciliation
remain pending.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
