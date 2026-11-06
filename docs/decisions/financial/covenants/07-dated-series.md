# COVENANTS-07: Dated observations, period calendars and series evaluation

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted decisions retain their scope.

**Implementation status:** Observed in the exact source paths identified below, including stated discrepancies and limitations.

**Validation status:** Source and identified test assertions inspected; tests, builds, numerical experiments and external benchmarks not executed. Test existence is not adoption or independent qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Construction and selection

`DatedMetricSeries::new` requires at least one row, sorts by calendar date, maps each date to `PeriodId::day(year,ordinal)`, rejects duplicate dates, and stores the metric map without validating its values. Stored periods are ascending and private. A missing metric read returns `None`; a stored nonfinite value is rejected only if a test actually reads it. There is no interpolation, forward filling, resampling, revision history or actual-versus-forecast classification.

The `ModelTimeSeries` abstraction supplies exact scalar lookup and inclusive start/end dates. `DatedMetricSeries` has both boundaries equal to the observation date. Its date implementation reconstructs `Date::from_ordinal_date(period.year(), period.index())` without checking frequency or membership in the stored map. This differs from the trait's documented requirement that unknown periods fail: an unrelated valid ordinal, or the index of a nondaily period, can still produce a date. The convenience wrappers use the owned period list, while generic APIs can receive caller-selected periods.

`evaluate_series_from_model` maps supplied periods to inclusive end dates and reads metrics lazily through `PeriodMetricSource`. `CovenantEngine::evaluate_series` preserves supplied row order, including duplicate or reversed dates; it does not construct a `DatedMetricSeries`. Both use the common `evaluate_dated` loop, validate the engine once if nonempty, choose date-effective specs, collect ordered reports and return the first error without partial output. Empty evaluation returns empty without validating the engine. Neither updates history, filters scope nor validates period starts or chronology.

Forecast wrappers differ: they use every sorted stored date, require nonempty strict chronology via the generic forecast, and require an explicit `reference_date` for stochastic dated rows. The generic default would anchor one day before the first daily observation, which does not identify the start of its reporting interval; the wrapper rejects that implicit horizon.

## Calendar boundary and rationale

The inspected [statements adapter](../../../../finstack-quant/statements-analytics/src/analysis/credit/covenants.rs) resolves stored fiscal/calendar periods and subtracts one day from the exclusive statement end. It passes all result periods, including actuals, to the engine. That adapter owns the half-open-to-inclusive conversion; the covenant crate neither reconstructs fiscal calendars nor selects forecast-only periods. Its methodology is separately recorded in [STATEMENTS-ANALYTICS-CREDIT-11](../statements-analytics-credit/11-covenant-statement-adapter.md).

Source documentation explains the explicit dated-row horizon requirement and on-demand reads. Rejecting all unused invalid data, sorting evaluation rows, or requiring a forecast flag are alternative boundaries; none is implied by the current API.

## Evidence

Owning [series module](../../../../finstack-quant/covenants/src/series.rs) and [period trait/forecast validation](../../../../finstack-quant/covenants/src/forward.rs). Inspected inline tests `series_sorts_rows_and_maps_each_date_to_a_daily_period`, `series_rejects_empty_and_duplicate_dates`, `stochastic_series_forecast_requires_a_reference_date`, and `forecast_and_series_evaluation_agree_on_the_breach_date` cover convenience paths.

These generic edge paths were not executed.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
