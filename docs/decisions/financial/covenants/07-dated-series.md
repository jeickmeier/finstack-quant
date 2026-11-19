# COVENANTS-07: Dated observations, period calendars and series evaluation

**Decision status:** Accepted 2026-11-17 for dated observations, exact period identity and read-only series coverage.

**Implementation status:** Dated sorting, duplicate-date rejection in the series constructor and explicit stochastic reference dates for dated wrappers are implemented. Shared period admission, empty-assessment validation, forecast eligibility and observation coverage remain pending.

**Validation status:** The 115-test Covenants Rust baseline and all five focused series tests passed on 2026-11-17 against current behavior. Unknown-period rejection and the accepted coverage and admission rules need qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Resolve exact period membership and validate dates before reading metrics, including when the selected scope has no active tests. Plain dated rows use their observation date as both inclusive start and end. Reject unknown periods and frequency mismatches. For model periods, use the source's stored calendar and assess on the last included day.
2. Sort plain dated rows and reject duplicate dates. Require selected model periods to have valid start and end bounds and strictly increasing test dates. An empty assessment validates the engine and scope, then returns zero coverage. A forecast requires at least one selected period.
3. Keep series operations read-only. Read required metrics on demand for applicable tests. A missing required observation yields an incomplete assessment, and an invalid required value yields a validation error. Carry the source classification of actual and projected observations, forecast only selected projected periods, and require an explicit reference date for stochastic forecasts over plain dated rows. [Accepted statement assessment contract](../statements-analytics-credit/11-covenant-statement-adapter.md#accepted-decision-for-verified-statement-covenant-assessments).

## Observed construction and selection

`DatedMetricSeries::new` requires at least one row, sorts by calendar date, maps each date to `PeriodId::day(year,ordinal)`, rejects duplicate dates, and stores the metric map without validating its values. Stored periods are ascending and private. A missing metric read returns `None`; a stored nonfinite value is rejected only if a test actually reads it. There is no interpolation, forward filling, resampling, revision history or actual-versus-forecast classification.

The `ModelTimeSeries` abstraction supplies exact scalar lookup and inclusive start/end dates. `DatedMetricSeries` has both boundaries equal to the observation date. Its date implementation reconstructs `Date::from_ordinal_date(period.year(), period.index())` without checking frequency or membership in the stored map. This differs from the trait's documented requirement that unknown periods fail: an unrelated valid ordinal, or the index of a nondaily period, can still produce a date. The convenience wrappers use the owned period list, while generic APIs can receive caller-selected periods.

`evaluate_series_from_model` maps supplied periods to inclusive end dates and reads metrics lazily through `PeriodMetricSource`. `CovenantEngine::evaluate_series` preserves supplied row order, including duplicate or reversed dates; it does not construct a `DatedMetricSeries`. Both use the common `evaluate_dated` loop, validate the engine once if nonempty, choose date-effective specs, collect ordered reports and return the first error without partial output. Empty evaluation returns empty without validating the engine. Neither updates history, filters scope nor validates period starts or chronology.

Forecast wrappers differ: they use every sorted stored date, require nonempty strict chronology via the generic forecast, and require an explicit `reference_date` for stochastic dated rows. The generic default would anchor one day before the first daily observation, which does not identify the start of its reporting interval; the wrapper rejects that implicit horizon.

## Calendar boundary and rationale

The inspected [statements adapter](../../../../finstack-quant/statements-analytics/src/analysis/credit/covenants.rs) resolves stored fiscal/calendar periods and subtracts one day from the exclusive statement end. It passes all result periods, including actuals, to the engine. That adapter owns the half-open-to-inclusive conversion; the covenant crate neither reconstructs fiscal calendars nor selects forecast-only periods. Its methodology is separately recorded in [STATEMENTS-ANALYTICS-CREDIT-11](../statements-analytics-credit/11-covenant-statement-adapter.md).

Exact membership prevents an unrelated period from selecting an effective covenant window. Classification and calendar dates come from the source. A plain observation date identifies the test date and carries no reporting-interval start for stochastic horizon inference.

## Evidence

Owning [series module](../../../../finstack-quant/covenants/src/series.rs) and [period trait/forecast validation](../../../../finstack-quant/covenants/src/forward.rs). Inspected inline tests `series_sorts_rows_and_maps_each_date_to_a_daily_period`, `series_rejects_empty_and_duplicate_dates`, `stochastic_series_forecast_requires_a_reference_date`, and `forecast_and_series_evaluation_agree_on_the_breach_date` cover convenience paths.

All five focused series tests passed. Generic unknown-period paths with inactive or empty sets and the accepted empty-selection behavior remain unqualified.

## Follow-up tracking

Implementation reconciliation and qualification are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
