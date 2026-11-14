# VALUATIONS-RATES-14-INFLATION-OPTIONS: Inflation option forward proxy and information expiry

- ID: VALUATIONS-RATES-14-INFLATION-OPTIONS
- Decision status: **Accepted — complete checked inflation option cashflows, expiry and risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, market and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked inflation option cashflows, expiry and risk

1. **Require explicit option terms and periods.** Support a single
   holder-long European caplet or floorlet or a strip of those
   optionlets, with one currency, positive exact Money notional
   `N` and signed Decimal simple annualized strike `K`. The
   contract or deliberately selected dated convention specifies the
   two contractual CPI references, accrual dates and actual payment
   rules. A single optionlet takes one period. Only a strip takes an
   explicit frequency and grid, stub and roll/EOM rules. Retain each
   checked positive accrual fraction `A` with its full day-count
   context. Qualify a nonannual period or stub on its actual simple
   annualized period ratio in both the contract and any live quote.
   A rolling twelve-month reference, a cumulative compounded-strike
   option and any trade premium are separate product terms outside
   this gross optionlet profile. Remove default schedule selection
   and ignored single-optionlet frequency.
   [Schedule grids](../core-conventions/008-schedule-generation.md),
   [inflation swap cash laws](13-inflation-swaps.md#accepted-decision-for-checked-inflation-swap-cashflows-and-risk),
   [ICE's rolling YoY example](https://idd.ice.com/IRHelp/Content/FM/YoY_Inflation_Floor.htm).

2. **Prepare the actual CPI and payment claims.** Apply the shared
   lag, Step/Linear, positive-anchor, vintage, basis and release rules
   at an explicit knowledge cut. Resolve both endpoints on one exact
   CPI series and basis. Keep reference, accrual, knowledge, valuation
   and payment dates independent. Remove implicit lag, interpolation,
   calendar and the below-50 CPI warning. Use the authored adjustment
   and nonnegative payment lag with the exact calendar where required.
   Require actual or qualified expected availability of all necessary
   CPI by the authorized payoff-determination cutoff. A proved paid
   or otherwise extinguished claim needs no inactive inputs. Ordinary
   future-payment valuation excludes same-day cash. The explicit
   entitled cut includes only payable known same-day amounts at factor
   one. Keep fixed unpaid cash active through the actual payment date.
   Qualify overdue unsettled claims separately.
   [Inflation reference resolution](../valuations-framework/inflation-references.md),
   [cashflow and payment rules](../valuations-framework/cashflow-export.md),
   [BLS release calendar](https://www.bls.gov/schedule/news_release/cpi.htm).

3. **Price fixed cash from the actual payoff.** For cap sign
   `η=+1` and floor sign `η=−1`, let
   `G=expm1(log(I_end/I_start))`. Pay `N max(η(G−A K),0)`.
   Evaluate the positive-level ratio, strike and cash arithmetic with
   checked scaling and cancellation. A wholly observed payoff uses
   this literal cash calculation, even with negative inflation or
   strike. It needs no live volatility, annualized quote forward,
   expiry or Black domain check. For the fixed branch use the
   dated nominal relative factor to the actual payment, or the
   entitled same-day factor one. Retain fixing and payment provenance.
   [Optionlet payoff and payment measure](https://arxiv.org/html/2405.05101v1#S3.SS2).

4. **Qualify the live forward and its approximation.** Use
   `F=G/A` as the simple annualized quote forward. When the entire
   denominator is known, every projected numerator contribution must
   be qualified as a linear expected CPI level under the actual
   payment-date measure. A general projection curve or a quote with
   a different payment basis does not establish that qualification.
   If any required denominator anchor remains projected, require a
   deliberately selected and qualified deterministic-ratio approximation
   on the matched CPI inputs. Persist its use and limitations. The
   expected ratio can differ from the ratio of projected levels,
   and quote volatility does not correct its mean, payment timing or
   covariance. A quote on a single anchor does not supply the volatility
   of a ratio or a partially fixed blend. Admit only quotes expressly
   qualified for the actual reference weights and information state.
   Refuse the live quote path if the two contractual references are
   proved identical as expressions on the same unresolved index
   inputs. Their ratio is fixed at one, so option time value would be
   spurious. Equal observed or projected numbers alone establish no
   such identity. Independent YoY forwards and stochastic CPI/rate
   models require their own qualified pricing law.
   [Dam et al. on stochastic inflation ratios](https://arxiv.org/pdf/1801.08804),
   [Ögetbil and Hientzsch on payment measure and correlation](https://arxiv.org/html/2405.05101v1#S3.SS2).

5. **Elect the live quote expiry.** `PublicationDate` uses the
   latest qualified expected release of all distinct, required
   unresolved positive-weight anchors across both references.
   `ReferenceDate` uses the quote-qualified lagged contractual
   end date, with its explicit calendar-date mapping. Keep that
   expiry distinct from the monthly anchors, latest release and
   payment. Require positive elapsed model time on the admitted
   ACT/365F clock and an expiry compatible with the actual payment.
   Use an intraday profile only with qualified release status and
   matching quote and model clocks. Remove the default expiry
   convention. An elapsed or unsupported live horizon errors on its
   selected convention, even while CPI is still pending. Repricing
   on a different expiry basis requires a matching selected quote.
   [Inflation fixing, publication and payment dates](https://quant.opengamma.io/Inflation-Cap-Floor-OpenGamma.pdf).

6. **Bind one exact live quote and model.** Select `Normal` or
   unshifted `Black76` explicitly on the simple annualized rate.
   Retrieve the explicitly selected typed scalar or expiry-and-absolute
   strike quote source. Match index/basis, reference tenor or actual
   period, seasoning, coverage, quote origin/clock, expiry convention,
   payment basis, native sigma units and model. Require the valuation
   origin for ordinary grid pricing. A different origin requires
   qualified aging first. Use only admitted
   in-domain interpolation on a grid and error outside. Remove the
   dummy zero tenor, automatic surface clamping, implicit overrides
   and Black default. A quote labelled for one term or convention
   cannot be relabelled. Apply an approved same-interval
   `σ_model² T_model=σ_source² T_source` bridge only as authorized.
   Normal accepts signed `F,K` within the admitted live strike
   domain `1+A K>0`. At or below the positive-CPI lower bound, the
   initial live quote profile is unavailable. Known cash still follows
   the contract. Treat Normal as a quote kernel with unbounded
   tails. For live Black require `F,K>0`, including at zero sigma.
   Keep shifted Black, gross-ratio Black and model switching outside
   this profile. Require nonnegative sigma. Exact zero sigma gives
   model intrinsic and retains live status. Remove positive sigma
   floors. Compute undiscounted model amount `N A U(F,K,σ,T)`
   with the shared unit kernel, then apply the actual dated discount
   factor once. Under matching inputs, caplet less floorlet equals
   `N D(G−A K)` on this model basis.
   [Volatility binding and clocks](../valuations-framework/model-resolution.md),
   [unit price kernels](../models-analytic/forward-normal-shifted.md),
   [Strata's Normal formulas](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/impl/option/NormalFormulaRepository.html).

7. **Use one raw valuation and truthful cash export.** Prepare one
   ordered optionlet ledger for pricing, risk and reporting. Sum
   checked raw contributions before the one final conversion to
   Money. Remove per-optionlet Money conversion from raw reprices.
   Declare and resolve the exact active CPI, nominal discount and vol
   inputs as the claim and selected quote require them. Enable the
   existing export only for a model that supplies this full
   decomposition. The built-in Normal and Black paths can export
   future known cash or modeled expected cash with corresponding dates,
   factors, selected model and anchor provenance. A paid or excluded
   row contributes no PV, and a live zero-vol result remains modeled.
   A custom replacement without a qualified decomposition is unsupported.
   Keep contractual cash separate from settlement history and modeled
   expectations. Remove the empty placeholder from qualified paths.
   [Checked cashflow export](../valuations-framework/cashflow-export.md).

8. **Reprice typed CPI, nominal and volatility risk on the same
   admitted basis.** Use record 13's direct annual gross CPI growth
   axis and stencil for `inflation01` per positive bp and
   `inflation_convexity` per squared annual decimal rate. Preserve
   actual fixing, observation and contractual inputs. Retire
   generic spot `gamma` for CPI curvature and retain nominal
   discount risk in its own `dv01`, typed `pv01` and
   `bucketed_dv01` experiments with the CPI and volatility inputs
   held. For `vega` move the actual
   active scalar or all nodes of the exact active grid in its
   declared native clock with forward, strike, expiry and discount
   fixed. Reapply any qualified clock bridge. Report per `.01`
   annual relative Black sigma or per an explicit absolute Normal
   rate increment. Require explicit calculation steps and actual
   represented widths. Use both full sides when admitted, including
   a zero lower sigma, or the shared full up move and center only
   for the lower nonnegative-sigma boundary. Missing inputs and
   failed sides error. A fixed unpaid amount can retain nominal
   risk while CPI and vega exposure are structurally absent. Live
   zero sigma alone does not prove absent vega. Quote replay and
   mixed risk require separate qualified experiments.
   [Record 13 CPI and nominal axes](13-inflation-swaps.md#accepted-decision-for-checked-inflation-swap-cashflows-and-risk),
   [option sensitivity stencils](../valuations-framework/option-sensitivities.md).

9. **Reconcile the existing product and consumers.** Keep one
   Rust owner for explicit schedule, model, observation and quote
   admission. Reconcile the registry, reporting and attribution
   metrics, current schemas and the existing generic Python and
   WASM pricing, metrics and export envelopes. Establish independent
   fixtures for release-date boundaries and partial fixings, stub payoffs
   and model domains, quote clock and support, deterministic cash
   and live expected cash, parity, raw aggregation and native risks.
   Qualify the projected-denominator approximation and quote coverage
   by index and payment terms before claiming market accuracy.
   [Product schema](../../../../finstack-quant/valuations/schemas/instruments/1/rates/inflation_cap_floor.schema.json),
   [other-factor attribution](../attribution/10-metric-other-factors.md).

## Observed behavior

### Choice, scope and rationale

Inflation cap/floor pricing treats the deterministic CPI forward ratio as the mean of a simple annualized inflation rate, then applies a Normal or Black caplet kernel. This avoids a joint CPI/rates simulation but does not calculate a stochastic ratio convexity correction. Black76 is the default; Normal is explicitly registered. Selection follows the requested model and volatility convention, not an automatic switch for negative inflation.

### Algorithm and controls

1. Build one period for a caplet/floorlet or a frequency schedule for a cap/floor. Defaults are short-front stub and ModifiedFollowing; absent calendar resolves to weekends-only. Accrual boundaries remain unadjusted.
2. Resolve start/end CPI with lag, interpolation and publication state. Require positive CPI and positive accrual alpha. Compute `F=(I_end/I_start-1)/alpha`.
3. If both references are known, set time to zero and use deterministic intrinsic payoff, requiring no vol quote. Otherwise ReferenceDate (default) uses lagged end date; PublicationDate uses the latest publication date among all unknown anchors and requires each unknown publication date explicitly.
4. Use ACT/365F to this expiry. An unknown CPI with nonpositive selected expiry errors and instructs the caller to supply publication-expiry quotes. Resolve vol at `(expiry, tenor=0, strike)` with surface clamping enabled and required Normal/Lognormal convention.
5. Price `N*alpha*D(as_of,pay)*option(F,K,sigma,T)` through the shared caplet convention dispatch and sum only payments strictly after valuation.

### Risk, limitations and fallbacks

Inflation01 is a central ±1 bp inflation-curve bump through the active metric repricer. Gamma is the second difference divided by the squared decimal bump; vega centrally bumps the stored surface and reports currency per volatility point. Parallel/bucketed rates risk delegates to the shared DV01 implementation. CPI below 50 emits a warning even though a positive rebased CPI can be economically valid; it is not rejected. Volatility surface clamping can flatten extrapolation. A volatility override may make a surface-only vega bump ineffective.

### Alternatives and tradeoffs

Publication-expiry quoting is supported as an explicit alternative to reference-date expiry. Stochastic CPI ratio expectation/convexity and market-calibrated YoY forwards are unimplemented alternatives. Bachelier supports negative forward inflation without lognormal positivity, but is a separate quote/model choice.

## Evidence

- [Inflation option implementation](../../../../finstack-quant/valuations/rates/src/inflation_cap_floor/types.rs) and [model adapter](../../../../finstack-quant/valuations/rates/src/inflation_cap_floor/pricer.rs): information expiry, deterministic proxy and kernel dispatch.
- [Inflation option metrics](../../../../finstack-quant/valuations/rates/src/inflation_cap_floor/metrics/mod.rs) and [inflation01](../../../../finstack-quant/valuations/rates/src/inflation_cap_floor/metrics/inflation01.rs): selected bump/reprice channels.
- [Pricing tests](../../../../finstack-quant/valuations/rates/tests/inflation_cap_floor/pricing.rs): formula and information-state behavior. [Production inflation audit](../../../../finstack-quant/valuations/tests/production_inflation_audit.rs): cross-product convention evidence, not independent model certification.
- [Inflation references](../../../REFERENCES.md#deacon-derry-mirfendereski-2004), [Black](../../../REFERENCES.md#black-1976), [Bachelier](../../../REFERENCES.md#bachelier-1900).

### Reconciliation evidence

The current raw method sums per-optionlet conversions to Money and
fetches discount before checking the payment set. Market dependencies
declare a volatility source even when the amount is fixed or the active
quote is a scalar. The export gate excludes the current two quote
models and the product's cashflow provider is a placeholder. The
metric registered as generic gamma is CPI curvature.
[Pricing and dependencies](../../../../finstack-quant/valuations/rates/src/inflation_cap_floor/types.rs),
[export capability gate](../../../../finstack-quant/valuations/core/src/instruments/cashflow_export.rs).

### Independent review evidence

- With a quarter-year stub, CPI `250→252`, strike `.03`, and one
  million notional, simple annualized inflation is `.032`.
  The caplet pays `500`, worth `495` at factor `.99`.
- In an illustrative snapshot at January 20, 2026, a three-month lag
  from April 1 gives reference expiry January 1. The qualified latest
  expected release is February 13. Reference time has elapsed by 19 days.
  Publication time has 24 days remaining. With one million notional,
  unit accrual and factor, and ATM Normal sigma `.01`, the matched
  publication quote gives PV `1,022.98497347`.
- In two equally likely payment-measure states, CPI pairs
  `(100,110),(200,202)` give ratio of means `1.04` and mean ratio
  `1.055`. With one million notional, `A=1`, `K=.04`, and
  `D=.95`, the proxy gives zero caplet-minus-floorlet. The state
  payoffs give `14,250`. Changing volatility cannot correct the
  ratio-mean difference in parity.
- Known CPI `250→248.75` with `A=.5`, `K=−.012`, and one
  million notional pays caplet cash `1,000`. The future value
  with factor `.98` is `980`. An explicitly entitled same-day
  valuation is `1,000`, and the ordinary same-day exclusion is zero.
- For live ATM Normal `F=K=−.01`, with `A=.5`, `σ=.01`,
  `T=.25`, `D=.98`, and one million notional, modeled expected
  cash is `997.35570100358` and PV is `977.40858698351`.
  Vega per `.0001` native sigma is `9.77408586984`. At sigma
  `1e−9`, PV is still positive at `0.000097740858698351`.
- On a matched one-year CPI growth axis with known base,
  `F(h)=.03+h`, `K=.03`, `A=1`, and the same Normal sigma,
  time, factor and notional, the `±1` bp raw stencil gives
  inflation01 `49` and curvature approximately
  `78,190,080.64` per squared annual decimal rate.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
