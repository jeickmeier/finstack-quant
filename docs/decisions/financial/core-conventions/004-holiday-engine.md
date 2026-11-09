# CORE-CONVENTIONS-004: Holiday materialization, cache equivalence and date-table horizons

- Decision status: **Accepted — complete holiday-engine package, 2026-11-07**.
- Implementation status: **Partly reconciled 2026-11-08**: item 7's fallible membership/counting contract is in place (trait methods return `Result` and callers propagate); the engine still reports missing tables and invalid rules as absence until the remaining items land, scoped to `dates/calendar/{algo,rule,types,year_cache}.rs`, generated lunar/equinox tables, `core/build.rs` / `core/build/generate_calendars.rs` registry generation, and the controlling Python CSV generation/rendering steps described below. The source ledger remains explicitly Rust-only.
- Validation status: **Evidence inspected/located; independent qualification pending**. Documentation and independent rule/date/computus-example checks passed; production calendar tests and the astronomical generator were not executed for this review.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

Built-in calendars materialize full-day closures from compiled rules and exclude Saturday/Sunday from business days. This gives deterministic settlement/accrual dates; it does not model exchange sessions, half-days or weekend working-day reopenings, or automatically incorporate future notices. Calendar data and jurisdiction logic are detailed in [005](005-jurisdiction-calendars.md).

## Accepted decision: holiday engine and data boundaries

Accepted on **2026-11-07** after whole-record review. This complete package
covers rule semantics, materialization, availability, caching and maintained-input
controls together. Runtime reconciliation and independent qualification remain
open. Jurisdiction-specific rule selection, coverage and eligibility remain
with [005](005-jurisdiction-calendars.md). The observed algorithms and
maintenance paths are recorded separately below.

1. **Keep a full-day closure engine with Saturday/Sunday weekends.**
   For the raw union of rule dates `H`, business days are dates that are
   neither Saturday/Sunday nor members of `H`. `is_holiday` reports rule
   membership, suppressing that label on weekends when `ignore_weekends`
   is true; it is not the complement of `is_business_day`. This reporting
   flag never reopens a weekend. The explicit weekends-only calendar has
   no additional rule closures. Do not promise sessions, half-days,
   non-Saturday/Sunday weekend regimes or working-weekend reopenings through
   this engine. A jurisdiction/purpose needing those capabilities must not
   be represented as fully supported by this model. Do not add a session
   or reopening framework in this record.

2. **Adopt the five observation transforms exactly.** Each fixed holiday
   has one nominal occurrence; observation replaces that rule's occurrence
   with the date below. Other rules may independently include the original
   date. The weekend predicate remains separate.

   | Observation | Saturday | Sunday | Other weekdays |
   | --- | --- | --- | --- |
   | None | Unchanged | Unchanged | Unchanged |
   | Next Monday | Add two days | Add one day | Unchanged |
   | Friday if Saturday / Monday if Sunday | Subtract one day | Add one day | Unchanged |
   | Monday if Sunday | Unchanged | Add one day | Unchanged |
   | Two-day weekend shift | Add two days | Add two days | Unchanged |

   The last transform describes the existing paired Christmas/Boxing-Day
   algorithm, not a general collision resolver or universal legal rule.
   Overlapping closures form a set; duplicate dates count once and do not
   trigger extra substitute holidays. Particular calendars must explicitly
   supply any additional statutory collision/substitution rules.

