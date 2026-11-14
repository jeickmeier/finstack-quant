# VALUATIONS-BONDS-15: Quote-reproducing curve DV01, key-rate risk and effective convexity

**Decision status:** Accepted — complete checked bond curve and effective risk package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, model, market and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked bond curve and effective risk

1. **Give bond curve metrics a fixed valuation basis.** Use raw
   native-currency holder PV `H` at original valuation `V`, before
   display rounding, including the admitted holder cash, contingent
   rights and retained claims. Use the same valuation and information
   cuts, position amount and currency on every side. Select the actual
   qualified model, registry/provider and contractual scope. Bond
   `dv01` is combined parallel direct rate-curve risk. Bond
   `bucketed_dv01` is separate direct key-rate repricing on that same
   basis. Neither field changes experiment because the bond has a
   price quote, exercise rights or credit. `yield_dv01` and the other
   fixed-cash yield metrics retain the accepted settlement experiment.
   An observed price held fixed is a mark. Settlement-strip risk and
   market-quote recalibration require separately identified experiments.
   [Holder and buyer cash](01-schedule-and-holder-cashflows.md),
   [selected bond model](03-pv-models.md),
   [yield risk](14-yield-risk.md) and
   [Strata calibrated and market-quote PV01 definitions](https://strata.opengamma.io/apidocs/com/opengamma/strata/measure/Measures.html).

2. **Prepare one quote-reproducing center when an observed price is
   the risk anchor.** Otherwise start from the qualified selected
   model including any admitted authored model spread or fitted state.
   Resolve exactly one authorized price driver to the native settlement
   dirty amount `Q` on the matching buyer claim. Apply signed accrued
   only for a clean conversion. A qualified yield, spread or ASW price
   conversion may supply `Q` once, with its original provenance and
   claim identity. For a quote-centered holder risk use the selected
   model's admitted OAS law and BONDS-12 inversion on the base snapshot,
   including for eligible straight discounting models. Check the actual
   represented anchor on the original valuation origin using buyer
   claim PV `J(s)=H(s)−Δ(s)`, financing `F=D_fin(V,S)` and the native
   residual `J(s)−F Q`. `Δ` is the qualified difference between the
   holder and buyer claim values. Use the raw modeled `H(s)` for base
   and both side reprices. Keep the original quote as provenance and
   disable its price short-circuit in the isolated scenario view. Freeze
   the admitted spread and its law across all shocks. Use the retained
   role bindings and rebuild affected live state. Do not retarget the
   observed mark on bumped markets, restart the valuation at settlement,
   or copy a settlement Z-spread into a model OAS slot. Where both a
   quote pin and another adjustment are present, apply accepted quote
   authority and conflict rules. A literal OAS observation needs its
   own qualified forward model law to supply a model spread. Reuse a
   solved anchor only for identical request, quote/claim, valuation,
   snapshot, dispatch, controls and pricing sample. A missing bridge or
   unqualified anchor makes that quote-centered request unavailable or
   an error under its request contract. A selected holder PV can still
   support an independently requested unanchored model experiment.
   [Settlement finance and retained claims](02-settlement-and-clocks.md),
   [settlement spreads and selected-model OAS](12-spread-quotes.md),
   [ASW price anchors](13-asset-swaps.md) and
   [issuer and financing roles](https://quant.opengamma.io/Bond-Pricing-OpenGamma.pdf).

3. **Shock only the exact active typed rate sources.** Resolve all
   active discount, projection and rate-reference bindings from the
   prepared model valuation, including a live make-whole benchmark or
   the actual selected tree-curve source where applicable. Resolve the
   same active source set for combined parallel, triangular buckets
   and effective risk. Use the qualified explicit binding without
   a tree-ID-to-bond-discount fallback. Inactive configured sources
   and sources used only to convert the original quote do not enter
   the shock. Deduplicate the same actual typed source once. One
   shock to a truly shared source affects all its active roles. Equal
   ID text is insufficient to merge distinct sources. Keep
   role-isolated or quote-calibrated risk separately identified.
   Use direct additive continuous zero-rate shocks on the complete
   selected factor evaluator before the held spread adjustment.
   Its multiplier is `exp(−b x w(x))`, where `b` is the annual
   decimal amplitude and `x` is the retained native-source year
   coordinate relative to that source's declared reference.
   No shift of yield, raw forward nodes, or the curve's origin.
   Validate every active source and fail missing, conflicting or
   wrong-kind roles. An unsupported empty direction is unavailable.
   [Complete source shocks](../core-market/rate-shocks.md) and
   [shared rate risk and source admission](../valuations-framework/finite-difference-rates.md).

4. **Rebuild rate-responsive state under the same model.** Keep
   the contract, historical fixings, legally committed events,
   non-shocked sources, volatility and fitted model parameters fixed
   as required by their owners. Use fixed active hazard and recovery
   source inputs for this direct rates experiment. Rebuild any
   model-internal fit to its shocked rates and frozen survival targets
   that the qualified pricing law requires. A quote-preserving credit
   recalibration is a separately identified experiment. Recompute
   affected live projected cash, tree fits, rate-dependent make-whole
   or exercise values, and feasible future stopping policies under
   the shocked market. Preserve the accepted exercise and settlement
   phases and owner work gates. A structural Merton route keeps an
   admitted fitted parameter under its own risk law. Use its qualified
   risk adapter where available without treating that fit as OAS.
   A model or custom pricer lacking a qualified comparable raw
   reprice for the selected experiment cannot supply the requested
   curve metric. Preserve independent unshocked prices and let the
   credit and option owners select their separate sensitivities.
   [Bond tree and exercise](04-rates-tree-exercise.md),
   [joint replay](06-joint-coupon-replay.md),
   [continuation policy](07-lsmc-policy.md),
   [structural risk](09-merton-calibration.md) and
   [bond credit risk](16-spread-credit-risk.md).

5. **Report signed currency per upward bp on explicit stencils.**
   Require a positive finite calculation step and an exact positive
   strictly increasing tenor grid when applicable, supplied explicitly
   or by an explicitly selected shared profile. Preserve requested and
   represented scenario amplitudes `b−<0<b+` in bp, and verify the
   actual source moves and required transforms. Compute
   `DV01=(H+−H−)/(b+−b−)` in native currency per upward bp. Combined
   parallel moves all active sources together. The triangular key-rate
   experiment moves one actual source at one exact tenor at a time.
   Use unit center weights, linear adjacent ramps and flat outer wings
   on the retained native source clock. A singleton is parallel for
   that source. Report exact source and tenor keys, per-source and
   overall checked bucket sums. Use standard labels only for actual
   standard coordinates and round-trippable labels otherwise. Reject
   attribution/key collisions before publishing. Finite sums of
   separate reprices can differ from the simultaneous parallel result,
   even with a partition of unity. Preserve actual results and apply
   a stated tolerance only to an expressly requested matched comparison.
   Keep the reported step distinct from the one-bp reporting unit.
   Remove silent floors, step repair, failed-side fallbacks and
   implicit one-sided repricing. Genuine finite zero and qualified
   structural zero retain distinct provenance.
   [Finite differences, rate risk and buckets](../valuations-framework/finite-difference-rates.md).

6. **Publish effective duration and unscaled effective convexity
   under their own keys.** Use `effective_duration` and
   `effective_convexity` for a qualified bond model on the same
   combined parallel direction and valuation basis as `dv01`, with
   explicitly resolved calculation width. Admit straight and optional
   profiles where the actual model can provide the comparable prices.
   Require a strictly positive finite raw claim center `H0` before
   external signed position weighting. A signed raw monetary DV01 has
   no positive-price denominator. For actual offsets `−c,0,a` in
   annual decimal rates, `a,c>0`, let `u=(H+−H0)/a` and
   `l=(H0−H−)/c`. Use
   `D_eff=−(c u+a l)/[(a+c) H0]` and
   `C_eff=2(u−l)/[(a+c) H0]`.
   These use the base-centered three-node interpolant. Symmetric
   moves reduce to `(H−−H+)/(2 H0 h)` and
   `(H++H−−2 H0)/(H0 h²)`. For identical symmetric reprices,
   `DV01=−H0 D_eff/10000`; a general endpoint secant and a
   base-centered slope can differ. Use checked differences and scaled
   arithmetic. Return unscaled convexity on the elected annual decimal
   shock. Remove implicit 25 bp, near-zero center fallback and the
   former `/100` presentation scale. At exercise switches and other
   kinks describe both results as finite-step repricing measures.
   [FHFA on effective duration and fixed-OAS repricing](https://www.fhfa.gov/sites/default/files/2023-05/Investment_Portfolio_Management_Module_Final_Version_1_0-508.pdf#page=62)
   and [Fornberg's unequal-grid stencil method](https://doi.org/10.1090/S0025-5718-1988-0935077-0).

7. **Qualify sampled risk on matched logical groups.** Use the
   model owner's fixed budget, exact grid/draw map and matched logical
   random addresses across scenarios. For joint exercise, refit
   affected reference and continuation policies on matched training
   stages, then freeze each scenario policy for matched, disjoint
   pricing stages. An equal seed alone does not qualify alignment.
   Form signed price differences and raw first- and second-order
   stencil contributions within independent pricing groups before
   accumulating. Count an antithetic pair as one group. Any sampled
   bucket-sum uncertainty must preserve cross-bucket pairing. Determine
   effective metrics from mean price and mean raw stencil, with
   positive mean center. Report the conditional sampling diagnostics
   for the paired raw quantities and center. If selected, apply an
   explicit positive half-width gate to the specified raw priced
   quantity or paired stencil in its stated units. No ordinary
   Student-t interval on a normalized ratio is supplied by these raw
   summaries. Keep fitted-center reuse, policy approximation, sampling,
   grid and bump-width effects identifiable. A fit and risk run that
   share pricing draws supply conditional run diagnostics and no
   independent fit validation. A pairing or scenario failure never
   silently selects independent runs or a different budget.
   [Bond paired addresses and policy refits](07-lsmc-policy.md),
   [structural calibration and replay](09-merton-calibration.md) and
   [shared paired risk method](../models-monte-carlo/25-finite-difference-greeks.md).

8. **Reconcile consumers with one declared experiment.** Share
   preparation and same-basis reuse across standalone and batch
   metrics. Report actual holder center, valuation basis, model and
   quote provenance, source/role scope, shock geometry, represented
   steps, value currency, position scaling and sign. Apply external
   signed position quantity once to monetary risk and keep normalized
   measures intensive. Publish validated complete bucket results.
   Distinguish inapplicable optional outputs from invalid explicit
   requests, missing required data and numerical or replay failures.
   Retire the obsolete `bond_risk_basis` routing switch and its
   `bullet_discountable`/`callable_oas` values across existing wire
   and host surfaces. Use explicit metric selection and the actual
   model/quote controls. Reconcile Rust, registry/catalog, Python,
   WASM, schemas, aggregation, attribution and UI. The shared risk
   view's credit consumers keep their admitted credit experiments;
   this contract does not supply credit or recovery shock rules.
   [Shared metric reporting](../valuations-framework/metric-orchestration.md)
   and [public metric meaning](../valuations-framework/0010-metric-identifiers.md).

## Observed behavior

### Choice and selection

A fixed price quote must be replaced by a model spread before curve-bump repricing can move its value. Plain noncredit quoted bonds are pinned with their solved periodic Z-spread; exercise-bearing or active hazard-model bonds are pinned with OAS on the original market. The selected pricing model/registry is preserved, price-driving overrides cleared, and the context restored after the risk calculation. Existing computed OAS is reused, otherwise solved from the quote. This is a calibrated risk representation, not a claim that the quote identifies a unique structural credit model.

### Algorithms and controls

Parallel `Dv01` has a material selection rule: on default Workout basis, **any price-driving quote or exercise right** selects the signed yield-DV01 formula of record 14. Only plain unquoted bonds use generic combined discount/projection curve bumps. Thus the label `Dv01` does not always mean a curve sensitivity. Under explicit CallableOas, unquoted bonds use generic combined curve DV01; quoted plain bonds use the spread-pinned view and generic curve DV01; quoted optional bonds bump only `tree_discount_curve_id`, falling back to the bond discount curve. Bumps are configured bp and use central currency P&L per one bp; effectively zero bump returns zero.

Bucketed DV01 always uses the quote-reproducing view and delegates triangular key-rate bumps to the framework, independently of Workout versus CallableOas. It can therefore have a different risk basis from default parallel `Dv01`; summing buckets need not recover the quoted-yield number.

Effective duration/convexity for optional CallableOas risk fixes OAS, reprices base and selected-tree-curve shifts of ±25 bp, and computes `D=(P_down−P_up)/(2P0 h)` and `C=(P_up+P_down−2P0)/(P0 h²)`. Near-zero base PV (`1e−10`) returns both zero. Reported bond convexity divides this `C` by 100. Exercise/default are recalculated under each market. This finite shock is distinct from the sensitivity framework's smaller configurable DV01 bump.

### Limits, alternatives and evidence

Shared bump interpolation/recalibration is owned by the framework/core-market records. The bond layer does not hold exercise fixed during model risk or recalibrate OAS on every bump. Generic combined risk and the optional quoted single-tree-curve route can bump different dependency sets. At exercise boundaries these finite differences depend on shock size; stochastic regression/pricing error can enter a central difference. Direct market-quote revaluation and frozen-cashflow risk are meaningful alternatives but answer different questions.

Owning code: [pinning/routing](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/risk_view.rs), [parallel/bucketed DV01](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/dv01.rs), [effective risk](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/effective.rs), [generic curve sensitivity](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/dv01.rs). [Bucket tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/metrics/bucketed_dv01.rs) check that quoted price does not force bucket risk to zero; [duration tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/metrics/duration.rs) inspect risk-basis choice. These establish routing/identities, not bump convergence over the supported instrument domain. Reference: [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income).

## Independent arithmetic and qualification

- Let `V=0`, `S=.5`, buyer cash `100` at `2`, retained coupon `5`
  at `.75`, security curve `.04` continuously compounded and separate
  financing rate `.03`. Against dirty `Q=90`, the continuous model
  spread is about `+.020180257829`, `J=88.660074564276` and
  `H=93.439415794507`. Applying `±1 bp` to the security source at
  fixed spread gives holder DV01 `−.018090465624`. Buyer-at-valuation
  and settlement-dirty DV01s are `−.017732015031` and
  `−.018000000120`. Starting afresh at `S` with the same shifted rate
  produces dirty `91.368410379966` and misses the original quote.
- On a native clock with `S=.5`, payment at `2`, unshifted origin
  factors `.96` and `.80`, and terminal cash `100`, a semiannual
  settlement Z of `.05` gives dirty `77.718112141713`. Shifting
  both origin factors by the same nominal spread and taking their
  ratio gives `77.718374754969`. The nominal shift and the relative
  factor calculation generally do not commute.
- With `V=S`, a two-year payoff centered at `97` and `±50 bp`,
  the parallel rate move gives DV01 `−.019400323335`, effective
  duration `2.000033333500`, and effective convexity `4.000033333444`.
  Two buckets at `1` and `3` years give a finite sum
  `−.019400080833` even though the weights partition unity. On
  a matched one-period par floater, explicit sharing of discount and
  projection cancels exactly under a source shock when the floating
  growth and payment discount are exact reciprocals.
- For `H(x)=100−400x+600x²`, with decimal offsets `−.01,+.02`,
  the three-node effective duration and convexity are exactly `4`
  and `12`. The endpoint DV01 is `−.0394`. For the illustrative
  callable kink `min(100,101−400x)`, the `±50 bp` shock gives
  effective duration `1` and convexity `−400`. At `±25 bp` all
  three prices are `100` and both finite measures are zero.
- Two synthetic pricing groups `(H0,H+,H−)=(80,79,82)` and
  `(120,116,126)` with `±100 bp` produce mean prices
  `(100,97.5,104)`. They yield effective duration `3.25`,
  convexity `150` and DV01 `−.0325`. Averaging path-group
  duration ratios would give a different number.

Independent 60- to 70-digit checks reproduce the anchor and curve
examples. Exact rational checks reproduce the stencil, kink and
paired-group examples.

Qualification requires independent dated anchors and holder/settlement
bridge fixtures, finite-step and grid refinement away from kinks,
reported up/down asymmetry at exercise switches, and separately
qualified sampled policy and pricing work. Cover signed or nearly zero
centers, valuation and settlement cuts, floating projections and live
make-whole roles, shared and distinct typed sources, buckets between
and outside knots, frozen hazard and rate-responsive model rebuild,
invalid or collapsed moves, failure isolation, and quote/metric order
independence. Reconcile standalone, bundled, portfolio and host values
on identical bases. Independent arithmetic does not certify the live
implementation or a market/vendor measure.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
