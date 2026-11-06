# CORE-CONVENTIONS-014: Gregorian/fiscal period boundaries, annualization factors and LTM windows

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to all five `dates/periods/` files and fiscal-year logic in `date_extensions.rs`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

Reporting periods carry explicit boundaries and distinguish Gregorian from fiscal identifiers. Calendar partitioning and statistical annualization are deliberately different: Daily has annualization factor 252 while actual daily period IDs range over 365/366 calendar days. LTM selects a calendar-year daily window rather than blindly taking 252 rows.

## Algorithm and conventions

Gregorian daily periods are ordinal date to next day. Weekly periods use ISO week-year/Monday to Monday, allowing 52/53 weeks independently of calendar year. Month/quarter/half-year/year periods span 1/3/6/12 months, half-open `[start,end)`. Fiscal start month/day is configurable with day 1–31 clamped to the actual start month length. A Jan-1 fiscal year is labeled with its calendar year; every other fiscal start belongs to the prior calendar year of its label. Daily/weekly fiscal indices offset from fiscal start, with week 53 truncated at next fiscal year start. Month blocks are anchored to fiscal start; the last block ends at the next actual fiscal start so February clamping does not leave a gap.

`PeriodId::from_date` maps to the containing period and verifies containment. A fiscal ID alone cannot recover its `FiscalConfig`; `bounds()` rejects it. Gregorian daily/weekly shifts add actual days/weeks; fixed-count kinds use ordinal year/index arithmetic. Fiscal weekly shifts use capacity 53; fiscal daily shifts without config may stay within days 1–365 (zero is identity even for day 366) but reject boundary crossing; `next_fiscal`/`prev_fiscal` accept config. `prior_observation_date` subtracts 1 calendar day, 7 days or 1/3/6/12 clamped months; no calendar rolling is applied.

Range parsers accept absolute IDs and abbreviated matching-kind endpoints; reject reversed ranges or mixed kinds/fiscalness. Builders validate actual bounds and cap output at 100,000 periods. The optional actuals cutoff must share kind/calendar and marks IDs <= cutoff as actual; no cutoff means all forecast. `PeriodId` sorting first compares year, then index/fiscalness for same kind; mixed kinds compare Gregorian bounds then kind/index, so it is not a fiscal-calendar-aware chronological ordering across mixed schemes.

Annualization factors are Daily252, Weekly52, Monthly12, Quarterly4, SemiAnnual2, Annual1. For non-daily LTM, create that many successive IDs ending at the requested ID; missing counts only failed shifts, not absent observations in supplied data. Weekly LTM is therefore 52 IDs even in a 53-week year. For daily LTM, take target end from matching supplied period or Gregorian `bounds`, subtract 12 months, filter/sort matching-calendar daily grid, and include rows whose starts lie in the window. Missing counts only calendar days before the first supplied grid row; interior gaps/weekends are accepted and duplicates are not repaired. A missing fiscal target cannot use Gregorian fallback bounds.

## Selection, limitations and alternatives

Default FiscalConfig is Jan 1; named US federal/UK/Japan/Australia starts are Oct1/Apr6/Apr1/Jul1. Fiscal labels and clamped midmonth rules are implementation choices, not proof of statutory financial-year requirements. Annualization factors do not validate data sampling. LTM completeness is a leading-coverage indicator, not a general missing-data validator. Retail 4-4-5/52-53-week fiscal calendars are not implemented.

## Evidence

[Period IDs/config](../../../../finstack-quant/core/src/dates/periods/mod.rs), [bounds/building](../../../../finstack-quant/core/src/dates/periods/calendar.rs), [parsing/stepping](../../../../finstack-quant/core/src/dates/periods/parse.rs), [LTM](../../../../finstack-quant/core/src/dates/periods/ltm.rs), [period tests](../../../../finstack-quant/core/src/dates/periods/tests.rs). Inspected examples include fiscal week53, leap fiscal partitioning, ISO week boundaries and clamped fiscal starts. LTM tests distinguish 365 calendar rows, 261 weekday rows and 59 leading missing calendar days in 2025. These are invariant/expected-date tests. [ISO8601](../../../REFERENCES.md#iso-8601) is the week/date background, not support for financial annualization factors.

No weighting/proration of partial weekly periods is performed by this layer.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
