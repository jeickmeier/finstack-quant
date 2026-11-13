# CORE-CONVENTIONS-007: Calendar tenor arithmetic and approximate frequency conversions

- Decision status: **Accepted — complete tenor/date-arithmetic package, 2026-11-07**.
- Implementation status: **Partly reconciled 2026-11-09** (#2 limits, #6 tenor bridge, #7 approximations, #8 nominal ratio, #10 epoch inverse, #11 WASM domain; #4 completed months and #5 roll rule on 2026-11-09 for core and hosts); #9 removals and the structured-credit month consumers remain, scoped to `dates/{date_extensions,tenor,mod}.rs`.
- Validation status: **Evidence inspected/located; independent qualification pending**. Documentation and independent calendar/rational-arithmetic examples passed; production Rust/host tests were not executed for this review.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

The API separates calendar arithmetic from approximate tenor-to-time conversions. The observed positive tenor has integer count and D/W/M/Y unit, capped at 200 years, 2,400 months, 73,200 days or floor(73,200/7) weeks. Those limits and the source's conversion/default choices are not independently adopted by their presence in the API. The accepted package below distinguishes date arithmetic, approximate time, nominal payment ratios and contractual compounding.

## Accepted decision: tenor and date-arithmetic conventions

Accepted on **2026-11-07** after whole-record review. This complete package covers tenor identity and
admission, exact date arithmetic, completed months, rolling/contextual clocks,
approximations, frequency boundaries and epoch conversion. Runtime reconciliation
and independent qualification remain open. The observed implementation remains separate below.

1. **Keep a positive integer calendar tenor and preserve its unit.**
   Retain one count plus `D`, `W`, `M` or `Y`; this is neither a business-day
   count nor an elapsed year fraction. Preserve count/unit identity in
   equality, hashing, display and wire state: `12M` and `1Y`, or `7D` and
   `1W`, may reach the same raw date without becoming structural aliases.
   [Strata's tenor model](https://strata.opengamma.io/apidocs/com/opengamma/strata/basics/date/Tenor.html)
   likewise keeps `12M` distinct from `1Y`; preserving W/D identity is this
   project's separate choice.
   Parse positive ASCII digits plus one unit, allowing outer whitespace and
   ASCII-case normalization. Canonical display has the numeric count and
   uppercase unit. Reject zero, negative/fractional counts, internal
   whitespace, unknown fields/units and money-market aliases rather than
   guessing spot/overnight settlement rules. Constructors, parsing,
   deserialization and both hosts must share admission.

2. **Replace the arbitrary 200-year cap with representation limits.**
   Retain the current count and exact conversion widths, with these bounds:

   | Unit | Admitted positive count | Exact conversion preserved |
   | --- | --- | --- |
   | D | `1..=u32::MAX` | Days fit `u32`. |
   | W | `1..=floor(u32::MAX/7)` | Seven times the count fits `u32` days. |
   | M | `1..=i32::MAX` | The month count fits the signed month-offset API. |
   | Y | `1..=floor(i32::MAX/12)` | Twelve times the count fits the signed month-offset API. |

   These are technical representation limits, not financial maturity caps.
   Applying a tenor must additionally produce a valid date from the supplied
   origin. Keep `months()` exact for M/Y and absent for D/W; keep `days()`
   exact for D/W and absent for M/Y. Do not substitute approximate counts
   into those exact helpers. Use checked widened multiplication/conversion,
   never wrapping or saturating casts.

3. **Adopt direct calendar arithmetic with last-valid-day clamping.**
   D adds the stated calendar days; W adds exactly seven times the count.
   M adds to the original date's absolute year/month index and preserves
   its day unless that target month requires clamping to its last valid day.
   Y is direct addition of twelve months per year. Use Euclidean year/month
   arithmetic throughout the supported proleptic Gregorian `Date` domain,
   including year zero and negative years where represented. Signed/zero
   primitive `add_days` / `add_months` offsets remain distinct from a positive
   Tenor. `end_of_month` selects the actual final day of the current month.
   [Java's documented month/year addition](https://docs.oracle.com/en/java/javase/25/docs/api/java.base/java/time/LocalDate.html#plusMonths(long))
   uses the same target-date clamping rule. A primitive addition does not
   retain an EOM anchor: repeated additions can drift and need not invert
   exactly after clamping. Anchored EOM/stub schedule construction remains
   with [008](008-schedule-generation.md).

4. **Define completed months by the same original-anchor arithmetic.**
   For `end >= start`, `months_until` returns the greatest integer `k >= 0`
   for which direct `start.add_months(k) <= end`. Evaluate every candidate
   against the original start, not an iteratively clamped chain. Equal dates
   return zero; reversed dates return an explicit input error rather than
   the observed zero clamp. Expose a checked `Result<u32>` through the existing
   error contract. This selected project measure changes January 30 to
   February 28, 2025 from zero to one completed month. It does not define
   loan seasoning, payment counts or contractual coupon periods; consumers
   needing those meanings must use their own actual schedules.

5. **Resolve tenor rolling explicitly.** Require a supplied business-day
   convention. `Tenor::add_to_date` first computes the raw target, then
   applies the [accepted 006 roll](006-business-day-adjustments.md#accepted-decision-rolling-and-joint-calendar-conventions)
   once. An omitted calendar is permitted only with explicit `Unadjusted`; a missing
   calendar with another convention is an input error, even if the eventual
   date happens to be a weekday. Remove host ModifiedFollowing defaults and
   the silent ignored-convention branch. A supplied calendar identifier must
   still be valid for Unadjusted, without unnecessary holiday queries.
   Neither weekday nor business-day stepping is inferred from a D/W tenor.

6. **Keep actual year fractions date- and context-based.** Apply the
   [accepted 002 tenor bridge](002-coupon-context-day-counts.md#accepted-decision-coupon-context-day-counts):
   `to_years_with_context` receives the caller's actual `DayCountContext`
   and selected convention. The endpoint-adjustment calendar and accrual
   context retain separate roles. Do not invent coupon frequency or periods
   from the tenor. Propagate required context/calendar failures; an adjusted
   end before the origin errors, and equality returns zero only after the
   selected day-count context is valid. ICMA and multi-coupon composition
   retain 002's explicit reference-period boundary.

7. **Name and define approximations explicitly.** Rename the context-free
   `to_years` to `to_years_approx`, with no compatibility alias. Retain this
   single declared approximation rather than choosing a basis from DayCount:

   | Tenor count `n` | Approximate years | Approximate whole calendar days |
   | --- | --- | --- |
   | D | `n/365` | `n` |
   | W | `7*n/365` | `7*n` |
   | M | `n/12` | `floor((365*n+6)/12)` |
   | Y | `n` | `365*n` |

   Retain `to_days_approx`; round positive half-day ties upward using exact
   integer/rational semantics, not an incidental floating-point evaluation
   order. In particular, `6M` gives 183 approximate days. The 365 basis is
   this approximation's convention, not a universal year length or automatic
   CAGR, coupon, volatility or analytics clock. These methods do not recover
   actual dated accrual or guarantee the same endpoint as calendar addition.
   Financial uses must explicitly own and justify the approximation; do not
   silently feed it back into contractual date generation.

8. **Keep nominal monthly/yearly payment ratios separate from day/week
   conventions and actual payment counts.** Retain `payments_per_year`
   as a checked nominal ratio: M gives `12/n` and Y gives `1/n`, including
   fractional values such as `5M -> 2.4` and `2Y -> 0.5`. Reject D/W for
   this operation rather than inferring 365, 252 or 52 observations/payments
   per year. An explicit product convention or actual schedule owns those
   choices. The result is not the number of payments observed in a calendar
   year. Do not impose ICMA's separate frequency domain on all Tenor values.
   Keep `from_payments_per_year(u32)` as the deliberately limited exact
   month constructor for `{1,2,3,4,6,12}`, returning `(12/payments)M`;
   reject zero/other integers. It is not an inverse for arbitrary fractional
   ratios or a way to recover a lost original unit (`1 -> 12M`, not `1Y`).

9. **Remove ambiguous inverse and compounding inference.** Remove
   `Tenor::from_years(years, DayCount)` and its host exports. A scalar
   fraction cannot reconstruct the original calendar tenor, and a Bus252
   basis cannot manufacture a calendar-day tenor. Do not replace it with
   another snapping tolerance, approximate inverse or automatic day-count
   conversion. Callers must retain or explicitly supply actual contractual
   tenor/date metadata. Remove rounded/floored `compounding_frequency`;
   payment cadence and quote compounding are separate inputs. Use the
   existing [Compounding representation](../../../../finstack-quant/core/src/math/compounding.rs)
   with an explicitly selected owning financial convention. A nominal ratio
   must not silently be rounded, floored or cast into a quote frequency.

10. **Keep epoch days exact, signed and free of time-zone semantics.**
    Preserve signed integer calendar-day numbering relative to civil
    `1970-01-01 = 0`, using the existing Julian-day offset 2,440,588.
    `days_since_epoch` and `date_from_epoch_days` must round-trip every
    supported Date. Change the inverse to the existing checked `Result<Date>`
    error contract: out-of-Date-range/overflow is an error, not `None` or a
    fallback date. This is not an instant, UTC midnight, seconds, DST or
    a historical local-calendar conversion. Date construction/parsing
    rejects impossible dates; calendar reform rules are not inferred.

11. **Preserve numeric and date domains across hosts.** Valid approximate
    day outputs fit `i64` and exact JavaScript numbers under the selected
    count bounds. Remove the WASM-only `i32` narrowing; no BigInt or alternate
    wire carrier is required for this bounded result. Preserve exact integer
    days/months, units, parsing, epoch errors and selected financial-domain
    errors across Rust/Python/WASM. Native-host date limitations must produce
    explicit conversion errors rather than clipping dates or pretending
    all native date types have Rust's domain. All financial calculations
    remain in Rust; bindings perform conversions and error mapping.

12. **Reconcile affected financial callers without inventing replacements.**
    Callers currently infer periods, ASW frequency or structured-credit/scenario
    dates from floating years, and some pricing paths round/floor inferred
    frequencies. Reconcile them against actual contract metadata and their
    own methodology records. This package does not adopt those pricing,
    seasoning, fiscal/LTM or statement formulas. Update canonical signatures,
    schemas/state validation, host declarations, parity ledger and examples
    together; remove obsolete inference paths rather than compatibility
    wrappers. The [existing follow-ups](../FOLLOW_UPS.md#core-conventions)
    own implementation and independent numerical/date qualification.

## Boundary examples for the accepted package

These are independent calendar/rational-arithmetic examples and accepted
request boundaries, not executed production tests or pricing qualification.

| Request | Required result |
| --- | --- |
| `2025-01-31 + 1M`; then another `1M` | February 28; then March 28. |
| `2025-01-31 + 2M` directly | March 31: the original anchor is retained for this single addition. |
| `2025-02-28 + 1M` | March 28, without inferred EOM anchoring. |
| `2024-02-29 + 1Y` | February 28, 2025. |
| `months_until(2025-01-30,2025-02-28)` | One completed clamped anniversary; observed implementation gives zero. |
| January 31 to February 27 / March 30 / March 31, 2025 | Completed-month counts zero / one / two. |
| Reversed `months_until`; equal dates | Input error; zero, respectively. |
| Raw `1D` from Friday January 3, 2025 | Saturday January 4; explicit Following with a supplied weekends-only calendar rolls it to Monday January 6. |
| Non-Unadjusted rule with no roll calendar | Input error, rather than silently keeping the raw target. |
| Approximate `1W` / `6M` / `1Y` | Years `7/365` / `1/2` / `1`; whole days `7` / `183` / `365`. |
| Actual Act365F over `2024-01-01 → 2025-01-01` | `366/365`, while the context-free `1Y` approximation is `1`. |
| Nominal payments/year for `5M` / `2Y`; request for `1W` | `2.4` / `0.5`; unsupported-unit error. |
| `from_payments_per_year(1)` / `(4)` / `(5)` | `12M` / `3M` / input error. |
| `201Y` construction; application beyond Date::MAX | The tenor is admitted within representation limits; an unrepresentable target date errors. |
| Epoch dates `1969-12-31`, `1970-01-01`, `2000-01-01` | Signed day numbers `-1`, `0`, `10957`. |
| Invalid epoch inverse or count beyond its unit's technical limit | Explicit conversion/input error, without saturation or an unavailable-result sentinel. |

## Observed algorithm and selection

`add_months` converts `(year,month)` to an absolute month index using Euclidean division, then clamps day to the target month's last valid day. It does not retain a permanent EOM anchor across repeated calls. `add_days` adds checked Julian days. `end_of_month` selects the actual month length. `months_until` computes month difference, subtracts one when start day exceeds end day except when both dates are month-end, then clamps negative counts to zero.

`Tenor::add_to_date` adds calendar days/weeks or clamped months/years, then applies the requested business-day convention only if a calendar is supplied. An absent calendar leaves the raw date even with a non-Unadjusted convention.

`to_years`: D/365, 7W/365, M/12, Y. `payments_per_year`: 365/D, 52/W, 12/M, 1/Y. `compounding_frequency` is removed (2026-11-12): Bond, TermLoan, RevolvingCredit and ABF each carry a required quote `Compounding` (O6). `to_days_approx`: round(D,7W,365M/12,365Y), using `CALENDAR_DAYS_PER_YEAR=365`.

`from_years` requires finite positive input. If `12*years` lies within `1e-4` of an integer, use months and choose years when divisible by 12. Otherwise round to days using 360 for the 30/360 family/ACT360, 365 for ACT365F/NL365/ACT365L, 252 for Bus252 and 365.25 for the remaining conventions. The resulting D tenor remains **calendar** days when added, even if inferred using a business-day basis. `from_payments_per_year` accepts only positive integer divisors of 12. Parsing accepts positive ASCII-digit counts followed by a single unit, ignores outer whitespace/case, and rejects money-market aliases.

`to_years_with_context` uses an actual end date and selected day count, with frequency equal to the tenor; the coupon-reference treatment is in [002](002-coupon-context-day-counts.md). Dates use `time::Date`; epoch conversion adds/subtracts Julian day 2,440,588 for 1970-01-01 with overflow returning `None` on inverse conversion.

## Observed conventions, limitations and failure

Approximations are not general inverses: 52 weeks in `payments_per_year` differs from 365/7 in `to_years`; Bus252-derived D tenors do not carry a holiday calendar; a 2Y compounding frequency is 1 while payments/year is 0.5. Small year fractions can round to a zero count and error. Date overflow errors; `months_until` loses direction by design. Actual fiscal-year labeling is covered in 014.

## Alternatives and tradeoffs

Actual date/day-count conversion and approximate conversion are both supported. Anchored schedule generation (008) avoids the drift inherent in repeatedly adding clamped months. Retaining a date origin and calendar for every tenor conversion would be more specific but would change this context-free abstraction.

## Evidence

[Tenor](../../../../finstack-quant/core/src/dates/tenor.rs), [date extension](../../../../finstack-quant/core/src/dates/date_extensions.rs), [date constants/epoch](../../../../finstack-quant/core/src/dates/mod.rs). Inline tests `test_from_years_prefers_months_and_handles_invalid_values`, `test_to_days_approx`, `test_add_to_date_end_of_month`, `test_from_payments_per_year_validation` and `test_months_until` provide boundary/formula examples. [ISO 8601](../../../REFERENCES.md#iso-8601) supports date representation, not the chosen financial approximation factors.

No accuracy bound for these approximations is established in the inspected evidence.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
