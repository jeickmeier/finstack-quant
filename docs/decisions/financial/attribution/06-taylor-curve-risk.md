# ATTRIBUTION-006: Taylor key-rate sensitivities and directional curve gamma

- Decision status: Accepted 2026-11-16 for checked Taylor curve sensitivities and directional gamma.
- Implementation status: Observed for the source scope listed below. Runtime, pricing, replay and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked rate-clock mapping, a credit twist, joint directional gamma, the zero-shock credit center and unequal-node curvature. Production attribution and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked Taylor curve sensitivities and directional gamma

1. Fix one qualified opening experiment. Use the selected opening holder pricer, date and cut, model parameters, settings, provider and currency. Identify the active typed sources and mutate each shared physical source once while retaining its roles. Follow the product's elected treatment of dependent hazard curves when rates move. Require explicit positive finite calculation bumps and a gamma election. Retire the implicit bump defaults and blanket `0.01–100 bp` bounds in favor of source-domain and represented-delivery checks. Verify common stochastic draws where required. [Accepted finite-difference contract](../valuations-framework/finite-difference-rates.md#accepted-decision-for-checked-finite-differences-and-rate-risk).
2. Match each rate move to its shock coordinate. Measure discount and projection continuous-zero changes on the same explicit calendar anchor and maturities in both snapshots. Construct triangular shocks on the opening source clock. Verify the mapping from reporting bp to native shock bp. With a matched anchor, a reporting year fraction of `0.75` and a native year fraction of `0.50` map a `+10 bp` reporting move to `+15 bp` native. Equal tenor labels alone fail the pairing. [Accepted comparable-measurement contract](../core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows).
3. Calculate rate first order on one admitted triangular grid. Require a grid with strictly increasing native coordinates, the prescribed triangular ramps and flat outer wings. Compute signed bucket sensitivity in currency per positive native bp from two delivered reprices, multiply each sensitivity by its matched native move and sum with checked arithmetic. Use the same grid for the directional experiment. Zero at every sampled node establishes zero for that sampled direction. Off-grid shape changes can remain unexplained. [Accepted rate-shock contract](../core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning).
4. Use exact par-risk quote coordinates for credit. Pair complete, comparable opening and closing CDS par-risk quotes by exact qualified source and quote identity. Use signed bucket CS01 on that basis. Allow explicitly elected parallel CS01 only if every required quote in scope has the same verified move. Retire the mean-spread fallback for empty or failed buckets. Use a qualified credit-role discount curve's own continuous-zero risk and move basis. CDS quote risk and native hazard intensity are separate quantities. [Accepted CDS and index risk contract](../models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk).
5. Measure elected gamma along the full within-curve move. For a qualified nonzero vector `d`, use `a = min(1, h / max |d_i|)`. Reprice the full `+a·d` and `−a·d` independently from the same qualified center. For symmetric delivery add `[(V+ − V0) + (V− − V0)] / (2a²)`. For credit, apply the full quote vector in one replay per affected source on each side and use the qualified zero-shock par-risk center. Any opening-to-center difference requires a separately qualified bridge or the experiment fails. The stencil captures within-curve cross-node curvature. Cross-curve, time and volatility interactions are outside its scope. [Accepted hazard quote replay contract](../calibration-framework/21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication).
6. Fail required experiments when their basis or delivery breaks. Reject collapsed required moves, invalid quotes and failed replay or repricing. Retain the actual scalar spans or the verified common vector amplitude. Check differences, scaling and accumulation. Distinguish structural zeros from evaluated zeros. Fixed stencil selection establishes no accuracy bound.
7. Publish checked Rust evidence. Retain typed source and bucket identities, both dates and clocks, requested and delivered moves, center and endpoint marks, first- and second-order effects and actual pricing work. Eligible cash follows [record 003](03-total-return-carry.md#accepted-decision-for-checked-period-cash-gross-carry-and-funding). Python and WASM project the checked result.

Independent examples verified signed credit bucket sensitivities `(2, 5)` in currency/bp with a `(+3, −3) bp` twist produce `−9` in currency. A separate `(+2, −2) bp` twist for `V(x,y) = 100 + 2x + 2y + 4xy` has zero first order and directional gamma of `−16`. A zero-shock par-risk center of `102` with legs `103` and `101` has zero gamma at unit amplitude. Using an unqualified opening mark of `100` would add a spurious `+2`.

## Observed choice and domain

Taylor computes opening-date/opening-parameter curve sensitivities times observed market moves. Optional second-order P&L uses the realized move direction, retaining within-curve cross-bucket curvature without constructing a Hessian. Discount curves declared in credit roles are excluded from the discount-rate loop; forward dependencies have their own loop. Unlike metrics context, this loop does not deduplicate an ID shared by discount and forward lists, and attempts each declared storage accessor.

## Algorithm and numerical controls

Rate and credit central-difference bump defaults are 1 bp, with finite permitted range 0.01–100 bp. Gamma defaults off. For each discount/forward curve, use the 11-year-coordinate grid `[0.25,0.5,1,2,3,5,7,10,15,20,30]`. Discount moves are continuous-zero changes ×10000; forward moves are rate changes ×10000, compared at the same numeric tenor in each snapshot. Triangular key-rate bumps use adjacent grid coordinates as support boundaries. Compute each signed sensitivity as `[V(+h)-V(-h)]/(2h)` in money/bp and first-order P&L with a Neumaier sum of sensitivity × observed bp move.

For directional gamma let `m=max(abs(move_i))`. If m=0, gamma P&L is zero. Otherwise `a=min(h/m,1)`, compose all triangular bumps at `±a*move_i`, and add `0.5*[V(+a move)-2V0+V(-a move)]/a²`. This is the quadratic contribution along the actual within-curve move, not gamma of an average rate. Cross-curve/time/volatility interactions are not added by this kernel.

Credit first requests BucketedCs01 using the recalibration pricing options, and validates exact quote bindings via the shared metrics shift parser. If that request/parser fails, credit factors are omitted and the result is invalid; aggregate fallback occurs only when a successful metric result contains no credit buckets. With buckets, first-order P&L is the Neumaier sum of CS01 × bucket move. Directional hazard gamma sequentially bumps exact replay quote indices by scaled realized changes; a missing index/recipe fails. Discount-style credit curves use triangular zero-rate bumps on supplied coordinates.

Without buckets, reprice ±parallel credit bump and multiply central CS01 by the standard-tenor mean credit move. Hazard bumps rebuild calibrated spread quotes, using the recipe discount curve; they never substitute an intensity bump for absent replay provenance. Discount-style credit uses zero-rate bumps. Optional aggregate gamma adds `0.5*[Vup-2V0+Vdown]/h² * mean_move_bp²`.

## Failure and alternatives

Bump/price failures are captured as notes and set `result_invalid`; endpoint errors and initial dependency-query errors propagate. Factor evaluation may run in ordered parallel batches. Minimum bump limits are implementation noise safeguards, not demonstrated accuracy bounds for every notional/instrument. No adaptive step choice or Richardson extrapolation is used. Taylor's mixed scalar/family treatment is continued in [record 7](07-taylor-volatility-and-restoration.md).

## Rationale

The source motivates directional differences by twist/cross-bucket convexity and quote-space credit units. Located tests `keyrate_gamma_captures_cross_bucket_convexity`, `directional_gamma_is_stable_for_dv01_neutral_twist`, `forward_keyrate_gamma_captures_cross_bucket_convexity`, and `hazard_credit_bumps_replay_quotes_and_require_recipe` are analytic synthetic/contract evidence, not an executed suite in this audit.

## Source and evidence

- [taylor.rs](../../../../finstack-quant/attribution/src/taylor.rs) — owning implementation.
- [metrics_based/shifts.rs](../../../../finstack-quant/attribution/src/metrics_based/shifts.rs) — owning implementation.

## Relationship to the accepted comparable-measurement package

[CORE-MARKET-22's accepted package](../core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows)
was accepted on **2026-11-08**. It requires explicit source role, measured quantity, coordinates,
clock and units before pairing a move with parameter or quote risk. Dated factor
rate differences use one declared calendar/reporting basis; direct intensity
and full CDS quote changes remain distinct. Preserve producer bucket/quote
identity and qualify actual directional risk mapping. This does not adopt this
record's Taylor order, repricing path, mixed/curvature approximations or residual
allocation.

## Follow-up tracking

Runtime reconciliation and validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). Production and host
qualification remain pending.
