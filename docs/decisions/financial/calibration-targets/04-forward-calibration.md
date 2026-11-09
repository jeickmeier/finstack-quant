# CALIBRATION-TARGETS-004 — Contractual-reset projection-curve global calibration

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Relationship to accepted projection factors

[CORE-MARKET-02](../core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates)
was accepted on 2026-11-08 and replaces scalar seed rates/reset-grid chaining
with positive projection factors. Reconcile the observed parameterization below:
fit factor state, retain explicit index/source identity and reprice resolved
index observations through the same contractual projector as valuation. Quote
selection, pillar placement, solver choice and market acceptance remain this
calibration owner's separate Draft decisions; this dependency does not adopt them.

## Choice and inputs

Projection curves fit all quotes simultaneously on actual contractual reset intervals; sequential Bootstrap is explicitly rejected. The retained forward curve uses a dense projection grid for DF chaining and a separate, sparse control-knot grid with approximately one parameter per quote. The source rationale is that off-grid/stub reset periods couple adjacent forwards. Discounting is supplied by an existing discount curve.

`ForwardCurveParams` deserialization defaults to GlobalSolve with numerical derivatives and CubicHermite. The [schema rationale](../../../../finstack-quant/calibration/src/api/schema.rs) identifies smooth shape preservation with zero/negative rates. `InterpStyle::default()` is Linear, distinct from omission of the serialized field. MonotoneConvex is rejected by the forward builder because it operates on discount factors; direct-builder defaults and inferred index conventions belong to [forward projection](../core-market/forward-projection.md).

## Algorithm

1. Prepare rates at `N=1,000,000`, with output projection ID, discount dependency and optional OIS override. Clock is explicit or inferred from matching index day counts. Default method is GlobalSolve; preserve the original quotes/clock/role in the replay recipe.
2. Recover deposit start/end, FRA start/maturity, futures resolved reference dates, or swap floating accrual periods. Swap schedules use their frequency/stub/BDC/calendar/EOM/payment lag, no reset-lag adjustment, raw accrual endpoints and no roll rule. Include zero and every interval start/end on the curve clock, require finite `0<=start<end`, sort and deduplicate within `1e-12`.
3. Control times: FRA/futures use interval start; swaps use the last floating accrual start with maturity as collision fallback. Deposits sorted by start/end use their own start for the first common-start deposit and the preceding deposit's end for subsequent deposits. Resolve collisions by moving the new swap, or an existing swap, to its maturity; unresolvable duplicates error. Sort controls and aligned quotes again. Seeds are quote implied rates, including the futures convexity subtraction.
4. Construct a ForwardCurve with selected interpolation (default CubicHermite), numeric tenor, dense projection grid and **FlatZero** extrapolation (constant final forward level). If first control is after `1e-12`, prepend origin with its rate; a single control is duplicated at projection-grid terminal time. Rate bounds are explicit lower/upper vectors from effective currency policy.
5. A projection deposit does not use its discount-only instrument PV. Its residual is `growth - r_quote*alpha_index`, where `growth=rate_between(t_start,t_end)*(t_end-t_start)=Pproj(start)/Pproj(end)-1`. This matches index accrual even when curve time uses a different day count. Other quotes use the instrument raw PV/N. Weights are the [annuity proxy](02-rate-preparation-and-residuals.md), based on prepared pillar maturity rather than control time. The target supplies no custom efficient Jacobian; framework numerical differentiation applies.
6. Solve with the forward-specific acceptance tolerance (default `1e-8` on native unweighted residuals, labeled PV/N in the report despite the deposit growth exception), attach the recipe and mark `forward_parameterization=contractual_reset_grid`.

## Limits and alternatives

The deposit residual is undiscounted accrual growth, while other PV/N residuals include discounting; both are dimensionless but not identical quote-error scalings. A dense reset grid does not mean every reset is a free fit parameter. The control-time collision policy is part of the model: unsupported overlapping instruments fail rather than automatically regularizing an overdetermined curve. Controls and weights can produce different interpolated forwards even when all benchmarks fit.

Alternative models include log projection DFs, piecewise forwards per reset interval with regularization, or fitting independent overlapping quote sets by least squares. Their identifiability and smoothness tradeoffs need explicit agreement. The default frozen terminal forward and the difference between point/average/DF-implied forwards follow [core projection behavior](../core-market/forward-projection.md).

## Evidence

- Owning [forward.rs](../../../../finstack-quant/calibration/src/targets/forward.rs) fully inspected, including grid/collision dispatch and residual construction.
- The inspected `deposit_residual_uses_index_day_count_for_accrual` establishes the explicit `365/360` rate conversion. Multiple-deposit control, configured bounds/weights, futures curve-clock, explicit day-count and quote-replay tests are located in the same file. FRA/futures/swaps are repriced through the public flow in [tests/repricing.rs](../../../../finstack-quant/calibration/src/tests/repricing.rs); these share production pricing kernels.

No test execution or independent vendor reconciliation was performed.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
