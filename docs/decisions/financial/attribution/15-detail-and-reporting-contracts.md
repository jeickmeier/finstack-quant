# ATTRIBUTION-015: Detail identities, schema encoding and table projection

- Decision status: Accepted 2026-11-16 for checked detail identities, report projections and schemas.
- Implementation status: Observed for the source scope listed below. Runtime, wire, schema and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked delimiter collisions, duplicate keys, partition closure, signed Waterfall cash and Decimal precision. Production and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked detail identities, report projections and schemas

1. Use ordered typed detail identities. Replace delimiter-encoded curve, FX and hierarchy keys with structured entries. Preserve source namespace, role and ID, exact producer coordinate, directed FX orientation, both mixed-pair axes and the applicable hierarchy or position path. Reject duplicate wire keys and duplicate resolved identities before insertion. Derive display labels from these identities. Preserve execution and producer order in arrays. [Accepted numeric wire contract](../core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity).
2. Make row ownership and additivity explicit. Identify each row's parent, partition and role as headline, subtotal, additive component, working row or diagnostic. Check each complete partition independently against its parent. Alternate curve and tenor views can explain the same effect. Count the effect once in the headline. Preserve a zero subtotal with offsetting nonzero children. [Accepted reporting and closure rules](13-target-currency-and-scaling.md#accepted-decision-for-checked-reporting-fx-sizing-aggregation-and-residual-qualification).
3. Preserve exact amounts, currency and availability in every view. Use Decimal Money in canonical rows, with canonical decimal text and currency where the transport lacks a Decimal type. Validate currency and amount basis within each partition before publication. Retain native evidence separately from closing-equivalent views. Distinguish unelected detail, requested-empty detail, structural zero, evaluated zero, advisory unavailability and invalid results. Keep binary64 conversion explicit for display or plots. [Accepted Money precision](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).
4. Project accepted carry and credit detail. Show gross and net carry, separate funding and any elected income, pull-to-par and roll-down partition. Retire the legacy coupon and roll-down `SourceLine` splits and use optional Money for those components. Show rates benchmark, credit carry and elected hierarchy in their own partition. Credit P&L detail retains measured shape, curvature, bridges and separate cash. Absolute adder magnitude is a diagnostic. [Accepted gross carry](03-total-return-carry.md#accepted-decision-for-checked-period-cash-gross-carry-and-funding) · [Accepted credit hierarchy](11-credit-factor-hierarchy.md#accepted-decision-for-checked-credit-hierarchy-and-spread-replay-allocation) · [Accepted credit carry](12-credit-carry-partition.md#accepted-decision-for-checked-rates-benchmark-and-credit-carry-allocation).
5. Keep Waterfall and sensitivity working auditable. Waterfall preserves executed order, cuts, before and after marks, and separate price, cash and all-in effects. Sensitivity rows preserve the actual experiment, producer coordinates, units and increments, including both axes of mixed risk. A bucket with just sensitivity and movement remains working. Publish bucket P&L when the producer supplies it. Keep actual pricing work and reuse visible. [Accepted Waterfall](04-ordered-waterfall.md#accepted-decision-for-checked-ordered-waterfall-revaluation) · [Accepted mixed risk](10-metric-other-factors.md#accepted-decision-for-checked-metric-spot-fx-volatility-inflation-and-mixed-factor-pl).
6. Separate wire decoding, semantic admission and replay qualification. Use bounded Rust decoding with exact supported markers and nested fields. Check identities, Money roles, availability and closure before projecting a restored result. Economic reuse or replay requires qualified historical inputs, cuts, controls and build evidence. Archived results can be shown with their replay limits. Python and WASM use the same Rust admission and projections. [Accepted execution](01-execution-and-endpoints.md#accepted-decision-for-checked-instrument-attribution-execution-and-endpoints) · [Replay policy](../../project/0025-auditable-results-and-replay.md#decision).
7. Keep the four existing Rust-owned schema artifacts for the input envelope, shared inputs, result envelope and bucket share. Publish them deterministically from the checked contract and propagate approved shape changes through generated schemas, declarations and consumers. Label shape examples as such. Examples advertised as runnable must execute with qualified inputs. Report tables remain output projections. [Accepted schema publication](../core-numerics/schema-publication.md#accepted-decision-canonical-schemas-compact-views-and-bounded-publication).

Exact examples verified that a rates headline of `10`, curve subtotal of `10` and tenor components of `6` and `4` sum to `30` if combined indiscriminately. Each detail partition explains `10`. An exact Decimal example preserved a `0.01` difference between large marks that became identical in binary64.

## Observed choice and representation

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

Runtime reconciliation and validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). Production and host
qualification remain pending.
