# VALUATIONS-OTHER-FIXED-INCOME-09: Term-loan workout yields, quote-pinned risk and default metrics

- **Decision status:** Accepted 2026-11-14 — full checked term-loan workout yields and risk package. Required quote compounding was previously accepted (O6, 2026-11-12).
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below. Reconciliation to the accepted contract is pending.
- **Validation status:** Live source and primary references inspected. Independent workout, spread, recovery and default checks run. Legal, model, market, runtime and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked term-loan workout yields and risk

1. **Bind one buyer purchase and one dated yield basis.** Use the
   accepted OFI-06/08 original valuation and information cuts,
   authorized actual settlement `S` and phase, matched transferred
   rights, positive purchased funded denomination and one approved
   dirty purchase. Convert a clean observation with signed cash
   accrued only when needed. An expressly elected model-derived
   dirty price needs the original-origin buyer claim and qualified
   financing bridge. Keep quote authority, financed buyer amount
   and raw holder PV distinct.

   Start each admitted scenario with the negative purchase at
   settlement. Include only buyer-entitled receipts and funding
   obligations on their actual dates and phases. A same-date
   receipt enters only when it transfers and can be ordered
   after the purchase. The first surviving net at settlement must
   remain negative after canonical date netting. Require at
   least one later effective date and exactly one sign reversal.
   Use the accepted canonical ACT/365F XIRR and the explicitly
   declared ACT/365F or equal-growth ACT/360 reporting basis
   `q_B = expm1((B/365) log1p(r_365))` for `B ∈ {365, 360}`.
   Verify the actual returned quote on that same stream under the
   accepted residual law. PIK changes par and enters the yield
   cash ledger only through the eventual entitled cash. Keep
   origination EIR and issue-return measures separate.
   [Accepted loan quote and yield law](06-loan-quotes-eir.md#accepted-decision-for-checked-loan-quotes-cash-cost-and-effective-interest),
   [original-cut loan valuation](08-term-loan-pricing.md#accepted-decision-for-checked-term-loan-valuation-and-exercise),
   [canonical XIRR](../core-conventions/017-irr.md).

2. **Give YTM and the two-, three- and four-year quotes distinct
   exits.** For `ytm` use the authorized scheduled maturity
   path without new voluntary exercise. Preserve actual payoff
   and surviving cash dates, including later settlement of
   eligible maturity cash. When a binding prior election changes
   redemption, label the locked exit by its actual terms. Only a
   buyer claim that can still follow the scheduled maturity path
   has scheduled YTM.

   For `yt2y`, `yt3y` and `yt4y`, elect a hypothetical full
   clean-par redemption on the exact settlement-relative
   Gregorian two-, three- or four-year date, with an explicitly
   chosen eligible adjustment rule and phase where applicable.
   Add years as a direct month offset with target-month clamp.
   Apply a business-day roll only under the chosen convention.
   Use checked calendar arithmetic and preserve the contractual
   schedule and actual payable earned cash through that
   hypothetical exit. Resolve commitment cancellation and
   surviving claims explicitly. This is a par repayment scenario.
   It creates no legal call, forecast sale price or exercise
   instruction. A date outside the live economic horizon, an
   impossible exit or inadequate claim coverage is unavailable.
   At the actual maturity endpoint reuse the ordinary path only
   when the economics and payable dates coincide. Remove the
   unqualified maturity cap and valuation-date horizon origin.
   [Checked tenor arithmetic](../core-conventions/007-tenor-date-arithmetic.md),
   [loan borrower payoff and claim phases](08-term-loan-pricing.md).

3. **Enumerate actual full borrower rights for YTC and YTW.**
   For `ytc` select the earliest reachable full borrower call
   by its authorized exercise cut after the buyer's purchase,
   with exact notice, eligibility, settlement and payment
   terms. A settlement-day call needs explicit phase admission.
   If distinct complete rights share the earliest cut, solve
   each applicable yield, report the lowest and preserve the
   co-minimizing identities. Establish that the earliest cut
   is complete. Later payoffs need no preparation for that
   standalone request. No reachable buyer call makes YTC
   unavailable.

   For `ytw` use the minimum finite returned yield over the
   complete still-reachable full borrower-call set and the
   no-new-exercise scheduled maturity path when reachable.
   Honor a binding notice or exercise that fixes the sole
   remaining transferable path, including deferred fixed cash
   if qualified. Enumerate the admitted finite authored dates
   or every eligible dated cut of a qualified bounded standing
   window, with explicit calendar and work controls. Preserve
   right and phase identities even on equal civil dates. A
   later adjusted payment never extends a call window. A
   prospective lender-controlled option is outside the borrower
   minimum. Tree nodes, fitted exercise, profitability and
   friction do not determine the legal candidate population.

   For each path use the accepted OFI-08 borrower payoff
   law, including actual payable earned cash and earned PIK,
   common cash, premium eligibility and authorized benchmark.
   Include cash through each authorized phase, replace principal
   redemption once, and stop only the extinguished rights
   and transferred obligations. Keep independent surviving
   cash and any admitted future costs on their actual dates.
   Same-day phase coverage, rights, currency, funding and
   quote basis must qualify for each required path. Unresolved
   conditional exercise, notice or a missing path fails a
   requested complete minimum. A reachable all-immediate
   exit with no later effective cash also prevents that
   complete yield. Never replace no-call YTC
   with YTM, drop a failed branch, or select by rounded yield.
   Report the finite pre-rounding minimum and all exact
   co-minimizing identities at the common quote basis. An
   independently requested fixed workout only needs its own
   qualified path.
   [Borrower call law](08-term-loan-pricing.md),
   [related complete bond workout law](../valuations-bonds/11-workout-yield.md),
   [filed notice and conditional repricing premium](https://www.sec.gov/Archives/edgar/data/1872789/000119312522097622/d297684dex1010.htm),
   [filed prepayment and allocation terms](https://www.sec.gov/Archives/edgar/data/1702780/000162828022032367/termloanthirteenthamendment.htm).

4. **Use separately elected DM, Z and OAS quote experiments.**
   `discount_margin` admits a qualified frozen floating buyer
   cash strip on the selected scheduled or fixed workout basis.
   Use the shared OFI-06 continuous or nominal-periodic curve
   law, the required independent quote compounding and its
   admitted source clock. An authorized clean, dirty or
   independently derived matched buyer amount can drive the
   inverse. A callable label does not require a clean-only
   quote or elect a workout. Use the explicit bracket,
   monotonicity admission and returned dirty-price residual
   gate. Remove self-targeting the unspread model to produce
   zero and contractual margin as a root guess. A literal
   observed DM is identifiable as supplied. Using it in a
   price or risk experiment requires the admitted cash strip.
   A fixed-rate loan's eligible settlement Z and a live
   callable loan's continuous tree OAS have their own laws.
   A reported OAS fit must run the actual selected model and
   registry on the OFI-08 original-origin buyer claim.
   [Shared loan DM and XIRR](06-loan-quotes-eir.md),
   [model OAS and buyer fit](08-term-loan-pricing.md).

5. **Elect the model risk center and hold its spread.** Choose
   an admitted unspread raw model, an expressly authored model
   spread, or an observed-price-fitted model. Fit the latter
   once at original `V` against `J(s)−FQ` with complete
   entitlement and financing. Use the authorized tree OAS
   law or the separately qualified fixed-strip settlement Z
   experiment with its holder bridge. Use the modeled raw
   holder `H(s)` for holder risk. A dirty observation and
   its funded holder value are distinct. Conflicting active
   authorities, failed fits or unsupported selected models
   make the dependent result unavailable or error. An
   unqualified quote cannot fall back to zero spread.

   Preserve original mark, actual selected model and registry,
   cash/contract, phases, fixing/credit state, quantity and
   quote provenance. Remove a quote short-circuit only in
   private comparable reprice views. Hold the fitted/authored
   spread and its law for all shocks and for the no-discretion
   comparison. Rebuild only the affected prepared state. An
   observed mark held fixed is a distinct reporting basis.
   Rate DV01 uses declared actual typed discount, projection
   and named reference-curve roles on their source-native
   continuous-zero perturbations. Hold hazard inputs and
   recognized cash fixed, and reprice any authorized model
   response. A source used only to finance the quote fit does
   not automatically enter model DV01. If a qualified holder
   bridge consumes a source, include its elected active role.
   The same actual shocked source moves all active linked
   roles in the experiment. Publish parallel or exact key-
   rate buckets, signed holder currency per positive bp and
   checked actual steps. Credit quote replay needs its own
   selected experiment.
   [Rate risk and represented stencils](../valuations-framework/finite-difference-rates.md),
   [related quote-reproducing risk view](../valuations-bonds/15-curve-risk.md).

6. **Distinguish par-CDS CS01 from loan settlement Z risk.**
   On an actually credit-consuming admitted route, use exact
   par-CDS quote replay. Reproduce the active hazard with its
   full original recipe, calibration discount, recovery and
   conventions; qualify its par-risk center. The calibration
   discount can differ from the loan discount. Use exact quote
   IDs or verified recipe indices. Keep original observations
   and any derived par-risk center distinct. Shock the selected
   full set or each target by explicit full `±h` bp, refit
   both directions and reprice the original raw holder claim
   at the held spread. Divide the raw holder price difference
   by the checked actual step span in bp. Report signed
   currency per positive bp under `cs01::<curve>` and
   `bucketed_cs01::<curve>::<quote>` for the single-source
   and exact quote series. An unused ID cannot select the
   experiment. Failed replay supplies no direct-intensity or
   synthetic-credit fallback.

   For an explicitly elected no-active-hazard loan Z route
   require a qualified fixed buyer cash strip, separately
   authorized when the instrument has live optionality.
   Allow the same continuous/periodic Z compounding as the
   qualified settlement strip. Use a declared Z center,
   checked implied Z, or explicitly elected unspread zero.
   Reprice at a common explicit two-sided spread step and
   report signed settlement currency per positive bp under
   `cs01::<instrument>`. Retain the loan's disjoint cash-band
   route where requested. On the exact native spread clock
   use strictly increasing positive upper bounds, first
   bound at or beyond the cash time, with the final band
   including the tail. Zero-time eligible unit-factor cash
   contributes no spread sensitivity. Use
   `bucketed_cs01::<instrument>::<upper-bound>` with an exact
   coordinate identity. Include admitted empty bands as zero
   and use the same anchor and stencil
   for all rows. Sum raw band contributions on the same
   basis. Identify them as cash bands, separate from CDS
   quote buckets and triangular interest-rate buckets. An
   unqualified live OAS or actual credit shock cannot stand
   in for a loan settlement Z experiment.
   [Shared quote and cash-band risk](../valuations-framework/credit-sensitivities.md),
   [CDS quote recalibration](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf#page=24).

7. **Value the live borrower option on the same model.** Report
   `embedded_option_value = H_live−H_disabled` in signed
   native holder currency. Disable only remaining discretionary
   borrower rights, preserving binding notices, already realized
   exercises and legally retained cash. Keep the same raw
   model, fitted grid, sources, time, claim, valuation cuts
   and OAS. Use the permitted future economic effects of
   retaining or removing that discretion. No refit of the
   disabled leg, different discounting model or clamping of
   the raw difference. Verified absence of discretionary rights
   permits structural zero. An unresolved right cannot establish
   absence. Explain the finite-grid/friction basis. This
   package adopts no embedded-option volatility experiment.
   [Matched live/disabled method](../valuations-bonds/18-embedded-option-risk.md).

8. **Select a recovery sensitivity and reprice from the matched
   center.** Require the admitted alive OFI-08 prompt-par recovery
   model, actual active issuer exposure and authorized recovery
   binding. On `recovery01` elect frozen intensity, or
   quote-preserving recovery with qualified source replay.
   Frozen intensity moves the authorized recovery cash binding
   while holding actual intensity and survival fixed. For a
   proven deterministic affine claim with unaffected exercise
   and exposure, use the exact `.01 × unit-recovery PV`
   coefficient even at an endpoint. Otherwise reprice the
   full model. Quote-preserving recovery coherently moves the
   admitted deal and calibration recovery bindings, holds
   authored CDS observations and the calibration discount
   fixed, and refits through the exact qualified recipe.
   Preserve any admitted existing offsets and preserve
   original quote types, including upfront. Missing
   provenance is a failure for replay. Zero-LGD calibration
   needs its own valid domain and cannot be rescued by a
   one-sided shock around an invalid center.

   Keep held model spread, actual cash and realized recovery
   fixed. Use the explicit positive absolute recovery step
   with the full central stencil where both sides qualify.
   Use the full one-sided stencil only when the opposite
   direction fails the prechecked recovery domain and the
   base and other side qualify. Report `.01 × ΔH/ΔR`
   using the checked actual recovery width. A failed fit or
   missing source never elects a smaller or one-sided step.
   Use the selected registered raw model at the center and
   every side. No native-model substitution or clipping.
   [Recovery mode and boundary policy](../valuations-bonds/17-default-risk.md),
   [CDS recovery risk and value on default](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf#page=37).

9. **Measure immediate economic default on the authorized phase.**
   For `jump_to_default` insert the hypothetical default on
   the admitted valuation phase of the same alive issuer and
   funded lender claim. Resolve priority relative to binding
   exercise, actual cash payments and principal events. Use
   the covered recoverable par `N_rec`, including eligible
   repayable principal through its payment boundary. Apply
   PIK and write-downs on their authorized economic dates.
   Unfunded commitments, accrued interest and premiums are
   outside the prompt recovery base. Identify which future
   funding, fees, coupons and borrower-right effects terminate
   and which claims legally survive. Detachment alone proves
   no default independence. Preserve independently surviving
   claim value `H_surviving` under its qualified treatment.

   Use prompt recovery `R N_rec` at factor one.
   Report `R N_rec + H_surviving − H0` against the matched
   unrounded raw holder value `H0`. No discount, probability
   weight or carried modeled recovery on the new immediate
   recovery itself. Never replace covered par with the
   end-of-day economic balance or subtract a naked settlement
   quote. An actual default, contractual acceleration premium,
   separate remedy or delayed recovery requires its own claim
   law. A default-contingent remaining coupon or funding
   obligation can affect the signed jump even with zero
   recoverable par. Proved absence of covered exposure and
   terminating claims can give a structural zero. This is
   signed economic event P&L. Regulatory default capital and
   expected loss retain their own definitions.
   [Selected term-loan prompt recovery](08-term-loan-pricing.md),
   [Duffie on fractional recovery of par](https://www.darrellduffie.com/uploads/1/4/8/0/148007615/duffiedefaultabletermmodels1998.pdf),
   [filed default acceleration terms](https://www.sec.gov/Archives/edgar/data/318833/000119312525053470/d935596dex101.htm).

10. **Keep result identity and availability across consumers.**
    Use one prepared Rust quote and risk basis per request.
    Retain exact workout and candidate identities, legal and
    payable dates, price authority, clock, residual, model and
    shock source, curve role, mode, actual step, currency and
    position amount and pre-rounding output. Keep selected
    scenario yield, held-model risk and fixed-strip risk
    distinguishable in the registered results. Require separate
    elected risk identities or requests for incompatible
    modes. Preserve successful independent measures and the
    reasons for unsupported or failed dependent results.
    Reconcile scoped pricing/metric dispatch, quote overrides,
    native APIs, Python/WASM, schemas and UI, position/portfolio
    aggregation and attribution. Carry a signed amount or
    position scale only once on its qualified basis. Report
    actual price authority, workout availability and recovery
    mode in the loan yield card.
    [Metric orchestration and reporting](../valuations-framework/metric-orchestration.md),
    [current loan yield card](../../../../finstack-quant-ui/registry/valuations/components/loan-yield-card/loan-yield-card.tsx).

## Accepted decision: required quote compounding

Accepted on **2026-11-12** as owner resolution O6 of the core-conventions
reconciliation, within the accepted
[CORE-CONVENTIONS-007 #9](../core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions)
separation of payment cadence from quote compounding. Runtime
implementation is tracked in the register.

The term loan carries a required, independent `Compounding` quote
convention. Settlement Z-spread and DM pricing and inversion, and
fixed-strip Z-spread sensitivity read that field. It has no default.
A convention preset can state it explicitly, independently of the
payment frequency and any other tenor. The lattice OAS uses the
separately accepted continuous law. CDS quote sensitivities use their
own market quote basis. [Record 08](08-term-loan-pricing.md)

## Observed behavior

### Choice and rationale

The [metric registry](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/mod.rs) separates yield conventions from model risk. Risk to a quoted price first fits and holds a spread/OAS, so curve bumps change model value instead of leaving a fixed quote unchanged. This pricing behavior was observed before adoption of the checked contract above.

### Scope and algorithm

- [Yield helpers](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/irr_helpers.rs) turn clean percent into dirty settlement money using funded outstanding plus principal already removed from balance but still unpaid, and cash accrued. Without a price quote, remove pre-settlement cash from model PV and divide by the settlement DF.
- YTM is dated XIRR on all purchased future cash settlements. YT2Y/YT3Y/YT4Y truncate at `as_of + n years`, capped at the last cash payment/maturity, and add exercise principal plus cash accrued. These are hypothetical redemption yields, not embedded-call optimal exercise.
- [YTC](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/mod.rs) takes the earliest candidate call; no candidate falls back to YTM. Candidates include future call start dates and, for an already active provision, the first coupon after settlement. [YTW](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/ytw.rs) is the minimum XIRR over these calls plus final maturity. Each candidate includes cash settled through exercise and an additional dirty redemption claim; make-whole claims use the same helper as pricing. This finite candidate set is narrower than every exercise date on the tree.
- [Discount margin](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/discount_margin.rs) is floating-only and solves on projected contractual flows. Callable loans require a clean quote. Without a quote on an otherwise eligible loan, the target is zero-margin PV, so the result is zero; contractual margin is the solver guess.
- [Risk view](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/risk_view.rs) clears price drivers and holds explicit OAS or solves OAS from the clean quote for Tree; Discounting holds explicit/fitted z-spread. OAS on Discounting is rejected. Registered parallel/bucketed DV01 use shared repricing. CS01 uses hazard repricing only for Tree plus credit curve. Otherwise it uses settlement Z-spread reprices on contractual flows, with a central stencil and disjoint spread-clock cash bands in the bucketed route. Thus explicit Discounting can ignore a supplied hazard curve for these metrics.
- Embedded option value is `callable − straight` using the pinned model. [Default risk](../../../../finstack-quant/valuations/fixed-income/src/term_loan/metrics/default_risk.rs) requires Tree plus hazard: Recovery01 delegates to the shared hazard helper and calls the native raw loan path for side reprices. The helper selects replay by provenance. Jump-to-default clamps recovery, uses the date-level economic balance and subtracts the context base value, without an independent surviving-claim decomposition.

### Controls, conventions and failure behavior

Yield outputs are annual-effective decimal XIRRs, OAS and DM decimal spreads, and sensitivity outputs cash per specified shared bump. Workout date selection uses `as_of`; the price leg is at settlement. A candidate occurring inside the settlement lag can therefore precede the purchase date; that boundary is not adopted here. Solver and fixing errors propagate; missing required quotes error. [Record 06](06-loan-quotes-eir.md) covers shared cost/EIR arithmetic; [record 08](08-term-loan-pricing.md) covers model restrictions.

### Alternatives and tradeoffs

A dense all-dates YTW search would align more closely with standing calls but increase work and require a redemption-date convention. Quote-fixed sensitivities describe mark carry only; spread-pinned sensitivities describe model risk at the mark. Both are meaningful outputs but are not interchangeable. Hazard CS01 and z-spread CS01 measure different credit perturbations.

### Evidence

[Registered metrics](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/metrics/registered_metrics.rs), [YTC](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/metrics/ytc.rs), [YTW](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/metrics/ytw.rs), [DM](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/metrics/discount_margin.rs), and [default risk](../../../../finstack-quant/valuations/fixed-income/tests/default_risk.rs) are located regression/invariant evidence, not rerun. [Fixed-income reference](../../../REFERENCES.md#fabozzi-fixed-income-handbook) is contextual.

## Independent arithmetic and qualification

- Dirty purchase `100` on 2025-01-01 and fixed alternate final
  receipts `104` on 2026-01-01, `103` on 2027-01-01, and
  scheduled maturity `116` on 2028-01-01 produce yields
  `4%`, about `1.48891565%`, and about `5.07175745%`.
  The first call supplies YTC and the later call gives YTW.
  A positive redemption before purchase followed only by the
  negative purchase can satisfy standalone one-sign-change
  XIRR while failing the buyer scenario.
- Buyer financing `.98` on dirty `100` with separately
  retained value `3` gives holder value `101`. A frozen
  settlement Z strip of `20` at one year and `100` at
  three years, zero source yields and Z, annual periodic,
  has central one-bp cash bands of about `−.00200000002`
  and `−.03000000100`, summing to `−.03200000102`.
- In a toy prompt-FRP amortizing loan with cash `20` at
  half-year and `84` at one year, par `100` then `80`,
  base rate `.035`, held OAS `.01`, intensity `.05`
  and recovery `.4`, holder PV is `97.1812108590`.
  Frozen-intensity Recovery01 is about `+.0430418725`.
  Under a toy continuous CDS calibration holding par quote
  `.03` and refitting intensity as `.03/(1−R)`, the
  central one-point quote-preserving result is about
  `−.0005328152`. The latter uses the stated toy premium law.
  For a node with hold value `100.10 + R`, redemption `100`
  and effective cash friction threshold `.50`, crossing the
  strict call boundary changes holder prices at recovery
  `.39/.40/.41` to `100.49/100.50/100.00`. Frozen-intensity
  central Recovery01 is `−.245` despite the positive direct
  recovery coefficient.
- With dirty quote `90`, financing `.98` and independent
  surviving PV `6`, the matched raw holder is `94.2`.
  At `R=.4` immediately before a due principal receipt of
  `20`, covered par `100` gives JTD `−48.2`. Immediately
  after payment, holder value `74.2` and covered par `80`
  give JTD `−36.2`. The surviving claim contributes `6`
  on both stress cuts.

Standalone arithmetic checks cover these cases. Qualification
still requires complete legally dated notice and workout fixtures,
actual quote-pinned holder and buyer valuation, replay and exact
bucket evidence, recovery/default entitlement and option
comparisons, runtime and host agreement. Repository pricing and
host suites were not run.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
