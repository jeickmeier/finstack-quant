# PORTFOLIO-EXECUTION-010 — Additive risk aggregation, omissions and tabular projection

- **Decision status:** Accepted 2026-11-15 — complete compatible aggregation, frozen FX, checked reduction, coverage, provenance and reporting package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent unit, FX, range, cancellation and coverage examples verified. Runtime, restoration and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio metric aggregation and reporting

1. **Aggregate only owner-declared linear measures on a compatible basis.**
   Match the canonical key, actual factor and role, units, denominator or
   bump, sign, information phase, horizon, clock and comparable valuation or
   shock basis. Normalize units only through a qualified owner conversion.
   Retain separate totals for a bond z-spread CS01 and a hazard-curve CS01
   sharing a key. Yields, ratios, unresolved units and undeclared custom
   values remain per holding.
   [Valuation metric orchestration](../valuations-framework/metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting).

2. **Apply signed sizing and reporting FX once.** Retain whether the measure
   is per unit or already sized. Use its actual currency and freeze qualified
   cash FX at the admitted valuation cut. Resolve any additional required
   currency within that same valuation state. Remove the later market
   query, PV-ratio fallback and near-zero PV threshold. Nonmonetary measures
   require no FX. Repricing a reporting-currency value under moving FX is a
   separate sensitivity.
   [Accepted portfolio FX](08-spot-and-forward-fx.md#accepted-decision-for-checked-portfolio-nav-and-cashflow-fx).

3. **Use checked arithmetic and deterministic reductions.** Keep
   Money-valued outputs in checked Money. Check the complete signed scale,
   normalization and FX product for raw values so premature intermediate
   overflow or underflow does not discard a representable contribution.
   Use separate checked Neumaier folds for portfolio and entity raw totals
   in holding order. Count each economic contribution once across
   structured and flat views. Invalid published values, required conversion
   failures and reduction failures abort the affected operation.
   [Portfolio execution and reduction](04-execution-order-and-reduction.md#accepted-decision-for-checked-portfolio-execution-and-nav-reduction).

4. **Report coverage by metric, coordinate and scope.** Use the valuation's
   explicit strict or best-effort risk policy. Preserve published values,
   structural inapplicability, failures, withheld batches and requests with
   no risk. A complete total requires an admitted contribution and success
   for every applicable required contribution. Explicit best effort may
   publish a labelled partial subtotal with at least one qualified success.
   A published zero counts. No qualified contributions means no numeric
   total. Interpret sparse coordinates using owner evidence. Admitted PV
   and book NAV retain their own coverage.
   [Valuation profiles](03-valuation-profiles-and-failures.md#accepted-decision-for-checked-portfolio-valuation-profiles-and-failures).

5. **Make lookups and series carry the financial basis.** Return the full
   metric identity and the typed coordinates, units, currency and coverage.
   A string-only lookup reports ambiguity if more than one qualified result
   shares a key. `get_total` exposes complete totals and `require_total`
   distinguishes unknown, unavailable and incomplete results. Expose partial
   subtotals through the typed result. A series returns exact descendants
   in declared order with their coordinate roles intact.

6. **Build the report from one qualified valuation.** Derive metrics and
   execution metadata from the same admitted valuation. Retain durable
   source identities, effective settings, risk request and FX evidence.
   Optional assembly time is audit metadata. Use one bounded Rust restore
   path for ordered inputs, units, coverage and provenance. Rebuild or
   verify derived totals before publication. Matching dates, holding IDs
   and NAV alone cannot establish a common calculation basis.
   [Auditable results and replay](../../project/0025-auditable-results-and-replay.md#decision).

7. **Preserve units, coverage and precision across hosts.** Keep position PV
   exact in canonical tables and use named binary64 display projections.
   Carry each measure's actual unit and currency, with no currency for rates
   and ratios. Preserve scoped coverage and typed issues through Python,
   WASM, the tear sheet and existing UI. Charts use declared factor and tenor
   roles. Reconcile schemas, declarations, parity and reporting fixtures.

## Independent calculation checks

- A EUR 10 sensitivity at frozen cash FX of `1.25 USD/EUR` gives USD 12.50
  even at zero PV. A later rate of `1.10` gives USD 11.00. With an owner
  conversion, USD 10,000 per unit decimal spread equals USD 1 per bp.
- Native endpoints EUR 102 and EUR 98 at frozen `1.20 USD/EUR` give a
  half-difference of USD 2.40. Repricing with endpoint FX of `1.21` and
  `1.19` gives USD 3.40 on that difference convention.
- The complete product of the maximum finite binary64 value, `2` and
  `.25` is representable even though the first multiplication overflows.
  The product of `1e−300`, `1e−100` and `1e200` is finite and nonzero even
  though multiplying the first two in binary64 gives zero. Ordered
  Neumaier retains `1` from `[1e16, 1, −1e16]`.
- A published zero with another holding structurally inapplicable is a
  complete zero. An additional applicable holding with a withheld batch
  makes zero a partial subtotal. With no successful contribution the
  aggregate has no numeric payload.

## Observed choice and rationale at the audit revision

Only metrics in the valuations additive whitelist are summed across holdings. Ratios/yields and unqualified risks lacking additive identity remain per-position. This preserves the difference between an extensive currency sensitivity and an intensive statistic; generic averaging of nonadditive measures is not implemented.

## Algorithm and conventions

For every position with a stored `ValuationResult`, copy its measure map. Multiply additive values by `metric_scale`; leave nonadditive values unchanged. Record native currency alongside this per-position map. Resolve its FX conversion even if the measures ultimately prove nonadditive:

1. Same native/base currency gives rate one.
2. Otherwise attempt a current spot lookup on the supplied market at `valuation.as_of`.
3. On **any** FX lookup failure, use `value_base.amount()/value_native.amount()` when `|native PV| > 1e-6`; otherwise fail conversion. The stored `fx_rate` field is not used as the fallback source.

For every additive scaled measure, omit nonfinite values from totals, record `SkippedMetric`, and retain the value in the per-position payload. Multiply the remaining value by the FX rate, then stream it into separate portfolio and entity Neumaier accumulators in position order. Finite operands can overflow at multiplication. The current checked accumulator rejects a nonfinite product at insertion. Total/entity maps preserve first metric/entity encounter order. Nonadditive metric names are returned sorted in `unaggregated_metrics`, and degraded positions are copied from the valuation.

`get_total` distinguishes missing from zero; `require_total` errors for absence. `metric_series(base)` decodes canonical qualified coordinates and preserves aggregate-map order rather than matching arbitrary string prefixes.

## Output projection

The positions table emits position/entity IDs, native/base values and currencies, optional FX rate and risk error. It does not carry every source flag, including `risk_metrics_complete`, inapplicability or triangulation. The per-position metric table emits the native-currency label for each row, including nonadditive measures whose true unit can be a rate or ratio. The aggregate table emits metric ID and total without a currency column. These are presentation projections, not typed unit evidence. A combined `PortfolioResult` stamps current schema version, config metadata, optional timestamp and valuation FX policy; it does not verify that its supplied metrics were calculated from that valuation.

## Limitations and alternatives

Requerying FX from a different market can change metric reporting currency conversion independently of the stored PV; the ratio fallback can conceal missing FX, use an old rate, and becomes unavailable around zero PV. A fixed valuation FX provenance contract is an alternative requiring a clear API rule. Nonfinite omissions make totals partial and must remain visible. Finiteness after conversion is required by [existing arithmetic invariants](../../../../INVARIANTS.md#12-arithmetic-and-aggregation).

Portfolio metric totals can be negative by sensitivity convention. No weighted yield/duration or residual normalization is silently created. Serialization of a skipped NaN is a host-format concern; retaining it in memory is not proof that all downstream serializers accept it.

## Evidence

[metrics.rs](../../../../finstack-quant/portfolio/src/metrics.rs), [dataframe.rs](../../../../finstack-quant/portfolio/src/dataframe.rs), [results.rs](../../../../finstack-quant/portfolio/src/results.rs) and the [additive whitelist](../../../../finstack-quant/valuations/core/src/metrics/aggregation.rs). Inspected the inline `1e16 + 1 - 1e16` cancellation test; located ordered FX-error and output-order cases. [Aggregation metric tests](../../../../finstack-quant/portfolio/tests/aggregation_metrics.rs) cover signed quantity, omissions and degraded risk; [table tests](../../../../finstack-quant/portfolio/tests/aggregation_grouping_and_df.rs) locate shape validation. These are algebraic/contract evidence. [Higham](../../../REFERENCES.md#higham-accuracy-and-stability) provides numerical context, not evidence that every current boundary is qualified.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
