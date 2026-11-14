# VALUATIONS-OTHER-FIXED-INCOME-12: Revolver quote schedules, exposure and draw-option attribution

- **Decision status:** Accepted 2026-11-14 — full checked revolver quotes, exposure and risk package. Required quote compounding was previously accepted (O6, 2026-11-12).
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below. Reconciliation to the accepted contract is pending.
- **Validation status:** Live source and primary references inspected. Independent settlement, cost, rate and credit risk, default and draw attribution arithmetic checked. Legal, market, runtime and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked revolver quotes, exposure and risk

1. **Bind the buyer, transferred funded par and settlement authority.**
   Use the exact lender share, authorized settlement date and phase,
   information cut, recognized payment state and identified transferred
   rights and obligations. Use an explicit actual settlement or the
   qualified trade-date, lag and calendar election in record 06.
   Obtain positive transferred funded denomination from covered state
   at settlement or its expressly authorized binding. Count pending
   repayable principal only to the extent it actually transfers on that
   denominator. A current drawn snapshot or simulated mean cannot
   supply an unknown settlement denomination. Unresolved future draws,
   pre-settlement changes or assignment rights require coverage.

   For the funded-par analytical clean quote use
   `dirty = clean_points × transferred_par/100 + signed_cash_accrued`.
   Require positive purchase Money. Separate the full assignment
   invoice, unfunded adjustment, LC, permanent reductions, billed fees,
   delayed compensation and trade costs. A full invoice enters a quote
   only with its own authorized scope and complete matching buyer cash
   and obligations. Adopt no generic assignment invoice formula.
   Preserve the actual amount payer and currency. A direct dirty amount
   needs no otherwise unused clean-to-dirty calculation.
   [Accepted shared loan quote basis](06-loan-quotes-eir.md),
   [LSTA filed terms, Purchase Price Calculation](https://www.lsta.org/wp-content/uploads/2022/05/AS-FILED_21-2726-Motion-for-Leave-to-File-Amici-Brief-of-LSTA-et-al-2d-Cir..pdf).

2. **Use owned accrued cash and the original-date buyer bridge.**
   Prepare the actual original coupon parents with record 10's
   elapsed balances, contractual cumulative accrual, rate and fixing
   observations. Apply the assignment's qualified entitlement and
   accrued rule to obtain signed cash accrued. Distinguish native
   earned interest, quotation adjustment, billed carry and completed
   but unpaid cash. A validated authoritative scoped accrued amount
   may be used when its inputs and authority match. Preserve
   any separation between receipt ownership and accrued paid on
   settlement. Do not prorate aggregate scheduled interest by a
   default day count or group distinct parents solely by payment
   day. An unresolved partial overnight amount stays unavailable.

   A selected model-derived dirty purchase uses `Q=J(V)/F(V,S)`.
   Here `J(V)` is the entitled buyer claim valued at the original
   cut under the qualified selected model, and `F(V,S)>0` is the
   separately authorized settlement financing factor on the same
   cash and position basis. At the same date use factor one after
   claim admission. If a retained-value subtraction is used, qualify
   the complete model-valued difference between holder and buyer
   claims. Preserve pre-settlement draws, credit contingencies,
   pending cash, financing and transferred obligations where relevant.
   Remove discount-only carry subtraction from a generic base PV.
   Missing buyer qualification leaves an independent raw holder
   result available.
   [Revolver cash preparation](10-revolver-cashflows.md),
   [original-parent accrued](../cashflows/13-accrued-interest.md),
   [buyer and financing bridge](../valuations-bonds/02-settlement-and-clocks.md).

3. **Quote one qualified dated buyer strip.** Freeze the complete
   elected scheduled maturity path including signed transferred draws,
   repayments, interest, fees and other admitted rights and obligations.
   Include entitled cash payable after purchase on the settlement
   date at factor one. Preserve a later payable date and exclude
   settled or seller-retained cash. Deterministic and expressly fixed
   paths use their actual prepared claims. An explicitly qualified
   survival-unweighted expected strip under record 11 can support a
   labeled DM or yield on projected expected cash, with independently
   known denomination and complete entitlement. Freeze the strip,
   run and population for the quote. This metric solves on the
   expected cash amounts. Expected individual path yield, live
   model risk and default-adjusted cash require their own bases.
   [Accepted expected-cashflow basis](11-revolver-monte-carlo-credit.md).

   Use record 06's floating-only curve DM on its native relative
   discount clock with the independent Continuous or nominal
   Periodic quote compounding. For an implied DM require nonnegative
   net sensitive buckets with at least one positive, a positive
   dirty target above spread-independent cash, explicit qualified
   bounds and work, and a returned dirty Money residual gate.
   Net only cash sharing the complete factor function. An
   authored DM can forward-price an admitted signed strip even
   where inversion fails the monotonicity gate. `price_from_dm`
   reports clean points on the same accrued and funded basis.
   Keep quote authority explicit, including the choice of input
   spread versus dirty-price fit. Remove preset guesses, hidden
   caps and an inferred unspread target.

   `ytm` uses the same qualified maturity path and prepends
   the negative dirty purchase at actual settlement. Net eligible
   same-date cash, require negative investment at settlement,
   later effective cash and exactly one sign reversal, then use
   canonical ACT/365F XIRR. Report the explicit annual-effective
   ACT/365F or equal-growth ACT/360 basis, checking the actual
   returned yield against the admitted stream. This package
   selects scheduled maturity yield for the revolver.
   [Accepted loan curves and returns](06-loan-quotes-eir.md),
   [checked dated XIRR](../core-conventions/017-irr.md).

4. **Keep two cost measures and the original EIR ledger.**
   `all_in_rate` follows record 06's signed eligible running
   coupon and fee cash divided by covered ACT/365F
   funded balance-time. Use explicit opening and closing
   holder cuts and actual cash dates, with a qualified
   deterministic, realized or fixed prepared path. Exclude
   upfront, OID, nonrecurring and third-party economics.
   A proven zero numerator over positive exposure returns
   zero. Zero funded exposure or missing history makes
   the ratio unavailable. Remove synthetic opening funding
   and reconstruction from averaged simulation rows.

   `weighted_average_cost` is a frozen-state approximation
   per unit of commitment. Require a covered state cut,
   positive commitment and explicit positive reporting
   horizon. Hold commitment, funded and eligible LC stocks,
   applicable margin and running fee rates and tier election
   at that cut. Respect authorized accrual windows and
   expiry. Use qualified existing original coupon and fee
   context, actual fixings or contractual projected index
   observations, and apply applicable controls before
   aggregating. Accrue synthetic interest on funded par,
   unused fee on eligible net capacity, usage on funded
   par, facility fee on commitment and owned LC or
   fronting fees on eligible LC face. Divide the complete
   synthetic accrued amount by frozen commitment times
   ACT/365F report length. This retains each component's
   contractual day count and isolates the frozen-state
   assumption. Report the actual cut, horizon and active
   components. Rates are annual decimal lender economics.
   Future changes in stock and forecast tier or margin,
   nonrecurring costs, discount, default and cash-payment
   timing are outside this proxy. Zero commitment makes
   the ratio unavailable.

   Origination reporting uses an explicit record 06 original
   ledger and recognition information. Preserve original
   dated funding, forecasts, fees and integral classifications
   and actual origination OID once. Use the accepted return
   and unrounded carry roll with a stated discharge residual.
   Retain `oid_eir_rate` and `oid_eir_carrying_value`, and
   replace `oid_eir_amortization` with
   `effective_interest_adjustment` under the accepted meaning.
   An accounting policy or a current drawn balance is
   insufficient to infer original eligible cash and charges.
   [Shared cash cost and EIR](06-loan-quotes-eir.md),
   [AASB 9, B5.4, fee classification](https://standards.aasb.gov.au/aasb-9-dec-2022).

5. **Publish exposure from exact phased lender state.**
   `available_capacity` is covered legally drawable net
   `C−D−L` in the admitted profile and is zero after a
   proven draw close. `utilization_rate=D/C` when commitment
   is positive. At zero commitment the ratio is unavailable.
   Keep fee-tier utilization `(D+L)/C` on its own basis.
   Require exact capacity, state and active legal rights,
   with no numeric floor or prior-anchor substitution.

   The native-currency `exposure_at_default` is immediate
   economic gross exposure on the chosen hypothetical default
   phase, `N+q_U U+q_L L`. Reuse record 11's same eligible
   funded and distinct pending par `N`, available undrawn
   `U`, eligible LC face `L` and approved valuation
   conversions. No market intensity is needed to measure
   this snapshot. Recovery, future expected utilization,
   event P&L and prudential EAD retain different result
   bases. Missing amounts, rights or phases remain errors.
   [Approved revolver default scope](11-revolver-monte-carlo-credit.md),
   [regulatory EAD ownership](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/36/inforce/2023-01-01/published/2022-12-08).

6. **Use same-path signed default valuation impact for expected loss.**
   Define `expected_loss = H_without_default_on_held_paths − H_live`.
   Compute the comparator on the admitted record 11 scenarios,
   with the same funding and spread-driven behavior, rates, bank
   factors, cash and amounts, phases and fee policy. Set borrower
   survival weights to one on the affected claims and omit
   matched modeled default settlements. Leave independent cash
   common. This comparator operates on the same paths and does
   not instantiate a new No Default model. Use stable local log
   survival to evaluate `cash × D × (−expm1(−J))` on eligible
   cash and subtract each signed default valuation contribution
   once. Preserve signed results including impacts of avoided
   future funding. Honor any actual active credit route including
   Spread Driven without an attached curve ID. A qualified
   no-default or exact zero-intensity case can give zero after
   admission. An unqualified counterfactual is unavailable.
   Compute the Monte Carlo estimator from paired per-path
   differences with record 11's independent units and uncertainty.
   Keeping a seed while zeroing the spread cannot establish
   matching funded amounts or fixed utilization. Accounting credit loss
   and a behavioral repricing scenario have different ownership.
   [Accepted survival and default treatment](11-revolver-monte-carlo-credit.md),
   [IFRS 9 accounting scope](https://www.ifrs.org/issued-standards/list-of-standards/ifrs-9-financial-instruments/).

7. **Anchor live risk to raw holder PV and name rate and credit shocks.**
   Use the actual selected record 11 pricer and raw present
   value at the authorized original date. Keep observed marks,
   settlement-strip spread centers and the raw model distinct.
   Live revolver risk uses the accepted model law, with
   settlement DM and Z confined to the authorized strip
   experiments. Report a held observed mark on its own stated
   basis. Preserve native lender currency, position, history,
   model, clocks and risk source keys across shocks.

   DV01 uses checked full two-sided continuous-zero bumps on
   explicitly selected active typed rate sources. On actual
   linked roles move the source coherently once and rebuild
   dependent bank and projection preparation. Hold known fixings,
   hazard evaluator, calibration inputs and recovery fixed in
   direct rate risk. Source-native parallel and exact triangular
   key-rate requests report signed holder currency per positive
   bp. Credit CS01 uses full two-sided exact par-CDS quote
   changes on an active replayable Frozen Hazard or Market
   Anchored source. Verify the original lossless recipe and
   qualified par-risk view, preserve actual original CDS
   observations, hold calibration discount and recovery fixed,
   and apply the changed source through the selected model.
   Rebuild the mean anchor on each Market Anchored side.
   Divide by the checked full width in quote bp. Report
   `cs01::<curve>` and exact
   `bucketed_cs01::<curve>::<quote>` where admitted.

   Separately elected `hazard01` can directly move a Frozen
   Hazard source's annual ending intensities on its own
   native clock, holding recovery fixed. Use exact source
   and parameter identities and signed holder currency per
   positive intensity bp. Use the stated
   full central stencil or a prequalified full one-sided
   move at a nonnegativity boundary. A failed replay, inactive
   source, authored Constant or CIR without an active compatible
   source does not select curve CS01 or hazard01. Leave
   unqualified spread-parameter shocks unavailable.
   For stochastic live risk, align exact grids, streams,
   factors and draws or use an independently qualified coupling.
   Compute the signed difference per estimator and carry
   sampling and grid evidence. A failed side propagates.
   [Shared DV01](../valuations-framework/finite-difference-rates.md),
   [shared credit risk](../valuations-framework/credit-sensitivities.md),
   [CDS quotation risk and recovery experiments](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf).

8. **Admit fixed-strip settlement Z risk as its own experiment.**
   Select one fixed signed eligible buyer strip with no active
   credit in that experiment, exact settlement phase and approved
   Continuous or Periodic quote compounding. An authored fixed
   benchmark can be elected separately for a modeled facility.
   A nominal expected schedule is insufficient for live risk.
   Use one explicit authored, qualified implied or expressly
   unspread Z center. An implied fit uses the admitted checked
   inverse and price-error gate. Hold that center, strip, native
   clock and target definition. Report central signed settlement
   currency per positive bp under `cs01::<instrument>`.
   Where requested, use strictly increasing exact positive
   upper bounds on that clock for disjoint cash bands. Assign
   each flow to the first upper bound at or beyond its time,
   with the final band including the tail. Report exact
   `bucketed_cs01::<instrument>::<upper-bound>` identities
   and return admitted empty bands as zero.
   Eligible unit-factor cash at
   time zero has zero spread sensitivity. Complete bands sum
   to the same pre-rounding parallel change on this fixed
   basis. Preserve signed funding and qualified upfront cash
   once. Separate bands from market quote buckets.
   [Checked spread-law and stencils](../valuations-framework/credit-sensitivities.md),
   [term-loan settlement cash bands](09-term-loan-risk-yields.md).

9. **Measure Recovery01 and immediate default on the active model.**
   Elect Frozen Intensity or Quote Preserving recovery on a
   qualified Frozen Hazard route. For Frozen Intensity change
   cash recovery alone and hold hazard, recognized cash and
   affected model and exposure state fixed. Where that state
   is genuinely unaffected and the prompt recovery law is
   affine, use the exact per-path `.01 × Σ (N+q_U U+q_L L) w`,
   with the approved discount/default-time weights `w`.
   It holds at admitted recovery endpoints. Quote Preserving
   holds original CDS observations and calibration discount
   and coherently moves the authorized deal and calibration
   recovery bindings while preserving approved offsets.
   Replay the full recipe, including upfront quotes and its
   actual calibration domain. Use explicit positive step,
   complete represented central moves or a prequalified full
   one-sided move at an actual domain edge, then normalize
   per `+.01` absolute recovery. Hold realized claims and
   pricer fixed and pair stochastic differences. Missing
   provenance or a failed reprice cannot elect a fallback.
   Holding Spread Driven spread while changing recovery
   changes `s/(1−R)`. This record leaves that coupled
   experiment outside Recovery01.

   `jump_to_default` uses an explicit immediate event phase
   for the same live holder claim. Admit any qualified selected
   live issuer credit route, including a stochastic base
   with a fully known admissible event snapshot. Use
   `K=R N+(R−1)(q_U U+q_L L)` once, add qualified
   independently surviving claim PV and subtract raw live
   holder PV. Value prompt default cash at factor one.
   Preserve timing and priority for pending principal,
   covered contingent funding and same-day obligations.
   Recognized default, unresolved payables or unsupported
   remedy rules require their own claim valuation. A No
   Default request alone supplies no immediate-default risk.
   [Prompt default scope](11-revolver-monte-carlo-credit.md),
   [matched economic default](09-term-loan-risk-yields.md),
   [CDS Recovery01 and value on default context](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf#page=37).

10. **Replace draw option cost with a qualified spread attribution.**
    Rename the standalone result and key to `draw_spread_attribution`.
    Request it explicitly on the admitted same paths with an
    active compatible modeled spread and authorized reference
    at the run anchor. Attribute only actual discretionary
    modeled funded draws at their exact economic phases.
    Keep accrued margin on a fungible principal pool with
    an additive linear cash margin and an explicitly accepted
    proportional repayment allocation for this diagnostic.
    Start tagged spread exposure `B` at zero. On a qualifying
    draw of amount `d`, add `d × (s_draw−s_anchor)`.
    Other funding adds no tagged exposure. On a repayment
    of `r` from pre-phase funded stock `D>0`, multiply
    `B` by `(D−r)/D`. Full retirement clears it exactly.
    Require a qualified phase order if simultaneous movements
    would change the attribution. An unresolved order
    leaves the requested diagnostic unavailable.
    Keep this attribution state separate from principal and
    contractual fee balances. Unsupported allocation across
    non-fungible tranches or nonlinear all-in rate controls
    makes the diagnostic unavailable. Capacity or LC change,
    mandatory funding and default funding supply no qualifying
    draws.

    For each qualified original coupon slice integrate the
    held tagged exposure against its original accrued margin
    measure. Attribute the resulting incremental cash to the
    actual payable dates. Use the same price-path discount
    and survival once. Return the negative of that incremental
    PV as signed native lender currency. Positive spread
    widening reduces this attribution and narrowing can
    increase it. Complete absence of eligible draws or equal
    admitted spreads can prove zero; a model label alone cannot.
    Report the mean and uncertainty using record 11's
    independent estimator units, with pairing when elected.
    Use the pricing loop without requiring full path capture.
    This is a comparison with a reference-spread coupon
    on the same eligible funding. It selects no exercise
    policy, independent option price or additive PV component.
    Structured-credit waterfalls retain their own method.
    [Empirical credit-line pricing and usage](https://arxiv.org/abs/2401.12301),
    [structured-credit owner](../valuations-structured-credit/16-revolver-utilization-and-option-cost.md).

11. **Use one checked metric preparation across public results.**
    Key reusable prepared state by cuts, entitlement, quantity,
    active market and valuation law and, where modeled, qualified
    run and scenario. Preserve exact compounding, clocks,
    observations, denominator, absolute amount, market source,
    actual bumps and model or quote basis. Declare annual decimals,
    price points per 100 and native-currency outputs in canonical
    metadata. Keep unavailable,
    failed, verified zero, and finite outcomes distinct.
    Never infer one risk or quote from the success of an
    incompatible measure. Require proper capture and sampling
    metadata for an optional modeled diagnostic. Reconcile
    selected registry metrics, direct Rust APIs, Python/WASM
    typed results, generated schemas, parity and UI, portfolio,
    scenario and attribution consumers. Propagate phase-
    qualified actual cash to shared carry/theta and keep
    survival-unweighted schedules identified as projections.
    No change to the shared theta, regulatory or accounting
    ownership.
    [Metric orchestration](../valuations-framework/metric-orchestration.md),
    [shared carry and theta](../valuations-framework/carry-theta-breakeven.md).

## Accepted decision: required quote compounding

Accepted on **2026-11-12** as owner resolution O6 of the core-conventions
reconciliation, within the accepted
[CORE-CONVENTIONS-007 #9](../core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions)
separation of payment cadence from quote compounding. Runtime
implementation is tracked in the register.

The revolving credit facility carries a required, independent `Compounding` quote
convention. Settlement curve DM and Z pricing and inversion and
fixed-strip Z sensitivity read that field. A convention preset can
state it explicitly, independently of payment frequency and other
tenors. There is no default. Live holder CDS and intensity shocks
use their own market or source coordinates.

## Observed behavior

### Choice and rationale

[Quote metrics](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/metrics/quotes.rs) apply a clean price to funded principal while retaining future fees and signed funding in the holder schedule. Stochastic facilities use the expected cashflow schedule, not an expectation of individually solved path yields. [Exposure metrics](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/metrics/exposure.rs) make undrawn and LC default funding visible. These are separate economic measures; their grouping does not establish a common calibration or accounting standard.

### Quote and cost algorithms

1. Build/cache the schedule at the valuation anchor. Settlement follows the facility's business-day lag. Deterministic settlement drawn replays future events; stochastic settlement drawn uses the current anchor balance. Quoted target is `clean_percent × settlement_drawn/100 + accrued`. Without quote, subtract discount-only cashflows in `(as_of,settlement]` from model PV and divide by settlement DF; survival over the lag is not removed.
2. Accrued is full-period scheduled interest times elapsed/full contractual day count. This prorates the scheduled coupon even if balances change within the period; it does not replay elapsed utilization. Calls supply default day-count context, unlike the coupon engine's enriched context. No current period/interest returns zero.
3. Floating-only discount margin uses future cash settlement flows and the periodic z-spread kernel in [record 06](06-loan-quotes-eir.md); quoted DM can invert to clean points only when settlement drawn is positive. YTM solves XIRR against the same target, including negative future draws. AllInRate divides cash interest plus fees by time-integrated drawn balance; a seasoned schedule inserts an opening funding leg to reconstruct its starting balance.
4. [OID/EIR](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/metrics/oid_eir.rs) rebuilds the schedule at issue, adds opening negative drawn principal and upfront receipt, and delegates to shared origination amortized-cost reporting. Include-fees defaults on. Current `drawn` is treated as opening issue funding for this report; a seasoned current-balance snapshot is not a reconstructed historical origination record.
5. [Approximate weighted cost](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/metrics/weighted_average_cost.rs) freezes current drawn/LC/commitment and fee tiers. Floating index is the remaining-time-weighted average of one contractual forward per period. Cost is interest plus current commitment, usage, facility and LC/fronting fees, divided by **commitment**, not drawn. It excludes upfront/fixed future fee timing and future balance/tier changes. Zero commitment returns zero.

### Credit and option metrics

- EAD is current `drawn + LEQ × undrawn + LC_default_draw`; availability is commitment minus drawn minus LC, and displayed utilization is drawn/commitment (LC is included for fee tiers separately).
- ExpectedLoss returns risk-free counterfactual PV minus base PV. The counterfactual removes the hazard identifier, default-draw factors and stochastic credit spread. It returns zero immediately if no hazard identifier exists, including explicit stochastic-credit configurations. Reusing a seed does **not** imply unchanged utilization when spread sensitivity is nonzero: removing spread changes utilization drift, so the difference can include behavioral funding effects despite the source comment promising identical paths.
- Recovery01 and JumpToDefault require deterministic draw events plus hazard curve. Recovery01 uses a ±1 percentage-point recovery shock with one-sided bounds; replayable credit calibration rebases hazards to unchanged par spreads. Immediate-default value is `R×drawn + (R−1)×(LEQ×undrawn+LC_draw)`, with no accrued; JTD subtracts base PV.
- [CS01](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/metrics/cs01.rs) uses quote-rebootstrap/direct-hazard risk when a credit curve exists. Otherwise deterministic cashflows receive periodic z-spread parallel/bucketed bumps; stochastic facilities without hazard identifiers reject because no deterministic holder schedule is available.
- [Draw option cost](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_pricing.rs) is an attribution on each simulated path. At each retained observation calculate positive change in commitment × utilization; multiply it by spread widening from the anchor and by the remaining discounted survival-weighted accrual annuity, then negate and sum. Narrowing can produce a positive value. Constant-spread configurations return zero even though their simulated spread has small CIR volatility. The MC mean/interval uses [record 11](11-revolver-monte-carlo-credit.md) aggregation. This is not an optimal-exercise option price or a separate additive component of total PV.

### Conventions, alternatives and limitations

Rates/yields/DM are annual decimals; prices are points per 100 drawn; PV/exposure/cost attribution are lender-side money. Quote metrics can mix credit-adjusted model targets with unweighted expected contractual cashflows. Recovery metrics intentionally exclude stochastic credit. A state-conditioned draw-option model, realized-default simulation, historical EIR ledger and elapsed-balance accrued are unimplemented alternatives; each changes the economic contract, not merely a numerical tolerance.

### Evidence

[Draw-option tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/draw_option_cost.rs), [weighted-cost tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/weighted_average_cost.rs), [default-risk tests](../../../../finstack-quant/valuations/fixed-income/tests/default_risk.rs) and [review cases](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/review_fixes.rs) provide located contract/limiting-case evidence, not executed validation. [Fixed-income handbook](../../../REFERENCES.md#fabozzi-fixed-income-handbook) supplies general quote context; no independent provenance for the exact draw-option attribution was established.

## Independent arithmetic and qualification

- Under the illustrative single-currency assignment formula, a
  quote of 70 on 3 million funded and 2 million unfunded gives a
  funded-price component of 2.1 million and an unfunded credit of
  .6 million, for an invoice of 1.5 million before other
  adjustments. The funded-par analytical convention establishes
  only its own quote amount.
  [LSTA filed terms](https://www.lsta.org/wp-content/uploads/2022/05/AS-FILED_21-2726-Motion-for-Leave-to-File-Amici-Brief-of-LSTA-et-al-2d-Cir..pdf)
- Transferred par 100, clean points 98 and accrued 4 give dirty
  102. A payable buyer receipt 2 on settlement and 110 one year
  later give annual Periodic DM 6% on a 4% base and ACT/365F YTM
  10%. Replacing the later strip with funding −20 at one year and
  receipt 143 at two years keeps the same forward dirty value
  and yield. The negative sensitive bucket fails DM inversion
  admission. A model-valued buyer claim of 103 with financing
  .98 gives dirty `5150/49` on the same claim basis.
- On two 73-day ACT/365F periods with commitment 1,000, LC use
  100 and funding 400 then 600, eligible running cash totals
  20.30 at coupon 8%, unused fee 1%, usage .5%, facility .25% and
  owned LC fees 1.75%. Funded balance-time totals 200, giving
  `all_in_rate=.1015`. The opening-state commitment proxy is
  `.04325`. An original net investment of 98 and a one-year
  receipt of 107.8 yield original EIR 10%, effective interest
  9.8, adjustment 2 and zero closing carry when contractual
  coupon cash is 7.8 and par repayment is 100.
- A raw 100 payment due in one year on a flat 3% continuous curve
  has central one-bp DV01 about `−.00970445535`. On the separate
  settlement strip `+20,−30,+120` at years 1, 2 and 3,
  continuous Z 1% gives value `97.9527507975` and signed central
  one-bp risk `−.028312016967`. The funding band contributes
  `+.005538698115`. Its negative amount also prevents the
  monotone implied-spread fit.
- At commitment 100, funded 40, LC 10 and pending clean par 7,
  conversion fractions .5 and .8 give economic EAD 80.
  Recovery .4 gives prompt net default cash −1. If the live
  value comprises terminating 34 and independently surviving 3,
  economic JTD is −35. The record 11 three-year phased default
  example gives same-path expected loss `5.4515890358` and
  frozen-intensity Recovery01 `.0874315352`. A separate case
  with −100 at year one, +100 at year two, 10% continuous
  discount, 20% intensity and full prompt recovery after
  funding gives signed expected loss `−2.2104470095`.
- With opening funded 60, a qualifying draw 40 at +200 bp and a
  repayment of 30 after the first of two quarters, tagged exposure
  is .8 and .56 over the two slices. With eligible discount-times-
  survival weights .96 and .90, the spread attribution is
  `−.318`. Carrying the whole draw through both quarters would
  give `−.372`.

These are standalone arithmetic checks. Legal assignment, servicing,
replay-source, paired risk and spread-attribution eligibility, runtime
and typed host qualification remain pending. Repository pricing and
host suites were not run.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
