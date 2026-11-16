# PORTFOLIO-EXECUTION-011 — Composite look-through and net/gross primitive exposures

- **Decision status:** Accepted 2026-11-15 — complete shared valuation, leaf identity, ordered FX, strict risk, checked net and gross, reconciliation and report-state package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent nested FX, signed lot, Money, rounding, ordered reduction and path identity examples verified. Runtime, restoration and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio primitive exposures

1. **Use the same prepared valuation tree for the headline and breakdown.**
   Bind effective dates, immutable market state, selected settings, registry
   and provider through the shared executor. Value frozen composite holdings
   with child-owned controls and inherit defaults only where unset. Reuse a
   leaf calculation only for an identical prepared request. Require an
   explicit ordered metric menu. An empty menu requests quantity and PV.
   [Composite valuation](../valuations-equity-and-composition/24-composite-valuation-history.md#accepted-decision-for-checked-composite-valuation-and-financed-history)
   and [prepared portfolio execution](04-execution-order-and-reduction.md#accepted-decision-for-checked-portfolio-execution-and-nav-reduction).

2. **Keep every leaf occurrence and verify its identity.** Record the exact
   holding ID and an ordered array of instrument IDs. A direct path has one
   segment. Nested paths follow authored leg order within portfolio holding
   order and retain repeated leaves across branches and holdings. Preserve
   zero holdings and exact offsets. Require the exact primitive ID, type
   and canonical definition under the accepted `c2` contract. Missing or
   conflicting definitions fail even if the quantities cancel.
   [Frozen composite holdings](../valuations-equity-and-composition/23-composite-sizing.md#accepted-decision-for-checked-composite-sizing-and-rebalance-dates)
   and [canonical artifact admission](06-materialization-validation.md#accepted-decision-for-checked-portfolio-materialization-and-validation).

3. **Apply signed sizing and FX along the actual path.** Report quantity in
   units of the declared primitive lot. Apply each frozen leg quantity and
   the portfolio holding scale once at their respective edges. Use checked
   Money through the actual ordered, dated tree conversions, then apply
   the portfolio cash-FX basis. Monetary measures start in their own
   declared currencies. Retain the actual path factors and source evidence.
   [Signed holding sizing](01-position-sizing-and-validation.md#accepted-decision-for-checked-portfolio-holdings-and-signed-sizing)
   and [portfolio FX](08-spot-and-forward-fx.md#accepted-decision-for-checked-portfolio-nav-and-cashflow-fx).

4. **Require strict coverage for requested primitive risk.** Admit only
   owner-qualified linear measures with a valid leaf decomposition. Use
   record 010's compatibility rules and retain separate series on
   incompatible bases even if their keys agree. Confirmed structural
   inapplicability contributes no number. A qualified zero counts. Missing
   or failed applicable risk aborts the report. An empty book returns empty
   paths and aggregates. Whole-composite FX01 remains with the composite
   owner unless it supplies a qualified leaf decomposition.
   [Portfolio metric aggregation](10-metric-aggregation-and-reporting.md#accepted-decision-for-checked-portfolio-metric-aggregation-and-reporting).

5. **Compute net and gross directly from signed paths.** Group by exact
   admitted primitive ID and publish groups in ID order. Fold in path
   order within each group. Net sums signed contributions. Gross sums the
   absolute value of each fully sized and translated contribution. Use
   checked Money for PV and Money-valued outputs. Use checked compensated
   binary64 sums for quantities and raw risk. Keep zero-net groups and fail
   on an unrepresentable net or gross.
   [Money precision](../core-conventions/012-money-decimal-precision.md).

6. **Reconcile on the valuation tree's declared basis.** Compare the paths
   with the headline from the same tree using the actual conversion and
   rounding boundaries. Disclose traceable rounding differences separately.
   Keep owner-qualified nonleaf adjustments separately identified. If an
   adjustment lacks a supported breakdown, report the reconciliation limit
   and keep the leaf values as calculated.

7. **Restore one checked report across the existing hosts.** Store ordered
   paths, verified definitions, valuation and FX provenance and typed risk
   coverage. Derive aggregates and tables in Rust. Restore through bounded,
   atomic admission of identities, references, order, Money, units and
   derivable totals. Preserve structured paths and exact Money in Python
   tables, and carry the basis and coverage through WASM and UI results.
   Qualifying the report for replay requires immutable source evidence.
   [Auditable results and replay](../../project/0025-auditable-results-and-replay.md#decision).

## Independent calculation checks

- A signed path with scales `−.5 × −2 × 3` produces three primitive lots.
  EUR 100 per lot at successive rates `.80 GBP/EUR`, `1.25 USD/GBP` and
  `.90 CHF/USD` gives CHF 270. A direct short of three lots at `.95 CHF/EUR`
  gives CHF −285. Net and gross quantities are zero and six, and net and
  gross PV are CHF −15 and CHF 555. Substituting a direct `1.10 USD/EUR`
  rate on the composite path changes net PV to CHF 12.
- The qualifying Money amounts `9,007,199,254,740,992.01` and
  `−9,007,199,254,740,992.00` retain a one-cent net. Binary64 conversion
  loses it. Two opposing maximum Money values can net to zero while their
  gross overflows. Per-path and subtotal conversion can also differ at a
  bounded rounding boundary.
- Paths `[root, a/b, leaf]` and `[root, a, b, leaf]` are distinct and have
  the same slash-joined display label.

## Observed choice and scope at the audit revision

The primitive report preserves every portfolio-to-leaf path and also groups identical primitive instrument IDs for net and gross exposure. Direct and composite holdings can therefore offset in the net view while remaining visible in gross exposure. It is an additive exposure report; nonadditive requested metrics are rejected before pricing.

## Algorithm

For a composite, call its own primitive exposure engine with calibration-backed pricing options, the market and portfolio date. For a direct instrument, require canonical instrument serialization, call `price_with_metrics`, and create a one-element path. The shared position mapper preserves portfolio order. This API has no `FinstackConfig` argument; it constructs its own pricing options rather than borrowing a prior portfolio valuation.

Each leaf's base value is obtained through spot conversion and then multiplied by the holding scale. Leaf quantity is composite leaf quantity times that scale, or the direct scale itself. Measures are multiplied by FX and scale; direct results are filtered to additive metrics. Path ordering and composite sizing are delegated to the [composition scope](../valuations-equity-and-composition/README.md).

For every available serialized leaf definition, calculate the canonical envelope hash. A repeated instrument ID with a different hash errors. Direct instruments must supply a definition. The portfolio adapter's composite registration is conditional. The live composite producer requires the runtime leaf and canonical serialization before pricing, and the portfolio report omits that definition evidence.

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

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
