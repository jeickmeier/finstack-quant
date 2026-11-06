# ATTRIBUTION-006: Taylor key-rate sensitivities and directional curve gamma

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

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

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
