# CORE-MARKET-14: Explicit volatility grids and native quote-parameter shocks

- Decision status: **Accepted — complete volatility-grid/shock package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths below; the accepted source, evaluator and shock reconciliation is unimplemented.
- Validation status: **Primary/source evidence and independent grid/clock, numerical/shock and consumer/host reviews inspected; standalone rational/high-precision examples executed 2026-11-08; production qualification pending.** Existing Rust/host tests were inspected or located, not executed. Examples do not certify production interpolation, risk or absence of arbitrage.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns VolSurface's annual-volatility quote grid, financial metadata,
admission and direct quote-parameter shocks. It preserves accepted
[ADR0045's shared volatility clocks and dated-use boundary](../conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility)
and the [explicit generic unit boundary](scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings).
The clock package remains optional for pure numeric sources and requires actual
source mapping at dated use; this proposal does not reopen that decision.

[Model interpolation](../models-analytic/volatility-source-interpolation.md),
[Black/normal/shifted pricing](../models-analytic/forward-normal-shifted.md),
[surface diagnostics](../models-analytic/surface-arbitrage-diagnostics.md),
[calibrated materialization](../calibration-targets/10-total-variance-materialization.md),
[calibration acceptance](../calibration-framework/17-surface-validation.md),
[scenario diagnostics](../scenarios/09-volatility-surface-shocks.md) and
[context aging](context-lifecycle.md) retain their remaining Draft choices.
This record defines source/evaluator compatibility without adopting their full
off-grid kernels, tails, price models, arbitrage thresholds or aging dynamics.

## Foundational findings

The current Strike label also says moneyness, although K, K/F and log(K/F) are
different coordinates. Normal and Black quotes have different physical units;
one numerical bump is not a universal vega unit. Adding displacement metadata
also silently retags unchanged quotes as shifted Black, without converting prices.

Shock paths disagree: additive/point operations floor negative results, direct
negative factors error, and bucket percentages below-100% instead floor to0.
Targets can clamp, snap or tolerance-match multiple nodes. Such behavior changes
the requested economic action and can distort finite-difference widths.

Finally, total-variance evaluation divides by max(T,EPSILON). It can therefore
change an exact tiny-expiry quote, or return0 after an unnecessary squared-vol
underflow. A stored annual sigma remains the parameter even when an evaluator
uses total variance as its working quantity.

## Accepted decision: explicit annual-volatility sources and exact grid shocks

Accepted on **2026-11-08** after independent grid/clock, numerical/shock and
consumer/host review. Runtime/caller/host reconciliation and independent
financial qualification remain pending; observed implementation stays separate below.

1. **Retain one explicit annual-volatility grid.** Require secondary-axis kind,
   quote type and Vol/TotalVariance interpolation-mode metadata at construction
   and on all host/wire paths; remove implicit Strike/Black/Vol defaults. Stored
   values are always annual sigma quotes, not total variance or option prices.
   BlackLognormal and ShiftedBlackLognormal sigma are relative units per square-
   root source year; Normal sigma is absolute underlying-quote units per square-
   root source year. Normal is not universally a decimal-rate volatility.
   Existing market/product owners bind actual asset/index, benchmark/fixed tenor,
   currency/price-rate scale or FX orientation and quotation identity. Verify
   agreement at consumption; no unit inference from IDs, numeric scale or quote
   type alone and no automatic normal/Black conversion.

2. **Admit a strict rectangular source without repairs.** Both axes must be
   nonempty, finite and strictly increasing, with checked rectangular size and
   expiry-major values. Expiry pillars are strictly positive; remove invented
   zero-expiry anchors, with no epsilon/minimum horizon or near-knot merging.
   Each sigma is finite and nonnegative, including0 and positive subnormals;
   impose no generic vol cap. Singletons are valid source artifacts; a consuming
   numerical method can require additional points explicitly. Do not sort,
   deduplicate, floor, clip or reconstruct missing rows.

   Strike means literal absolute strike K in the associated underlying units,
   never moneyness. For a Strike grid, Normal permits signed K, Black requires
   K>0, and shifted Black requires every stored K+s_row to be finite and>0.
   Tenor means a strictly positive underlying reference-tenor coordinate with
   explicit mapping at its owner, distinct from expiry/coupon accrual; it is not
   a strike and supplies no known K-domain check. Use secondary_values for the
   generic stored axis and secondary-coordinate filters, removing the ambiguous
   generic strikes field/accessor rather than retaining an alias. Moneyness
   inputs require an explicitly owned conversion using their actual forwards
   and units, not another alias inside this source.

3. **Keep displacement part of the actual quote convention.** Require exactly
   one finite signed displacement per expiry row for ShiftedBlackLognormal;
   Normal and ordinary Black carry none. Zero displacement is valid but does
   not silently change the declared convention. Shifts use the same units as
   forward and strike. Remove automatic quote retagging in with_displacements
   and builder attachment: select the convention explicitly and construct a
   complete compatible source. Changing displacement/convention is a new
   explicit financial construction or qualified quote conversion, not an
   ordinary volatility bump. Actual Black/shifted-Black consumers must also admit
   finite positive queried F+s and K+s, with s=0 for ordinary Black. Normal
   retains its finite signed F/K domain. Preserve convention/axis mismatch guards and
   strict unshifted-consumer refusals; source checks do not replace them.
   Off-grid displacement interpolation and tails retain their model owner.

4. **Preserve the accepted source clock and dated-use admission.** Add/preserve
   the existing accepted expiry_clock: Option<VolatilityClock>, using its actual
   calibration reference, day count and full required context. A pure numeric
   grid may omit it; coordinates then have only their declared numeric source
   meaning. Dated producers record the mapping actually used, and dated active
   consumers resolve complete metadata or an explicit matching source contract.
   Require agreement when both are supplied; no Act/365F fallback, inferred
   origin, curve-clock borrowing or retagging. Preserve clock metadata through
   shocks, clones, materializers and state.

   Ordinary direct dated pricing requires calibration origin=valuation as_of.
   A different origin needs an explicitly owned seasoned-source/rolling model.
   Cloning the old grid or querying it at remaining time is not that model;
   price/CPI frozen-level reuse does not authorize volatility aging. Source
   expiry and model remaining time retain the accepted positive-interval
   variance bridge only for operations supporting that representation. An
   inactive scalar-vol/lifecycle branch does not inspect unused surface data.

5. **Respect canonical sigma in every active evaluator.** An admitted exact
   grid query returns the stored sigma, including0, under either interpolation
   mode. Implied-volatility lookup requires finite positive requested expiry;
   actual zero-time/expired pricing branches have their own deterministic
   contract and need no fabricated zero-time vol quote. Vol mode uses sigma;
   TotalVariance selects W=T*sigma^2 as the interpolation quantity, with Normal
   W retaining squared underlying units. It is not an arbitrage certificate.
   Reconcile the existing evaluator to stable checked requested-output arithmetic:
   no max(T,EPSILON), unnecessary sigma^2 overflow/underflow, cached-zero repair
   or error-to-NaN. Do not require an unrequested W or sigma^2 to fit merely to
   return an admitted sigma. A separately requested unrepresentable variance
   can fail while sigma remains valid. Preserve deliberate supported query/tail
   domains at the model owner; this record does not select the complete off-grid
   algorithm or introduce a second core interpolation engine.

6. **Shock annual sigma parameters in explicit native units.** Use one checked
   replacement kernel for parallel, point and selected-grid changes. Support
   Additive/Fraction with finite native sigma delta b and Multiplicative/Factor
   with finite k>=0, giving sigma'=sigma+b or sigma'=sigma*k on selected nodes.
   Financial adapters explicitly convert absolute vol points, rate bp or relative
   percentages from their actual quote representation. Reject generic RateBp
   and Percent pairs rather than call .01 a universal vol point: for Black it
   is one absolute percentage point of relative vol; for Normal it is .01 of
   the actual underlying units. A relative1% move uses factor1.01.

   Admit intentional sigma0/k0. Reject negative changed quotes, negative factors,
   nonfinite results and a positive product lost to zero. Remove every additive,
   point and bucket floor; do not cap a stress at the domain boundary. A direct
   no-op preserves exact quotes after full admission. Update atomically, without
   changing ID unless explicitly requested, axes, quote type, units, displacements,
   clock or interpolation mode. These are sigma-parameter changes even under
   TotalVariance metadata; they are not variance shocks, continuous off-grid
   sigma shifts, SABR/SVI parameter changes or option-quote recalibration.

7. **Resolve exact source targets without nearest-node policy.** Parallel
   selects all nodes. A point requires an exact stored expiry/secondary pair
   or an explicitly resolved in-range row/column identity on that source.
   Bucket selectors are the Cartesian product of supplied exact expiry and
   secondary coordinate sets: None means the full axis and [] is an intentional
   empty selection. Require finite unique coordinates and an exact match for
   every supplied target, with no tolerances, clamping, expiry snapping or
   silent unmatched no-op. Validate the complete source/request, axis meaning,
   units, values and all filters before empty/identity shortcuts. Reports identify
   explicit empty selections as intentional no-ops. Date/tenor selectors must
   first map through the actual admitted source clock and appropriate coordinate
   owner; scenario as_of/Act365F and a proximity threshold are not substitutions.
   Off-grid delivery or interpolated quote risk is a separately selected financial
   operation, not implicit weighting in a direct stored-node shock.

8. **Restore exact scratch state within its actual scope.** Context scratch
   risk uses the same admitted shock kernel and saves the original surface Arc/
   state. Restore that exact source on success/failure under the existing scoped
   LIFO protocol. Do not re-resolve coordinates, subtract a bump or expose an
   old nearest-node unbump repair. A stale token must not overwrite an unrelated
   intervening replacement; preserve scoped ownership/order rather than add a
   new versioning or restoration framework. No partial node mutation escapes a
   failed request.

9. **Keep risk and diagnostics financially qualified.** Zero sigma is valid
   source data; actual pricing methods own their zero-limit/domain support.
   Remove a scenario-wide assertion that every zero-vol Black consumer fails.
   For finite differences, the actual risk owner declares units, scheme and
   effective width: use admitted plus/minus sources, an explicitly qualified
   one-sided scheme or a domain failure. Do not silently clip/shrink shocks and
   divide by a nominal2h. Report raw bucket sums/residuals honestly; the current
   bucketed-vega residual already does this and must not be rescaled to force
   equality. Finite bucket PV changes need not sum exactly to a parallel change.

   Preserve existing convention-specific diagnostic refusals. Rectangular
   admission, total-variance mode, positive quotes or successful fit/shocks do
   not prove calendar/butterfly freedom, density positivity or model correctness.
   Do not apply a Black/Strike theorem or one tolerance scale indiscriminately
   to Normal/Shifted/Tenor grids. Diagnostic/repair/fit-acceptance policies remain
   separately Draft. Historical calibration reports do not automatically qualify
   the changed grid's current fit; replay/recalibration is a distinct request.

10. **Reconcile strict state and actual producers/hosts together.** Store the
    explicit source ID, ordered expiry/secondary axes and kind, native sigma
    grid, quote/interpolation conventions, compatible displacement vector and
    accepted optional expiry clock. Retain authoritative quotation/unit/source
    association at existing market/construction owners. Reject missing required,
    unknown, malformed or incompatible state without defaults, repairs, old
    ambiguous-axis aliases or migrations. Reconcile SABR/SVI and other materializers,
    active model resolution, actual dated pricers, scenarios/Greek widths, context
    scratch and calibration transport/diagnostics. Update Rust/Python/WASM builders,
    getters/options, stubs/declarations, schemas, parity, generated examples and
    goldens together; bindings convert types and propagate existing errors.
    No new surface class, host pricing implementation or volatility framework
    is introduced. FX-delta/cube/forward-variance sources retain their own records.

## Observed implementation and discrepancies

Core defaults Strike/Black/Vol, admits expiry0 and signed Tenor values and has no
expiry_clock. Vols are finite/nonnegative, axes ordered/rectangular and singletons
allowed. Shifted Black requires aligned row shifts; attachment silently retags
the convention without converting the quotes. Source construction does not
enforce convention-specific known strike domains.

Parallel additive/point shocks floor at0. Direct scaling rejects negative k;
bucket percent<-100 instead collapses values to0. Point scratch clamps and picks
nearest nodes; unbump repeats that selection. Buckets match within
1e-9*max(abs(grid),1), while scenario expiry selection has a separate.02year/2%
snap. Scenario preview rejects all zero quotes, including untargeted existing
zeros. These behaviors are observed, not adopted choices.

Many actual pricing adapters already guard quote/axis conventions and preserve
resolved displacement; they are not universally ignoring metadata. The remaining
reconciliation includes truthful clocks/units, explicit construction and range,
exact selection and domain consistency. The model evaluator's total-variance
branch materializes squared values and divides by max(T,EPSILON), so a positive
tiny expiry or extreme finite sigma can lose exact-node fidelity.

## Independent examples and qualification limits

On **2026-11-08**, twelve standalone Fraction/200-digit Decimal/binary64 cases
were evaluated without production imports:

- Black sigma.20 plus one absolute vol point gives.21; relative1% gives.202.
  Separately, Normal decimal-rate sigma75bp=.0075 plus1bp gives.0076, whereas
  adding.01 gives.0175, or175bp.
- For P(sigma)=sigma, sigma=.005,h=.01, clipping the negative central endpoint
  gives derivative.75 rather than1. This was a separate finite-difference case.
- At exact T=1e-20,sigma=.2, division by max(T,2^-52) gives.00134217728 instead
  of.2. Separately, sigma1e200 squares to infinity and sigma1e-200 squares to0
  in binary64, although each requested sigma is representable.
- In total-variance interpolation between (T1=1,sigma1=.1) and (T2=2,sigma2=.3),
  T=1.5 gives approximately.251661147842. Adding.01 to both source sigmas gives
  approximately.260959767014: an off-grid move.009298619172, not.01.
- Linear sigma between (1,.2)/(2,.15) has endpoint W=.04/.045, but
  W(5/3)=5/108, approximately.0462963, exceeds the final W. Increasing endpoint
  W alone does not make that raw-vol interpolant calendar-monotone.
- Expiries1 and1+5e-10 both match the old1e-9 filter around1 despite being
  distinct source nodes. This was a separate exact-target counterexample.
- For P=(sigma1+sigma2)^2, sigmas.2/.3 and h=.01, the parallel finite change
  is.0204 and summed individual changes are.0202, leaving residual.0002.
- At ATM F=K=.01,sigma=.2,T=1, attaching shift.02 triples
  the Black price's forward prefactor. Attaching a shift is not quote conversion.
- A factor0 intentionally gives sigma0; a negative factor from relative-150%
  is inadmissible rather than another zero stress. A smallest-positive-subnormal
  sigma times.5 has a positive exact product but rounds to0. These formed the
  twelfth intentional-zero versus underflow/domain case.

Production qualification must independently cover positive/near-zero expiry,
singletons, shape/order and known strike/shift domains; exact quotation/unit/
tenor association and active clock agreement/aging admission; node fidelity,
extreme sigma/time and stable requested variance versus sigma; units/zero/
negative/range/underflow shocks, exact targets/empty filters and atomic scratch
restoration, including failed/stale scope use; actual model zero limits, risk
widths/bucket residuals and convention-specific diagnostics. Verify source and
calibrated materializers, host/state/schema/parity and all financial consumers.
These expressions, round trips and same-pricer fits do not certify production
accuracy, market repricing or global arbitrage freedom.

## Evidence and alternatives

Inspected [core VolSurface](../../../../finstack-quant/core/src/market_data/surfaces/vol_surface.rs),
[shared axis admission](../../../../finstack-quant/core/src/market_data/surfaces/mod.rs),
[model evaluator](../../../../finstack-quant/models/src/volatility/source.rs),
[active convention resolution](../../../../finstack-quant/valuations/core/src/instruments/common_impl/vol_resolution.rs),
[context scratch](../../../../finstack-quant/core/src/market_data/context/ops_bump.rs),
[scenario adapter](../../../../finstack-quant/scenarios/src/adapters/vol.rs),
[SABR materializer](../../../../finstack-quant/calibration/src/targets/vol.rs)
and [SVI materializer](../../../../finstack-quant/calibration/src/targets/svi.rs).
Existing source, model, scenario, lifecycle and risk tests were inspected or
located, not executed; existing bucketed-vega reports already retain residuals.

[Strata surface metadata](https://strata.opengamma.io/apidocs/com/opengamma/strata/market/surface/Surfaces.html)
distinguishes strike, tenor and moneyness representations. Its [shifted-Black source](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/capfloor/ShiftedBlackIborCapletFloorletExpiryStrikeVolatilities.html)
retains index/valuation identity and an expiry shift curve. These are primary
context for explicit meaning, not a prescription for these source choices.
[Gatheral](../../../REFERENCES.md#gatheral-volatility-surface) gives broader
surface/model context; grid storage and mode labels alone approve no model.

Clipped stress, nearest-node delivery, shifted-quote conversion, arbitrage repair
and stale-source forward-variance aging are different financial operations.
They are not fallback modes of this direct source-parameter shock package.

## Follow-up tracking

Runtime reconciliation and independent qualification are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
