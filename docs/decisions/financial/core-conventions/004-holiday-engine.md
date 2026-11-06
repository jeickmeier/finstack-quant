# CORE-CONVENTIONS-004: Holiday materialization, cache equivalence and date-table horizons

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `dates/calendar/{algo,rule,types,year_cache}.rs`, generated lunar/equinox tables, `core/build.rs` / `core/build/generate_calendars.rs` registry generation, and the controlling Python CSV generation/rendering steps described below. The source ledger remains explicitly Rust-only.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

Built-in calendars materialize full-day closures from compiled rules and exclude Saturday/Sunday from business days. This gives deterministic settlement/accrual dates; it does not model exchange sessions, half-days, exceptional future notices or weekend working-day reopenings. Calendar data and jurisdiction logic are detailed in [005](005-jurisdiction-calendars.md).

## Algorithm

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

An algorithmic Easter rule, bounded tabular lunar/equinox dates and published exact-date notices coexist; none establishes universal calendar completeness. Rule caching preserves implementation behavior but cannot improve dataset accuracy. Alternatives requiring agreement include failing outside certified calendar windows, explicit projection flags, session-aware calendars or maintained weekend-reopening tables. They are not implemented here.

## Evidence

[Rule engine](../../../../finstack-quant/core/src/dates/calendar/rule.rs), [computus and nth weekday](../../../../finstack-quant/core/src/dates/calendar/algo.rs), [cache](../../../../finstack-quant/core/src/dates/calendar/year_cache.rs), [calendar predicate](../../../../finstack-quant/core/src/dates/calendar/types.rs), [generator](../../../../finstack-quant/core/build/generate_reference_data.py). [Rule tests](../../../../finstack-quant/core/src/dates/calendar/rule_tests.rs) include hand-specified dates and a 1990–2040 golden digest pinned from earlier implementation behavior; the digest is a regression invariant, not independent market evidence. [Coverage tests](../../../../finstack-quant/core/src/dates/calendar/rule_coverage_tests.rs) cover span offsets, observation modes, out-of-range tables and weekday arithmetic. Cache tests compare production rule scanning with cache results. [Meeus](../../../REFERENCES.md#meeus-1991) supplies background, not certification of the maintained CSV dates.

The renderer's `--check` was executed successfully only as a source-artifact consistency check; the lunar generator and Rust tests were not run. [Renderer tests](../../../../scripts/tests/test_generate_core_reference_data.py) exercise malformed/missing data, drift detection and isolated idempotent output; they were inspected, not executed, and do not independently test lunar/solar calculations. The 1970–2150 cache/table range must not be described as full published-holiday validation.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
