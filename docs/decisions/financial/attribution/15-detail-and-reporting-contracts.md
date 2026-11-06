# ATTRIBUTION-015: Detail identities, schema encoding and table projection

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and representation

Output detail is a view of calculation results, not a second independently recomputed attribution. Rates/credit tenor maps encode tuple keys as `curve_id|tenor`, split on the last separator when decoding. FX maps encode `FROM/TO` and validate both currency names. Optional tenor maps preserve absence. SourceLine constructors merely store total and optional rates/credit parts; they do not enforce `total=rates+credit` or matching currencies. Broader currency validation belongs to PnlAttribution residual/reporting APIs.

## Projection algorithm

Wide rows first validate all attribution currencies, then expose headline/endpoints, factors, residual, percentage, repricing count and invalid flag. Long detail rows iterate existing maps in insertion order (hierarchy maps use BTree order), split dotted `kind` into factor/sub labels and convert each Money to f64 amount plus currency text. Long rows include aggregate/subtotal and component rows together: summing all rows double counts hierarchy levels. Carry emits total, income, optional rate/credit income split, pull-to-par, roll-down splits and funding. Credit factor rows emit generic, levels and bucket totals, adder, curve shape and optional issuer adder; they omit the absolute adder diagnostic and the separate credit-carry hierarchy structure.

Waterfall rows preserve executed step order and price-only PV differences. Sensitivity export emits one summary row per factor plus per-tenor rows: summaries carry explained/gamma/repriced values, bucket rows intentionally have those totals absent, retaining only sensitivity and movement. Rates/keyrate summaries expose their work through sensitivity steps even when aggregate detail maps remain absent. These flatteners do not recompute residual or qualify tolerances; only the wide row checks all currencies. Stored result invalidity remains material to consumers.

`schema.rs` registers four JSON artifacts and constructs demonstration input/output payloads. `bin/gen_schemas.rs` delegates schema generation/check commands to core. These files contain schema/example plumbing, not valuation or attribution kernels; example market emptiness does not imply an executable pricing fixture. JSON-schema support is feature-gated in the public structs, with method/default and wire contracts in record 1.

## Rationale and limitations

Separate summary and work rows make calculations inspectable while preserving units. This is an inferred presentation rationale. Table f64 conversion does not preserve every Decimal bit; structured Money results remain the richer contract. Located long-row tests check field/row mapping and waterfall/sensitivity shape; schema/roundtrip tests verify encodings, not economic correctness. No numerical method is adopted by successful serialization.

## Source and evidence

- [detail.rs](../../../../finstack-quant/attribution/src/detail.rs) — owning implementation.
- [long_rows.rs](../../../../finstack-quant/attribution/src/long_rows.rs) — owning implementation.
- [schema.rs](../../../../finstack-quant/attribution/src/schema.rs) — owning implementation.
- [bin/gen_schemas.rs](../../../../finstack-quant/attribution/src/bin/gen_schemas.rs) — owning implementation.
- [result.rs](../../../../finstack-quant/attribution/src/result.rs) — owning implementation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
