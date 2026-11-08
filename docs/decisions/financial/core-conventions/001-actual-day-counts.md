# CORE-CONVENTIONS-001: Actual-day accrual, year partitioning and business-day bases

- Decision status: **Accepted — complete actual-day package, 2026-11-07**.
- Implementation status: **Observed; reconciliation pending**, scoped to `dates/daycount/{mod,act_act,other}.rs` and the context contract.
- Validation status: **Evidence inspected/located; independent qualification pending**. Documentation and independent formula-example checks passed; production tests and vendor fixtures were not executed or regenerated.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

`DayCount` explicitly selects each accrual convention; there is no implicit default convention. These kernels support interest accrual and curve-time conversion, rather than instrument-specific eligibility. ISDA and ICMA references are named in the source; adoption of a convention for a particular contract is not established by the enum.

## Accepted decision: actual-day conventions

Accepted on **2026-11-07** after whole-record review. This package covers the
seven variants below and their common input/result boundary. Acceptance
records the methodology; runtime reconciliation and independent qualification
remain open. The observed implementation is recorded separately below.

1. **Select the convention explicitly.** Keep the canonical enum and wire
   names, including the existing human-input case/outer-whitespace
   normalization. Do not infer a convention from currency, instrument type,
   observation frequency or another clock. Instrument records own their
   contractual selection. Calculate from the dates supplied; business-day
   adjustment is a separate operation, not an implicit accrual step.

