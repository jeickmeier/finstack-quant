# CORE-CONVENTIONS-014: Gregorian/fiscal period boundaries, annualization factors and LTM windows

- Decision status: **Accepted — complete reporting-period/LTM package, 2026-11-07**.
- Implementation status: **Observed; reconciliation pending**, scoped to all five `dates/periods/` files and fiscal-year logic in `date_extensions.rs`.
- Validation status: **Evidence inspected; independent date/convention examples passed; production qualification pending**. Documentation links/status/count checks and independent ISO/fiscal/leap/LTM examples passed, including positive anchored partitions across a 400-year Gregorian cycle. Linked Rust/host tests were located or inspected, not run; these checks do not certify the current implementation.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

This record owns reporting-period identities, Gregorian/fiscal partitioning,
calendar context, stepping/ordering, actual/forecast classification and LTM window
geometry. [Accepted 007](007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions)
owns checked anchored date arithmetic. [Accepted analytics frequency](../analytics/01-panels-windows-and-wire.md#accepted-decision-explicit-observation-frequency),
[actual initial valuations](../analytics/01-panels-windows-and-wire.md#accepted-decision-panels-windows-wire-and-summary)
and [analytics grouping](../analytics/03-periods-lookbacks-and-win-loss.md#accepted-decision-calendar-grouping-lookbacks-and-win-loss-statistics)
retain their scopes; the package does not reopen those decisions. A calendar
partition, a statistical observation frequency and a financial aggregation rule
have distinct meanings.

## Accepted decision: reporting periods, fiscal context and LTM windows

Accepted on **2026-11-07** after whole-record review. Runtime reconciliation
and independent production/host qualification remain pending; observed behavior remains recorded separately below.

1. **Retain exact Gregorian partitions and half-open boundaries.** A daily
   reporting period is one calendar day, not one trading session. Gregorian
   weeks are ISO week-year Monday-to-Monday periods, with 52 or 53 valid weeks
   and possible calendar-year crossing. Months/quarters/half-years/years begin
   January-based blocks of 1/3/6/12 months. All concrete periods are positive
   `[start,end)` intervals; the exclusive end must be representable. No holiday
   rolling, business-day omission, accrual convention or financial year fraction
   is implied. `PeriodKind`'s six exact snake_case wire labels remain unchanged.

2. **Use an explicit fiscal opening and literal label rule.** Retain validated
   `FiscalConfig(start_month,start_day)` with independent month `1..=12` and
   nominal day `1..=31`. Clamp the day separately to the opening month's actual
   length in each opening year; February 30/31 deliberately mean its last day.
   Only January 1 uses its opening calendar-year label. Every other opening,
   including January 15, uses opening year plus one. Thus FY2025 with October 1
   opens 2024-10-01; FY2025 with January 15 opens 2024-01-15. This is a library
   labeling convention, not a claim about statutory labels. Keep explicit
   `calendar_year()` for fiscal January 1; remove implicit `FiscalConfig::default`
   and jurisdiction-named US/UK/Japan/Australia presets. Callers select their
   actual month/day and legal/reporting regime; no country-to-calendar inference.

3. **Partition fiscal years from the actual clamped opening.** Fiscal days
   partition the year into its actual 365/366 calendar days. Fiscal weeks are
   successive seven-day blocks from that opening: blocks 1..52 are seven days,
   block 53 is the remaining one or two days. They are not ISO weeks, 52/53-week
   retail years, 4-4-5 periods or a full seven-day week at year-end. Fiscal M/Q/H
   boundaries are direct 1/3/6-month multiples from the actual opening anchor,
   using 007's clamped month arithmetic rather than chained additions. The final
   block ends at the next independently resolved fiscal opening. Annual periods
   use those actual openings. Adjacent periods must meet without gaps/overlaps;
   February clamping may make the final block a different length. Do not replace
   these rules with nominal-day restoration or EOM preservation.

4. **Retain symbolic IDs and carry one explicit calendar context.** `PeriodId`
   remains `(year,index,kind,fiscal)`; a fiscal string does not encode its opening
   rule. Reuse the existing `PeriodCalendar` as the canonical public context
   (`Gregorian` or `Fiscal(FiscalConfig)`) and retain it once in `PeriodPlan`.
   Bound/date-map/shift operations require that explicit context and reject a
   Gregorian/fiscal mismatch; do not silently relabel an ID. Unconfigured
   Gregorian builders reject explicit FY text. Fiscal builders may accept
   unprefixed IDs as fiscal because their configuration is explicit; emitted
   fiscal IDs always have FY. Standalone FY parsing validates symbolic capacities
   only (D<=366, W<=53, M<=12, Q<=4, H<=2, annual index 1), with actual validity
   checked when resolved under a configuration. IDs from different fiscal
   contexts are not interchangeable economic keys; owning models retain context.
   No per-ID configuration duplication, calendar registry or retail-calendar
   framework is added.

5. **Make the grammar exact, signed and round-trippable.** After outer trimming,
   accept optional case-insensitive FY, an ASCII signed decimal year fitting
   `i32`, and either no suffix for annual or exactly one D/W/M/Q/H plus an ASCII
   unsigned decimal index in range. Year zero/negative years use the library's
   proleptic Gregorian convention. Leading plus/zeros and letter case normalize;
   reject internal whitespace, separators, decimal/exponent notation, repeated
   designators and missing/zero indices. Canonical output uses uppercase FY and
   designators, normalized signed year, D/W/M widths 3/2/2 and unpadded Q/H.
   Gregorian parsing and constructors validate actual requested bounds, including
   leap days/ISO week counts/exclusive ends; make `annual` fallible too. FY IDs
   remain symbolic until configured resolution. Do not restrict a fiscal label
   to the Date year range when its requested concrete interval is representable.

6. **Keep inclusive ranges and strict actuals classification.** Accept one
   `..` separator with complete ID endpoints or a right endpoint consisting of
   exactly one matching designator plus index in the left year. Annual ranges
   require an absolute right year; signed absolute endpoints are classified by
   grammar, not first-character digit tests. Both endpoints must have one kind
   and compatible calendar context; reversed ranges error, equal IDs produce
   one period. Require every requested bound to fit and cap final output at
   100,000 periods, rejecting excess without truncation or partial publication.
   An explicit actuals cutoff must have the same kind/context and valid bounds,
   even outside the range: before/after it yields all forecast/all actual.
   Omitted cutoff means all forecast. Never infer actuals from today's date.

7. **Use one checked temporal stepping path.** Resolve the source under the
   explicit calendar, validate it even for zero shift, then return a checked
   target of the same kind/context. Gregorian daily/weekly stepping uses actual
   1/7-day offsets and ISO year transitions. Fiscal daily stepping traverses
   actual fiscal year lengths; fiscal weekly stepping traverses its 53 reporting
   blocks, including the short final block. M/Q/H/A step ordinal reporting slots
   with checked integer/year arithmetic and validate target bounds. Consolidate
   `next_fiscal`/`prev_fiscal` and context-free fiscal shortcuts into canonical
   context-aware `shift`; remove the special day-1..365 path and silent fiscal
   reflagging. `from_date` must identify exactly one containing resolved interval.
   A fiscal-year label calculation alone does not certify representable interval
   bounds. Date/year/resource failures remain errors, not fallback dates or gaps.

8. **Make ID ordering structural and total.** Replace date-based/fallback
   comparisons with lexicographic `(year,kind_rank,index,fiscal)` ordering, where
   ranks are Daily, Weekly, Monthly, Quarterly, SemiAnnual, Annual and false
   precedes true; use that same rank for `PeriodKind` ordering. This is
   deterministic key ordering consistent with equality;
   it is not chronological order across kinds or fiscal contexts. Chronological
   work sorts already resolved intervals by start, end and structural ID under
   their known context. Reject mixed-kind/context plans instead of using this
   structural order to infer financial chronology or actuals. This removes the
   source-observed nontransitive mixed-kind comparator.

9. **Keep statistical annualization separate from partition cardinality.**
   Rename `PeriodKind::periods_per_year` to `annualization_factor`, retaining the
   conventional mapping 252/52/12/4/2/1 for explicitly selected daily trading,
   weekly, monthly, quarterly, semiannual and annual observations. It is not an
   observed count, calendar validity check, business-calendar assumption, LTM
   length, payment frequency or conversion of dated time. Accepted analytics
   still requires explicit frequency or a positive finite scalar factor; no
   frequency is inferred from reporting IDs/rows/dates and this package does not
   adopt downstream omitted-factor defaults. Calendar counts and reporting-slot
   logic use actual year/period definitions, not this statistical map. Reconcile
   each non-risk use under its financial owner rather than mechanically renaming
   an incorrect factor use. Remove `prior_observation_date` and synthetic initial
   valuation inference; literal date offsets remain available through 007, while
   analytics requires the real initial valuation date already accepted.

10. **Validate concrete periods/plans at every construction and restore boundary.**
    Use private validated state and read-only accessors. A resolved Period needs
    its calendar to validate positive canonical bounds matching its ID; a naked
    fiscal object cannot claim validation from dates alone. Retain calendar once
    at plan level. Standalone fiscal Period JSON/pickle capture and restoration
    are unsupported: use the validated PeriodPlan envelope carrying its calendar.
    Standalone Gregorian Period state can use its inherent Gregorian context;
    validate its bounds through the same path, without an alternate unchecked
    constructor. A plan is nonempty, homogeneous,
    chronological, duplicate-free, canonical and contiguous, at most 100,000 rows,
    with matching adjacent boundaries and IDs. Actual flags form one true prefix
    followed by false, including either empty prefix/suffix; each flag is explicit
    on restore, not recomputed from a wall clock. Validate the whole state before
    publishing it, including config, kind/flag/index/bounds and context consistency.
    Unknown/missing required fields and invalid state fail without sorting,
    deduplication, clipping, default context or restoration migrations. Existing
    flat/custom period state is reconciled with its actual owning use, not silently
    coerced into this canonical plan.

11. **Define reporting LTM and daily LTM separately; reject weekly LTM.**
    `ltm_window` takes a target plus a validated plan retaining calendar context;
    require the target to be present in that plan. For M/Q/H/A, select the last
    12/4/2/1 complete canonical reporting periods ending at the target. For daily,
    use every canonical calendar day starting in
    `[target.end.add_months(-12),target.end)`, with the start clamped under 007.
    Daily fiscal IDs are resolved under the plan's configuration, never Gregorian
    fallback. Daily means full calendar-day coverage: sparse business-day models
    are not admitted as complete daily LTM, and absent rows do not prove weekends
    or holidays. Their owners can use explicit dated sums/rollups under their own
    coverage and aggregation contract; no inferred closure or new business-grid
    registry is introduced. Reject weekly generic LTM: 52 weeks is 364 days and
    fiscal week53 may be short; neither is automatically twelve months. Owners
    select any 52-week or partial-week allocation calculation separately.
    Reporting-period LTM need not start on the same date as subtracting twelve
    months from its end when fiscal/leap clamping differs. No implicit proration.

12. **Make LTM completeness a precise geometric result.** Return existing plan
    slots selected in chronological order through the target; `missing` counts
    expected canonical reporting slots absent from that required window, and
    `required = slots.len()+missing`. `is_complete` means missing zero, not valid
    financial values or publication provenance. For a valid contiguous plan,
    only leading history can be absent; interior holes/duplicates/mixed contexts
    are invalid-plan errors, not repaired missing counts. Date-domain/shift errors
    are errors, not swallowed into missing; incomplete representable history is
    an ordinary incomplete window. Do not partial-sum, backfill, rescale to a year
    or annualize an incomplete LTM. The core does not filter actual/forecast slots;
    callers retain those flags and decide eligibility. They also require suitable
    additive period flows, consistent currency/units and complete finite values;
    window geometry does not authorize summing stocks, rates or overlapping YTD
    totals. Owners retain exact missing-value/result, numeric summation, rounding
    and ratio rules. Malformed window errors propagate through statement/report
    callers rather than becoming indistinguishable NaN/None availability results.

13. **Reconcile the full Rust/host/caller surface together.** Update canonical
    types/re-exports, parser/stepper/builders, Python/WASM APIs and documented
    exceptions, schemas/parity, examples and actual analytics/statements/reporting
    consumers in a working slice. Hosts reject booleans/fractional/nonfinite/out-of-
    range integer inputs, invalid dates/configuration, malformed syntax and missing
    required calendar context, without truncation or implicit January 1. Preserve
    canonical Rust naming, supported integer domains and value/wire semantics.
    Remove retired names/defaults/presets and approximate completeness paths without
    aliases or compatibility layers. No runtime change, statutory fiscal regime,
    business-observation calendar, retailer schedule, financial annualization default
    or statement aggregation policy is approved merely by observing it in source.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Gregorian 2020W53 | `[2020-12-28,2021-01-04)`; 2021-01-01 maps to that ISO week-year. |
| Gregorian 2021W53 / 2023D366 | Error: nonexistent ISO week / nonleap day. |
| Fiscal Jan1 / Jan15 / Oct1, label FY2025 | Open 2025-01-01 / 2024-01-15 / 2024-10-01, respectively. |
| Nominal Feb29, FY2024 | Open 2023-02-28; monthly block2 starts 2023-03-28; final end 2024-02-29. |
| Fiscal year of 365 / 366 days | Week53 has 1 / 2 days; it is not a full ISO/retail week. |
| Explicit FY ID in Gregorian builder; FY bounds/shift without config | Error without Gregorian substitution or fiscal reflagging. |
| FY2025D366 | Symbolic parse may succeed; configured resolution rejects it if that FY has only 365 days. |
| Negative/year-zero IDs and signed absolute range endpoints | Round-trip under the proleptic domain; actual requested bounds must fit. |
| `2025Q1..Q4` / `2025Q1..QQ4` / `2025Q1..M4` | Four quarters / error / error. |
| Equal endpoint; mixed kinds; reversed range; 100,001 periods | One period / error / error / resource error, with no partial plan. |
| Cutoff omitted / before range / after range | All forecast / all forecast / all actual; matching valid kind/context still required. |
| Fiscal day366 shifted across year-end | Resolve actual fiscal length and target under config; no 365-day shortcut. |
| Mixed FY2025W53, 2025 and 2025M12 keys | Stable structural total order; chronological uses resolve their actual contexts. |
| Daily reporting count in 2025 / selected daily-trading annualization | 365 calendar days / statistical factor252; neither infers the other. |
| Initial return endpoint without actual initial valuation date | Cannot synthesize the baseline by subtracting one period. |
| Restored duplicate, wrong bound/context, interior gap or true-after-false flag | Atomic validation error, without sorting or repair. |
| Standalone fiscal Period capture/restore without plan envelope | Unsupported; fiscal context is retained once in validated PeriodPlan state. |
| Monthly/Q/H/A LTM ending at a present target | Last12/4/2/1 reporting periods; required history counted independently of values. |
| FY2025 Feb29 annual period | `[2024-02-29,2025-02-28)`; daily window at that exclusive end instead starts 2024-02-28. |
| Daily LTM ending 2025-12-31 exclusive | `[2024-12-31,2025-12-31)`, 365 expected calendar days. |
| 2025 weekday-only daily grid | Not a complete calendar-day plan/window; missing weekends are not inferred closed. |
| Weekly target passed to generic LTM | Explicit unsupported-kind error, not a mislabeled 52-week sum. |
| Three available quarters of four required | slots3, missing1, required4, incomplete; no partial annualized total. |
| Complete calendar window but missing/nonfinite values, stocks or overlapping YTD | Geometry alone does not establish valid financial aggregation. |
| LTM target absent or date subtraction outside supported range | Error, distinct from representable but insufficient leading history. |

## Observed algorithm and conventions

Gregorian daily periods are ordinal date to next day. Weekly periods use ISO week-year/Monday to Monday, allowing 52/53 weeks independently of calendar year. Month/quarter/half-year/year periods span 1/3/6/12 months, half-open `[start,end)`. Fiscal start month/day is configurable with day 1–31 clamped to the actual start month length. A Jan-1 fiscal year is labeled with its calendar year; every other fiscal start belongs to the prior calendar year of its label. Daily/weekly fiscal indices offset from fiscal start, with week 53 truncated at next fiscal year start. Month blocks are anchored to fiscal start; the last block ends at the next actual fiscal start so February clamping does not leave a gap.

`PeriodId::from_date` maps to the containing period and verifies containment. A fiscal ID alone cannot recover its `FiscalConfig`; `bounds()` rejects it. Gregorian daily/weekly shifts add actual days/weeks; fixed-count kinds use ordinal year/index arithmetic. Fiscal weekly shifts use capacity 53; fiscal daily shifts without config may stay within days 1–365 (zero is identity even for day 366) but reject boundary crossing; `next_fiscal`/`prev_fiscal` accept config. `prior_observation_date` subtracts 1 calendar day, 7 days or 1/3/6/12 clamped months; no calendar rolling is applied.

Range parsers accept absolute IDs and abbreviated matching-kind endpoints; reject reversed ranges or mixed kinds/fiscalness. Builders validate actual bounds and cap output at 100,000 periods. The optional actuals cutoff must share kind/calendar and marks IDs <= cutoff as actual; no cutoff means all forecast. `PeriodId` sorting first compares year, then index/fiscalness for same kind; mixed kinds compare Gregorian bounds then kind/index, so it is not a fiscal-calendar-aware chronological ordering across mixed schemes.

Annualization factors are Daily252, Weekly52, Monthly12, Quarterly4, SemiAnnual2, Annual1. For non-daily LTM, create that many successive IDs ending at the requested ID; missing counts only failed shifts, not absent observations in supplied data. Weekly LTM is therefore 52 IDs even in a 53-week year. For daily LTM, take target end from matching supplied period or Gregorian `bounds`, subtract 12 months, filter/sort matching-calendar daily grid, and include rows whose starts lie in the window. Missing counts only calendar days before the first supplied grid row; interior gaps/weekends are accepted and duplicates are not repaired. A missing fiscal target cannot use Gregorian fallback bounds.

## Observed selection, limitations and alternatives

Default FiscalConfig is Jan 1; named US federal/UK/Japan/Australia starts are Oct1/Apr6/Apr1/Jul1. Fiscal labels and clamped midmonth rules are implementation choices, not proof of statutory financial-year requirements. Annualization factors do not validate data sampling. LTM completeness is a leading-coverage indicator, not a general missing-data validator. Retail 4-4-5/52-53-week fiscal calendars are not implemented.

## Evidence

The source-observed mixed-kind comparator combines Gregorian-bound comparisons
and enum-rank fallbacks. With `A=FY2025W53`, `B=2025` and `C=2025M12`, its branches
imply `A<B`, `B<C`, `C<A` because Gregorian 2025W53 does not exist. This is a static
nontransitivity finding, not an executed Rust reproduction. The Gregorian builder
also accepts explicit FY IDs while its bounds engine stays Gregorian; range parsing
strips repeated designators, and signed annual/right-endpoint handling is inconsistent.
The stronger package above therefore requires reconciliation rather than describing
current parser/order/state behavior as already compliant.

[Python period bindings](../../../../finstack-quant-py/src/bindings/core/dates/periods.rs),
[WASM period bindings](../../../../finstack-quant-wasm/src/api/core/periods.rs),
[statement TTM](../../../../finstack-quant/statements/src/evaluator/formula_aggregates.rs),
[reporting LTM](../../../../finstack-quant/statements-analytics/src/analysis/reports.rs)
and [capital-structure LTM](../../../../finstack-quant/statements-analytics/src/analysis/credit/capital_structure.rs)
locate actual host/caller reconciliation boundaries. Their current default factors,
sparse-grid fixtures and error-to-unavailable conversions do not adopt those choices.

[The time project's ISO-week documentation](https://time-rs.github.io/book/api/format-description.html)
distinguishes ISO week-year/Monday boundaries from calendar years. It is background
for the week rule, not a pinned core build qualification or support for the fiscal
label, partial-week, LTM or statistical annualization decisions.

[Period IDs/config](../../../../finstack-quant/core/src/dates/periods/mod.rs), [bounds/building](../../../../finstack-quant/core/src/dates/periods/calendar.rs), [parsing/stepping](../../../../finstack-quant/core/src/dates/periods/parse.rs), [LTM](../../../../finstack-quant/core/src/dates/periods/ltm.rs), [period tests](../../../../finstack-quant/core/src/dates/periods/tests.rs). Inspected examples include fiscal week53, leap fiscal partitioning, ISO week boundaries and clamped fiscal starts. LTM tests distinguish 365 calendar rows, 261 weekday rows and 59 leading missing calendar days in 2025. These are invariant/expected-date tests. [ISO8601](../../../REFERENCES.md#iso-8601) is the week/date background, not support for financial annualization factors.

No weighting/proration of partial weekly periods is performed by the observed layer.

The 2026-11-07 whole-record review passed documentation links/anchors,
status/count and whitespace checks. Independent calendar/date examples checked ISO
boundaries, fiscal labels/clamping/short weeks and distinct daily/reporting LTM
intervals. All 148,800 nominal fiscal openings across one 400-year Gregorian cycle
produced positive accepted anchored month/quarter/half-year/annual partitions.
This qualifies those convention examples, not production parser/order/state/host
behavior or financial aggregation. No runtime source/tests/fixtures were changed,
and no production Rust/host test or build pass was run.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
