# CALIBRATION-TARGETS-005 — CDS hazard calibration and par-spread risk representation

- **Decision status:** Accepted 2026-11-15 — complete single-name CDS hazard calibration package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent settlement, intensity-domain and zero-par calculations verified. Production CDS fitting, market references, restoration, replay and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for explicit single-name hazard calibration and par-risk qualification

1. **Fit one exact credit profile at fixed recovery.** Select a nonempty
   set of eligible CDS quotes for one reference entity, seniority,
   currency and legal credit profile. Resolve each quote's dated
   convention or complete bespoke terms. Freeze the independent
   discount source, fitted hazard role, trade date and valuation close.
   Require exact effective recovery agreement in `[0, 1)` across quotes,
   prepared contracts and the fitted source. Reject expired protection
   and rows without genuine live hazard exposure, including contracts
   with an earned but unpaid coupon. Retire first-quote convention
   inference, regional fallbacks and silent quote filtering.
   [Accepted CDS preparation](../calibration-framework/07-cds-materialization.md#accepted-decision-for-single-name-cds-calibration-and-quote-settlement).

2. **Fit signed protection-buyer raw PV per notional.** Require explicit
   positive notional and reuse the full CDS pricer. Positive observed
   running par uses its declared clean or dirty basis with a finite
   positive selected annuity. Preserve the contractual coupon and signed
   fraction for upfront. Clean upfront `u` gives signed buyer cash
   `N(u − cα)` after accrued premium. Actual settlement fraction `q`
   gives `Nq`. Use one checked settlement cash conversion and discount
   eligible cash once at its actual payment date. Require explicit
   same-day inclusion. Positive cash is paid by the protection buyer.
   [Accepted CDS legs](../models-and-valuations-credit/12-cds-legs.md#accepted-decision-for-checked-cds-legs-and-quote-basis) ·
   [ISDA converter specification](https://www.cdsmodel.com/assets/cds-model/docs/ISDA%20Standard%20CDS%20Contract%20Converter%20Specification%20-%20Sept%204%2C%202009.pdf).

3. **End constant annual intensity segments at literal contractual
   maturities.** Require an explicit hazard base date, source clock and
   calendar context. Use finite nonnegative `λᵢ` on `(tᵢ₋₁, tᵢ]`,
   implicit `S(0) = 1` and a constant final intensity. Keep rolled
   coupon, default cash and upfront payment dates separate. Reject
   nonpositive or colliding represented knot times. Simultaneous fitting
   may retain multiple independently identified rows at the same
   contractual maturity against one knot. Remove the hidden core cap of
   10/year. An elected economic cap is a separate final check over
   its declared segments and tail.
   [Accepted intensity source](../core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation).

4. **Elect bootstrap or simultaneous fitting explicitly.** Bootstrap
   requires one identifying quote per new segment and proof that later
   segments cannot change its pricing inputs. Require explicit finite
   intensity bounds, seed, scan coordinates, order, tolerances and work
   limits. Admit a valid computed zero or a Brent result from a qualified
   continuous bracket, followed by the native fit gate. Simultaneous
   fits require aligned native seeds or an elected qualified private
   bootstrap seed. Use the open logistic map for free bounded
   intensities and keep fixed values and elected endpoints, including
   exact zero, outside the optimizer vector. Use checked central
   differences, augmented QR, explicit finite nonnegative weights and
   actual identification of every free segment. Retire implicit
   prior-curve and spread-over-LGD seeds, LGD and weight floors,
   hidden scan grids and automatic fallback.
   [Accepted bootstrap](../calibration-framework/10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing) ·
   [Accepted scalar search](../calibration-framework/11-scalar-root-search.md#accepted-decision-for-bounded-scalar-search-and-candidate-evidence) ·
   [Accepted weighted solve](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates).

5. **Gate the exact delivered market before publication.** Use the
   fitted hazard's actual output role and the frozen discount state
   for every trial, probe and final reprice. Report every consumed
   quote in authored order and apply its unweighted native tolerance,
   including at zero weight. All-fixed curves undergo the same gates
   with zero optimizer work. Validate actual pricing support, elected
   economic checks and affected bindings. Publish the evaluator,
   construction record and fit report atomically. Keep solver
   termination separate from financial acceptance.
   [Accepted fit reports](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted curve validation](../calibration-framework/16-curve-validation.md#accepted-decision-for-curve-validation-and-publication).

6. **Preserve original quotes and qualify par-spread risk separately.**
   Keep original par observations literal. Retain each original
   upfront's coupon, amount and basis. Derive its paired model-implied
   par in bp through the canonical CDS pricer on the delivered market
   and an explicit clean or dirty basis. Bind both views to the exact
   quote, terms, maturity and recovery. Remove indicative par
   interpolation. Preserve a valid zero model-implied par and the
   valid original fit. A spread replay requiring that row fails
   positive-par admission without flooring or dropping it.
   [Accepted CDS observations and risk](../core-market/hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery) ·
   [Accepted recipe bindings](17-calibration-replay.md#accepted-decision-for-exact-rate-and-hazard-construction-and-qualified-replay).

7. **Replay the selected risk basis on the explicit market.** Requalify
   the original fit and its dependencies even for zero requests.
   Spread risk fits an explicit par-risk center and applies signed bp
   moves to exact quote targets. Dependency changes and recovery refits
   preserve original quote forms. Recovery refits hold discount fixed,
   preserve historical recovery and derive a fresh par-risk view.
   Direct intensity risk and frozen-intensity recovery remain separately
   elected operations. Deal and horizon scenarios retain their existing
   contracts.
   [Accepted hazard replay](../calibration-framework/21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication) ·
   [Accepted deal and horizon replay](../calibration-framework/22-hazard-deal-and-horizon.md#accepted-decision-for-deal-overrides-and-horizon-hazard-replay).

## Independent calculation checks

- On 10 million notional, 500 bp coupon, accrual fraction `.04`,
  dirty annuity `.42`, protection PV `.02002` per notional and
  settlement factor `.98`, clean upfront `+.001` gives buyer cash
  `−10,000` and exactly zero raw fit residual. Treating `+.001`
  as the actual cash fraction instead gives `−19,600` raw PV.
- At constant intensity 12/year over `1/12` of a source-clock year,
  default probability is `1 − exp(−1)`, about 63.2121%. The source
  intensity is valid under the accepted domain independently of
  any elected final economic cap.
- At zero intensity with unit settlement factor, dirty annuity
  `.20`, accrued fraction `.025` and coupon `.05`, clean upfront
  `−.00875` exactly offsets the premium PV. The model-implied par
  is zero, so the original quote fits and the row is ineligible for
  the positive-par replay domain.

## Observed choice and domain at the audit revision

Calibrate nonnegative, ending-knot constant hazard rates against complete CDS contract PV, with fixed curve recovery. Par spreads and upfront-plus-running quotes enter the same PV/N residual. The spread-over-LGD approximation is only an initial guess, never the calibration objective. The curve's survival interpolation is intrinsically log-linear; `par_interp` affects stored indicative spread reporting.

## Observed algorithm and controls

1. Extract CDS quotes; public preflight validates entity, currency and recovery agreement within configured tolerance, positive notional and quote domains. Resolve the first quote's CDS convention; subsequent conventions must have matching currency, family, day count and calendar. Optional parameter doc clause must match exactly or belong to the same family. Without quote context the registry primary family (fallback IsdaNa) chooses a key. Build quote instruments with discount/credit IDs and optional CDS valuation convention.
2. Pillar time uses the resolved CDS maturity and convention day count. Bootstrap begins with no artificial origin hazard. Construct a HazardCurve with base date, issuer/seniority/currency, fixed recovery and trial hazard knots; sync a matching credit-index reference in scratch context. Residual is full instrument raw NPV divided by caller notional.
3. Initial hazard is `(spread_or_running_coupon_bp/10000)/max(1-R,minimum_lgd_for_hazard_guess)`, clipped to configured hazard bounds (default `[0,10]` per year). Prefer finite curve recovery; an internal NaN sentinel falls back to quote recovery, although the public path requires consistency. If the guess fails, use the preceding hazard or 0.01.
4. Scan includes lower/upper bound and center, 48 log-spaced values from up to four decades below to two above center (positive floor `1e-10`), plus 16 upper-tail log points to configured maximum when needed. Sorted values are deduplicated at absolute `1e-12`. Unlike discount/base correlation, this target retains the default exhaustive bracket search.
5. GlobalSolve takes positive finite quote times, seeds from an existing same-ID hazard curve when available, otherwise spread/LGD or 0.01, and sorts/rejects duplicate maturities. Parameters remain direct hazards; validate each against configured bounds before building. Weights are only the selected time scheme floored at `1e-3`; no annuity factor. Default fit budget is `1e-8` PV/N.
6. After fitting, retain original quote payloads and resolved date/time pillars. Par quotes remain unchanged for spread risk; upfront quotes are repriced to a **par spread** on the calibrated full market while retaining ID/entity/convention/recovery/pillar. Attach original calibration inputs, par-space risk inputs and serialized params/config to a validated replay recipe; rebuild with derived par points and requested interpolation. See [017](17-calibration-replay.md).

## Observed discrepancies and alternatives

The target accepts a configured `hazard_hard_max` above 10 but does not forward it into `HazardCurveBuilder::max_hazard_rate`; the core builder retains its default 10/year cap. Such trials can fail construction despite satisfying the target's configured domain. This is a confirmed static control mismatch; the frequency and numerical impact have not been reproduced. Separately, post-fit hazard validation has its own policy bounds in the [framework](../calibration-framework/16-curve-validation.md); widening a solver bound does not automatically widen adoption policy.

Alternatives are log hazards/cumulative-hazard controls, direct spread bootstrap with exact CDS repricing, or joint recovery/hazard fitting. Fixed recovery and one parameter per maturity are material identification assumptions. Global filtering of nonpositive times differs from bootstrap's explicit rejection; fit reports and replay still require carefully defined active inputs.

## Evidence

- Owning [hazard.rs](../../../../finstack-quant/calibration/src/targets/hazard.rs); [CDS construction](../calibration-framework/07-cds-materialization.md#accepted-decision-for-single-name-cds-calibration-and-quote-settlement); [core survival](../core-market/hazard-survival.md).
- [tests/hazard_curve.rs](../../../../finstack-quant/calibration/src/tests/hazard_curve.rs) includes rejection, distressed spread, smoothness and replay-tampering cases; selected par repricing uses the production CDS builder/pricer. [tests/repricing.rs](../../../../finstack-quant/calibration/src/tests/repricing.rs) checks report versus market-built CDS and upfront-to-par replay. Scan resolution/monotone survival tests are located inline. They are integration/invariant evidence, not external ISDA standard-model certification.

The ISDA model version label is provenance metadata, not approval.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
