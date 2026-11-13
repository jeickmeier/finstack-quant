# MODELS-AND-VALUATIONS-CREDIT-15: CDS option forward calibration and spread quadrature

- Decision status: **Accepted — complete checked CDS option valuation package, 2026-11-13**.
- Implementation status: Current behavior observed below. The accepted contract requires Rust, quote, reporting and host reconciliation.
- Validation status: Source, selected tests and primary references inspected. Independent forward, calibration and exercise examples evaluated. Production, host and external-reference qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked CDS option valuation

1. **Require explicit exercise, underlying and default terms.** Select
   a European payer or receiver on a single-name CDS or an untranched
   single-curve index. Require positive original option notional in one
   currency, the exact reference entity or index family and series,
   legal expiry, cash or physical exercise, an explicit exercise
   settlement date on or after expiry, the fixed underlying coupon
   and the resolved
   [CREDIT-12](12-cds-legs.md#accepted-decision-for-checked-cds-legs-and-quote-basis)
   schedule, coverage, documentation clause, recovery and default
   settlement. Index valuation consumes an authoritative
   [CREDIT-14](14-index-valuation.md#accepted-decision-for-checked-cds-index-valuation-and-cashflows)
   exposure snapshot with exact strike/current version bindings and
   a qualified transition on the same original notional basis.
   Admit finite nonnegative coupon bp and recovery in `[0,1)`
   for this model. Select single-name knockout, single-name front-end
   protection with the terms in decision 5, or non-knockout index.
   A single index hazard does not supply the joint event law for
   index knockout. Constituent and stochastic expiry-factor models
   are outside this approximation. Use expiry at valuation close
   on the civil date axis. Keep the exercise settlement date and
   the delivered CDS default-payment lag distinct. Remove Spot/
   Forward and roll-date fallbacks, the vendor valuation default,
   and the coupon inferred from strike.

2. **Use the dated CDS legs to anchor the forward clean value.**
   Value the residual delivered trade conditional on survival at expiry
   close `e`. Require delivered protection and post-expiry premium
   accrual from the next civil day through unadjusted maturity inclusive.
   The first residual period can straddle expiry. Keep its authored
   accrual origin and include the full unearned scheduled coupon and
   covered accrual on default (AoD). Remove completed coupon entitlements
   and separately booked underlying cash from the delivered trade.
   Use local hazard, accrual-end survival and actual payment dates.
   In settlement-date units the clean mark per surviving notional is
   `F=(P_e−c A_dirty,e)/D(e,z)+cα`, where `z` is the exercise
   settlement date and `α` is contractual accrued premium per unit
   coupon through expiry close. This prices the same clean rebate at
   `z` on the underlying and exercise sides. Use the checked
   relative factor directly, and take `D(e,e)=1`. Do not divide
   unconditional survival probabilities or extend maturity by a day.
   If a contract starts accrual at delivery, the corresponding
   pre-delivery rebate is zero. Future delivered PV expressed back
   at valuation is `D(v,z) Q(v,e) F` per live notional. Apply this
   identity consistently when settlement differs from expiry.
   Deferred coverage after the next civil day and unsupported
   coupon gaps require a separate state model.
   [White, sections 3.2–3.4 and 4.2](https://quant.opengamma.io/CDS-Options-OpenGamma.pdf).

3. **Select a bounded continuous-premium state law.** Keep the
   dated curve target and use an explicitly approximate
   spread-dependent expiry mark for diffusion. Let `H>0` be the
   sum of post-expiry premium accrual fractions through maturity.
   For the straddling period use full accrual less the expiry
   accrual on its original clock. Let `g=1−R` and `c` be the
   fixed decimal coupon. At nonnegative state spread `s`, define
   `x=sH/g`, `A(s)=H[-expm1(−x)/x]` and
   `W(s)=g[-expm1(−x)]−c A(s)`. Use `A(0)=H` and
   `W(0)=−cH`. The state proxy uses zero discount, continuous
   premium through default or maturity, and payout at default,
   on the post-expiry accrual clock. The dated forward anchor still
   uses the selected market curves, discrete coupons and settlement.
   The state law includes only post-expiry accrual, so there is no
   detached subtraction that can turn its annuity negative.
   For `c>=0`, protection strictly increases with spread and the
   premium coefficient decreases. The mark increases from `−cH`
   toward `g`, with `g` unattainable at a finite spread. This
   replaces the scheduled-only triangle mark and its multiple-root
   risk. Register the replacement as CDS spread quadrature and
   retire the Bloomberg-named dispatch.
   The published model motivates calibration of a cash-valued forward
   and the continuous-premium approximation.
   [White, sections 6.7 and 7](https://quant.opengamma.io/CDS-Options-OpenGamma.pdf).

4. **Bind the native strike to explicit clean exercise cash.**
   Spread strike is a positive finite decimal spread `K`. Require
   the frozen positive clean strike annuity `A_K` resolved from the
   contractual strike convention and its recorded source. The buyer
   clean payment per strike notional is `(K−c) A_K`. The option
   pricer consumes that bound input and does not derive it from
   the stochastic state proxy or re-fit the strike under a market
   shock. For single-name the strike notional factor is one. For
   index spread strikes require the contract's fixed strike factor
   `f_0`. Dynamic expiry-factor strike terms are outside this
   model. Clean-price strike is supported for non-knockout index
   only. Admit positive finite clean price in percent of par
   (including above 100) and convert exactly once. The clean
   payment per original notional is `G=f_0(1−K_pct/100)`. For index
   spread strikes it is `G=f_0(K−c) A_K`. In physical exercise the
   common accrued rebate is included in the actual settlement
   cash under the resolved contract. Do not charge it twice in the
   clean option payoff. The option pricer has no universal 10%
   strike cap. An exercise amount or cash basis outside the
   supported domain fails explicitly.
   [White, section 4.2](https://quant.opengamma.io/CDS-Options-OpenGamma.pdf).

5. **Distinguish single-name and index front-end protection.**
   For valuation close `v` and expiry close `e`, compute survival
   `Q=exp(−J(v,e))` from local integrated hazard. For a granted
   front-end window, require its explicit first covered day and
   end it at expiry close. The first remaining day is the later
   of the authored first day and `v+1`. If it is later than
   expiry, its mass is zero. Otherwise, with `l` the close of
   the preceding day, use
   `p_FE=exp(−J(v,l))[-expm1(−J(l,e))]`. For full remaining
   coverage this is the stable conditional PD through expiry.
   A single-name survival branch calibrates to `E[W]=F`, then
   pays `N D(v,z) Q E[(ξ(W−G))^+]`, with `ξ=+1` payer and
   `−1` receiver. Knockout has no pre-expiry event cash. In the
   selected single-name front-end style, a covered pre-expiry
   default grants the payer `N D(v,z) g p_FE` separately with
   no strike charge, while the receiver has no claim on that
   branch. A default branch with a separate strike charge would
   be a different contract.

   For a non-knockout index with current surviving factor `f>0`,
   calibrate the per-current-notional mark to
   `E[W]=Q F+g p_FE`. Absorb expected future front-end loss once
   into that target. Keep confirmed, exercise-entitled realized
   loss `L` in original-notional units as a separate term inside
   exercise. Pay
   `N D(v,z) E[(ξ(f W−G+L))^+]`, with no extra outer survival.
   Require `0<=f<=f_0<=1`, `f_0>0`, and
   `0<=L<=f_0−f` on one coherent exact factor/loss basis.
   Validate these relations before rounding any division or
   transformation. Loss already paid separately under the option
   entitlement is excluded. An auction settlement on the underlying
   can still establish a loss the option owes. The factor never
   stands in for the confirmed loss. Fully depleted `f=0` skips
   the live model and uses deterministic `N D(v,z)[ξ(L−G)]^+`
   with complete entitlement state. No factor division, floor,
   joint-default or future stochastic factor claim.
   [White, sections 4.3 and 6.3–6.8](https://quant.opengamma.io/CDS-Options-OpenGamma.pdf).
   [S&P DJI, price-strike example](https://www.spglobal.com/spdji/en/documents/campaigns/cds-indices-primer.pdf#page=26).

6. **Use a volatility input bound to this model.** The live clock is
   ACT/365F from valuation close to legal expiry. Select an explicit
   annual decimal volatility or a checked, identity-bound
   Black-lognormal Strike surface quoted for this spread law.
   Require underlying/tenor and strike-unit compatibility and the
   source reference date at valuation. Query the surface at its
   own checked expiry coordinate and native strike in decimal spread
   or clean-price percent. Both strike conventions quote volatility
   of the model spread. Preserve total variance when
   bridging a supported source clock to the model clock. Extrapolation
   and unsupported clocks fail. Admit zero volatility. Retain annual
   model volatility at most 5 as an explicit operational domain,
   with no positive floor. A deterministic outcome with no live
   state does not query volatility. Old model or vendor vol quotes
   require explicit qualification or reinversion under the new law.
   [Volatility sources](../core-market/volatility-grids-and-shocks.md).

7. **Calibrate a unique spread location and check the returned fit.**
   For positive volatility use
   `log S=log m−a²/2+a Z`, `a=σsqrt(T)`, and `Z∼N(0,1)`.
   Form the log state directly with checked arithmetic. Evaluate the
   bounded state using stable small-argument ratios and checked
   large-argument limits, without exponentiating an unneeded
   spread. For the selected settlement-unit target `F_*`,
   admit `−cH<=F_*<g`. The exact lower bound has deterministic
   zero spread. For an interior target the expectation is strictly
   increasing with `m`, so the calibration has at most one root
   and its exact unbounded mathematical range spans the interior.
   Use the checked
   [Brent solver](../core-numerics/brent-root.md) on a supplied
   bracket of log locations `[−744,709]` with at most 100 iterations
   and `1e−12` coordinate tolerance. These are numerical work
   limits. Normalize the mean expectation and residual by
   `M=max(cH,g)`. Use the integration budgets in decision 8,
   require a resolved sign at each bracketing endpoint relative to
   those budgets, and accept an endpoint itself only if its
   recomputed fit passes. Recompute the selected fit and require
   normalized absolute residual plus the interior budget and
   omitted-tail bound at most `1e−10`. Reject unattainable targets,
   unbracketed cases, nonfinite states and exhausted work.
   For exact zero live variance use the admitted target directly
   in the payoff, with no calibration or quadrature. Retire the
   arbitrary distressed forward cap and first-sign-change search.
   Quote forward value, any forward par display, strike cash and
   calibrated spread have distinct meanings.

8. **Use one bounded integration method for the mean and payoff.**
   Integrate the standard normal density times the checked mark
   or payoff on `[−8,8]`. Normalize each by its analytic absolute
   support bound, using the maximum magnitude over the mark or
   payoff endpoints. A zero bound is an exact zero. The normalized
   omitted mass is at most `2 Φ(−8)<1.25e−15`. Use checked
   [adaptive Simpson](../core-numerics/quadrature.md), starting with
   panels of width at most .5 and splitting at the unique exercise
   crossing when inside the grid. The crossing is a checked
   bracketed inversion of the same monotone cash mark in the log
   spread interval mapped from the retained grid. Use the same
   Brent coordinate tolerance and 100-iteration limit. Determine
   exact zero or linear payoff cases from the analytic bounds.
   Split the absolute normalized estimator budget `1e−12`
   across panels, with maximum depth 32 per panel and at most
   100,000 total integrand evaluations per expectation. Accumulate
   in increasing panel order. The interior estimator is operational,
   while the normal tail allowance bounds omitted mass under the
   stated model. A positive payoff beyond the retained grid
   remains a tail approximation. Identify this outcome and its
   allowance, including when the retained estimate is zero.
   Do not label it an exact zero or claim relative tail accuracy.
   Evaluate final notional, factor, survival and discount products
   with scaling to preserve representable monetary results.
   Propagate errors and reject unresolved required arithmetic,
   invalid crossing geometry or failed budgets. Check payer/receiver
   parity on matched inputs. For the index the difference is
   `N D(v,z)(f F_*−G+L)`. For single-name knockout it is
   `N D(v,z) Q(F−G)`, with the admitted front-end claim added
   for the payer where applicable. Compare numerical parity residual
   with the combined integration allowances and accepted forward
   fit residual, in common units. Use a scaled calculation for any
   linear payoff over the entire support.

9. **Separate live option value from confirmed exercise and settlement.**
   Live diffusion pricing applies strictly before legal expiry to
   an eligible underlying. A confirmed earlier single-name default
   routes to resolved knockout or confirmed option event cash under
   the selected terms. At and after expiry require authoritative
   exercise status and actual signed dated cash when entitled.
   Cash settlement needs a confirmed amount or explicit expired
   without exercise status. Physical settlement uses the resolved
   delivered CDS and eligible exercise cash through the existing
   product owners. The option stops representing those rights once
   transferred. The live physical value uses the modeled clean
   economic equivalent only for admitted delivery terms. Do not
   guess an expiry fixing, repeat confirmed cash, infer default
   from rounded survival, or automatically price an expired
   contract at zero. Same-day cash entitlement is explicit under
   [CORE-CONVENTIONS-016](../core-conventions/016-discounting.md).
   Already settled cash is history. An overdue unresolved claim
   belongs to explicit settlement state.

10. **Share checked Rust admission, raw premium and provenance.**
    One validated immutable contract and state path serves
    direct, registry, Python and WASM routes and strict restore.
    Expose one checked raw option value, with Money finalization at
    the reporting boundary. Record selected exercise/default and
    strike basis, dates, active sources, forward target, model vol
    basis and numerical qualification outcomes. Align dependencies
    with actual branches. Reconcile host declarations, parity,
    generated schemas, documentation, fixtures and teaching
    examples. Greek conventions, bump policies and implied-vol
    inversion and branch handling belong to
    [CREDIT-16](16-cds-option-risk-and-inversion.md), which must
    consume the selected pricing basis and lifecycle. Independent
    model, market and host qualification remain implementation work.

## Observed behavior

### Choice and rationale

BloombergCdso dispatch uses a lognormal spread distribution with an endogenous flat-hazard annuity and mean calibrated to the curve-implied forward CDS value. The Bloomberg label identifies the intended convention; independent vendor parity is not established by the label.

### Scope and input domain

European payer/receiver options on single-name or index CDS. Spread strikes must be positive decimal spreads; clean-price percent strikes require a non-knockout index, explicit coupon and original strike factor. Current index factor is positive and at most one; price-strike realized losses cannot exceed removed original notional. Physical delivery is valued only before expiry.

### Algorithm

1. Build a synthetic protection-buyer CDS using BloombergCdswClean, explicit underlying start or Spot/Forward convention, and current index-factor notional. Obtain scheduled premium accruals after expiry, relative DFs, conditional survival and the previous-coupon-to-expiry accrual stub. The forward clean annuity is the scheduled-coupon PV less the expiry stub, divided by discounted surviving notional (or no survival divisor for non-knockout index options). Forward protection extends the synthetic maturity by one day.
2. At spread state s use credit-triangle hazard s/LGD and `L(s)=Σ accrual_i exp(−s t_i/LGD) forwardDF_i − accruedStub`. Forward swap value is `V(s)=(s−coupon)L(s)`.
3. Front-end protection is LGD times conditional default probability from as-of to expiry. Index forward spread includes this once through FEP/bootstrapped-annuity. Solve `E[V(S)]=(forwardSpread−coupon)×bootstrappedAnnuity`, with `S=m exp(−σ²T/2+σ√T Z)`. Normal trapezoid quadrature calibrates m; log-m bracketing doubles around the forward spread, at most 30 expansions within [1e−8,100], and Brent solves to 1e−12. Forward spread above .10 is rejected.
4. Integrate `max(sign×V(S)+strikeAdjustment+realizedLossAdjustment,0)`. Spread adjustment is sign×(coupon−strike)×L(strike); price adjustment is sign×(price/100−1)×originalFactor/currentFactor. Realized index loss contributes sign×loss/currentFactor separately.
5. Payoff expectation uses adaptive Simpson with approximately half-unit normal panels, total tolerance 1e−12 and depth 32. Multiply by current factor and survival-to-expiry for knockout or single-name options. Non-knockout single-name payer FEP is added outside the exercise payoff. Discount all option settlement amounts to exercise settlement date (expiry if absent).

### Selection and controls

Only BloombergCdso is family-registered. Volatility is explicit implied-vol override or BlackLognormal Strike-axis surface at native strike; expiry uses ACT/365F. Volatility ceiling is 5. Normal integration tails are max(6,4σ√T); mean calibration uses fixed step .05 from [Bloomberg CDSO constants](../../../../finstack-quant/valuations/core/src/constants.rs). Default protection convention is Spot, settlement Cash, knockout false, index factor one; single-name coupon defaults to spread strike. Explicit exercise-settlement/underlying-start dates override defaults.

### Conventions

Spread strike is decimal, coupon bp, clean price percent of par, realized loss fraction of original notional, and volatility annualized decimal. Call means payer and Put receiver. Forward annuity time uses days/365; cash settlement delay changes discounting without changing legal expiry or volatility time.

### Limitations, failure and fallback

Nonpositive forward annuity, tiny LGD, failed calibration bracket and distressed forward fail. Survival ratios are clamped, warning if materially nonmonotone. Flat-annuity states can become negative when a positive stub is subtracted at high spreads; the numerical and financial accuracy of these states is not established by the calibration domain limits. Post-expiry pricing errors; physical settlement at expiry errors because delivery state is absent. No automatic standard-CDS Black-76 fallback exists.

### Alternatives and tradeoffs

Constant-annuity Black-76, normal spread or default-state models are possible alternatives, not selected pricers. The implemented annuity-aware payoff accounts for spread-annuity dependence at the cost of nested calibration and a restricted distressed domain.

## Evidence

The [quadrature kernel](../../../../finstack-quant/valuations/credit/src/cds_option/bloomberg_quadrature.rs),
[pricer](../../../../finstack-quant/valuations/credit/src/cds_option/pricer.rs),
[contract](../../../../finstack-quant/valuations/credit/src/cds_option/types.rs),
[parameters](../../../../finstack-quant/valuations/credit/src/cds_option/parameters.rs)
and [strike](../../../../finstack-quant/valuations/credit/src/cds_option/strike.rs)
own the observed behavior.
[Python](../../../../finstack-quant-py/src/bindings/valuations/typed_credit/cds_option.rs),
[WASM](../../../../finstack-quant-wasm/src/api/valuations/typed/cds_option.rs)
and the [index credit lesson](../../../../docs-site/content/learn/C/C4-index-credit-cdx-tranches-and-credit-index-options.mdx)
expose the current product and model. The inline tests and
[parameter tests](../../../../finstack-quant/valuations/credit/tests/cds_option/test_parameters.rs)
locate regression evidence. The
[Bloomberg reference](../../../REFERENCES.md#bloomberg-cdso)
identifies the intended source for the historical implementation.
Production tests and vendor comparisons were not run.

### Independent review evidence

- The observed scheduled-only toy mark `(s−.01)exp(−5s/.6)`
  has two spreads at mark .025, about .04697993 and .28984186.
  With one future coupon of accrual .25, time one, LGD .60
  and detached accrued stub .125, annuity at spread .60 is
  about −.03303.
- For the proposed state with `H=5,g=.60,c=.01`, total
  volatility .40 and target .04, independent composite
  Simpson integration and bisection give mean .01888442580743.
  With fixed clean strike cash .04605548253282 per unit,
  payer and receiver expectations are .01070033002798 and
  .01675581256080. Their difference matches target less strike
  cash. The two-sided normal tail mass beyond six standard
  deviations is about 1.973e−9. Beyond eight it is about 1.244e−15.
- With expiry discount .98, settlement discount .95, survival .90
  and valuation-date clean PV .031 per unit, settlement forward
  .03625730994152 recovers .031 under settlement discount and
  survival. Expiry normalization followed by settlement
  discounting instead gives .03005102040816.
- A deterministic single-name payer with 10 million notional,
  hazard integral .02, LGD .60, settlement discount .98,
  surviving mark .025 and strike cash .01 has survival-branch
  value about 144,089.20. If the terms grant the separate full
  remaining front-end claim, that branch adds 116,431.80.
- With index `f=.80,f_0=1,L=.06`, a clean price strike of 95%
  and deterministic loss-adjusted mark .04, payer exercise cash
  is .042 per original notional. At .95 settlement discount
  and 10 million original notional, premium is 399,000.
  With strike 107%, current factor .99 and eligible realized
  loss .009075, the
  [published strike inputs](https://www.spglobal.com/spdji/en/documents/campaigns/cds-indices-primer.pdf#page=26)
  imply an equivalent live clean strike of 107.9873737%.
  The original cash equation remains usable at very small or zero factor.
- These calculations check model choices, signs and numerical
  boundaries independently. Production work includes model-premium
  and strike data, tail behavior, dated settlement and host parity.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
