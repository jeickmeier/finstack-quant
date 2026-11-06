# ATTRIBUTION-001: Method dispatch, endpoint valuation and execution contracts

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and scope

`attribute_pnl` explicitly dispatches Parallel, ordered Waterfall, Taylor or MetricsBased. The enum default is Parallel; the execution-policy default is Serial, a separate scheduling choice. This applies to instruments implementing the valuations instrument contract, with two market states and a non-reversed date interval. The raw `pnl_bridge` is a separate two-price mark-to-market calculation and does not collect period cash or validate date order.

## Algorithm and selection

1. Validate `T1 >= T0`; build the opening instrument from the optional model-parameter snapshot. `None` and the snapshot `None` variant retain the closing instrument.
2. Repricing methods use supplied unscaled `prepared_endpoints`, when present, otherwise price the opening instrument in the opening market/date and the closing instrument in the closing market/date. Supplied endpoints are trusted at this layer; portfolio adapters own preparation/stamp validation.
3. Seed headline MTM using date-matched FX conversion into the closing valuation currency. Persist endpoint PVs only when both are already in that currency.
4. Run the selected method, stamp the request rounding context and recompute residual; override tolerances if supplied. Optional credit hierarchy/carry detail is additive to existing aggregate factors: errors in this postprocessing become notes. Finally translate to an optional reporting currency.
5. The JSON execution route reconstructs validated market contexts and instrument payloads, strictly parses named metric strings, and contains panics as `Internal` errors. A rounding-scale override probes opening value currency, falling back to notional currency on a failed probe. No override means no probe. A rate-bump override populates `valuations.sensitivities.v1`; Taylor has separate bump controls.
6. Batch execution reconstructs markets once, processes instruments in input order, and aborts on the first error. An empty batch returns before market/date validation. Result envelope and input envelope accept only the exact `finstack_quant.attribution/1` marker; canonicalization is schema deserialization/reserialization, not a full numerical run.

## Conventions and limits

P&L amounts are unscaled instrument money until a caller explicitly scales. Configuration changes metadata/metric controls; it is not evidence that every instrument's direct `value` uses that configuration. Waterfall ignores parallel scheduling; other methods can use ordered parallel evaluation. Repricing counts include logical endpoints even when prepared endpoints avoid those calls. Strict validation is default true but controls the waterfall model-parameter failure branch, not a universal switch over all methods. Tolerances are stamped without a separate finite/nonnegative validation here.

## Rationale and alternatives

The observed separation preserves the selected economic decomposition across host and native entry points; this is an inferred design rationale, not historical adoption. Alternative endpoint reuse with mandatory provenance at this layer, or universally strict error behavior, is not implemented. No runtime change is implied.

Execution tests locate rounding-currency probe/fallback behavior; spec tests locate exact schema gating and roundtrips. Those are contract evidence, not independent pricing benchmarks.

## Source and evidence

- [lib.rs](../../../../finstack-quant/attribution/src/lib.rs) — owning implementation.
- [execution.rs](../../../../finstack-quant/attribution/src/execution.rs) — owning implementation.
- [spec.rs](../../../../finstack-quant/attribution/src/spec.rs) — owning implementation.
- [helpers.rs](../../../../finstack-quant/attribution/src/helpers.rs) — owning implementation.
- [result.rs](../../../../finstack-quant/attribution/src/result.rs) — owning implementation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
