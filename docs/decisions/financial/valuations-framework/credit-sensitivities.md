# VALUATIONS-FRAMEWORK-10 — Quote-space CS01, direct-hazard and z-spread alternatives, and recovery risk

- Decision status: **Accepted — complete checked credit sensitivities package, 2026-11-13**.
- Implementation status: **Observed, with reconciliation pending** across shared calculators, calibration replay, product callers and hosts.
- Validation status: Source and primary references inspected. Independent bounded quote, hazard, recovery and settlement examples checked. Runtime, calibration replay and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked credit sensitivities

1. **Declare the risk experiment and freeze its valuation basis.** Select
   par-quote CS01, direct intensity risk, or periodic z-spread risk by the
   registered metric and eligible product terms. Resolve the actual active
   targets, amount and cash basis, effective date and information phase,
   currency, position, dispatch and explicit step controls. Hold the deal
   terms and recognized history fixed except for an input expressly changed
   by the selected experiment. Use comparable reprices and checked signed
   stencils under [record 09](finite-difference-rates.md#accepted-decision-for-checked-finite-differences-and-rate-risk).
   Missing recipes, inactive declarations and lookup failures cannot select
   a different financial experiment. A supported bond with no hazard follows
   the accepted [no-hazard z-spread decision](../valuations-bonds/0011-bond-cs01-without-hazard-curve.md).
   An active hazard with missing required data is an error. An unsupported
   request is unavailable. Return structural zero only when the owner
   establishes no selected exposure after admission.

2. **Bind exact quote inputs and qualify each replay.** Resolve every
   selected active hazard and its own exact discount binding, recovery and
   contractual conventions. Its calibration discount can differ from the
   deal's discount. Require a lossless original recipe and a qualified
   par-risk view, keeping an original upfront quote separate from its
   derived par-risk input. Use exact quote IDs or verified recipe indices
   matched to those IDs. Preserve source and target market roles and
   require unshocked source reproduction and accepted target fits. A trade
   cash or display override cannot rewrite calibration terms. A newly
   authored quote center requires its own qualified market scenario.
   Remove first-curve selection, nearest quote matching and replay
   fallbacks. Rebuild a shared source once per scenario only when the
   complete bindings agree, then update all of its active references.
   [Accepted quote and hazard operations](../core-market/hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery),
   [accepted replay](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

3. **Report the exact quote-space CS01 experiment.** Require an explicit
   positive common step `h` in bp. Shift the complete selected par-quote
   set by `±h` for joint parallel, one selected source set at a time for
   per-source parallel, or one exact quote at a time for bucketed risk.
   Verify every required represented move. Hold the discount market,
   recovery and deal cash fixed. Refit the affected sources and reprice
   the full position on each side. Report `(V+−V−)/(2h)` in holder
   currency per positive 1 bp, with checked arithmetic and identified
   targets. Use `cs01::<curve>` and `bucketed_cs01::<curve>::<quote>`
   for the single-source and exact quote series. Quote IDs supply identity,
   while dates and rounded tenors serve as metadata. Include the full
   selected curve set in the joint parallel basis. A finite sum of buckets
   or separately bumped sources is a separate result. Preserve coupon
   risky PV01 as a different cash coefficient. Retain owner-specific
   CDS, index, option and tranche measures and fit/price tolerances.
   [White, §8](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf),
   [Strata quote buckets](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/credit/SpreadSensitivityCalculator.html).

4. **Report direct intensity shocks as hazard01.** Shift canonical
   annual intensity parameters additively while holding recovery fixed.
   Use one explicit parallel, exact ending-parameter or triangular
   shape. For triangular risk sample the weights at the actual ending
   knots. The last parameter's weight applies through its constant tail.
   Admit only finite nonnegative changed intensities and validate the
   whole move. Report signed holder currency per intensity bp under
   `hazard01::<curve>` and `bucketed_hazard01::<curve>` with exact
   coordinates. Remove LGD scaling and its near-zero fallback. Use
   central when both full directions are admissible. Where the owner
   permits one-sided risk and exactly one full direction crosses the
   nonnegativity boundary, use the other full move and the same qualified
   center. Select before pricing. Missing data, failed reprices and lost
   moves propagate. An actual parameter change gives no current fit
   authority for the old quote set.

5. **Prepare settlement z-spread cash and its anchor explicitly.** Use
   the product's admitted quote anchor date, entitled signed cash and
   currency, dated relative discounts, and complete contextual spread
   clock. Settlement-date cash follows the owner's entitlement rule.
   A zero-time row requires a base factor of one for this profile and
   has zero spread sensitivity after admission.
   The shared periodic profile uses the declared positive integer
   frequency and adds annual decimal `z` to each base periodic zero rate
   under [the accepted periodic kernel](../core-numerics/compounding.md#accepted-decision-explicit-compounding-conversion-and-periodic-spreads).
   Keep quote compounding independent of coupon frequency. Distinguish
   quote-date dirty value from valuation-date PV using the accepted owner
   bridge for detached cash and settlement discounting. Preserve the bond's
   [required quote compounding](../valuations-bonds/12-spread-quotes.md#accepted-decision-required-quote-compounding)
   across pricing, inversion and sensitivity. Select an explicit
   authored z-spread, an implied spread from an admitted dirty quote, or
   the explicitly admitted unquoted zero-spread model basis. Conflicting
   active anchors error. A required dirty quote or failed solve cannot
   become a zero-spread anchor. Other quote compounding conventions
   retain their product-owned admission and are outside this shared
   periodic kernel. [Strata bond pricing and quote-date amounts](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/bond/DiscountingFixedCouponBondProductPricer.html).

6. **Qualify a z-spread inverse on the same amount basis.** Match the
   actual dirty target, currency, quote date, quantity and entitled cash.
   Require a finite owner-qualified admissible bracket and an identified
   root, or an explicit owner rule selecting a root on a supported domain.
   Each selected positive-time cash row must admit its shifted periodic
   rate and factor. Nonnegative cash with at least one positive amount at
   positive time gives a strictly decreasing price. Signed cash can have
   multiple roots.
   Use the [accepted fallible Brent solver](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search)
   with explicit work and coordinate controls. Require a separate price
   residual gate in native currency on the returned spread. Propagate
   pricing errors directly and remove the synthetic `1e12` objective.
   A flow set with no spread exposure can have zero risk while providing
   no identifiable implied spread.

7. **Preserve the z-spread metric's stencil and attribution.** The shared
   periodic cash-band route uses the checked central spread stencil.
   A product with a separately defined +1 bp price change retains that
   basis and discloses it. Keep `cs01::<instrument>` for qualified bond
   z-spread risk under [ADR 0010](0010-metric-identifiers.md). A credit
   quote bucket or optional-bond OAS has its own economics and cannot
   stand in for the required z-spread experiment. When the owner admits
   cash bands, put each positive-time flow in the first upper bound
   `≥t`, with the last band also containing the upper tail. Use exact
   increasing positive spread-clock coordinates and exact keys. Empty
   admitted bands have zero contribution. Return each requested band once
   the risk basis is admitted, including a qualified all-zero series.
   With fixed cash and factors,
   a complete disjoint band sum reconciles algebraically to the same
   central parallel price difference before rounding. This claim requires
   one common anchor, stencil and amount basis. A product with no
   admitted bucket route does not report parallel as a bucket series.

8. **Select the recovery experiment independently of provenance.**
   Frozen-intensity recovery preserves the actual intensity, clock and
   conditional survival. Change contractual cash recovery, source recovery
   metadata or both only according to the owner-approved binding. When
   independent contractual recovery alone changes, retain the original
   source and fit. A source metadata change does not carry old current-fit
   authority. Quote-preserving recovery holds original observations and
   the discount market fixed, changes the admitted contract and calibration
   recoveries coherently, and refits every affected source. Check source
   conflicts and scope spillover and keep realized recoveries and settled
   cash fixed. Report holder currency per `+.01` absolute recovery. Use
   the owner's approved exact affine coefficient where applicable. For
   finite differences use an explicit positive step and precheck both
   complete recovery vectors, including the actual stricter replay domain.
   For the admissible two-sided common amplitude use
   `.01(V+−V−)/(2h)`, with `h` in absolute recovery fraction.
   An owner-approved one-sided rule uses the full admissible direction and
   the qualified center when the other direction crosses a bound, scaling
   the signed difference by `.01/h`. If
   neither direction qualifies, fail. Remove member-wise clamps, width
   cutoffs and recipe-presence dispatch. Retain actual moves, stencil and
   pricing allowances. [CDS and index recovery](../models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk),
   [CDS option recovery](../models-and-valuations-credit/16-cds-option-risk-and-inversion.md#accepted-decision-for-checked-cds-option-risk-and-implied-volatility),
   [tranche recovery](../models-and-valuations-credit/19-tranche-risk.md#accepted-decision-for-checked-tranche-risk-and-default-scenarios).

9. **Keep the instrument-owned rate component separate.** An approved
   internal rate metric moves only its declared rate component under its
   product clock and cash/model basis. Preserve `dcf::wacc01` and
   `real_estate::discount_rate01` with their named meanings. Require the
   zero-move preparation to reproduce the qualified base. Use the checked
   positive-up stencil per bp and expose the actual moved inputs. Keep
   financing-market DV01 and credit risk separate. DCF and property
   owners establish which terminal and discount terms respond and when
   an admitted route has structural zero.

10. **Reconcile existing pricing, replay and reporting paths together.**
    Keep calibration with its provider and product economics with their
    owners. Use the same typed targets, checked results, source and stencil
    provenance, and financial basis in native, registry, portfolio and
    attribution consumers and the existing Python/WASM routes. Preserve
    the selected metric identifiers and coordinate schema through reports
    and wire checks. Qualify independent quote fits, multi-curve coverage,
    boundary stencils, settlement and signed cash, and host behavior.
    Fit acceptance alone does not establish sensitivity accuracy.

## Observed behavior

### Choice and selection

There are three economically distinct credit-risk methods. Generic CS01 requires a lossless hazard calibration recipe and a recalibration provider. Bond-like `ZSpreadCs01` registrations can explicitly remain z-spread-only or delegate when a credit dependency exists: replayable hazard selects quote-space CS01, while an unreplayable hazard selects the direct-hazard approximation. This selection is observed behavior, not approval to treat the measures as interchangeable.

### Algorithms and conventions

**Quote-space CS01.** Rebuild the hazard curve with every calibration spread quote shifted by ±`b` bp, keep the requested discount curve/conventions and any deal quote override, reprice under the selected dispatch, and report `(PV_up−PV_down)/(2b)`. Parallel `b` must exceed `1e−10` bp and be finite. Bucketed CS01 uses exact source calibration quote indices exposed by the provider, not the generic triangular tenors. Rebuild once per up/down quote, store provider labels, and sum using compensation. Generic curve resolution chooses the **first** declared credit and discount curves; it does not aggregate all credit curves. A missing required curve/recipe/provider fails.

**Direct-hazard approximation.** Set hazard bump `b_h=b/(1−R)` when LGD lies in `(1e−6,1]`, otherwise `b_h=b`. The shared calculator shocks in parallel or enumerates existing ending tenors for individual buckets and normalizes back to spread bp. This uses the approximation `spread≈hazard×LGD`, with recovery fixed. If a down-hazard bump fails, use the up/base one-sided derivative. The fallback catches any down-bump error, including failures unrelated to nonnegativity.

**Z-spread CS01.** Cache only cashflows strictly after settlement as `(time, settlement-to-payment DF, amount)`, with time on the discount curve's day count. Apply the core periodic-compounding z-spread discount factor and compensated sum. Anchor priority is an explicit z-spread quote, a spread solved to the supplied dirty quote, then zero. The Brent anchor solve starts at zero with ±0.05 bracket size, tolerance `1e−10`, and verifies reproduced price within `1e−8×max(|target|,1)`; pricing failures inside the objective return `1e12`. Parallel risk uses `z±b/10000` and is keyed by instrument ID. Buckets assign each cashflow to the first tenor at/after its time, with all remaining cash in the final bucket. These are disjoint cashflow bands, **not triangular curve shocks**. Empty future cash yields zero.

**Recovery01.** Shock recovery ±0.01, clamp each to `[0,0.999]`, divide by the actual available width, and report PV change per +0.01. Use an up/base or base/down difference if the opposite distance is at most `1e−6`; return zero if neither side moves. A replayable hazard is recalibrated with changed recovery; any replay failure errors rather than reverting to frozen hazard. An unreplayable curve instead keeps hazard knots fixed and updates its recovery metadata. An explicit trade recovery is shifted alongside curve recovery, preserving their base offset subject to individual clamps.

The separate risk-free-component DV01 interface asks an instrument to shift its internal rate by ±`b/10000`. Live uses include corporate WACC and property discount rate. The framework returns the signed PV-per-bp central difference. Product implementations define which terms move.

### Limitations

- The common `sensitivity_central_diff` helper returns zero for `|b|≤1e−10` in release builds and debug-asserts in debug builds, unlike the guarded generic width helper. Some callers only enforce strictly positive `b`.
- Z-spread bucket labels use rounded display labels. Distinct custom fractional-year buckets can therefore collide when stored as qualified metrics, unlike exact DV01 labels.
- Provider replay is an execution dependency; this record does not certify the external calibration algorithm.

## Evidence and alternatives

[cs01.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01.rs), [cs01_direct_hazard.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01_direct_hazard.rs), [cs01_z_spread.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cs01_z_spread.rs), [recovery01.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/recovery01.rs), [rf_component_dv01.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/rf_component_dv01.rs), and [recalibration.rs](../../../../finstack-quant/valuations/core/src/recalibration.rs) provide the formulas, delegation and replay contract. Inline tests cover behavior and formula reconciliation; none was run. [Duffie–Singleton](../../../REFERENCES.md#duffie-singleton-1999) provides general intensity/recovery context. Independently calibrated quote-bump benchmarks, direct-hazard approximation error, boundary stencils and key-label uniqueness remain adoption evidence gaps.

### Independent review evidence

- In a stylized two-year calibration with unit discount, recovery `.40`
  and annual premiums paid only on surviving payment dates, survival is
  `S1=.6/(.6+q1)` and `S2=(.6−q2 S1)/(.6+q2)`. With quote pairs
  `(.012,.020)` and `(.030,.035)`, the value is `100 S2(A)+60 S2(B)`.
  Central ±1 bp shocks of `A1`, `A2` and `B1` give
  `+.000516757423`, `−.030911428851` and `+.000499937520` per bp.
  Their sum is `−.029894733907` and the joint shock gives
  `−.029894734719`. This distinction is at finite step size.
- For a one-year recovery-of-par claim with unit discount,
  `V=100[R+(1−R)exp(−λ)]`. At `R=.40` and `λ=.00005`, a 1 intensity
  bp down step is outside the domain. The full up/base step gives
  about `−.00599940003250` per intensity bp. For ending knots 1y
  and 3y at levels `.02,.04`, integrated hazard through 5y is `.18`.
  Adding one intensity bp at 3y gives `.1804`, including the tail.
- The same one-year claim at `R=.40`, `λ=.10` has frozen Recovery01
  `+.0951625819640`. In this stylized constant-hazard case only,
  continuous premium at quote `.06` gives `λ=.06/(1−R)`. Refitting
  at `R±.01` gives central Recovery01 `+.00468005530359`. With
  recoveries `.002,.998` and a common `.003` step, each full direction
  breaches a different bound even in the closed cash domain.
  [White, §4.6](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf).
- For frequency `2`, times `.5,1,2`, base periodic rates
  `.03,.04,.05`, cash `4,6,110` and spread `.01`, PV is about
  `107.3660302753`. Central ±1 bp gives `−.019726786886` per bp.
  Bands ending at `1` and `1.4` contribute `−.000749393406`
  and `−.018977393480`. The +1 bp price change is `−.019724433613`.
  Separately, annual compounding, unit factors and signed cash
  `221,−121.2` at times `1,2` admit both `.01` and `.20` as spreads
  reproducing dirty value `100`.
- A one-year appraisal claim of `100` at a 10% annual discount rate
  gives `−.0082644628782` currency per bp at a central ±1 bp step.
  Fixed financing cancels from this difference on the same basis.

Production qualification requires actual provider replay, dated
settlement, price and risk benchmarks, and existing host checks.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