2. **Adopt these formulas for a forward interval.** Let `D` be the exact
   integer number of elapsed calendar days, `end - start`.

   | Variant | Accepted definition |
   | --- | --- |
   | `Act360` | `D / 360`. Leap days remain in `D`. |
   | `Act365F` | `D / 365`, including in leap years. |
   | `OneOne` | One for every nonempty forward interval. This is a contractual period unit, not elapsed calendar years or prorated accrual. |
   | `Nl365` | `(D - L) / 365`, where `L` counts February 29 in `(start,end]`. |
   | `ActAct` (ISDA) | Partition `[start,end)` at calendar-year boundaries; divide each segment's actual days by that year's 365 or 366 and sum. A complete calendar year contributes one. |
   | `ActActAfb` | Use the explicit QuantLib-style backward-year algorithm below. |
   | `Bus252` | `B / basis`, where `B` counts supplied-calendar business days in `[start,end)` and `basis` is 252 unless explicitly overridden by a positive `u16`. |

   Fixed denominators can produce fractions greater than one. `OneOne` is
   nonadditive when an interval is split into multiple contractual periods.
   NL/365 deliberately uses a different leap-day endpoint rule from ISDA;
   do not homogenize them. The fixed-day and ISDA formulas follow
   [2006 ISDA Definitions, sections 4.13 and 4.16(a), (b), (d), (e)](https://www.sc.com/en/uploads/sites/66/content/docs/2006-ISDA-Definitions.pdf).
   Equal-date `OneOne = 0` is this API's boundary policy, rather than a claim
   that the contractual 1/1 definition itself prorates empty periods.
   [Strata's NL/365 definition](https://strata.opengamma.io/day_counts/#nl365)
   confirms the selected leap-day endpoints.

3. **Name the AFB interpretation precisely.** Starting at `end`, step
   backward by 12 months with calendar-day clamping. If the candidate is
   February 28 in a leap year, move it to February 29. Accept each candidate
   on or after `start` as one whole year and continue from it. The remaining
   fraction is actual days in `[start,residual_end)` divided by 366 if that
   interval contains February 29, otherwise 365. This selects the
   [QuantLib AFB algorithm](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/time/daycounters/actualactual.cpp)
   for the existing `ActActAfb` name. It does not claim universal AFB vendor
   parity: [Strata documents differing AFB interpretations](https://strata.opengamma.io/apidocs/com/opengamma/strata/basics/date/DayCounts.html#ACT_ACT_AFB).
   In particular, `2004-02-28 → 2005-02-28` gives `1 + 1/365`, and
   `2004-02-28 → 2008-02-28` gives `4 + 1/365`. Both leave a one-day
   residual ending on February 29; that excluded endpoint does not select
   366. Do not impose universal interval additivity or anniversary-equals-one
   rules on this variant.

4. **Make Bus/252 context and count meaning explicit.** A calendar is
   required, including for equal dates. A weekends-only calendar is valid
   when supplied deliberately; absence of a calendar is an input error.
   A non-252 override is an explicitly generalized business-day basis,
   not the standard 252 denominator. Do not infer it from holidays in a
   particular year or from analytics observation frequency. Require the
   integer count to satisfy `0 <= B <= D`; invalid custom-calendar counts
   are calculation errors. Cached and scanned counting must agree exactly.
   An interval containing no business days legitimately has zero accrual.
   Calendar coverage, projection and refresh policies remain owned by
   [004](004-holiday-engine.md) and [005](005-jurisdiction-calendars.md);
   successful counting does not certify complete holiday data.

5. **Validate context before equal-date shortcuts.** All supplied context
   must be structurally valid: reject zero business basis, empty/inverted
   supplied coupon boundaries and unresolved registered calendar IDs.
   Apply the same boundary to constructors, state restoration and direct
   runtime contexts; public-field construction must not bypass validation.
   Require missing fields only where the selected convention needs them;
   context-free variants need no calendar or coupon metadata. Valid but
   irrelevant fields may remain present. Validate selected-convention
   requirements before returning equal-date zero. Coupon-specific frequency,
   reference-grid and enclosing-period rules belong to
   [002](002-coupon-context-day-counts.md), not this package.

6. **Keep unsigned and signed results deliberate.** With valid context,
   equal dates return zero for all seven variants. The unsigned API rejects
   reversed dates; the signed API calculates the chronological forward span
   with the same applicable context and negates it. This establishes signed
   antisymmetry for these seven variants, without adopting termination-flag
   or coupon-metadata reversal rules for other variants. Those remain with
   [002](002-coupon-context-day-counts.md) and [003](003-thirty360.md).
   Distinct dates may legitimately produce zero under NL/365 or Bus/252;
   consumers requiring a strictly positive clock enforce their own
   [accepted clock eligibility](../conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility).

7. **Adopt financial formulas, not incidental rounding order.** Count days
   and complete years exactly in integers before conversion. Compute ISDA
   with an integer complete-year count and at most two partial-year
   fractions, avoiding a year-by-year floating-point accumulation contract.
   Use the supported `Date` domain without an arbitrary 50-year cap;
   qualify arithmetic near its boundaries rather than claiming accuracy
   from a source comment. An AFB backward probe may stop below `Date::MIN`
   when it is provably before any valid `start`; failure to construct a
   boundary actually needed by the calculation is an error. Forward
   results must be finite and nonnegative; signed reverse results finite
   and nonpositive. Invalid inputs or failed calculations return explicit
   errors, not NaN, infinity, fabricated zero, clipping, a denominator floor
   or substitution of another convention. No bitwise agreement with a
   vendor or obsolete summation order is promised.

8. **Keep one canonical contract across hosts.** Reconcile the existing
   Rust API, `DayCountContextState`, Python/WASM wrappers, canonical wire
   forms and public documentation together. Retain the existing context
   representation rather than adding a competing day-count framework or
   compatibility aliases. Acceptance records methodology;
   runtime implementation and independent numerical qualification remain
   separately tracked in the [owning follow-ups](../FOLLOW_UPS.md#core-conventions).

## Boundary examples for the accepted package

These are independently calculated formula examples, not executed production
tests or regenerated vendor output. Each assumes valid applicable context.

| Dates / setup | Required result |
| --- | --- |
| `2024-02-28 → 2024-03-01` | Act360 `2/360`; Act365F `2/365`; ISDA `2/366`; NL/365 `1/365`; OneOne `1`. |
| `2024-01-01 → 2025-01-01` | Act360 `366/360`; Act365F `366/365`; ISDA `1`; NL/365 `1`. |
| `2023-12-31 → 2024-01-02` | ISDA `1/365 + 1/366`. |
| `2024-02-28 → 2024-02-29` | NL/365 `0`; reversing through the signed API also yields zero. |
| `2024-02-29 → 2024-03-01` | NL/365 `1/365`. |
| Explicit toy calendar with Saturday/Sunday closed, `2024-03-01 → 2024-03-04` | Bus252 `1/252`, because Friday is included and Monday excluded. |
| Same toy calendar, `2024-03-02 → 2024-03-04` | Bus252 `0`. |
| Equal dates with missing Bus252 calendar or invalid supplied context | Input error, rather than a zero shortcut. |

## Observed algorithm and conventions

- `Act360` and `Act365F`: actual elapsed calendar days `end - start`, divided by 360 or 365.
- `OneOne`: 1 for every nonempty forward interval regardless of its length, including intervals shorter or longer than a year. This is a contractual period unit, not a calendar-year approximation.
- `Nl365`: `(elapsed days - count of Feb 29 in (start,end]) / 365`; the end leap day is removed while a starting leap day is not.
- `ActAct` (ISDA): partition `[start,end)` at January 1, divide each partial year's actual days by that year's 365/366, and add exactly 1 sequentially for each full intervening year. The sequential accumulation is part of observed floating-point behavior.
- `ActActAfb`: step backwards by 12 months from the end, bump a step landing on February 28 in a leap year to February 29, and accept each step on/after start as one whole year. Divide the remaining actual days by 366 if February 29 lies in `[start,residual_end)`, otherwise 365.
- `Bus252`: count calendar-business days in `[start,end)` using the supplied `HolidayCalendar`, divided by `bus_basis.unwrap_or(252)`. Built-in calendars use the year-prefix-count cache; custom calendars default to a daily scan. This is not universally 252 calendar days.

`year_fraction` rejects reversed dates before dispatch, and returns zero for equal dates before validating convention-specific context. `signed_year_fraction` negates the forward calculation when reversed and clears the termination-date flag for that reversed calculation; other context fields remain unchanged. The 30/360 termination consequence is in [003](003-thirty360.md).

## Selection and controls

Callers choose the enum and `DayCountContext`; missing Bus/252 calendar and zero basis error. `DayCountContextState::try_new` rejects inverted/empty coupon boundaries and zero basis; `to_ctx` resolves only registered calendar IDs. Runtime contexts and public state fields can still be constructed directly. Calendar snapshots reject custom calendars without a restorable registry identifier. Parsing accepts canonical serde labels with case/outer-whitespace normalization, not retired aliases.

## Limitations, failure and fallback

There is no automatic substitution of one day count for another. AFB's backwards year stepping terminates if month arithmetic cannot represent an earlier date. Full-year accumulation and residual arithmetic use `f64`. Equal-date zero can bypass otherwise-required context. Public raw day-count helpers have different reversed-interval behavior; see 003. Calendar completeness is bounded by the underlying dataset, not by the successful return of Bus/252.

## Alternatives and tradeoffs

All named variants are supported alternatives selected explicitly. An elapsed-day fixed denominator avoids coupon metadata but is economically different from ISDA/AFB splitting. Calendar-year ISDA and end-anchored AFB can differ around February and anniversary dates. No signed-accrual policy that automatically swaps coupon metadata is implemented.

## Evidence

Owning code: [dispatch and signs](../../../../finstack-quant/core/src/dates/daycount/mod.rs), [ISDA/AFB](../../../../finstack-quant/core/src/dates/daycount/act_act.rs), [NL/365 and Bus/252](../../../../finstack-quant/core/src/dates/daycount/other.rs), [context validation](../../../../finstack-quant/core/src/dates/daycount/context.rs). Inline tests `nl365_excludes_end_leap_day_but_includes_start_leap_day` and `act_act_isda_per_year_loop_is_not_bulk_addable` cover endpoint and numerical-order choices. [Day-count tests](../../../../finstack-quant/core/tests/dates/daycount.rs) and [golden comparisons](../../../../finstack-quant/core/tests/golden/daycount_quantlib_tests.rs) provide formula and stored-reference evidence; golden provenance has the limitation stated in 003. [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions) is the canonical reference entry.

Accuracy across all representable centuries is not certified by the located examples.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
