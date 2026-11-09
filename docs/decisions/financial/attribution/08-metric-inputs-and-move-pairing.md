# ATTRIBUTION-008: Metric selection, horizon matching and exact market-move pairing

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and execution

The direct metrics entry consumes caller-provided opening and closing valuation results, using opening sensitivities only. The request adapter prices a cloned opening-parameter instrument with theta horizon set to the exact positive calendar-day window when overrides are supported; closing price requests no metrics. The default metric menu is narrowed to the instrument registry's applicable subset; explicitly named metrics remain strict. If any opening hazard dependency lacks calibration provenance, the default menu drops CS01, BucketedCs01, CsGamma and rate/spot/vol credit cross metrics with a note; explicit requests do not receive this fallback. There is no alternate intensity-risk substitution.

The adapter attaches the request config and supplied or cached recalibration pricing options. If an opening model snapshot was explicitly supplied, it separately reprices closing market with opening parameters and records the difference as model-parameter P&L. Direct prepriced attribution does not do that extra decomposition or validate valuation-result provenance beyond the consumed inputs.

## Pairing algorithm and conventions

- Form unique rate IDs, discount first then forward, excluding credit-role curves. Parse `bucketed_dv01::<curve>::<coordinate>` with exact month/year coordinates, sort, reject duplicate numeric coordinates and require every declared rate curve to have buckets if any does. Pair every supplied coordinate with the same-tenor continuous-zero or forward-rate move ×10000. Missing/nonfinite movements reject the input.
- For calibrated hazard BucketedCs01, require closing replay bindings, exact bucket count and exact producer labels; resolve each closing quote by both quote ID and pillar date, rejecting absence/ambiguity. Move is closing minus opening quote coupon in bp, and retain opening quote index for directional replay. Without an opening recipe, supplied numeric coordinates use per-tenor credit-curve measurement; any populated credit map must cover every declared credit curve.
- Aggregate rate shifts try discount then forward measurement. Standard mean sampling uses years `[0.25,0.5,1,2,3,5,7,10,30]`, distinct from the 11-point Taylor grid. Aggregate credit uses the core family-aware credit move (par CDS spread for hazards, continuous-zero for discount-style credit); only missing-curve/not-found errors are omitted, other measurement errors propagate.
- Primary spot move uses the first declared scalar; FX uses `fx_exposure()`'s primary pair. Volatility uses each unique declared source: opening Act/365 expiry plus reference strike when available; otherwise ordinary surfaces must have uniform moves over the union of opening/closing grid coordinates to within 1e-9 vol points. Scalar vol changes convert decimal to points by ×100. Multiple measurable vol sources must have the same move to that tolerance; otherwise aggregate vega attribution is flagged invalid when relevant vol/cross metrics exist. Sources represented only by unsupported stores are skipped.

## Carry input contract

Period cash is added to headline MTM using the shared instrument settlement boundary. Carry metrics are period totals, never multiplied by day count. Same-day carry is zero. If CarryTotal, Theta or CouponIncome is supplied, a positive finite `theta_period_days` must match within 1e-9; absent horizon is accepted only for a one-day window. CarryTotal is financing-net, so FundingCost is mandatory (explicit zero for unfunded) and added back to obtain gross carry. Otherwise Theta is used; with neither, carry is zero with a note. Missing carried accrued/partition values are not synthesized.

## Failure and evidence

Direct metrics uses zero repricings in metadata, request execution two logical endpoint calls plus optional parameter reprice. Default residual tolerances are 10 currency units and 1 percent. Some unavailable aggregate measurements produce zero contributions plus notes; incomplete supplied buckets fail rather than silently degrade. Return/input dates and metric producer settings remain a trust boundary on prepriced inputs. Tests in `metrics_based/tests.rs` locate nonstandard/fractional keyrates, incomplete coverage, duplicate CDS display labels with exact IDs/pillars, invalid closing pillars, horizon mismatch and unsupported volatility twists. These support contract behavior; none ran in this audit.

## Source and evidence

- [metrics_based/mod.rs](../../../../finstack-quant/attribution/src/metrics_based/mod.rs) — owning implementation.
- [metrics_based/context.rs](../../../../finstack-quant/attribution/src/metrics_based/context.rs) — owning implementation.
- [metrics_based/shifts.rs](../../../../finstack-quant/attribution/src/metrics_based/shifts.rs) — owning implementation.
- [metrics_based/carry.rs](../../../../finstack-quant/attribution/src/metrics_based/carry.rs) — owning implementation.

## Relationship to the accepted comparable-measurement package

[CORE-MARKET-22's accepted package](../core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows)
was accepted on **2026-11-08**. It removes availability-driven discount/forward, credit-storage and
projection/fixing measurement choice, with explicit typed source/quantity and
resolved coordinates. Required rows cannot disappear into a changed-population
mean or zero; preserve exact quote IDs/pillars and validate full quote/contract
meaning. First-scalar/primary-pair proxies do not establish complete multi-source
measurement. Canonical results retain units, actual dates/clocks and comparison
basis. This does not adopt this record's complete metric selection, horizon,
carry, allocation or status policy.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
