# CALIBRATION-TARGETS-005 — CDS hazard calibration and par-spread risk representation

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Relationship to accepted intensity sources

[CORE-MARKET-04](../core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation)
was accepted on 2026-11-08: explicit source-clock/context selection and strict
positive ending knots, no core hazard cap, and local intensity-based probability
evaluation. Reconcile source construction and repricing to that contract.
The target's explicit solver/fit bounds, recovery assumptions, quote selection
and market acceptance remain this calibration owner's separate Draft choices.
The observed cap mismatch below remains unreconciled in runtime.

## Relationship to accepted quote/risk boundaries

[CORE-MARKET-05](../core-market/hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery)
was accepted on 2026-11-08: remove incomplete core quote/interpolation fields,
preserve lossless original/par-space risk inputs, and use explicit quote
recalibration with exact bindings/dependencies. Direct intensity risk and
frozen-intensity recovery replacement remain distinct operations. Reconcile
reporting/replay construction to that contract; this does not adopt the target's
remaining solver, fit or quote-domain choices.

## Choice and domain

Calibrate nonnegative, ending-knot constant hazard rates against complete CDS contract PV, with fixed curve recovery. Par spreads and upfront-plus-running quotes enter the same PV/N residual. The spread-over-LGD approximation is only an initial guess, never the calibration objective. The curve's survival interpolation is intrinsically log-linear; `par_interp` affects stored indicative spread reporting.

## Algorithm and controls

1. Extract CDS quotes; public preflight validates entity, currency and recovery agreement within configured tolerance, positive notional and quote domains. Resolve the first quote's CDS convention; subsequent conventions must have matching currency, family, day count and calendar. Optional parameter doc clause must match exactly or belong to the same family. Without quote context the registry primary family (fallback IsdaNa) chooses a key. Build quote instruments with discount/credit IDs and optional CDS valuation convention.
2. Pillar time uses the resolved CDS maturity and convention day count. Bootstrap begins with no artificial origin hazard. Construct a HazardCurve with base date, issuer/seniority/currency, fixed recovery and trial hazard knots; sync a matching credit-index reference in scratch context. Residual is full instrument raw NPV divided by caller notional.
3. Initial hazard is `(spread_or_running_coupon_bp/10000)/max(1-R,minimum_lgd_for_hazard_guess)`, clipped to configured hazard bounds (default `[0,10]` per year). Prefer finite curve recovery; an internal NaN sentinel falls back to quote recovery, although the public path requires consistency. If the guess fails, use the preceding hazard or 0.01.
4. Scan includes lower/upper bound and center, 48 log-spaced values from up to four decades below to two above center (positive floor `1e-10`), plus 16 upper-tail log points to configured maximum when needed. Sorted values are deduplicated at absolute `1e-12`. Unlike discount/base correlation, this target retains the default exhaustive bracket search.
5. GlobalSolve takes positive finite quote times, seeds from an existing same-ID hazard curve when available, otherwise spread/LGD or 0.01, and sorts/rejects duplicate maturities. Parameters remain direct hazards; validate each against configured bounds before building. Weights are only the selected time scheme floored at `1e-3`; no annuity factor. Default fit budget is `1e-8` PV/N.
6. After fitting, retain original quote payloads and resolved date/time pillars. Par quotes remain unchanged for spread risk; upfront quotes are repriced to a **par spread** on the calibrated full market while retaining ID/entity/convention/recovery/pillar. Attach original calibration inputs, par-space risk inputs and serialized params/config to a validated replay recipe; rebuild with derived par points and requested interpolation. See [017](17-calibration-replay.md).

## Discrepancies and alternatives

The target accepts a configured `hazard_hard_max` above 10 but does not forward it into `HazardCurveBuilder::max_hazard_rate`; the core builder retains its default 10/year cap. Such trials can fail construction despite satisfying the target's configured domain. This is a confirmed static control mismatch; the frequency and numerical impact have not been reproduced. Separately, post-fit hazard validation has its own policy bounds in the [framework](../calibration-framework/16-curve-validation.md); widening a solver bound does not automatically widen adoption policy.

Alternatives are log hazards/cumulative-hazard controls, direct spread bootstrap with exact CDS repricing, or joint recovery/hazard fitting. Fixed recovery and one parameter per maturity are material identification assumptions. Global filtering of nonpositive times differs from bootstrap's explicit rejection; fit reports and replay still require carefully defined active inputs.

## Evidence

- Owning [hazard.rs](../../../../finstack-quant/calibration/src/targets/hazard.rs); [CDS construction](../calibration-framework/07-cds-materialization.md); [core survival](../core-market/hazard-survival.md).
- [tests/hazard_curve.rs](../../../../finstack-quant/calibration/src/tests/hazard_curve.rs) includes rejection, distressed spread, smoothness and replay-tampering cases; selected par repricing uses the production CDS builder/pricer. [tests/repricing.rs](../../../../finstack-quant/calibration/src/tests/repricing.rs) checks report versus market-built CDS and upfront-to-par replay. Scan resolution/monotone survival tests are located inline. They are integration/invariant evidence, not external ISDA standard-model certification.

The ISDA model version label is provenance metadata, not approval.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
