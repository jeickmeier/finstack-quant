# PORTFOLIO-EXECUTION-010 — Additive risk aggregation, omissions and tabular projection

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and rationale

Only metrics in the valuations additive whitelist are summed across holdings. Ratios/yields and unqualified risks lacking additive identity remain per-position. This preserves the difference between an extensive currency sensitivity and an intensive statistic; generic averaging of nonadditive measures is not implemented.

## Algorithm and conventions

For every position with a stored `ValuationResult`, copy its measure map. Multiply additive values by `metric_scale`; leave nonadditive values unchanged. Record native currency alongside this per-position map. Resolve its FX conversion even if the measures ultimately prove nonadditive:

1. Same native/base currency gives rate one.
2. Otherwise attempt a current spot lookup on the supplied market at `valuation.as_of`.
3. On **any** FX lookup failure, use `value_base.amount()/value_native.amount()` when `|native PV| > 1e-6`; otherwise fail conversion. The stored `fx_rate` field is not used as the fallback source.

For every additive scaled measure, reject non-finite values from totals by warning and recording `SkippedMetric`; keep the value in the per-position payload. Multiply the remaining value by the FX rate, then stream it into separate portfolio and entity Neumaier accumulators in position order. The post-FX product is not rechecked for finiteness. Finite value and FX inputs can overflow at that multiplication; compensation does not make that safe. Total/entity maps preserve first metric/entity encounter order. Nonadditive metric names are returned sorted in `unaggregated_metrics`, and degraded positions are copied from the valuation.

`get_total` distinguishes missing from zero; `require_total` errors for absence. `metric_series(base)` decodes canonical qualified coordinates and preserves aggregate-map order rather than matching arbitrary string prefixes.

## Output projection

The positions table emits position/entity IDs, native/base values and currencies, optional FX rate and risk error. It does not carry every source flag, including `risk_metrics_complete`, inapplicability or triangulation. The per-position metric table emits the native-currency label for each row, including nonadditive measures whose true unit can be a rate or ratio. The aggregate table emits metric ID and total without a currency column. These are presentation projections, not typed unit evidence. A combined `PortfolioResult` stamps current schema version, config metadata, optional timestamp and valuation FX policy; it does not verify that its supplied metrics were calculated from that valuation.

## Limitations and alternatives

Requerying FX from a different market can change metric reporting currency conversion independently of the stored PV; the ratio fallback can conceal missing FX, use an old rate, and becomes unavailable around zero PV. A fixed valuation FX provenance contract is an alternative requiring a clear API rule. Nonfinite omissions make totals partial and must remain visible. Finiteness after conversion is required by [existing arithmetic invariants](../../../../INVARIANTS.md#12-arithmetic-and-aggregation).

Portfolio metric totals can be negative by sensitivity convention. No weighted yield/duration or residual normalization is silently created. Serialization of a skipped NaN is a host-format concern; retaining it in memory is not proof that all downstream serializers accept it.

## Evidence

[metrics.rs](../../../../finstack-quant/portfolio/src/metrics.rs), [dataframe.rs](../../../../finstack-quant/portfolio/src/dataframe.rs), [results.rs](../../../../finstack-quant/portfolio/src/results.rs) and the [additive whitelist](../../../../finstack-quant/valuations/core/src/metrics/aggregation.rs). Inspected the inline `1e16 + 1 - 1e16` cancellation test; located ordered FX-error and output-order cases. [Aggregation metric tests](../../../../finstack-quant/portfolio/tests/aggregation_metrics.rs) cover signed quantity, omissions and degraded risk; [table tests](../../../../finstack-quant/portfolio/tests/aggregation_grouping_and_df.rs) locate shape validation. These are algebraic/contract evidence. [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) provides numerical context, not evidence that every current boundary is qualified.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
