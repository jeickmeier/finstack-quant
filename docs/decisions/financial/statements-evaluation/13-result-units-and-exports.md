# STATEMENTS-EVALUATION-013 — Result units, Money views, tables and dated flow exports

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and data domain

`StatementResult` retains raw `f64` node-period values as canonical computational output, with separate value-type metadata, monetary views, per-cell source and diagnostics. It can represent undefined results. Table and dated-schedule exports are projections of those values, not recalculations or automatic investor/issuer perspective conversions. The unit/sign boundary remains governed by the model and [financial perspective policy](../conventions/0044-financial-perspectives-and-boundary-conversions.md).

## Algorithm and controls

1. Store each numeric result and its `Value`, `Forecast`, `Formula` or `WhereMasked` source. The only numeric mode is Float64; execution metadata records counts/timing/parallel status and warnings, not the workspace-wide ResultsMeta market/rounding audit stamp. Finalization associates inferred/declared Scalar or Monetary currency types and attempts to construct Money entries only for representable finite monetary values.
2. Nonfinite numeric results remain in the numeric map but are omitted from Money. Finite values outside Money's representable range are likewise retained numerically, omitted monetarily and produce a warning named `NonFiniteValue` despite the input being finite. The parallel Money view is therefore not guaranteed to have every numeric cell.
3. Numeric serialization uses the core nonfinite codec (`nan`, `inf`, `-inf` strings) rather than silently converting undefined to zero. `get_or` substitutes its fallback only when a cell is absent, not when it is present NaN; `get_scalar` requires explicit Scalar type metadata.
4. Long tables iterate node/period insertion order and include raw value, declared unit/currency and available source information. Their monetary-value column is selected from type metadata and raw numeric value, not by requiring a successful Money conversion; it can therefore contain a numeric value absent from the Money map.
5. Wide tables use the sorted union of periods, retain one column per node, and fill absent cells with NaN. They omit the long table's currency/type/source columns. A wide numeric table is not by itself a complete unit/provenance envelope.
6. `node_to_dated_schedule` walks the supplied model periods in order, skips absent values and attaches either start date or default final included date (`exclusive end-1`). It preserves raw values, including nonfinite ones. It does not require a monetary node, convert currencies, infer cash-flow signs, discount, or divide a period amount over dates.

## Rationale, failure and alternatives

Separating numeric evaluation from monetary representation preserves undefined/scaled outputs while exposing a typed view where possible. That rationale is inferred from behavior; it does not establish that every export is appropriate for downstream pricing. Missing values, undefined values and representability failures must remain distinguishable. A caller using a raw long table or dated export must verify units and intended perspective; silently assuming that all rows have a Money counterpart is unsafe for the observed API.

Strict finite-only export, rejection of unrepresentable money, typed dated cashflows and complete provenance envelopes are possible alternatives with different caller contracts. No universal rejection of NaN is proposed, because [ADR 0009](../conventions/0009-missing-and-invalid-data.md) explicitly requires per-operation treatment.

## Evidence

[Results](../../../../finstack-quant/statements/src/evaluator/results.rs), [table export](../../../../finstack-quant/statements/src/evaluator/export.rs), [dated export](../../../../finstack-quant/statements/src/evaluator/cashflow_export.rs), [engine finalization](../../../../finstack-quant/statements/src/evaluator/engine.rs). Located [result export tests](../../../../finstack-quant/statements/tests/spec/results_export_tests.rs), [dated schedule tests](../../../../finstack-quant/statements/tests/integration/dated_cashflow_export_tests.rs), and [engine regressions](../../../../finstack-quant/statements/tests/finance_engine_regressions.rs) for nonfinite monetary cells and finite Money overflow. These are schema/boundary evidence, not validation that an arbitrary statement line is an investable cashflow. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
