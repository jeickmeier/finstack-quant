# CORE-CONVENTIONS-001: Actual-day accrual, year partitioning and business-day bases

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `dates/daycount/{mod,act_act,other}.rs` and the context contract.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

`DayCount` explicitly selects each accrual convention; there is no implicit default convention. These kernels support interest accrual and curve-time conversion, rather than instrument-specific eligibility. ISDA and ICMA references are named in the source; adoption of a convention for a particular contract is not established by the enum.

## Algorithm and conventions

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

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
