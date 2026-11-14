# VALUATIONS-RATES-11-CMS-CONVEXITY: CMS reference swaps, convexity approximation and local risk

- ID: VALUATIONS-RATES-11-CMS-CONVEXITY
- Decision status: **Accepted — complete checked CMS convexity pricing and risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, model and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked CMS convexity pricing and risk

1. **Bind the exact CMS benchmark and reference swap.** Require a
   configured CMS fixing `IndexId` with its currency, exact positive
   month/year tenor, benchmark conventions and dated publication
   setting. Keep it distinct from the reference swap's floating
   `IndexId`, projection evaluator and reference discount evaluator.
   Keep the CMS payment discount and funding roles explicit as well.
   Sharing needs an actual qualified typed evaluator; equal IDs are
   insufficient. Use complete authored terms or one qualified dated
   convention edition. An override must agree with the benchmark and
   active quote's underlying; do not synthesize a fixing key from
   a curve ID or approximate years. Remove currency-selected reference
   indexes, inferred leg conventions and same-ID curve substitution.
   [Dated conventions](../valuations-framework/convention-selection.md),
   [checked fixing sources](../core-market/scalar-series-and-fixings.md),
   [ICE Swap Rate methodology](https://www.ice.com/publicdocs/ICE_Swap_Rate_Calculation_Waterfall_Methodology.pdf).

2. **Resolve the reference IRS on its actual cash basis.** Advance
   the contractual CMS observation date by the selected settlement
   business-day rule and resolve the maturity from the exact nominal
   tenor and roll. Prepare both leg grids with their own accrual,
   day-count references, stub, roll, calendar, reset, payment and lag
   rules. On the common eligible cash cut set
   `A_ref=Σ_j α_j D_ref(V,p_j)` for the fixed PV per unit rate and
   notional. Set `F=PV_float_per_unit/A_ref` using the actual
   qualified term or overnight IRS coupon law and exact active
   sources. A floating frequency by itself does not determine the
   benchmark period or qualify an overnight factor-ratio shortcut.
   Require positive representable `A_ref`; a missing or degenerate
   underlying has no forward on this basis, and failed calculations
   propagate. Keep this actual curve annuity separate from the
   flat-yield approximation below. [Accepted IRS terms](03-interest-rate-swaps.md),
   [Strata CMS period terms](https://strata.opengamma.io/apidocs/com/opengamma/strata/product/cms/CmsPeriod.html).

3. **Prepare the CMS and funding claims before resolving market data.**
   Require exact positive Money notional and finite signed rates,
   strike, cap, floor and spreads on their declared Decimal rate or
   bp bases. Require `floor≤cap` where both are present. Use validated
   period rows with actual accrual endpoints, positive contractual
   accrual, observation and payment dates and day-count context.
   Separate CMS reset, reference start and coupon payment. Resolve
   funding periods independently; do not copy CMS frequency, inject
   generic lags or use payment dates as accrual boundaries. Fixed
   funding needs no reset. Simple floating funding uses its own
   index, actual benchmark interval and accrual, publication authority
   and exact projection role, plus the contractual outside spread.
   Use the explicit holder payment cut; include future eligible
   payments and use factor one for explicitly included known same-day
   payable cash. Require authorized determination before payable
   cash. A settled or unentitled claim does not require inactive data.
   An overdue unpaid amount requires qualified outstanding-claim
   treatment before valuation or export.
   [Schedule terms](../core-conventions/008-schedule-generation.md),
   [funding and fixing law](../cashflows/06-floating-rate-projection.md).

4. **Use publication authority to separate observed from live.** Use
   the exact eligible fixing and admitted publication version, also on
   a same-day cut where it is already published. A required missing
   fixing errors. A same-day or otherwise unpublished fixing may
   project only when permitted and when the active source and model
   can represent a qualified positive remaining interval. Otherwise
   the live result is unavailable. Date equality cannot turn the
   projected forward into known intrinsic or set option time to zero.
   An observed claim uses its literal signed rate with intrinsic
   option payoff, or its actual collar, and no live reference or
   volatility lookup. Hypothetical fixings retain separate provenance.
   [Publication and expiry clocks](../conventions/0045-financial-clocks-and-date-boundaries.md).

5. **Select the live approximation and its compatible quote inputs.**
   Explicitly choose the CMS adjusted-forward Black76 approximation
   or the separately qualified [StaticReplication owner](12-cms-replication.md).
   Remove implicit model selection and automatic fallback after an
   unsupported term or failure. For this approximation require one
   exact compatible unshifted Black strike source for the actual
   CMS reference swap, tenor, currency and conventions. Use its
   qualified dated expiry clock, mapping and supported boundaries.
   No default clamp, generic tenor-axis reinterpretation or implicit
   conversion of Normal or shifted Black quotes. Look up ATM
   volatility at the unadjusted `F` for the mean correction and
   strike volatility only for active Black option payoffs at the
   relevant strike. A qualified tenor-bound strike slice must retain
   its source identity and quote law. Require the calibrated origin
   or a qualified dated aging operation, with total variance
   preserved by any admitted source/model clock bridge. This
   package admits neither live negative-forward Black pricing nor an
   automatic Normal, shifted or replication replacement. Known
   negative fixings remain valid on the contractual cash basis.
   [Source and model selection](../valuations-framework/model-resolution.md),
   [dated strike grids](../core-market/volatility-grids-and-shocks.md).

6. **Define one dated flat-yield proxy and the first-order correction.**
   Explicitly admit the flat-yield approximation with positive integer
   compounding frequency `m`. Let `u_j` be qualified ACT/365F
   times from resolved reference swap start to actual reference fixed
   payments, with actual accruals `α_j`; let `δ` be signed ACT/365F
   time from that start to the actual CMS payment. `δ` can be negative
   even when the CMS payment follows its authorized determination.
   With `q=1+r/m>0`, define
   `A_flat(r)=Σ_j α_j q^(−m u_j)` and
   `w(r)=q^(−m δ)/A_flat(r)`. Use the geometric form only on the
   actual regular grid `α_j=1/m,u_j=j/m`. Actual stubs and lags
   enter through the dated cash proxy and require product
   qualification. For weights `p_j` proportional to the terms of
   `A_flat`, use `ℓ(r)=w′(r)/w(r)=m(Σ_j p_j u_j−δ)/(m+r)`.
   Evaluate in scale with checked ordered sums, stable logarithms
   and exact zero-yield limits; the unneeded absolute weight need
   not be formed. Price the approximate mean as
   `F_adj=F+ℓ(F) F² v_ATM`, with `v_ATM=σ_ATM²T` on the
   admitted quote interval. Keep the sign of the correction. Admit
   live `F>0` and a finite positive `F_adj` on this route, including
   literal zero volatility after validation. Remove tiny-rate,
   invalid-time and invalid-domain zero sentinels. The exact payment
   measure uses a normalized bond-to-annuity density. This rule
   linearizes it and keeps first order in total variance. Its
   adjusted-forward option formula is also an approximation;
   it does not reproduce Hagan's strike-dependent cap/floor correction
   or the normalized expectation in record 12. Qualify rate, tenor,
   expiry, total variance, smile and payment-delay regimes before use.
   [Hagan, sections 2–3 and Appendix A](https://www.deriscope.com/docs/Hagan_Convexity_Conundrums.pdf).

7. **Price options and collars on one mean and cash basis.** A
   standalone call/put uses the checked unshifted Black unit option
   law at `F_adj,K` and the admitted strike quote. Under the
   selected positive-support law, `K≤0` gives a linear call
   `F_adj−K` or zero put, without a strike-volatility query. Apply
   the rest of the active admission rules; do not call Black at an
   invalid strike. For a swap coupon with spread `s`, use
   `F_adj+s−C(cap−s)+P(floor−s)`, omitting any absent option.
   Both option terms are undiscounted on the same mean, units, cut
   and clock. With a known fixing use the same identity with
   observed rate and intrinsic terms. A collar with equal bounds
   is a fixed rate at that bound and needs no fixing, reference
   projection or volatility for its cash amount. Reject incoherent
   queried option prices or a result outside its contractual bounds
   rather than clipping the expected coupon. This check is local
   and does not certify the whole smile. Multiply each eligible
   cash or model rate by its own `N α D_pay` once (use contractual
   coupon accrual here, not a reference annuity). Pay CMS means
   receive funding less CMS PV; receive CMS reverses that sign.
   Apply each position sign once to the signed coupon economics.
   Discount each funding amount at its own eligible payment.
   [Checked Black law](../models-analytic/forward-normal-shifted.md),
   [CMS payoff definitions](https://strata.opengamma.io/apidocs/com/opengamma/strata/product/cms/CmsPeriod.html).

8. **Use selected-method curve and volatility risk.** Use explicit
   checked `dv01`, `pv01` and `bucketed_dv01` for combined,
   per-source and triangular direct rate risk, signed per positive bp
   on the exact active reference, funding and payment evaluators.
   Shock a qualified shared evaluator once through its roles.
   Retire the one-sided CMS option `delta` and `rho` curve
   aliases. Select replay separately with its own qualified inputs
   and center. Use signed raw reprices on the same method, terms,
   observation cut and cash basis. For active CMS option volatility
   risk, use the shared checked `vega` and `volga` on the
   declared native Black sigma direction, reporting per `.01`
   relative annual sigma and its square. Use
   `cross_gamma_rates_vol` for four matched full reprices on a
   declared typed projection zero-rate axis and native vol axis,
   in currency per positive bp times `.01` Black sigma. Preserve
   any other role of the exact same bumped evaluator. Retire the
   old CMS curve-times-vol `vanna` alias. Preserve actual represented
   widths, centering, quote clock and mapping. Re-evaluate ATM at
   the bumped forward and strike quote as the declared source
   mapping requires. Remove the local Black-plus-convexity vanna
   shortcut. Use the shared one-sided vega rule only for a genuine
   admitted domain boundary; a failed side is not such a boundary.
   Known unpaid claims can retain discount risk after live forward
   and volatility exposures end; funding may still have live rate
   exposure. [Rate finite differences](../valuations-framework/finite-difference-rates.md),
   [option volatility stencils](../valuations-framework/option-sensitivities.md),
   [cross-factor risk](../valuations-framework/cross-factor-risk.md).

9. **Isolate the approximation's convexity contribution.** Define
   `convexity_adjustment_risk=PV_full−PV_CA0` only within the
   admitted adjusted-forward method. Compare raw signed values from
   one prepared contract, holding strike vol, other inputs, funding,
   cash cut and scenario adjustment basis fixed. Only set the
   mean correction to zero; do not turn off the option model or
   replace a failed base. Return structural zero where the correction
   is provably inactive. A replication price cannot use this
   approximation comparator. Any replication counterpart requires
   a separately approved same-method experiment in record 12.
   Preserve the scope of shared CMS-spread callers for
   [record 17](17-cms-spread.md).

10. **Use the selected prepared price for cashflow reports and hosts.**
    From the same period rows, source bindings, model and cuts,
    distinguish known contractual amounts from projected CMS/funding
    rates, modeled optionlets and expected cash. Retain actual
    dates, accruals, signs, fixing and model provenance and the
    applicable discount factors. Reconcile admitted PV components
    to the selected raw and finalized value under the shared
    amount rules. If the selected model has no complete qualified
    decomposition, report unavailable. Remove approximation fallback,
    fabricated accrual dates, placeholder cash and different
    valuation-date filters between export and PV. Reconcile existing
    Rust, registry, report and cashflow consumers, the Python and
    WASM envelope routes, schemas, declarations and parity.
    A horizon crossing an unresolved fixing requires a qualified
    observation scenario and the actual holder ledger. A modeled
    expected coupon cannot serve as the realized fixing.
    Qualify actual benchmark editions and source clocks, irregular
    schedules, signed payment delay, small rate limits, adjusted
    forward support, fixed/live transitions, collar coherence,
    same-method reports and all risk endpoints independently.
    [Cashflow export](../valuations-framework/cashflow-export.md),
    [metric orchestration](../valuations-framework/metric-orchestration.md),
    [holder horizon](../valuations-framework/carry-theta-breakeven.md).

## Observed behavior

### Choice, scope and rationale

CMS options and swaps default to Black76 with an annuity-weight convexity correction. Reference forwards are actual dual-curve swap rates using resolved reference-swap conventions; the payment-measure correction uses a flat-yield annuity approximation. Historical CMS coupons require observed swap-rate fixings, not a live forward. The rationale inferred from the code is a cheap correction with explicit payment delay; adoption and accuracy domain remain open.

### Algorithm and controls

1. CMS tenors must be positive month/year tenors. Explicit fixed/float frequency and day counts override registry fields. The index also supplies settlement days, calendar and business-day convention. USD requires an explicit index; implicit EUR/GBP/JPY mappings use ESTR/SONIA/TONAR OIS. Reference start advances fixing by market settlement business days.
2. Calculate reference swap floating-leg PV divided by fixed-leg annuity, with separate forward and discount curves, short-front stub and registry payment lag. The shared forward engine enforces term tenor where the convention is not OIS.
3. Define flat-yield annuity `A(r)=(1-(1+r/m)^(-m*tenor))/r`, and payment weight `w(r)=(1+r/m)^(-m*delay)/A(r)`. Near zero (`abs(m*tenor*r/m)<1e-3`) use a fourth-order annuity series and its derivatives; otherwise use `ln_1p`/`expm1` forms. The standalone par-annuity helper returns tenor for `abs(r)<1e-9`.
4. The convexity correction is `(w'(F)/w(F))*F^2*sigma_ATM^2*T`. Nonpositive F, T, tenor or invalid/nonpositive frequency returns zero in this helper; future stochastic Black pricing separately rejects nonpositive forward/strike and requires Black-lognormal quotes. ATM vol drives convexity; strike vol prices the option. ACT/365F measures expiry and signed payment delay.
5. CMS options sum `N*alpha*D*Black(F+correction,K,sigma_K,T)`. Fixed-but-unpaid coupons use historical intrinsic payoff; fixing equal to valuation uses live forward intrinsic. CMS swaps add spread to adjusted CMS; caps subtract a call at `cap-spread`, floors add a put at `floor-spread`. Pay means pay CMS/receive funding. Fixed funding uses contractual rate; simple floating funding uses exact past reset or period forward plus bp spread.
6. CMS swap schedule construction fixes short-front/ModifiedFollowing, zero payment lag and shared CMS frequency for generated funding periods; funding reset defaults to accrual start while CMS reset uses index settlement lag. Explicit funding periods preserve separate accrual/reset/payment dates and must be contiguous with positive accrual.

### Risk conventions and limitations

Option Delta and Rho are one-sided forward/discount curve differences per bp; Vega is one-sided surface bump per vol point; Volga is central second difference per squared vol point. StaticReplication Vanna uses four mixed forward-curve/vol reprices, normalized per decimal rate and vol point. Default Vanna instead sums a local Black vanna plus Black gamma times `2*convexity/ATM_vol`, at adjusted forward. It does not differentiate every forward dependence of the convexity correction or the smile; it is an approximation to the complete derivative.

ConvexityAdjustmentRisk subtracts a zero-correction **Black approximation** PV from the context base PV even when the base model is StaticReplication. It therefore need not isolate only the convexity correction within one method. CMS swap cashflow export likewise uses the default approximate CMS coupon path; it is not a replication-model-specific expected schedule.

### Alternatives and tradeoffs

[Normalized smile replication](12-cms-replication.md) is explicitly available and more expensive. A full term-structure payment-measure model would replace the flat-yield weighting assumption. Negative-rate normal CMS pricing is not implemented by this Black branch, despite a replication error message suggesting Black76/Hagan/Bachelier for negative regimes.

## Evidence

- [CMS conventions/weight derivatives](../../../../finstack-quant/valuations/rates/src/cms_common.rs), [forward swap engine](../../../../finstack-quant/valuations/rates/src/forward_swap_rate.rs), [option approximation](../../../../finstack-quant/valuations/rates/src/cms_option/pricer.rs), [option validation](../../../../finstack-quant/valuations/rates/src/cms_option/types.rs), [risk](../../../../finstack-quant/valuations/rates/src/cms_option/metrics.rs).
- [CMS swap pricing](../../../../finstack-quant/valuations/rates/src/cms_swap/pricer.rs), [funding and cashflow construction](../../../../finstack-quant/valuations/rates/src/cms_swap/types.rs), [swap metric selection](../../../../finstack-quant/valuations/rates/src/cms_swap/metrics.rs).
- [CMS option tests](../../../../finstack-quant/valuations/rates/tests/cms_option/test_pricing.rs) and inline CMS swap tests provide sign, fixed-coupon, cap/floor and schedule evidence. These do not establish full local-vanna accuracy over a smile.
- The existing [CMS option schema](../../../../finstack-quant/valuations/schemas/instruments/1/rates/cms_option.schema.json) and [CMS swap schema](../../../../finstack-quant/valuations/schemas/instruments/1/rates/cms_swap.schema.json) describe the current schedule and market fields.
- [Hagan CMS convexity](../../../REFERENCES.md#hagan-2003-cms-convexity) is the canonical reference entry; the precise chosen approximation and admissible regimes still require agreement.

### Independent review evidence

- With actual fixed accruals `.45,.55` and factors `.98,.95`,
  the reference PV per unit rate is `.9635`. Floating accruals
  `.5,.5` and rates `.04,.05` on those payments give floating PV
  `.04335` per notional and forward `.044992215880`.
- For regular 10Y semiannual, `F=.04,σ_ATM=.20,T=2` and
  `δ=.25`, the flat annuity is `8.175716672299` and the
  adjustment `5.862451372367` bp. At `δ=−2/365` the
  correction rises to `6.183053037690` bp; at delay 20 it is
  negative. With stub accruals `.25,.5,.5` and fixed
  payment times `.25,.75,1.25`, at delay `.25` the dated
  proxy instead has annuity `1.208752781944` and correction
  `.745965656556` bp.
- A known rate `.027`, spread `.004`, floor `.015` and cap
  `.030` give coupon cash `15,000` on notional one million
  and accrual `.5`. Funding `.045+.002` at accrual `.25`
  gives `11,750`. Pay CMS with future discount factors
  `.98,.99` gives PV `−3,067.50`; with the CMS cash included
  same-day it is `−3,367.50`. Excluding that same-day amount
  gives `11,632.50`.
- A standard 5Y semiannual proxy with `F=.045,K=.05,T=2,δ=.35`,
  ATM/strike volatilities `.20,.23`, notional one million,
  accrual `.5` and payment factor `.97` gives call PV
  `1,995.24973677`, versus `1,917.05585057` when only the
  mean correction is disabled. Under the synthetic rate shock
  `F(x)=1.045 exp(x)−1` and a parallel vol move on both quotes,
  full mixed risk at steps `1e−5` is about `.494459`
  per bp times `.01` Black sigma. The old local formula gives
  about `.451744` after conversion to this rate axis.
- An internally inconsistent smile can break the collar identity
  numerically. With `F_adj=.04,T=1`, floor `.03` and cap `.05`
  and zero spread, a floor volatility `.05` and cap volatility
  `1.8` give an out-of-bounds result of about `.016400862`.
  That request must fail instead of reporting or clipping the coupon.

## Relationship to the accepted observed CMS boundary

[CORE-MARKET-12's complete accepted package](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings) requires a configured CMS
fixing IndexId matching the actual reference swap/tenor/conventions/publication
setting, distinct from the underlying rate index and projection curve. Remove
floating-year/fuzzy-month fixing keys; existing positive calendar tenor
resolution remains at the CMS owner. Reject mismatched overrides/wrong series
and qualify publication/source version before exact retrieval. Record 12 was accepted on **2026-11-08**, with reconciliation and qualification
pending. The product decision above separately owns the approximation,
publication-qualified payoff and local risk.

## Relationship to the accepted volatility association boundary

[CORE-MARKET-14's complete accepted package](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) requires the actual CMS
strike-vol source to match the reference benchmark and fixed swap tenor, quote
units/convention and source clock. A Strike grid is not an expiry/tenor matrix,
and an ID or numerical scale alone cannot establish this association. Preserve
existing guards and qualify active dated lookup under accepted ADR0045; no
implicit source-origin roll is selected. The accepted CMS product decision above owns the pricing model.
Record 14's complete source/shock package was accepted on **2026-11-08**,
with reconciliation and independent qualification pending.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
