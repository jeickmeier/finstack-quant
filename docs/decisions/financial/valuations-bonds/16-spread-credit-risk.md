# VALUATIONS-BONDS-16: Par-CDS CS01 versus bond-spread fallback and spread duration

**Decision status:** Accepted — complete checked bond spread and credit risk package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, market replay, model and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked bond spread and credit risk

1. **Select CS01 from the active valuation and admitted claim.**
   Use par-CDS quote replay when the qualified selected model
   consumes an active hazard source. Resolve its actual typed
   binding from the prepared model, including any custom registry
   model with a qualified replay and reprice contract. An attached
   or co-present unused hazard ID does not select credit risk.
   A missing, wrong-kind, ambiguous or unreplayable required active
   source fails that experiment. A supported bond with no active
   hazard uses the qualified settlement Z-spread experiment under
   ADR 0011. On an option-bearing bond it requires an expressly
   elected and independently qualified fixed cash path, such as
   the scheduled no-voluntary-exercise scenario where the quote
   law admits it, or a qualified fixed workout. BONDS-11-dependent
   paths remain unavailable until qualified. Remove implicit OAS
   substitution, clean-triggered workout selection and fallback
   after any selected replay or pricing failure. A live optional
   model without a qualified fixed Z strip has no no-hazard
   CS01 under these rules. Direct intensity, OAS and recovery
   shocks retain their own experiments and authorities.
   [No-hazard bond CS01](0011-bond-cs01-without-hazard-curve.md),
   [selected-model authority](03-pv-models.md) and
   [settlement spread laws](12-spread-quotes.md).

2. **Reconstruct the exact CDS calibration and par-risk center.**
   Bind every selected active hazard, the calibration's discount
   source, recovery, CDS contract model and terms, the original
   market and immutable construction record, and the qualified
   recalibration provider. The calibration discount can differ
   from the bond's discount. Reproduce the complete original
   hazard under its recorded fit and source-reproduction gates.
   Qualify the unshocked par-risk view to the same center and
   original source basis. Preserve original upfront or other quote
   observations and their terms alongside any derived par inputs.
   Each target is the original exact quote ID within its typed
   source, or a verified recipe index with that same binding.
   Validate all bindings and required dependencies. Use no first
   curve, inferred discount, nearest-tenor, deal-quote replacement
   or synthetic hazard fallback. A mismatched reconstruction is
   a replay failure. Recentring to another hazard is a separately
   authorized market scenario.
   [Exact hazard and quote operations](../core-market/hazard-quotes-and-risk.md),
   [qualified replay](../core-market/calibration-provenance.md) and
   [CDS quote risk](../models-and-valuations-credit/13-cds-and-index-risk.md).

3. **Reprice raw holder PV at the original valuation date.**
   Use comparable bond position, currency, dispatch, history and
   claim scope on both sides. For a price-anchored model use the
   accepted BONDS-15 center on the qualified base snapshot. Normalize
   the authorized original price to settlement dirty and, where
   fitted, qualify the selected model's OAS against the financed
   buyer claim. Use raw holder PV in the resulting risk experiment
   and freeze the model spread across sides. A literal spread or
   previously reported scalar needs matching basis and admission
   to supply that center. A model-centered request uses its own
   qualified base terms. Preserve the actual historical settlement,
   fixings and realized events. Hold discount, recovery inputs,
   contractual cash terms and nonselected market state fixed. The
   shock can change survival and conditional recovery weights or
   model-dependent future exercise. Rebuild the affected credit fit
   and continuation policy under the same model. In sampled routes
   use matched logical draw addresses, disjoint training and pricing
   stages, affected policy refits, paired raw differences and the
   admitted conditional error and work controls. No live quote pin,
   refit of OAS to the old mark or change of valuation origin on
   shocked sides.
   [Quote-reproducing holder center](15-curve-risk.md),
   [bond replay and sampling](07-lsmc-policy.md) and
   [shared credit sensitivities](../valuations-framework/credit-sensitivities.md).