3. **Adopt exact-date, weekday and Gregorian Easter semantics.** Exact
   dates occur only in their specified nominal year. Recurring month/day
   rules may specify February 29, with no occurrence in a common year;
   impossible patterns such as April 31 are invalid. Nth weekdays support
   `-5..=-1` and `1..=5`; negative means counting backwards from month end.
   A nonexistent fifth occurrence is a valid empty result, while zero or
   any other ordinal is invalid. Weekday shifts mean inclusive on-or-before
   or on-or-after, moving at most six calendar days. Easter uses the selected
   integer Gregorian computus, with Easter Monday one day after Easter
   Sunday and an explicit signed calendar-day offset thereafter. It is
   ecclesiastical Easter, not a calculation from observed astronomical
   phases; see [USNO](https://aa.usno.navy.mil/faq/easter) and the
   [Royal Observatory algorithm](https://robinfo.oma.be/en/astro-info/calendar/algorithm-for-the-computation-of-the-easter-date/).
   Support positive representable Gregorian years, explicitly proleptic
   before historical adoption; this does not approve a jurisdiction's
   historical calendar or an Orthodox Easter rule. An active Easter rule
   outside that domain errors rather than silently contributing no holiday.

4. **Make span and bridge domains explicit.** A span has positive length
   `1..=255` and emits `anchor + offset + k` for `k = 0..len-1`, in calendar
   days, from each supplied anchor occurrence. Reject zero length as an
   invalid definition; omitting a rule or an inactive effective range is
   how a rule is absent. Do not implicitly roll span dates or remove their
   weekends. ChinaBridge takes one unobserved festival occurrence per active
   nominal year, or a legitimate absent occurrence; reject pre-observed or
   multi-occurrence festival sources. Keep its weekday expansion: Mon/Wed/Fri
   emits the festival; Tue emits preceding Mon plus Tue; Thu emits Thu plus
   following Fri; Sat/Sun emits following Mon. It neither reopens weekends
   nor establishes an official Chinese holiday schedule. Country-specific
   adoption, notices and estimates remain with 005.

5. **Gate once on the nominal event year and retain spillovers.**
   Inclusive `from_year` / `to_year` bounds apply to the underlying nominal
   event year before observation, offsets, spans or bridges. Do not gate
   again on the resulting closure date's year. Each annual event is
   materialized once for its nominal year; transforms operate on those
   occurrences without independently rematerializing prior-year anchors.
   Gather all potentially contributing nominal years and then filter the
   resulting sorted, unique closure set to the requested dates. This is an
   explicit engine convention; jurisdictional exceptions belong in data.

6. **Evaluate the complete supported transform reach.** Derive finite
   candidate anchor ranges from validated composed offsets, observation
   shifts and span lengths. Do not silently stop at the requested year plus
   or minus one, or truncate the existing signed-offset domain. Preserve
   nominal-year identity through nested transforms. Use checked date/year
   arithmetic and reject invalid or cyclic rule definitions. Skip an
   unavailable probe only when it is provably unable to affect the request;
   a coarse search range is not proof that every probe is required. A
   required unrepresentable date/source year is an explicit error. Avoid
   unnecessary out-of-range construction, such as requiring next January
   to find the last weekday of a representable December.

7. **Distinguish valid absence from missing required data.** An inactive
   effective rule, a common-year February 29 or a nonexistent fifth weekday
   can validly emit no date. An active lunar/equinox table lacking a needed
   year, unsupported calendar coverage, invalid definition or failed
   calculation must error. Use fallible canonical rule/calendar membership
   and counting operations, propagate their failures through financial
   callers, and remove warning-plus-empty/false behavior and compatibility
   paths. No failed calculation may publish a partial calendar or cache
   entry. Source availability must cover potentially contributing spillover
   years; irrelevant neighboring years must not create false coverage errors.

8. **Separate coverage, projection and cache horizons.** Extend compiled
   built-ins' existing calendar metadata with purpose, supported ranges, frozen
   source edition/revision and evidence basis, distinguishing published,
   supported recurring-rule and projected ranges within mixed datasets.
   The 1970–2150 cache and current table extents are neither universal legal
   coverage nor approval for every calendar/year. Queries outside the
   declared domain fail. Forecast use requires explicit adoption for the
   named calendar/purpose/range in 005 or its consuming product; preserve
   the projected status and never relabel a forecast as published because
   it appears in a table. Japanese equinox holidays become official through
   the preceding year's Gazette publication; earlier astronomical dates
   remain forecasts, as [NAOJ explains](https://www.nao.ac.jp/faq/a0301.html).
   Use recorded publication evidence, not the current wall clock, to assign
   that status. Custom calendars retain their explicit provider contracts
   and optional metadata under [001](001-actual-day-counts.md#accepted-decision-actual-day-conventions);
   missing custom metadata must not invent built-in coverage or a fallback.
   No generic projection-policy framework is introduced here.

9. **Require cache and direct evaluation to agree exactly.** Retain the
   bitset/prefix cache as an optimization of the same validated closure set
   and error contract. Keys distinguish the immutable calendar definition
   and source data, not only its display ID; process-local rule identity is
   sufficient for genuinely immutable built-in arrays. Never cache partial
   success after a missing dependency. A year entry may be used only when
   that complete entry is admissible. If a valid query needs fewer data than
   the full year, evaluate it directly rather than introducing a cache-only
   coverage error or publishing a partial year entry. Outside the cache window, direct
   evaluation is permitted only when the needed rules/data are supported.
   Count business days in `[start,end)`, once per date, using exact integers;
   validate the calendar and requested domain before equal-date zero and reject reversed
   unsigned ranges. Prefix and scan results must match exactly, with
   `0 <= count <= elapsed days`. Cache equivalence proves computational
   consistency, not the correctness or completeness of holiday sources.

10. **Admit maintained JSON strictly and make financial controls explicit.**
    Reject unknown fields recursively, malformed/case-colliding IDs,
    impossible dates, invalid weekday ordinals, zero span lengths,
    inverted year bounds and invalid rule composition before registry
    generation. Keep explicitly supported informational fields such as
    `name`, `notes` and `comment`; they do not alter financial behavior.
    Require a fixed rule's observation choice, including explicit `none`,
    the calendar's `ignore_weekends` Boolean and a span's offset, including
    explicit zero. Omitted effective bounds mean unbounded on that side,
    subject to declared data/domain coverage; they do not grant projection
    eligibility. Canonical built-in IDs are nonempty lowercase ASCII within
    the existing 32-byte standalone lookup domain and unique under ASCII
    case folding. Retain case-insensitive standalone lookup without implicit
    whitespace trimming; unknown IDs error without selecting weekends-only.

11. **Render deterministically from frozen, independently covered inputs.**
    Cargo registry generation and `gen-write` consume checked-in JSON/CSV
    without network access or automatic astronomical refresh. Require exact
    CSV headers and row widths, valid dates, no duplicate/missing years and
    exactly one date for each year in each declared festival-table range.
    Reject extra trailing cells rather than ignoring them. Declare table
    ranges independently of the cache window; changing cache bounds must
    not manufacture or require new holiday data. Keep stable ordering,
    deterministic rendering and a read-only drift check. Other outputs of
    the shared renderer retain the financial owners in [010](010-sifma-settlement.md)
    and [012](012-money-decimal-precision.md).

12. **Make astronomical maintenance reproducible and qualification explicit.**
    Keep refresh separate from rendering and pin its existing library and
    environment through maintained dependency controls. Record the exact
    generator revision, command, library version, source/publication inputs,
    output revision and civil-time assumptions. Qualify the library's UTC+8
    interpretation and the selected UTC+8 Chinese/Qing-Ming and UTC+9 Japanese
    date conversion. Ordinary lunar month/day selection is explicit;
    statutory effective years, observance and exchange closures remain
    calendar rules. Establish the CNY input's provenance and reproducible
    refresh path within this maintenance process. Independently compare
    dated vectors, including dates near local midnight and official
    Japanese publications, before claiming agreement. Remove unsupported
    cross-library/official-calibration assertions; maintained rows and a
    passing renderer check alone do not certify market dates.

13. **Keep ownership and host behavior coherent.** Preserve the existing
    rule/calendar/generator components and propagate the selected validation,
    availability and error semantics through Rust, Python, WASM, wire/schema
    forms, metadata and consuming calculations. Jurisdiction delegates must
    obey the same closure/error boundary; their legal contents are reviewed
    in 005. No new calendar registry, automatic data service, compatibility
    layer or market-wide session engine is adopted. Methodology agreement
   does not implement or independently qualify these changes; the
    [existing follow-ups](../FOLLOW_UPS.md#core-conventions) own that work.

## Boundary examples for the accepted package

These are independently calculated rule examples and proposed admission
outcomes, not executed production tests or verified jurisdiction calendars.

| Rule / request | Required result |
| --- | --- |
| Nominal January 1 with Friday-if-Saturday observation, effective from 2022 | The 2022 occurrence contributes `2021-12-31`, even when requesting closures in 2021. |
| Same rule effective only through 2021 | Its 2022 occurrence cannot contribute `2021-12-31`. |
| Exact `2020-01-01`, span length 1 and offset 800 days | Emit `2022-03-11`, including when querying 2022; do not lose its 2020 anchor. |
| Fixed February 29 in 2025; fifth Monday of February 2025 | Valid empty occurrence sets. |
| April 31, nth weekday zero or span length zero | Reject the definition. |
| On-or-after Monday from `2025-03-03` | Remain `2025-03-03`; the search is inclusive. |
| Two-day weekend-shift rules for December 25 and 26, 2021 | Emit `2021-12-27` and `2021-12-28`, respectively. |
| ChinaBridge toy festival Tuesday `2025-03-04` | Emit Monday March 3 and Tuesday March 4; no claim about an actual festival. |
| Two rules emit the same weekday closure | One holiday date and one business-day exclusion, without an invented extra substitute. |
| Needed active table year absent; inactive rule with no needed lookup | Coverage error for the former; a valid empty occurrence for the latter. |
| Supported date outside cache window | Same answer/error as direct evaluation; cache bounds alone neither reject nor authorize the calendar. |

## Observed algorithm

- Fixed-date rules apply one of five observations: none; weekend to next Monday; Saturday to Friday/Sunday to Monday; Sunday-only to Monday; or add two days to Saturday/Sunday (paired Christmas/Boxing-Day substitution).
- Nth-weekday rules calculate positive positions from month start or negative positions from month end using weekday modular offsets; candidates outside the requested month are rejected. Weekday-shift rules are inclusive on-or-before/on-or-after, scanning at most seven days.
- Easter Monday uses the integer Gregorian Meeus/Jones/Butcher-style computus in `easter_monday`; Easter offsets add signed calendar days with checked addition.
- Exact dates emit only in their year. Spans emit `len` consecutive calendar days from each start plus `offset`; zero length emits none. Span materialization includes prior-year starts when needed. `dates_in_year` evaluates neighboring base years and filters candidates back to the requested year to retain observed cross-year closures.
- Effective wrappers gate inclusively by requested year at `dates_in_year`, and by base year at inner materialization. The position of a year gate is therefore material for cross-year observations.
- China bridge rules emit festival alone on Mon/Wed/Fri; preceding Monday plus Tuesday festival; Thursday plus following Friday; or following Monday for a weekend festival. They do not reopen any weekend.
- Chinese New Year and seven festival/equinox helpers look up `(month,day)` in generated perfect-hash tables for 1970–2150; absent years return `None`, so that holiday rule contributes no date.

The year cache materializes rule dates into six `u64` holiday bitsets and 367 cumulative business-day counts. Interval counts subtract prefixes for each year over `[start,end)`; `start >= end` gives zero. Cache keys include calendar ID, year, ignore-weekends flag and rule-slice identity/length. Outside 1970–2150 it falls back to daily/rule evaluation; `is_holiday` logs a once-only warning. A successful fallback can omit tabulated holidays.

## Selection, conventions and controls

Built-in `Calendar::is_business_day` is always `!Saturday/Sunday && !holiday`. `ignore_weekends` only suppresses weekend labels in `is_holiday`; it never makes weekends business days. The build-generated registry is included from `OUT_DIR`; [calendar JSON definitions](../../../../finstack-quant/core/data/calendars) are the financial inputs. All eight lunar/equinox tables contain one valid date per year over their stated range, as enforced by their generator. No holiday approximation is selected automatically when table lookup returns none.

## Build-time financial selection

[Build entry](../../../../finstack-quant/core/build.rs) regenerates the registry from the calendar-data directory. [Generator](../../../../finstack-quant/core/build/generate_calendars.rs) reads every `.json`, rejects duplicate exact IDs, orders IDs through a BTreeMap and emits static Rule arrays plus a registry. Missing `observed` emits `Rule::fixed` (no substitution); missing `ignore_weekends` emits false; missing span offset emits zero. Fixed/Easter/nth-weekday/span/ChinaBridge year bounds wrap the selected inner rule in `Effective`. Month/weekday/observed/direction names parse case-insensitively. Registry lookup tries exact ID then ASCII-lowercased ID up to 32 bytes; it does not trim a standalone ID. Only weekends-only is publicly exposed as a static; all other calendars resolve by ID.

The generator ignores unknown JSON fields (including descriptive `name` fields by design) and does not validate date existence, nth-weekday range or from/to ordering. The inspected current data use the intended fields, but a misspelled future observation/year control could be silently ignored; invalid dates can disappear when runtime Date construction fails. Semantic validation of financial definitions is an open gap, not proof of an incorrect current holiday row.

## Lunar/equinox input generation and CSV rendering

[Festival input generator](../../../../finstack-quant/core/build/gen_lunar_festivals.py) is a manual maintenance tool with an executable regeneration command, not a step invoked by Cargo or `mise run gen-write`. It delegates calendar/astronomical calculation to `lunar_python` over the inclusive `BASE_YEAR`/`END_YEAR` read from `algo.rs` (currently 1970–2150). For each lunar year, `Lunar.fromYmd(...).getSolar()` converts the ordinary lunar month/day pairs Dragon Boat `(5,5)`, Mid-Autumn `(8,15)`, Buddha's Birthday `(4,8)` and Chung Yeung `(9,9)`; a Gregorian year different from the requested year raises an error. For Qing Ming and Japanese equinoxes, it reads the `清明`, `春分` and `秋分` solar-term instants from the library's JieQi table selected through April 1 of each year. It treats the library times as UTC+8, adds zero hours for Qing Ming or one hour for Japan's UTC+9, and retains the local month/day. There is no runtime astronomical calculation or alternate-library fallback. The script writes seven CSVs; it does not generate Chinese New Year, apply statutory effective-year gates, substitute observed holidays or decide exchange closures. Those controls remain in calendar definitions and the rule engine.

[Reference-data renderer](../../../../finstack-quant/core/build/generate_reference_data.py) is the separate `gen-write`/`gen-check-sources` step: it takes maintained CSVs as authoritative and neither fetches nor recomputes their dates. It requires the exact date header, sorts rows, requires exactly one row for every year in the shared range and validates each Gregorian date before emitting the CNY and seven festival perfect-hash tables. It also sorts/validates ISO currency identities and scales and SIFMA month/class observations before rendering their Rust tables (and available sibling WASM currency factories); unpublished SIFMA class cells stay `None`, without estimation. See [012](012-money-decimal-precision.md) and [010](010-sifma-settlement.md). `--check` compares expected source text with checked-in artifacts and fails on drift without writing; it does not re-run lunar calculations or verify the CSVs against official publications.

Neither Python script copies or queries QuantLib. The festival generator's header claims agreement with `lunardate` over 1970–2099 and calibration to an official Chinese calendar, but no comparison is executed by either script and that provenance was not independently established here. Its documented `uv run --with lunar_python` command does not pin the library version, and neither `lunar_python` nor `lunardate` was found in the inspected root dependency lock/configuration. A recorded generator-library version and independent dated vectors are absent; Japanese future equinox dates remain astronomical projections subject to official announcements. The separate Chinese New Year CSV's originating calculation was not established by these two generators. Import/conversion failures propagate rather than silently selecting approximate dates.

## Limitations and alternatives

An algorithmic Easter rule, bounded tabular lunar/equinox dates and published exact-date notices coexist; none establishes universal calendar completeness. The accepted package requires declared coverage, explicit forecast eligibility and fallible queries when required data are unavailable. Cache equivalence cannot improve dataset accuracy. Session-aware calendars, other weekend regimes and maintained weekend-reopening machinery are outside this engine's selected scope. The observed warning/omission paths remain implementation reconciliation work.

## Evidence

[Rule engine](../../../../finstack-quant/core/src/dates/calendar/rule.rs), [computus and nth weekday](../../../../finstack-quant/core/src/dates/calendar/algo.rs), [cache](../../../../finstack-quant/core/src/dates/calendar/year_cache.rs), [calendar predicate](../../../../finstack-quant/core/src/dates/calendar/types.rs), [generator](../../../../finstack-quant/core/build/generate_reference_data.py). [Rule tests](../../../../finstack-quant/core/src/dates/calendar/tests/rule.rs) include hand-specified dates and a 1990–2040 golden digest pinned from earlier implementation behavior; the digest is a regression invariant, not independent market evidence. [Coverage tests](../../../../finstack-quant/core/src/dates/calendar/tests/rule_coverage.rs) cover span offsets, observation modes, out-of-range tables and weekday arithmetic. Cache tests compare production rule scanning with cache results. [Meeus](../../../REFERENCES.md#meeus-1991) supplies background, not certification of the maintained CSV dates.

The renderer's `--check` was executed successfully only as a source-artifact consistency check; the lunar generator and Rust tests were not run. [Renderer tests](../../../../scripts/tests/test_generate_core_reference_data.py) exercise malformed/missing data, drift detection and isolated idempotent output; they were inspected, not executed, and do not independently test lunar/solar calculations. The 1970–2150 cache/table range must not be described as full published-holiday validation.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
