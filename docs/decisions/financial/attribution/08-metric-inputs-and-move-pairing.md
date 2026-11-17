# ATTRIBUTION-008: Metric selection, horizon matching and exact market-move pairing

- Decision status: Accepted 2026-11-16 for checked metric selection, horizon matching and market-move pairing.
- Implementation status: Observed for the source scope listed below. Runtime, metric, pricing, cash lineage and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked native rate-clock mapping, a nonparallel credit move, directed FX, model-parameter repricing, exact carry horizon and cash reconciliation. Production attribution and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked metric selection, horizon matching and market-move pairing

1. Elect the opening metric profile explicitly. Select a qualified product profile or exact metric list, including explicit curvature, cross-effect and advisory-detail elections. Resolve applicability and one authoritative producer per consumed output on the prepared instrument, lifecycle, registry, pricer and provider. Consume side outputs only when the plan binds them. Reject duplicate or overlapping additive representations, unsupported named requests and missing required outputs. Retire the broad default menu and blanket credit-metric drop. [Accepted metric orchestration](../valuations-framework/metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting).
2. Apply identical evidence rules to fresh and supplied valuations. Require opening holder PV and the elected opening profile, and closing holder PV only. Bind the unscaled holder basis, instrument terms, effective dates and cuts, frozen markets and history, pricing controls, currency and applicable FX and stochastic evidence. When model parameters change, use the same selected pricer at the closing cut and compute `V1 − V_close_with_opening_params`. Admit that counterfactual from qualified prepared evidence or actually reprice it on either entry point. [Accepted execution contract](01-execution-and-endpoints.md#accepted-decision-for-checked-instrument-attribution-execution-and-endpoints) · [Accepted portfolio endpoint contract](../portfolio-attribution/08-portfolio-pnl-endpoints.md#accepted-decision-for-checked-portfolio-pl-attribution).
3. Pair moves to the producer's exact risk coordinates. Measure only the elected axes, in producer order, with complete required coverage. Retain typed source and role, coordinate, shock geometry, sign, units, currency, dates and clocks. Reject missing, ambiguous, duplicate or incompatible bindings. For direct discount and projection zero-rate risk, use common calendar anchors and maturities and map reporting bp to the actual native shock basis. Rate quote risk uses matched calibration quotes. Retire inference from tenor labels or bare IDs. [Accepted comparable measurements](../core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows).
4. Keep credit risk bases distinct. Pair CDS CS01 to exact qualified par-risk quotes by identity, pillar and contract basis. Preserve original quote form, replay provider, zero-shock center and any qualified center bridge. An explicitly selected `Hazard01` uses matched native intensity parameters and clocks. Credit-role discount risk uses its own zero-rate basis. Select one additive representation per effect. Missing replay evidence fails required par-risk coverage. [Accepted CDS risk](../models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk) · [Accepted quote replay](../calibration-framework/21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication).
5. Require a matching direction for scalar and mixed metrics. Use the exact spot price or percentage axis, directed FX quote and route, native volatility quote and reporting increment, or declared CPI quantity and period. Match both axes of a mixed metric independently. An aggregate scalar sensitivity consumes a move only when the complete observed vector follows its declared shock direction with one verified amplitude. Otherwise use elected compatible bucket or per-source metrics or fail the required comparison. Keep averages diagnostic. Retire first-scalar, primary-pair and generic volatility reference-point proxies. [Record 009](09-metric-rates-and-credit.md) and [record 010](10-metric-other-factors.md) own the contribution and curvature formulas. [Accepted option and FX sensitivities](../valuations-framework/option-sensitivities.md#accepted-decision-for-checked-option-and-fx-sensitivities).
6. Require carry evidence for the actual interval. Match opening and closing dates, information and settlement cuts, held-market holder basis and eligible held cash ledger. Use elected gross carry or recover it from net `CarryTotal` plus its matching signed funding charge once. If both representations are supplied, check agreement. Reserve `theta_period_days` for dated theta. Retire elapsed-day matching, the one-day exception, generic Theta fallback and prorated or zero substitutes. An admitted same-date comparison on the same settlement basis has structural zero elapsed carry and period cash. [Accepted cash and carry](03-total-return-carry.md#accepted-decision-for-checked-period-cash-gross-carry-and-funding) · [Accepted horizon evidence](../valuations-framework/carry-theta-breakeven.md#accepted-decision-for-checked-horizon-theta-carry-and-breakeven).
7. Use closing cash for headline P&L and publish checked evidence. Compute `V1 − V0 + C_close` with the independently qualified closing ledger. Allocate each held-to-closing cash change once to a qualified effect, including changes that net to zero. Unsupported allocation or missing required evidence fails complete attribution. Retire the implicit 10-currency-unit and 1% thresholds and apply qualified shared residual controls. Rust publishes checked amounts, matched risk and move rows, typed availability and actual pricing attempts separately from reuse. Python and WASM project those results. [Accepted execution contract](01-execution-and-endpoints.md#accepted-decision-for-checked-instrument-attribution-execution-and-endpoints) · [Shared residual and reporting record](13-target-currency-and-scaling.md).

Exact examples verified that a `(+3, −3) bp` credit twist with sensitivities of `(2, 5)` currency per bp explains `−9` despite a zero mean move. A separate carry example reconciled `+6` headline P&L to `+1` gross carry, `+3` market price effect and `+2` of changed cash. Reusing the held ledger for headline cash gave `+4`.

## Observed choice and execution

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

Runtime reconciliation and validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). Production and host
qualification remain pending.
