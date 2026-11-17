# ATTRIBUTION-001: Method dispatch, endpoint valuation and execution contracts

- Decision status: Accepted 2026-11-16 for checked instrument attribution execution and endpoint valuation.
- Implementation status: Observed for the source scope listed below. Runtime, pricing, replay and host reconciliation with the accepted contract are pending.
- Validation status: Independent arithmetic checked date-matched FX, closing-equivalent eligible cash and distinct endpoint pairs with identical MTM. Production pricing and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked instrument attribution execution and endpoints

1. Require explicit Parallel, ordered Waterfall, Taylor or MetricsBased attribution. Validate method-specific controls before pricing and reject inapplicable controls. Scheduling is independent of financial method. The mark-to-market-only `pnl_bridge` uses the same checked endpoint preparation.
2. Freeze the held instrument economics, including contract terms and deal size. Bind both immutable markets and the selected pricer, registry, settings and provider. Retain requested and effective valuation dates and cuts. Reject reversed comparisons before pricing. An explicitly ordered same-date comparison has zero elapsed carry and period cash. Where the model has parameters, require qualified opening parameters or an explicit election to use the closing parameters at opening. [Accepted pricing lifecycle](../valuations-framework/pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle).
3. Use unscaled PV-only endpoints for Parallel, Waterfall, Taylor and the bridge. MetricsBased uses opening PV and the complete applicable opening metric profile, including carry over the actual interval, plus closing PV. Admit prepared values only when instrument, endpoint role, market, date and cut, pricing controls, currency and required FX and stochastic basis match. Retire bare prepared Money pairs and the speculative rounding-currency probe. Bind any currency-specific rounding override to the qualified valuation or reporting currency. [Accepted portfolio endpoint profiles](../portfolio-attribution/08-portfolio-pnl-endpoints.md#accepted-decision-for-checked-portfolio-pl-attribution).
4. Preserve both native and reporting marks and their dated FX evidence. Compute reporting MTM as `q1 × V1 − q0 × V0`. A change in native valuation currency requires a qualified common basis. Full attribution adds eligible period cash under [record 003](03-total-return-carry.md#accepted-decision-for-checked-period-cash-gross-carry-and-funding) and labels the closing-equivalent reporting amount. Keep marks, MTM, eligible cash and residual explicit. [Record 013](13-target-currency-and-scaling.md) owns shared translation and residual rules. The caller applies holding scale to the as-built instrument result.
5. Replace the broad strict-validation switch with typed failures for missing applicable inputs, failed pricing or FX, and invalid arithmetic. Proven inapplicability can contribute zero. Explicitly elected advisory detail may report typed unavailability. Invalid results cannot qualify as reconciled. Validate supplied tolerances as finite and nonnegative in their declared currency or percentage units.
6. Default to serial scheduling. Waterfall stages are sequential and independent work uses at most one parallel axis. Report actual valuation attempts and reused marks separately. Validate shared inputs even for an empty batch. Publish atomically or report the first failure in logical input order with instrument, method and stage context.
7. Use one bounded checked Rust contract across native and host routes, with exact supported schema markers and strict field admission. Canonical JSON establishes wire validity. Semantic admission and source-backed replay establish qualification as pricing evidence. Python and WASM consume the checked Rust outputs and typed errors. [Auditable result and replay policy](../../project/0025-auditable-results-and-replay.md#decision).

Independent arithmetic gave USD `22` MTM for EUR marks `100 → 110` at USD/EUR `1.10 → 1.20`, including USD `10` of opening-principal translation. With EUR `5` of eligible period cash, full closing-equivalent P&L was USD `28`.

## Observed choice and scope

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
