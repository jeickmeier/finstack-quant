# PORTFOLIO-EXECUTION-011 — Composite look-through and net/gross primitive exposures

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and scope

The primitive report preserves every portfolio-to-leaf path and also groups identical primitive instrument IDs for net and gross exposure. Direct and composite holdings can therefore offset in the net view while remaining visible in gross exposure. It is an additive exposure report; nonadditive requested metrics are rejected before pricing.

## Algorithm

For a composite, call its own primitive exposure engine with calibration-backed pricing options, the market and portfolio date. For a direct instrument, require canonical instrument serialization, call `price_with_metrics`, and create a one-element path. The shared position mapper preserves portfolio order. This API has no `FinstackConfig` argument; it constructs its own pricing options rather than borrowing a prior portfolio valuation.

Each leaf's base value is obtained through spot conversion and then multiplied by the holding scale. Leaf quantity is composite leaf quantity times that scale, or the direct scale itself. Measures are multiplied by FX and scale; direct results are filtered to additive metrics. Path ordering and composite sizing are delegated to the [composition scope](../valuations-equity-and-composition/README.md).

For every available serialized leaf definition, calculate the canonical envelope hash. A repeated instrument ID with a different hash errors instead of merging unlike economics. Direct instruments must supply a definition. Composite leaf definitions are registered only when the returned runtime leaf supplies one, so this adapter alone does not prove collision checking for an absent optional definition; upstream composition guarantees must be considered.

Group paths in a `BTreeMap` by instrument ID, producing sorted aggregate order:

- `net_quantity = sum(q_path)`; `gross_quantity = sum(|q_path|)`.
- `net_value = sum(PV_path)`; `gross_value = sum(|PV_path|)`.
- For each metric, net is the signed sum and gross is the sum of absolute path contributions.

Gross is taken **after** each path's economics, FX and holding sign. It is not a sum of absolute original instrument notionals. The implementation uses ordinary f64 additions for quantities/measures and reconstructs Money from each updated f64 value sum.

## Limitations and alternatives

No separate finite-value guard exists for metric contributions/accumulators here. The [compensated-summation rule](../../../../INVARIANTS.md#12-arithmetic-and-aggregation) applies to mixed-sign cancellation and broad dynamic ranges. The Money constructor validates value representability, but it does not fix prior f64 cancellation. This is a static implementation difference from the compensated NAV/metric reducers; numerical materiality was not reproduced.

Aggregating before absolute value would produce absolute net exposure, a different and generally smaller quantity than this gross measure. Product-/issuer-level concentration grouping is another possible report, not an implemented alternative to primitive-ID grouping. Independent valuation accuracy, completeness of composite leaf definitions and sensitivity source support remain with owning instruments; this record covers portfolio scaling, identity and reduction.

## Evidence

[primitive.rs](../../../../finstack-quant/portfolio/src/primitive.rs), [position scaling](../../../../finstack-quant/portfolio/src/position.rs) and [FX helper](../../../../finstack-quant/portfolio/src/fx.rs). Inspected the inline overlapping composite/direct test: opposite quantities net to zero while gross quantity/value remain positive. That is a useful algebraic sign check, not independent leaf-pricing or ill-conditioned aggregation evidence. [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) supplies context for the unresolved accumulation qualification.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
