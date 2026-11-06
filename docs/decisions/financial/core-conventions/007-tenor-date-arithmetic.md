# CORE-CONVENTIONS-007: Calendar tenor arithmetic and approximate frequency conversions

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `dates/{date_extensions,tenor,mod}.rs`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

The API separates calendar arithmetic from approximate tenor-to-time conversions. A positive tenor has integer count and D/W/M/Y unit; count is limited to 200 years, 2,400 months, 73,200 days or floor(73,200/7) weeks. The cap bounds computation and representable financial horizons; contractual adoption of these limits is not documented.

## Algorithm and selection

`add_months` converts `(year,month)` to an absolute month index using Euclidean division, then clamps day to the target month's last valid day. It does not retain a permanent EOM anchor across repeated calls. `add_days` adds checked Julian days. `end_of_month` selects the actual month length. `months_until` computes month difference, subtracts one when start day exceeds end day except when both dates are month-end, then clamps negative counts to zero.

`Tenor::add_to_date` adds calendar days/weeks or clamped months/years, then applies the requested business-day convention only if a calendar is supplied. An absent calendar leaves the raw date even with a non-Unadjusted convention.

`to_years`: D/365, 7W/365, M/12, Y. `payments_per_year`: 365/D, 52/W, 12/M, 1/Y. `compounding_frequency`: round(1/to_years), floored at 1; this differs deliberately from fractional payments/year, especially for weekly and multi-year tenors. `to_days_approx`: round(D,7W,365M/12,365Y), using `CALENDAR_DAYS_PER_YEAR=365`.

`from_years` requires finite positive input. If `12*years` lies within `1e-4` of an integer, use months and choose years when divisible by 12. Otherwise round to days using 360 for the 30/360 family/ACT360, 365 for ACT365F/NL365/ACT365L, 252 for Bus252 and 365.25 for the remaining conventions. The resulting D tenor remains **calendar** days when added, even if inferred using a business-day basis. `from_payments_per_year` accepts only positive integer divisors of 12. Parsing accepts positive ASCII-digit counts followed by a single unit, ignores outer whitespace/case, and rejects money-market aliases.

`to_years_with_context` uses an actual end date and selected day count, with frequency equal to the tenor; the coupon-reference treatment is in [002](002-coupon-context-day-counts.md). Dates use `time::Date`; epoch conversion adds/subtracts Julian day 2,440,588 for 1970-01-01 with overflow returning `None` on inverse conversion.

## Conventions, limitations and failure

Approximations are not general inverses: 52 weeks in `payments_per_year` differs from 365/7 in `to_years`; Bus252-derived D tenors do not carry a holiday calendar; a 2Y compounding frequency is 1 while payments/year is 0.5. Small year fractions can round to a zero count and error. Date overflow errors; `months_until` loses direction by design. Actual fiscal-year labeling is covered in 014.

## Alternatives and tradeoffs

Actual date/day-count conversion and approximate conversion are both supported. Anchored schedule generation (008) avoids the drift inherent in repeatedly adding clamped months. Retaining a date origin and calendar for every tenor conversion would be more specific but would change this context-free abstraction.

## Evidence

[Tenor](../../../../finstack-quant/core/src/dates/tenor.rs), [date extension](../../../../finstack-quant/core/src/dates/date_extensions.rs), [date constants/epoch](../../../../finstack-quant/core/src/dates/mod.rs). Inline tests `test_from_years_prefers_months_and_handles_invalid_values`, `test_to_days_approx`, `test_add_to_date_end_of_month`, `test_from_payments_per_year_validation` and `test_months_until` provide boundary/formula examples. [ISO 8601](../../../REFERENCES.md#iso-8601) supports date representation, not the chosen financial approximation factors.

No accuracy bound for these approximations is established in the inspected evidence.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
