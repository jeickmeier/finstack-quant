# CORE-CONVENTIONS-005: Jurisdiction-specific substitutions and maintained holiday selections

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `dates/calendar/jurisdictions.rs` and all 28 `core/data/calendars/*.json` definitions.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

The implementation distinguishes banking, fixing, accrual and exchange calendars by ID. Rule selections are contractual data, not interchangeable labels. The complete inspected rule parameters, including year gates, are reproduced in [the calendar selection appendix](calendar-selections.md); its source links resolve to all 28 data files. Those definitions are an observed snapshot, not a claim that every year is externally verified.

## Algorithm: Hong Kong

`hkhk` and `hkex` use the same jurisdiction function. Start with January 1 and December 25/26, Sunday-shifted; three Chinese New Year days; Qing Ming, Dragon Boat, Chung Yeung; Good Friday, Easter Saturday and Easter Monday; and the day after Mid-Autumn. A Sunday within the Chinese New Year block substitutes the day before New Year for 1983–2012, otherwise day four. The day after Mid-Autumn falling Sunday uses festival day in 1983–2011, otherwise Monday.

Add May 1/Buddha's Birthday from 1999, July 1/October 1 from 1997. Historical branches include April 21/July 1/first August Monday through 1982, second June Saturday plus following Monday through 1997 (third Saturday in 1984), last August Monday through 1996 plus preceding Saturday from 1983, August third Monday/October 2 in 1997–98, July 2 in 1997, and dated 1981/1986/2015 closures. Sunday-shift every resulting date, sort, deduplicate, and for each duplicate collision append the next date not Sunday and not already a holiday. Saturdays are not skipped by that collision algorithm, though the business-day predicate separately excludes them.

## Algorithm: Japan

`jpto` and `jpx` combine `japan_public_holidays` with January 2/3 and December 31. The national function includes Jan 1, Feb 11, Apr 29, May 3/5, Nov 3/23 and tabulated equinoxes. Coming-of-Age moves from Jan 15 to second January Monday in 2000; Respect-for-Aged from Sept 15 to third September Monday in 2003; Marine Day starts in 1996 and changes July 20 to third July Monday in 2003; Sports Day changes Oct 10 to second October Monday in 2000. Olympic 2020/2021 branches replace Marine/Sports/Mountain dates. Mountain Day begins in 2016; May 4 becomes explicit from 2007; Emperor's Birthday is Dec 23 for 1989–2018 and Feb 23 from 2020, with dated accession/wedding/funeral closures.

From 1986, insert a non-Sunday day sandwiched between two national holidays. For Sunday national holidays on/after 1973 day 102, add Monday; from 2007 skip over other national holidays before selecting the substitute. Sort and deduplicate. The skip set is the national dates, not the entire expanded substitute/citizens set.

## Selection, conventions and controls

The appendix records every selected rule. Material distinctions include `usny` Sunday-only federal substitution; `nyse` Saturday-New-Year exception and 2025 mourning closure; `sofr` repo closures even when `sifma` has an early trading session; `cato` versus `tsx` September/November closures; `chzh` versus `six` and `target2`/`defr` versus `eurex` December 24/31 closures. `brbd` selects national accrual holidays rather than B3 trading closures. `nerc` supplies six off-peak holiday labels; its generic business-day predicate still excludes Saturdays/Sundays and is not a power hourly peak-shape calculator.

## Limitations, failure and fallback

Calendar definitions explicitly limit moving Indian exchange festivals to 2024–2026, Singapore moving holidays to 2024–2027, and Matariki dates to 2022–2052. China Spring Festival/Labour Day dated spans cover 2004–2026; outside that range named estimates are used, and single-day bridge rules approximate historical arrangements. There is no runtime completeness error inside those uncovered windows. All calendars use a full-day model. Hong Kong/JP historical algorithms extend beyond independently stored fixture years. Upcoming exceptional holidays require data maintenance.

## Alternatives and tradeoffs

Separate banking/fixing/trading IDs are supported; using a different existing ID is a caller choice, not a fallback. Exact published annual tables improve provenance but require refreshes; recurring rules extrapolate efficiently but can omit future exceptions. Universal cross-market holiday defaults and session-aware opening schedules are not implemented.

## Evidence

[Jurisdiction code](../../../../finstack-quant/core/src/dates/calendar/jurisdictions.rs), [published annual-calendar tests](../../../../finstack-quant/core/tests/dates/calendars.rs), [published weekday closures](../../../../finstack-quant/core/tests/dates/fixtures/published_weekday_closures.csv), and [Japan source fixture](../../../../finstack-quant/core/tests/dates/fixtures/japan_public_holidays.csv). The inspected test `calendars_match_complete_published_years_including_open_days` checks every weekday in listed years, including unlisted open days; `japan_public_holidays_match_cabinet_office_1970_through_2027` compares a stored Cabinet Office snapshot. The fixture identifies source URLs and access date; this audit did not refresh them. [Fed K.8](../../../REFERENCES.md#fed-k8-holidays) is the central banking-calendar reference.

Calendar-specific provenance references live mainly in data/fixture comments; central reference coverage is incomplete.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
