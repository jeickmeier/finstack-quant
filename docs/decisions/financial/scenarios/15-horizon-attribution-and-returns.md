# SCENARIOS-015: Horizon attribution bridge and return normalization

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

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

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
