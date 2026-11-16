# SCENARIOS-015: Horizon attribution bridge and return normalization

- Decision status: **Accepted 2026-11-15 — complete single-instrument horizon, checked attribution and return normalization package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent cash-aware return, contribution reconciliation and annualization examples verified.** Production attribution, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for checked scenario horizon attribution and returns

1. **Keep the horizon to one instrument and market actions.** Use
   the same authored deal at both endpoints with product-qualified
   lifecycle and settlement state. Admit market-only same-day shocks
   or one optional roll. Reject active instrument mutations and
   statement operations during preflight, including zero-valued
   mutations. Share date and fixing preparation with record 013.
   Distinguish an omitted roll from an admitted zero-day roll. Either
   can produce market-shock P&L. Annualization requires a positive span.
   [Accepted time roll](13-market-time-roll.md#accepted-decision-for-checked-scenario-time-rolls-and-horizon-replay).

2. **Use one checked pair of marks and a complete settlement ledger.**
   Price the opening and fully shocked closing states through the
   shared executor with the elected model, options, configuration and
   provider. Pass those exact marks and their provenance to attribution.
   Form the headline from comparable holder marks and phase-eligible
   settlement cash on the same scenario path. Require explicit,
   qualified FX where the holder basis uses conversion. Complete required
   horizon cash is distinct from the optional inventory carry outcome.
   [Shared executor](../../architecture/0046-portfolio-execution-operational-contract.md#decision-and-scope) ·
   [Holder basis](../valuations-framework/carry-theta-breakeven.md#accepted-decision-for-checked-horizon-theta-carry-and-breakeven).

3. **Elect the method and require checked attribution.** Select
   Parallel, Waterfall, MetricsBased or Taylor with its applicable
   controls. Elect scheduling independently. Require valid endpoint
   stamps, complete required cash and factor calculations, consistent
   currency and checked Money closure. Retain genuine advisories.
   Qualify the residual separately against the larger of an explicit
   nonnegative native-currency amount and an explicit nonnegative
   fraction of absolute headline P&L. Reject a failed residual check.
   The attribution records own the factor algorithms.
   [Attribution methodology](../attribution/README.md) ·
   [Residual qualification](../attribution/13-target-currency-and-scaling.md).

4. **Publish the complete non-overlapping contribution summary.**
   Include cross-factor P&L, residual and the separately identified
   reporting FX translation bucket. Mark translation inactive on the
   native reporting basis and distinguish it from pricing FX exposure.
   Divide applicable buckets by the same opening mark.
   [Canonical attribution buckets](../../../../finstack-quant/attribution/src/result.rs).

5. **Normalize cash-aware P&L by the positive opening mark.** Use
   the checked Money ratio on the same-currency opening holder mark.
   With a zero or negative opening mark, keep the monetary P&L
   and report returns and normalized contributions as unavailable.
   Eligible arithmetic holding returns may be below `−100%`.
   [Money ratio](../core-conventions/012-money-decimal-precision.md).

6. **Annualize on ACT/365F and classify wipeout in Money.** Require
   positive elapsed civil days. For positive growth calculate the
   stable annual equivalent using log and exponential-increment
   arithmetic. Determine exact wipeout and excess loss in Money
   before any binary64 ratio. Exact wipeout gives `−100%`.
   Loss beyond opening capital leaves annualization unavailable.
   Near wipeout, derive growth from the positive monetary remainder.
   An unrepresentable annualization is a typed calculation error.
   [Dated growth](../analytics/02-wealth-and-annualization.md#accepted-decision-wealth-means-cagr-relative-wealth-and-cash-series).

7. **Use one checked Rust result across hosts.** Keep exact Money,
   endpoint provenance, method controls and residual qualification.
   Derive summaries from the validated underlying result on restore.
   Unavailable derived scalars use None/null. Invalid inputs and
   calculation failures use typed errors. Record 017 owns wire
   representation.
   [Reporting record 017](17-reports-and-replay.md).

## Independent calculation checks

- An opening mark of `100`, closing mark of `98` and
  eligible cash of `7` yield P&L `5` and return `5%`.
  Contributions `2, 1, −1, 2, 1` include cross-factor P&L
  `2` and residual `1`. Together they reconcile to `5%`.
- A `10%` gain over `366` civil days gives about
  `9.971359%` on ACT/365F. Opening `1` and P&L
  `−0.9999999999999999999999999999` have positive growth
  `1e−28`. Over 36,500 days the annual equivalent is about
  `−47.519254%` even though the displayed holding ratio rounds
  to `−1`. An exact excess loss can also round to `−1` and
  remains ineligible for annualization.

## Observed choice and rationale at the audit revision

Observed choice: construct a future shocked market through the scenario engine and delegate attribution to the shared attribution crate, then calculate returns from the resulting total P&L. The same instrument instance is used at both endpoints, so instrument-mutating scenario operations are rejected explicitly rather than silently omitted from attribution.

## Ordered calculation and selection

Resolve an optional named calendar strictly; unknown IDs error. Clone the t0 market. If a time-roll operation advances the date, resolve that horizon with the same roll-date adapter, obtain this instrument's canonical schedule and materialize crossed fixings into the clone. Invoke `ScenarioEngine::apply` with no statement model, no instrument inventory and no rate bindings. This avoids a separate roll carry valuation and uses the projected fixings for endpoint pricing. Statement operations therefore remain unsupported through the missing-model path.

Build `AttributionRequest` from the original t0 market/date and completed t1 market/date. Use engine configuration and recalibration provider through `PricingOptions`, **`ExecutionPolicy::Parallel` and `strict_validation=false`**. `AttributionMethod` is caller-selected and defaults to `Parallel`; Waterfall, MetricsBased and Taylor variants are delegated. Execution parallelism and the economic Parallel attribution method are separate choices. The returned attribution must stamp `pv_t0` and `pv_t1`; these become initial/terminal values without duplicate endpoint valuations. The attribution kernels, residual allocation and approximation controls belong to [attribution methodologies](../attribution/README.md).

## Return formulas and units

`total_return = attribution.total_pnl.amount / initial_value.amount` is a decimal return. It is NaN if currencies differ or the initial mark is zero/negative. Factor contribution uses the same positive-initial-value/currency gate and divides the requested factor's P&L by that initial mark; liability returns are deliberately undefined here rather than sign-inverted.

A positive actual calendar-day horizon yields `annualized_return=(1+R)^(365/days)-1`. Missing/nonpositive horizon, nonfinite return, `R<-1`, or nonfinite exponentiation yields `None`. Exactly `R=-1` yields `Some(-1)`. No business-day annualization, explicit fee/funding adjustment or external-flow time weighting is added. Total return uses attribution's cashflow-aware total P&L, not a freshly inferred `PV1/PV0-1` identity.

`summary()` converts nonfinite ratios to `None` for JSON/null and returns factor contributions for the nine factors in `default_waterfall_order`. Cross-factor P&L and residual are present in the underlying attribution result but are not separate entries in that summary map. Consequently the summary factor map alone need not sum to total return. Currency is the initial-value currency.

## Evidence, limitations and alternatives

Owning source: [horizon.rs](../../../../finstack-quant/scenarios/src/horizon.rs); shared method declarations [attribution result](../../../../finstack-quant/attribution/src/result.rs), [factor order](../../../../finstack-quant/attribution/src/waterfall.rs). Inspected horizon unit tests assert no-op P&L, factor-plus-residual reconciliation, formula ratio, invalid/negative/zero-denominator handling, total-loss annualization, named-calendar shifts, and one endpoint pricing each. A floating horizon test compares terminal value across all four attribution methods to the same engine-generated market: cross-method plumbing evidence, not independent pricing accuracy.

Non-strict attribution may return warning-qualified decompositions; the horizon wrapper does not promote these to failure. Approximation methods can leave material residual on large shocks. A full factor-plus-residual summary, liability/exposure-normalized returns, or money/time-weighted portfolio returns are distinct alternatives. No alternate return convention is adopted.

## Follow-up tracking

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#scenarios).
