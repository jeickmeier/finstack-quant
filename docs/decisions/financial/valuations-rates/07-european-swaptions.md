# VALUATIONS-RATES-07-EUROPEAN-SWAPTIONS: European swaption forwards, settlement annuities and quote Greeks

- ID: VALUATIONS-RATES-07-EUROPEAN-SWAPTIONS
- Decision status: **Accepted — complete checked European swaption pricing and risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, model, calibration and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked European swaption pricing and risk

1. **Prepare one explicit European contract and dated underlier.** Admit
   the holder option on one single-currency, constant positive exact
   Money notional fixed-for-floating IRS, without principal exchange.
   Payer is the right to pay fixed at the exact signed Decimal strike
   `K` and receive floating; receiver reverses the legs. The option
   mark is gross; any separate trade premium belongs to the trade cash
   owner. Resolve the same nominal leg span and independently resolve
   the actual accrual, day-count, benchmark, publication and payment
   grids under [rates 03](03-interest-rate-swaps.md). Require exercise no
   later than either leg's first actual effective start and retain the
   exercise cut separately from the start and payment dates. Admit
   stubs, calendars, lags, rolls and term or overnight coupons only on
   explicitly selected and qualified terms. Remove the generic strike
   magnitude cap, convention/calendar/lag inference, and silent
   underlier changes. A named convenience convention must resolve the
   whole contract. A reset can occur before exercise: keep authoritative
   observations and require quote compatibility with any seasoning.
   [Strata resolved swaption terms](https://strata.opengamma.io/apidocs/com/opengamma/strata/product/swaption/ResolvedSwaption.html).

2. **Derive the forward from the actual matched cash basis.** On the
   same cut, with dated factors on one qualified discount source, set
   `A_curve=Σ_i α_i D(V,p_i)` on eligible fixed payments. Compute the
   receive-floating leg, retaining signed coupon amounts, through the
   accepted IRS term or overnight law, including spread.
   Set `F=PV_float/(N A_curve)` before payer/receiver direction.
   Require a positive representable annuity; an absent eligible quote
   annuity is unavailable. Preserve failed arithmetic and missing
   required observations as errors. Remove the tiny-annuity forward
   zero and the matching-ID/missing-forward shortcuts. Any internal
   telescoping optimization must prove equivalence on an expressly
   shared evaluator, including contiguous benchmark/accrual boundaries,
   payments, eligibility, fixings and spread. For a qualified zero-spread
   quote mapping only, derive the same-cut spread contribution
   `c=PV_spread/(N A_curve)` from eligible floating cash, and use
   `F_q=F−c, K_q=K−c` for the quote. Otherwise require a source matching
   the actual contract. Preserve contractual rates for settlement and
   explicitly qualify the spread mapping for the chosen settlement and
   quote law. [Strata par-rate calculation](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/swap/DiscountingSwapProductPricer.html),
   [QuantLib's zero-spread quote mapping](https://github.com/lballabio/QuantLib/blob/master/ql/pricingengines/swaption/blackswaptionengine.hpp).

3. **Require one explicit settlement profile.** `Physical` uses
   `A_curve` for live quote pricing and identifies the actual IRS
   deliverable at the authored strike. Cash profiles require explicit
   contractual determination and payment dates, with determination at or
   after exercise and payment no earlier than determination. Support
   `CollateralizedCashPrice` with the same live `A_curve` only for a
   qualified matched collateral basis. A payment-date settlement annuity
   `Σ_i α_i D(P,p_i)` on the same eligible payments and coherent relative
   factors satisfies `D(V,P) A_P=A_curve`; discount once. Require
   those payments to remain eligible on the settlement basis. Bind the
   exact collateral and settlement-rate method. An alternative curve or a
   determination-date value carried to a different payment date needs
   its own qualified law; unsupported cases are unavailable. The
   realized cash obligation comes from the authorized settlement
   determination. Remove the generic cash default, the `IsdaParPar`
   alias for curve annuity and the `ZeroCoupon` maturity-lump proxy.
   Do not infer a legal settlement method from these formulas.
   [ISDA cash settlement methods](https://www.isda.org/a/ecigE/Cash-Settlement-Methods-cheat-sheet.pdf),
   [ISDA confirmation timing example](https://www.isda.org/a/LbigE/20211001-MarkitServ-2021-ISDA-Definitions-Implementation-Working-Group-Session-5-Updated.pdf),
   [ARRC on physical delivery and collateralized cash price](https://www.newyorkfed.org/medialibrary/Microsites/arrc/files/2020/ARRC_Swaption_Consultation.pdf).

4. **Use a dated flat-yield ParYield quote approximation.** Declare the
   profile, settlement annuity anchor, periodic compounding frequency
   `m` as a positive integer, yield clock and payment date `P`.
   Take `τ_i>0` from the declared annuity anchor to each eligible
   fixed payment on that clock and use actual fixed accruals `α_i`.
   Evaluate the pre-exercise settlement yield at the unshifted
   contractual forward `F` for this profile. With `q=1+F/m>0`, use
   `A_cash=D(V,P) Σ_i α_i q^(−m τ_i)` with checked stable powers,
   exact zero-yield accrual sum and positive representable result.
   Admit stubs and lags where the profile supplies their exact accruals
   and times. The regular geometric reduction is equivalent only on
   the actual regular grid `α_i=1/m, τ_i=i/m`. A different yield
   convention or unsupported basis is unavailable. This freezes the
   settlement yield at its forward value for the live quote price;
   it is neither a stochastic cash-annuity expectation nor authority
   for the realized settlement amount.
   [Henrard, Swaption Pricing, §5](https://quant.opengamma.io/Swaption-Pricing-OpenGamma.pdf),
   [Strata par-yield quote pricing](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/swaption/VolatilitySwaptionCashParYieldProductPricer.html).

5. **Select the registered quote law and its exact inputs.** Explicitly
   choose Normal, Black76 unshifted or Black76 shifted. Use one typed
   annual scalar plus its complete clock and applicable displacement,
   or one exact compatible grid, SABR cube or qualified slice. Authored
   SABR requires a deliberately selected complete qualified source,
   including underlying and clock. Require the source's actual
   underlying convention, expiry coordinate, strike and
   tenor or moneyness as applicable, and the declared source
   boundary. For a standard month-based tenor coordinate, verify the
   contractual nominal term against the resolved unadjusted references
   and roll, and convert the canonical months to years only under the
   source's convention. A bespoke term needs an explicit qualified
   mapping. Remove the month-difference heuristic and nearest/default
   coordinate guesses. A scalar quote uses no source tenor or same-ID
   market artifact. Keep SABR construction forwards separate from the
   pricing forward and declare the approved frozen or live-rebased smile
   policy. Preserve the selected output family and actual shift;
   a Normal quote may retain SABR shift provenance without shifting the
   Normal payoff. Pass the complete resolved quote into pricing and
   Greeks without stripping it to a bare sigma and resolving again.
   Use the source origin and actual expiry interval, with
   a total-variance bridge only for a qualified common physical
   interval. Bridge evaluated sigma, never SABR parameters. Remove
   the presence-driven override/SABR/surface ladder, implicit shifted
   promotion, and default clamping.
   [Shared model inputs](../valuations-framework/model-resolution.md),
   [SABR source rules](../core-market/sabr-cube-data.md),
   [dated volatility clocks](../conventions/0045-financial-clocks-and-date-boundaries.md)
   and [Strata swaption volatility inputs](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/swaption/SwaptionVolatilities.html).

6. **Price the live holder option in native currency.** Use the checked
   unit law `U(F_q,K_q,σ,T)`, call for payer and put for receiver,
   and multiply once by `N A` on the selected settlement profile.
   Normal admits signed coordinates; Black requires both effective
   coordinates strictly positive after the selected displacement.
   Validate even on the admitted zero-volatility intrinsic branch.
   Require an admitted positive clock for a live option, including a
   qualified intraday clock before a same-day exercise cut. If only
   a date is available for that phase-sensitive valuation, return
   unavailable. Pricing never rounds the forward or strike onto a
   different contract. Preserve checked raw PV and finalize through the
   common Money boundary. [Checked forward kernels](../models-analytic/forward-normal-shifted.md).

7. **Keep exercise, lapse, cash receipt and delivery distinct.** Do
   not assign a date-only expiry intrinsic or a blanket zero after expiry.
   A separately qualified exercise-value calculation may use known
   exercise inputs at the cut; it does not authorize exercise, lapse or
   payment. At and after the decision cut require authoritative state.
   An exercised physical claim transfers the exact executed IRS once
   to the lifecycle owner. An exercised cash claim requires the
   authorized settlement amount, contractual payment date and
   entitlement, discounted on its qualified curve while unpaid
   (factor one at an included same-day payable cut). An overdue
   unresolved claim requires a qualified outstanding-claim treatment;
   calendar passage alone does not prove receipt. Prove extinguishment
   or successful handoff before reporting zero for the option scope.
   Post-exercise holder value and risk belong to the surviving
   settlement or delivered-swap owner and must not be counted twice.
   Missing required state stays unavailable or errors as appropriate.
   Horizon risk across exercise under
   [framework 13](../valuations-framework/carry-theta-breakeven.md)
   requires a qualified election, delivery or cash scenario; rolling
   the date alone supplies none.
   [Checked lifecycle](../valuations-framework/pricing-lifecycle.md).

8. **Compute local forward and quote-volatility risk on the live price.**
   Hold the evaluated sigma, quote translation, shift, expiry, discount
   sources and other terms fixed. Move the decimal forward and report
   native currency per one decimal rate and per its square. Use
   `N(A U_F + A′ U)` and `N(A U_FF + 2 A′ U_F + A″ U)`. Curve
   annuity is held; ParYield follows the same adopted `A(F)`, with
   `A′=−D Σ_i α_i τ_i q^(−m τ_i−1)` and
   `A″=D Σ_i α_i τ_i(τ_i+1/m) q^(−m τ_i−2)`. This measures the
   derivative of the declared quote pricing formula. Do not include a
   SABR smile response in the local result. Quote vega holds the forward
   and annuity, uses `N A U_σ`, and includes the applicable
   `sqrt(T_source/T_model)` bridge Jacobian. Report currency per `.01`
   relative Black or shifted Black sigma, or a required explicit
   absolute annual Normal increment (e.g. `.0001`). Apply the unit
   once. Keep unsupported and live singular kernel Greeks unavailable.
   Local option exposures stop on a proved successor branch; its
   outstanding cash or swap exposures may remain.
   [Shared option sensitivities](../valuations-framework/option-sensitivities.md),
   [Henrard, §§6.1–6.2](https://quant.opengamma.io/Swaption-Pricing-OpenGamma.pdf).

9. **Separate curve risk, source shocks, replay and Hull–White.** Use
   `dv01` for combined direct, `pv01` for exact typed per-source direct
   rows and their checked sum, and `bucketed_dv01` for triangular
   direct key-rate risk, all signed per positive bp. Shock a proved
   shared evaluator once through all of its roles. Retire the generic
   discount-DV01-as-`rho` alias. Use checked comparable full raw
   reprices; rebuild forward, settlement annuity and declared
   quote-coordinate mapping, retaining the active sigma source and
   explicitly identifying any qualified held-evaluated-sigma
   experiment. Grid risk needs its own exact native directions and
   (for bucketed vega) the matching framework 11 comparator. Rate
   quote or volatility calibration replay requires a separate explicit
   request, qualified center, exact targets and complete sides.
   European Hull–White exercise, parameter domain, selected-model
   curve Greeks and short-rate sigma vega remain with
   [record 08](08-hull-white-swaptions.md). They cannot substitute for
   forward partials or quote vega. Apply the shared represented-step
   rule to that owner; do not implicitly shrink sigma bumps to half the
   live sigma. Bermudan and LMM routes retain their separate owners.
   [Shared rate risk](../valuations-framework/finite-difference-rates.md).

10. **Invert the live gross premium on the selected quotation.** Use
    one standalone `implied_vol` operation in Rust, Python and WASM.
    Require the actual European settlement profile, prepared live
    forward and strike, positive horizon and `W=N A`, explicit quote
    family, clock and applicable shift, and finite nonnegative
    same-currency gross as-of `Money`. Freeze `W` and all inputs
    other than quote sigma. This experiment needs neither a prevailing
    market sigma nor a SABR/Hull–White fit. Validate Black's exclusive
    gross upper bound first, `W(F_q+shift)` for payer or
    `W(K_q+shift)` for receiver. Normal has no finite model upper
    bound. Reject below intrinsic; only exact intrinsic on an eligible
    live contract can return zero. Fixed, exercised, lapsed and
    nonpositive-horizon routes have no identifiable quote inverse.
    Require `max_vol`, `vol_tol`, `max_iterations`, native-currency
    `price_abs_tol` and dimensionless `price_rel_tol` under the shared
    `IvControls`. Use log-OTM Brent on `[0,max_vol]`. Compare original
    cash and separately
    gate gross PV and independent OTM time value on the actual returned
    annual sigma with absolute currency and relative tolerances.
    Preserve typed precision, bracket, search and repricing errors.
    Remove hardcoded ceilings, expiry-zero and the swaption's
    baseline-dependent generic implied-volatility metric and override
    route. Keep other products' routes and fitted model calibration
    with their owners. [Accepted bounded inversion](../models-analytic/implied-volatility-solvers.md).

11. **Reconcile the existing producers and consumers.** Use the shared
    prepared schedules, source and settlement semantics across native
    construction and restore, raw and registered pricing, cash
    handoff, risk, the Python and WASM bindings, stubs, generated
    schemas and parity ledger. Coordinate contractual tenor, forward
    and settlement metadata with [swaption forward calibration](../calibration-targets/13-swaption-forwards.md),
    [cube publication](../calibration-targets/14-swaption-cube.md) and the
    [Hull–White quote adapter](../calibration-rate-models/03-atm-swaption-quotes.md).
    Preserve the exact active dependencies, actual basis and
    unavailable/error states in reports. A placeholder cash export
    gives no evidence of settled or zero cash.
    Independently qualify payer/receiver, multi-curve spread/seasoning,
    actual stubs and payment lags, exact quote/collapse boundaries,
    live versus delivered/outstanding claims, cash and curve Greeks,
    active-source risk and repricing, and the two cash gates of the
    inverse. Same-kernel vendor-named tests or roundtrips cannot
    establish independent market or settlement qualification.

## Observed behavior

### Choice, scope and rationale

European swaptions use a Black/shifted-Black or Bachelier option on a projected par swap rate, multiplied by notional and a settlement-specific annuity. `vol_model` sets the default model; explicit Black/Normal pricers reject mismatch. SABR supplies a quote when parameters exist and there is no implied-vol override. HW tree and LMM paths are separate records. [ADR 0029](0029-rate-option-exercise-and-volatility.md) is the existing selection draft.

### Algorithm and conventions

1. Require positive notional, expiry no later than start, ordered underlying dates, matched fixed/float spans, finite strike magnitude at most 2 and valid legs/SABR params. Resolve leg sentinels/EOM with IRS helpers. Standard convenience underlier uses semiannual 30/360 fixed, quarterly ACT/360 float, ModifiedFollowing/no stub, zero lags; registered overnight ID can select compounding.
2. Fixed annuity is unpaid `sum(alpha_i*D_i)`. In the tightly matched single-curve case use discount-ratio coupons on the fixed schedule. When same-ID simple projection has no forward object, calculate discount-ratio coupons on the float schedule including spread. Otherwise price the floating leg of a zero-fixed-rate IRS and divide by N and fixed annuity. Near-zero annuity returns forward zero.
3. Physical, CollateralizedCashPrice and IsdaParPar use curve annuity. CollateralizedCashPrice is the default cash-settlement method. ParYield uses a regular-frequency geometric annuity based on period count and forward rate, discounted to underlying start; actual stub accrual sizes are not individually used in that formula. ZeroCoupon uses fixed-day-count total tenor times terminal DF.
4. Resolve volatility at ACT/365F expiry, contractual underlying tenor and strike. The tenor helper uses the positive calendar-month difference divided by12, ignores day-of-month and rejects spans with no positive month difference. Implied-vol override wins; otherwise embedded SABR produces Normal or Black quote consistent with vol_model, including its shift. Otherwise resolve surface/provider quote; clamp unless model extrapolation is Error. Apply displacement to both F and K; no conversion between incompatible Normal and Black sources.
5. Return `N*A*Black_or_Normal(F,K,sigma,T)`. Exactly at expiry return intrinsic times settlement annuity; after expiry return zero even for physical settlement. The object does not carry a post-exercise delivered swap or settlement claim.

### Risk and inverse solve

Analytic Delta/Gamma/Vega multiply kernel Greeks by N and the **held-fixed** annuity; Vega divides by 100. They do not differentiate ParYield annuity dependence on F or SABR smile dependence on F. HW Delta/Gamma use shared repricing; HW quote Vega errors and directs the caller to HwSigmaVega, which centrally bumps explicit sigma by `min(shared_bump,sigma/2)` and scales to 0.01. Generic combined DV01, discount-only Rho and triangular bucketed DV01 remain full repricing channels.

ImpliedVol solves quoted nonnegative premium divided by notional with Brent tolerance `1e-12`, bracket `[0,1]` Normal or `[0,5]` Black; it validates price at sigma zero first and requires final PV residual within `1e-8*max(abs(target),1)`. At/after expiry it returns zero. This solves a constant quote vol, not SABR parameter calibration.

## Evidence

Cash-settlement annuities are materially distinct supported branches; equality of IsdaParPar and curve-annuity behavior is observed and must not be inferred to cover all possible market settlement definitions. Full cash-annuity stochastic expectation is not implemented by the closed-form multiplier. [Implementation](../../../../finstack-quant/valuations/rates/src/swaption/types/swaption.rs), [settlement definitions](../../../../finstack-quant/valuations/rates/src/swaption/types/definitions.rs), [model dispatch](../../../../finstack-quant/valuations/rates/src/swaption/pricer.rs), [Delta](../../../../finstack-quant/valuations/rates/src/swaption/metrics/delta.rs), [Gamma](../../../../finstack-quant/valuations/rates/src/swaption/metrics/gamma.rs), [Vega](../../../../finstack-quant/valuations/rates/src/swaption/metrics/vega.rs), [implied vol](../../../../finstack-quant/valuations/rates/src/swaption/metrics/implied_vol.rs), [metric registration](../../../../finstack-quant/valuations/rates/src/swaption/metrics/mod.rs).

[Forward/annuity tests](../../../../finstack-quant/valuations/tests/instruments/swaption/pricing/forward_annuity.rs), [QuantLib parity](../../../../finstack-quant/valuations/tests/instruments/swaption/integration/quantlib_parity.rs), [implied-vol tests](../../../../finstack-quant/valuations/tests/instruments/swaption/metrics/implied_vol.rs) are located formula/fixture/roundtrip evidence, not executed. The test file labelled QuantLib parity constructs a local Black expectation with production annuity/CDF helpers; its name does not establish independent vendor agreement. Inline analytic registry tests in `pricer.rs` compare default/explicit PV and raw-value paths, including SABR. References: [Black](../../../REFERENCES.md#black-1976), [Bachelier](../../../REFERENCES.md#bachelier-1900), [SABR](../../../REFERENCES.md#hagan-2002-sabr), [ISDA 2021](../../../REFERENCES.md#isda-2021-interest-rate-derivatives-definitions).

The existing [registry](../../../../finstack-quant/valuations/core/src/pricer/registry.rs)
prices a base PV before running the inversion metric. The
[shared quote resolver](../../../../finstack-quant/valuations/core/src/instruments/common_impl/vol_resolution.rs)
can probe a same-ID source even with a scalar override. Existing
adapters include the [typed Python swaption](../../../../finstack-quant-py/src/bindings/valuations/typed_rates.rs)
and [typed WASM swaption](../../../../finstack-quant-wasm/src/api/valuations/typed/rates.rs).

### Independent review evidence

- With fixed accruals `.48,.52`, floating accruals `.47,.53`, payment
  factors `.97,.94`, and floating benchmark rates `.04,.05` plus
  `.001` spread, the curve annuity is `.9544` and floating PV per
  unit notional is `.0441001`. The forward is `.046207145851`.
  An unjustified start/end discount shortcut with `.99,.94` gives
  `.052388935457`.
- With `m=2`, `F=.045`, `D(V,P)=.98`, accruals `.25,.5,.5` and
  yield times `.25,.75,1.25`, the dated ParYield annuity is
  `1.179692362295`; the three-period regular formula gives
  `1.406249464230`. At zero yield the dated answer is exactly
  `1.225`. Pending authorized cash of `5,900` at `D=.998`
  is worth `5,888.20` after exercise, and `5,900` at an included
  same-day payable cut.
- For a Normal payer with the same accruals and yield times,
  `N=1m,F=.025,K=.020,σ=.01,T=1,D(V,P)=.97`, the ParYield PV is
  `8,284.337833`. Raw forward delta and gamma are approximately
  `813,987.608148` and `40,435,356.276807`; holding the cash annuity
  fixed would instead give `820,913.856204` and `41,797,685.516753`.
  Normal quote vega per `.0001` is about `41.797685517`.
- With half-year fixed and floating accruals, dated factors `.98,.95`
  at shock coordinates one and two years and held projection rates
  `.02,.06`, the starting forward is `.039689119171`. For a
  physical Normal payer with `N=1m,T=1,σ=.01,K=.04`, a central
  discount-only ±1 bp shock gives about `−1.02279103` per bp when
  the forward is recomputed. Freezing the forward instead gives
  `−.55237105` per bp. Discount-only curve risk is not a local
  forward-held discount partial.
- On a 360-day live interval, source ACT/360 Normal sigma `.01`
  bridges to about `.010069204978` on model ACT/365F with the same
  integrated variance `.0001`. A one-day January-to-February
  underlier must not acquire a one-month quote tenor from the
  change in calendar month.
- With exact shifted Black test inputs `W=128,F_q=1/64,K_q=−1/64`
  and `shift=1/32`, the payer and receiver admissible gross cash
  intervals are `[4,6)` and `[0,2)`. A quote at the exclusive
  upper bound cannot be inverted; an otherwise eligible exact lower
  bound has zero IV.

## Relationship to the accepted SABR source boundary

[CORE-MARKET-16's complete accepted package](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning)
requires active cube requests to select Normal/Black/ShiftedBlack explicitly,
retain the actual evaluated displacement and agree with the source expiry clock
and underlying reference. Match the contractual tenor and distinguish stored
construction forwards from the live pricing forward; choose any frozen/rebased
smile model explicitly. Scalar sigma plus explicit model inputs must not inspect
unused same-ID cubes. Record 16 was accepted on **2026-11-08**, with reconciliation/qualification
pending. The accepted product decision above owns settlement, pricing, Greek
availability and the selected smile policy.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