4. **Use checked par-quote central shocks and exact buckets.**
   Require an explicit finite positive common calculation step `h`
   in bp. In a joint parallel shock move the complete selected set
   by `±h`, qualifying the represented vector moves and their common
   amplitudes. For per-source parallel move the exact selected source
   set at a time. Rebuild any common actual source once per scenario
   when its bindings agree. For bucketed CS01 move one exact bound
   quote at a time. Use the checked actual applied scalar width in bp
   for a single-quote secant and the declared common applied width
   for a multi-quote direction. The symmetric common case is
   `(H+−H−)/(2h)` in signed native currency per upward par bp.
   Refit affected sources on both sides and require the accepted fit
   and all pricing checks. Invalid, collapsed or failed directions
   error and never select clipping, a one-sided stencil or a smaller
   step. Publish joint `cs01`, source-qualified `cs01::<curve>` for
   the source-specific result, and `bucketed_cs01::<curve>::<quote>`.
   A single source and joint shock can share a result. Retain exact
   quote identity and order, and keep tenor and date as metadata.
   Report the checked bucket sums as such. Finite bucket or source
   sums can differ from the simultaneous parallel result. Reject any
   attribution key or role collision before publishing.
   [Checked stencils](../valuations-framework/finite-difference-rates.md),
   [White on par-quote rebootstrap and buckets, §8.1](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf#page=24)
   and [Strata CDS bucket definitions](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/credit/SpreadSensitivityCalculator.html).

5. **Use the same BONDS-12 settlement Z law on both no-hazard sides.**
   Prepare the authorized buyer-entitled dated cash strip at actual
   settlement `S`, including any explicitly qualified frozen projected
   cash. Preserve the exact payment phase, amounts, source-native
   relative factors and their matching spread-time coordinates. A
   floating or other nonstandard bond needs a qualified Z cash strip
   where the settlement law admits one. Its curve DM remains a distinct
   quote experiment. Use an authorized explicit Z, a qualified implied
   Z from one dirty target on the same strip, or an expressly admitted
   unquoted zero-Z basis. Reject conflicting authorities. Use signed
   accrued or the qualified funding/buyer bridge only if needed to
   produce the original dirty anchor. Use the BONDS-12 monotone inverse
   and returned-spread residual gate for price-implied Z. An admitted
   signed strip at an authored Z need not admit that inverse. Keep the
   required quote compounding fixed. Admit Continuous and checked
   nominal Periodic with explicit positive integer frequency on the
   native source clock under BONDS-12. Do not infer frequency or use
   Simple. Same-day and qualified zero-time unit factors contribute
   eligible cash to price with zero spread response. Financing and
   retained holder cash do not enter the direct settlement-Z reprices.
   [Settlement entitlement and carry](02-settlement-and-clocks.md),
   [Z and quote compounding](12-spread-quotes.md) and
   [Strata issuer Z-spread factors](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/DiscountFactors.html).

6. **Retain the bond-specific forward one-bp Z price change.**
   For `h=.0001` annual decimal, report the raw signed settlement
   currency P&L `P_S(z+h)−P_S(z)` under `cs01::<instrument>` and
   the same parallel headline. This fixed one-bp scenario has no
   configurable CDS-style step or secant rescaling. Validate the
   represented upward move and the actual factor domain, and preserve
   its provenance. Compute the signed change before price display or
   Money rounding. Use stable checked factor differences and signed
   accumulation when subtracting two totals would erase a representable
   response. Distinguish a qualified insensitive strip or true numeric
   cancellation from failed inputs, a lost step or incomplete model
   scope. A finite signed dirty response on an admitted strip needs
   no positive-price denominator. `bucketed_cs01` is unavailable on
   the no-hazard Z route. Remove the parallel-as-bucket alias. A cash
   band decomposition would require its own qualified definition.
   The absolute difference from a symmetric one-bp P&L is generally
   second order in step for a smooth price. Its relative size to a
   nonzero first-order response is generally first order. Remove the
   unsupported relative error claim.
   [Instrument-qualified metric keys](../valuations-framework/0010-metric-identifiers.md)
   and [shared Z versus CDS stencil rules](../valuations-framework/credit-sensitivities.md).

7. **Normalize the same forward Z pair for spread duration.**
   Define `spread_duration=−[P_S(z+h)−P_S(z)]/[P_S(z) h]` at the
   same fixed `h=.0001`. Require a finite strictly positive raw
   center on the admitted settlement buyer claim before external
   signed position weighting. Remove the near-zero price/notional
   floor. Check the represented result and preserve genuine signed
   values. The metric can run on a separately qualified Z strip of
   a bond priced with an active hazard model. It does not normalize
   the bond's par-CDS CS01 or OAS exposure. Keep it intensive under
   position scaling. Attribution uses it only with a change in Z
   on the same claim, compounding, clock and units. Preserve signed
   and zero spread levels and do not divide by the spread level.

8. **Carry exact risk basis through the Rust producers and hosts.**
   Publish actual price basis, phase, selected market and quote
   IDs, step and stencil, currency, amount and model or cash scope.
   Reuse preparation only under identical bindings and scenario
   identity. Keep display labels distinct from source identity. A
   direct result and its headline, bucket total or qualified view
   cannot be counted twice. Apply signed position weighting once
   to monetary risk. Aggregate only on a matched factor and value
   basis. A CDS market-spread move cannot use an instrument Z
   fallback as its sensitivity. Update exact quote-ID producers,
   attribution and portfolio consumers, and reconcile schemas,
   UI, Python and WASM with the Rust calculation. Distinguish
   actually inapplicable optional fields from invalid explicit
   requests, missing required data, and failed arithmetic, fit or
   repricing. Preserve independent results under the shared metric
   contract. Spillover to mixed rate/credit, recovery, margin and
   direct intensity experiments remains with their owners.
   [Shared metric basis](../valuations-framework/metric-orchestration.md),
   [mixed factor risk](../valuations-framework/cross-factor-risk.md)
   and [bond default risk](17-default-risk.md).

## Observed behavior

### Choice and selection

[Accepted ADR 0011](0011-bond-cs01-without-hazard-curve.md) owns the no-hazard z-spread contract for supported bonds. [ADR 0010](../valuations-framework/0010-metric-identifiers.md) owns instrument-qualified attribution. The observed option-bearing OAS branch requires reconciliation with the accepted decision above. [Shared credit sensitivities](../valuations-framework/credit-sensitivities.md) owns the general estimators and their distinct stencils and bucket methods.

When the active model is HazardRate or RatesCredit, parallel/bucketed CS01 delegates to framework par-CDS quote shocks and hazard recalibration. A present hazard curve must carry a replayable calibration recipe; hand-built hazard curves error before OAS quote pinning. A missing curve is left for the shared path to diagnose. Merely attaching a credit ID while selecting a noncredit model does not select hazard CS01.

Without a credit-consuming model, optional bonds use OAS risk and plain bonds use Z-spread risk. Both are forward one-bp differences, keyed by instrument ID (`cs01::BOND_ID`), rather than par-CDS sensitivities. Bucketed CS01 on this route returns the parallel fallback; it does not create a fictitious credit tenor curve.

### Algorithms and conventions

For fallback CS01, hold all model inputs fixed and evaluate `PV(s+0.0001)−PV(s)` through the selected model/registry. Plain Z-spread base and bumped values are converted from valuation PV to settlement dirty with the canonical carry bridge; optional OAS values remain valuation PV. Required solved Z must be finite. Optional OAS comes from computed OAS, or zero if no price driver; a price-driven bond without computed OAS errors. Base and bumped PVs and final difference must be finite.

Spread duration deliberately always uses the deterministic quote/workout Z kernel, even for hazard-priced bonds: `SD=−[P(z+h)−P(z)]/(P(z)h)`, `h=0.0001`. It is the quote-spread exposure used for attribution, not normalized canonical CS01. Near-zero denominator is rejected at `1e−12·max(abs(notional),1)`. The kernel's compounding is periodic, not continuous exponential spread discounting.

### Limitations, alternatives and evidence

The forward difference includes a second-order price term. Its deviation from central one-bp P&L is order `h²` in absolute price and generally order `h` relative to the first-order sensitivity. The source comment claiming about `10^−8` **of CS01 magnitude** is not a general bound. Settlement versus valuation anchoring also differs between fallback variants.

Owning code: [CS01 routing and fallback](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/cs01.rs), [spread duration](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/spread_duration.rs), [risk view](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/risk_view.rs), [shared CS01](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01.rs). CS01 inline tests inspect selected custom-registry repricing and early rejection of unreplayable hazards. This is dispatch/domain evidence; independent spread-bump convergence remains open. [Credit valuation reference](../../../REFERENCES.md#hull-white-2000-cds) supports model background, not approval of the bond fallback.

Live reconciliation also reaches the [legacy inactive-curve bucket regression](../../../../finstack-quant/valuations/tests/instruments/bond/metrics/credit_quote_risk.rs),
[credit shift attribution](../../../../finstack-quant/attribution/src/metrics_based/credit.rs),
[observed shift identity matching](../../../../finstack-quant/attribution/src/metrics_based/shifts.rs) and
[fixed-income attribution](../../../../finstack-quant/portfolio/src/fi_attribution.rs).

## Independent arithmetic and qualification

- For entitled immediate cash `5` and a two-year cash payment `100`
  on a semiannual discount basis with base nominal zero `.08` and
  Z `.02`, `P_S(z)=87.270247479188`. The forward one-bp CS01 is
  `−.015668657969` and spread duration is `1.795418074444`.
  The symmetric one-bp change is `−.015670523507`, with relative
  difference about `.000119` versus the forward result. The immediate
  cash contributes to the duration denominator with zero CS01.
- With unit base factors, continuous Z `0`, and admitted signed cash
  `+100` at year one and `−90` at year three, direct dirty is `10`,
  the upward one-bp CS01 is `+.016996450388`, and spread duration
  is `−16.9964503883`. The signed strip does not admit the ordinary
  monotone price-to-Z inverse.
- In a two-payment-date discrete credit fixture, use discount factors
  `.98` and `.95`, recovery `.40`, and par quotes `150` and `200` bp.
  CDS premiums pay on survival and protection pays at each interval
  end, with no accrual on default in this fixture. With survival
  `q_i`, discounts `D_i`, decimal spreads `s_i` and loss fraction
  `L=.60`, the two fit equations are `s1 D1 q1=L D1(1−q1)` and
  `s2(D1 q1+D2 q2)=L[D1(1−q1)+D2(q1−q2)]`. Solve both on
  every relevant side. A
  bond paying `5` then `105` on survival and `40` on interval-end
  default has raw center `100.609126671912`. A `±10 bp` parallel
  rebootstrap gives CS01 `−.019307802724`; the exact quote buckets
  give `+.000038379714` and `−.019346132915`. Their sum is
  `−.019307753201`. This checks the replay and signed aggregation
  arithmetic on the discrete fixture. Production CDS conventions
  require independently dated premium, protection and accrual tests.

Exact rational checks reproduce the periodic and discrete CDS
fixtures. A 65-digit calculation reproduces the continuous signed
cash example.

Qualification covers original upfront replay and exact quote IDs,
distinct calibration and bond discounts, actual active and inactive
credit, raw versus dirty bases, fitted-spread hold, affected policies
and paired diagnostics, quoted and unquoted Z, periodic and continuous
factors, ex-coupon and immediate cash, fixed-exit and live-only option
paths, nonzero small prices, invalid represented steps, genuine zero,
position scaling, bucket availability and typed attribution. Compare
standalone, bundled and host results to independent examples. Current
routing tests and the arithmetic here do not certify the runtime.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
