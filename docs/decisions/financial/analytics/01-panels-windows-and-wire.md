# ANALYTICS-001: Performance panels, observation spans, defaults and wire contracts

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice, domain and rationale

`Performance` accepts one common strictly increasing date grid and one price or simple-return column per uniquely named ticker. Each ticker has a contiguous observed span; only leading/trailing `NaN` gaps are allowed. Prices must be finite and positive with at least two observations; returns must be finite and strictly above −1. This enforces the accepted missing-versus-invalid distinction in [ADR 0009](../conventions/0009-missing-and-invalid-data.md). No forward filling, interior deletion, price adjustment or total-wipeout handling is selected by this API.

## Algorithm and selection

1. Validate nonempty grid/columns, name count, strict chronology and column lengths. Locate each finite span and reject invalid values within it or non-NaN values outside it.
2. For prices, compute adjacent `P[t]/P[t−1]−1` and place returns at the ending dates. An unrepresentable/nonpositive ratio becomes NaN and is then rejected by assembly. For return columns, preserve observations and trim permitted edge gaps.
3. Price panels retain their actual initial valuation date. Return panels prepend `frequency.prior_observation_date(first_return_date)`: daily subtracts one calendar day; weekly seven days; monthly/quarterly/semiannual/annual subtract 1/3/6/12 months with date clamping. This is an assumed date, not observed data, and remains inconsistent with the previously agreed actual-initial-date direction.
4. Store compact columns plus global half-open spans. A single-ticker statistic uses its own active span; a benchmark statistic intersects ticker, benchmark and active spans. The omitted benchmark is ticker zero; an explicit missing name is an error.
5. `reset_date_range(start,end)` selects return ending dates inclusively using partition points. Reversed/out-of-range dates can produce an empty window. Drawdowns are recomputed from wealth one at that window's start; full-history peaks do not carry into a reset window. CAGR includes the price date immediately before the first selected return.
6. Restoration deserializes a separate state and re-runs assembly, including span/date/return checks; caches are rebuilt and the saved active window is restored. Malformed state is not trusted.

Frequency is an explicit label, default Daily; regular spacing, trading-calendar consistency and return provenance are not inferred or checked. Annualization counts are 252/52/12/4/2/1. Exported convenience defaults are annual cash rate 0, per-period MAR 0, monthly aggregation, rolling window 63, confidence .95, five drawdown episodes and annualize=true. Most Rust operations still require their own arguments; these constants do not override explicit parameters.

For the tail-risk methods and summary, Rust takes confidence explicitly; Python and WASM use the Rust-owned `DEFAULT_CONFIDENCE` when omitted. Their Gaussian/CF methods pass an omitted horizon as `None`, meaning one observation period; panel frequency does not implicitly annualize VaR. [Record 08](08-moments-gaussian-and-cornish-fisher.md) owns horizon calculations and failure behavior, and [ADR 0033](0033-value-at-risk-method-selection.md) owns cross-estimator selection and its pending rationale. Observing these defaults does not adopt them under [ADR 0008](../conventions/0008-financial-defaults-and-cagr.md).

## Wire behavior, limitations and failures

`PerformancePanel.series` serializes every nonfinite float as `null`, and reads null as NaN. Therefore an infinite edge value rejected by direct construction loses its identity if the panel is serialized before validation: it can return as an allowed missing gap. This is a confirmed transformation in source; the end-to-end case was not executed. It requires reconciliation with ADR 0009, not adoption as a missing-data repair.

`Performance` state contains only validated finite returns. `BetaResult`, `GreeksResult`, `PeriodStats` ratio fields and `DatedSeries` use explicit core nonfinite wire adapters; `RollingGreeks` has plain float vectors despite undefined windows, a separate qualification gap in [record 12](12-rolling-regression.md). Schema registrations/generation describe shapes and are not financial validation. The fixed `summary` table calls 22 existing metrics, uses zero MAR/threshold for Sortino/downside/Omega, historical VaR/ES at supplied confidence, and the current default CAGR; it does not select alternative estimators.

## Alternatives and tradeoffs

Keeping actual valuation dates preserves known holding periods; synthesizing a frequency step makes a return-only panel convenient but invents the first interval. The actual-date requirement is already agreed. Explicit filling/resampling, a cash-flow-adjusted return builder, and total-wipeout support would be separate policies; none is implemented here.

## Implementation and validation evidence

- Owning [panel/state/window logic](../../../../finstack-quant/analytics/src/performance/mod.rs), [panel wire adapter](../../../../finstack-quant/analytics/src/performance/panel.rs), [restoration](../../../../finstack-quant/analytics/src/performance/serialization.rs), [defaults](../../../../finstack-quant/analytics/src/defaults.rs), [summary](../../../../finstack-quant/analytics/src/performance/scalar.rs), and [core frequency/date step](../../../../finstack-quant/core/src/dates/periods/mod.rs).
- The [Python](../../../../finstack-quant-py/src/bindings/analytics/performance.rs) and [WASM](../../../../finstack-quant-wasm/src/api/analytics/performance.rs) tail-risk/summary wrappers establish the shared omitted-argument behavior above. These selected caller traces do not extend the Rust source ledger to complete host API or parity qualification.
- [Correctness regressions](../../../../finstack-quant/analytics/tests/correctness_regressions.rs) locate edge-ragged/interior-gap, invalid-date, empty-overlap and window-reset cases. Inspected [audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) include `ragged_window_round_trip_preserves_alignment_and_initial_dates` and `initial_drawdown_uses_known_peak_date_in_full_and_windowed_panels`; these test state invariants, not the truth of a synthesized date.
- [Schema contract tests](../../../../finstack-quant/analytics/tests/schema_contract.rs) are shape evidence. [Schema registry](../../../../finstack-quant/analytics/src/schema.rs), [generator](../../../../finstack-quant/analytics/src/bin/gen_schemas.rs), and [crate exports](../../../../finstack-quant/analytics/src/lib.rs) add no estimator.

No data cleaning or schema generation was run.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
