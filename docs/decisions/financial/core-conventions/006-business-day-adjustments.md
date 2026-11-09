# CORE-CONVENTIONS-006: Business-day rolling, joint calendars and bounded stepping

- Decision status: **Accepted — complete rolling/joint-calendar package, 2026-11-07**.
- Implementation status: **Partly reconciled 2026-11-08**: no default convention (enum, cashflow/statements wire forms, host tenor rolls), strict joint-ID parsing, year+month Modified comparison and uncapped minimal searches (2026-11-09) landed; resolver fallbacks and composite coverage remain, scoped to `dates/calendar/{business_days,composite}.rs` and business-day methods in `date_extensions.rs`.
- Validation status: **Evidence inspected/located; independent qualification pending**. Documentation and independent toy-calendar/date checks passed; production calendar tests were not executed for this review.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

Settlement and payment adjustment uses a named convention over one calendar or the intersection of member business days. The observed `BusinessDayConvention::default()` is ModifiedFollowing; schedules do not automatically invoke it because their convention field initially is `None`. The accepted package selects explicit conventions, calendar identity, search/error boundaries and joint-calendar semantics together; observed defaults do not establish contractual selection.

## Accepted decision: rolling and joint-calendar conventions

Accepted on **2026-11-07** after whole-record review. This is the complete
rolling, stepping, counting, joint-calendar and resolution package. Runtime
reconciliation and independent qualification remain open.
Inherit the accepted [004 engine/data boundary](004-holiday-engine.md#accepted-decision-holiday-engine-and-data-boundaries)
and [005 purpose/coverage matrix](005-jurisdiction-calendars.md#accepted-decision-jurisdiction-calendar-purposes-and-coverage).
The observed source behavior remains separate below.

1. **Select the financial calendar and roll convention explicitly.**
   Remove the universal `BusinessDayConvention::default()` path; the common
   rolling API must not infer ModifiedFollowing or any other convention.
   Require an explicit calendar identity/provider, including an explicit
   `weekends_only` choice when that is the intended model. Use the existing
   strict registry resolver for standalone and composite IDs; remove the
   redundant public `resolve_calendar(Option<_>)` and
   `resolve_joint_calendar(Option<_>, Option<_>)` fallback wrappers rather
   than preserving `None` as weekends-only or silently dropping a required
   market leg. The caller specifies every contractually required member;
   a deliberately single-market contract may select that member alone.
   Product, tenor and schedule omission/default policies retain the owners
   in [007](007-tenor-date-arithmetic.md), [008](008-schedule-generation.md)
   and their instrument records. They must resolve their own accepted policy
   explicitly before invoking the common engine; this package does not adopt
   every observed host/template default.

2. **Adopt all six named adjustment definitions.** An already-business
   date remains unchanged for the five business-day-seeking conventions.
   `Unadjusted` is the identity whether or not the date is business.

   | Convention | Selected definition for a closed date |
   | --- | --- |
   | `Unadjusted` | Return the supplied date. |
   | `Following` | First business date after it. |
   | `Preceding` | First business date before it. |
   | `ModifiedFollowing` | Following unless it leaves the original `(year, month)`; then Preceding from the original date. |
   | `ModifiedPreceding` | Preceding unless it leaves the original `(year, month)`; then Following from the original date. |
   | `Nearest` | Minimum absolute calendar-day distance; choose Following on equal distance. |

   Following, ModifiedFollowing and Preceding are defined in
   [2006 ISDA section 4.12(a)(i–iii)](https://www.sc.com/en/uploads/sites/66/content/docs/2006-ISDA-Definitions.pdf).
   Unadjusted and ModifiedPreceding are separately adopted explicit variants,
   also described by [Strata](https://strata.opengamma.io/apidocs/com/opengamma/strata/basics/date/BusinessDayConventions.html).
   Correct the source's section numbering and claims that ISDA defines every
   enum variant in that section.

3. **Make Nearest's external meaning precise.** Retain the true-distance,
   following-on-ties algorithm, matching
   [QuantLib's calendar adjustment](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/time/calendar.cpp).
   Remove the unsupported FpML-equivalence claim: official
   [FpML 5.9 NEAREST](https://www.fpml.org/spec/fpml-5-9-3-wd-3/html/reporting/schemaDocumentation/schemas/fpml-shared-5-9_xsd/complexTypes/BusinessDateRange/businessDayConvention.html)
   and Strata select direction by weekday, which can give a different answer.
   Keep the canonical local name/wire label; an external mapping must respect
   the actual formula and source version rather than reinterpret an identical-looking name.
   Do not add another unrequested nearest variant or compatibility alias.

4. **Use checked, necessary searches without an arbitrary closed-day cap.**
   Remove the 100-day guard and its inconsistent effect on repeated stepping.
   Progress monotonically through representable dates using the actual
   selected calendar; terminate on a proven answer or a genuine required
   coverage/date boundary. A long supported closure does not itself justify
   failure. Calendar/data/calculation errors propagate; they are not closed
   days, missing search directions or reasons to switch calendars.
   Modified conventions may reverse once all candidate dates within the
   original year/month are proved closed; they need not classify an
   irrelevant next-month date. Compare year and month, not month number
   alone, and reverse from the original date. A completely closed original
   month can make the final result fall in a neighboring month; there is no
   unconditional same-month guarantee. For Nearest, search only as far as
   needed to prove minimal distance and the tie rule. A one-sided result is
   allowed only when the other side is provably unable to improve it, such
   as representable-date exhaustion; unavailable needed data are errors.
   Do not query farther irrelevant dates after the answer is already proved.

5. **Keep deliberate identities separate from business-day assertions.**
   Validate supplied identifiers and request structures even for
   `Unadjusted` and zero shifts. These operations then return the input date
   without unnecessary holiday classifications or a claim that it is open.
   They do not require unused holiday data. This identity policy does not
   amend 004's calendar-query/count-domain validation or the accepted
   day-count context validation in 001–003. A zero settlement lag's actual
   eligibility or subsequent rolling is owned by that product's workflow.

6. **Adopt explicit signed-offset semantics.** `add_business_days(n)`
   accepts a signed `i32` count, excludes the start and counts reached
   business dates in the sign's direction. A closed start is allowed and is
   not pre-rolled; zero returns it unchanged. Only the classifications of
   actually needed candidate dates are required. `add_weekdays(n)` follows
   the same start/zero/direction rule with Saturday/Sunday excluded and all
   holidays deliberately ignored; it is not a market settlement calendar.
   Preserve exact integer counting and checked date arithmetic, handling
   `i32::MIN` without signed absolute-value/negation overflow. Week-block
   arithmetic may optimize the weekday helper only when exactly equivalent
   to direct counting. A required result outside the supported date/domain
   errors, with no saturation, fabricated zero or partial shift. Shifting
   back from a closed origin need not reconstruct that origin.

7. **Retain 004's exact unsigned count boundary.** Count business dates
   once in `[start,end)`, with `0 <= count <= elapsed calendar days`.
   Equal dates yield zero after calendar/requested-domain validation;
   reversed unsigned ranges error. Built-in, custom and composite providers
   obey the same contract and propagate failures. Cached/scan counts must
   agree exactly; consumers validate supplied custom counts before financial
   use. Counting and stepping intentionally have different endpoint rules:
   a Saturday plus one business day can be Monday while the count from
   Saturday up to that Monday is zero.

8. **Adopt intersection of business days for required joint markets.**
   A joint business date requires every member to be open; joint holiday
   labels are the union of member holiday labels. Use each member's actual
   business predicate, including its weekend rule, rather than negating
   `is_holiday`. Validate all required member domains/classifications before
   Boolean reduction so a known closure cannot hide another required member's
   error. Joint coverage is the intersection of member supported domains,
   including gaps; retain every member's purpose, provenance and forecast
   status. A joint result is not fully published if a required member is
   projected, and forecast eligibility must hold for every required member.
   Apply adjustment once to this joint predicate, never sequentially to
   individually adjusted dates. Count each joint date once, not the sum of
   member counts. Do not turn a labels-only or otherwise ineligible calendar
   into a settlement calendar through composition. No union-of-business-days
   financial variant is adopted.

9. **Parse and persist joint identity strictly.** A composite must have
   at least one member. Reject leading, trailing, doubled or whitespace-only
   empty tokens, such as `+a`, `a+`, `a++b` and `a+ +b`; do not silently drop
   them. Trim and ASCII-case-normalize composite members, sort/deduplicate
   canonical IDs and collapse one distinct member to that member. Preserve
   004's standalone-ID rule without implicit whitespace trimming. Flatten
   nested joint identities consistently, reject unknown/retired members and
   reject an empty internal composite instead of treating it as weekends-only.
   Keep the existing registry/interner and canonical-ID representation,
   without a second registry or resolver family. Existing day-count contexts
   may round-trip a registered joint ID only when each member is restorable
   under the same frozen registry. The ID alone is not a data-version pin:
   reproducible persistence must record/check the actual member editions in
   the owning calculation state. Custom providers retain 001's explicit
   runtime/restoration boundary; no invented registered identity is allowed.

10. **Reconcile errors, source claims and host contracts together.**
    Remove obsolete guard/error-payload assumptions, generic defaults,
    optional fallback wrappers and permissive empty-member parsing. Preserve
    distinctions among invalid requests, unknown calendars, unavailable data,
    required date exhaustion and failed calculations. Do not downgrade errors
    to unchanged dates or use one calendar/search side as repair for another's
    missing data. Reconcile Rust, Python/WASM, wire/schema forms, metadata,
    parity entries and callers together; operation-owned defaults retain
    their own adoption status. The [existing follow-ups](../FOLLOW_UPS.md#core-conventions)
    own implementation and independent qualification. Methodology acceptance
    does not execute those changes or qualify settlement eligibility.

## Boundary examples for the accepted package

These are independently calculated toy-calendar examples, not executed
production tests or certification of any jurisdiction calendar. Unless stated
otherwise, the toy calendar closes Saturday/Sunday and only the named dates.

| Request / toy setup | Required outcome |
| --- | --- |
| Saturday `2025-01-04`, Following / Preceding | Monday January 6 / Friday January 3. |
| Saturday `2025-05-31`, ModifiedFollowing | Friday May 30, rather than Monday June 2. |
| Sunday `2025-06-01`, ModifiedPreceding | Monday June 2, rather than Friday May 30. |
| Closed Wednesday `2025-01-08`, adjacent Tuesday/Thursday open | Nearest returns Thursday January 9 on the tie; the FpML weekday rule would select Tuesday. |
| Every date in June 2025 closed; ModifiedFollowing from June 15 | Friday May 30: the original month cannot contain a valid result. |
| Calendar A closes January 29, 2025; B closes January 30/31 | Joint ModifiedFollowing from January 29 returns January 28. Sequential rolls can incorrectly end on January 29, closed in A. |
| Business-day shift from Saturday January 4, 2025 | `+1 → January 6`; `-1 → January 3`; `0 → January 4`. |
| Same toy calendar, count `[2025-01-04,2025-01-06)` | Zero; shifting and half-open counting have distinct endpoints. |
| Toy calendar closed from `2025-01-01` until a known open `2025-05-01` | Following reaches May 1 after 120 days; no artificial 100-day failure. |
| Closed input, following date open at distance one; preceding date at distance one unavailable | Nearest may return following: the unknown side cannot beat the preferred tie. |
| Closed input, preceding date open at distance one; following date at distance one unavailable | Error: the unavailable following date could win the tie. |
| A required joint member is closed and another member has unavailable needed data | Error, independent of member ordering. |
| `GBLO + nyse+NYSE`; `nyse+`; omitted financial calendar | Canonical `gblo+nyse`; invalid empty-member expression; missing-input error, respectively. |
| Unadjusted or zero shift, valid identity/request but unused holiday data unavailable | Input date unchanged without asserting openness. Equal-date business-day counts retain 004's domain-validation requirement. |

## Observed algorithm and conventions

If date is already business, return unchanged. Unadjusted returns it even when closed. Following/Preceding seek forward/backward one calendar day at a time. ModifiedFollowing reverses to Preceding if the forward result changes month; ModifiedPreceding is symmetric. Nearest runs both searches, picks the smallest calendar-day distance and chooses following on ties; if only one direction is representable it uses that one.

The search starts on the supplied date and fails when more than 100 steps have been attempted; it never accepts a business day reached after that limit. Checked date addition prevents date-range overflow. `add_business_days(n)` excludes the starting day, repeats directional seeking for each requested day, and returns the input unchanged for zero; it prechecks that at least `n` calendar days fit. `add_weekdays` handles an initial weekend directionally, then uses whole five-day/seven-day week blocks plus remainder; it excludes holidays by design.

`count_business_days` uses `[start,end)` and returns zero for reversed/equal dates rather than a signed count. Built-ins use cached prefix sums; custom/composite calendars scan.

A joint calendar marks a holiday if any member does, and a business day only if every member does. An empty internal composite behaves weekends-only. Registry expressions `a+b` lowercase/trim, drop empty members, sort and deduplicate, then intern the union of closures; unknown members error. A single deduplicated member resolves directly. `resolve_calendar(None)` and `resolve_joint_calendar(None,None)` explicitly select weekends-only; one supplied joint leg uses that leg alone.

## Observed selection and controls

The public functions accept an explicit calendar/convention; IDs may be registered composites. Composite IDs round-trip in day-count contexts if every member is restorable. The 100-day search bound is internal and not caller-configurable. This is bounded per consecutive search, not a total 100-day tenor cap.

## Observed limitations, failure and fallback

Unknown explicit IDs never silently become weekends-only in these helpers. Schedule warning policies can skip adjustment separately (008). Modified rolls compare month, and their fallback is contractual behavior, not a missing-data repair. Generic calendars have no non-Saturday/Sunday built-in weekend configuration. An `is_holiday` result alone is insufficient to determine openness where weekends are not labeled holidays.

## Observed alternatives and tradeoffs

All six conventions and explicit weekends-only/joint IDs are supported. Alternatives such as union of business days, settlement lags counting the trade day, or unlimited closed-day search are not implemented. Daily scanning is slower for custom calendars but uses their exact supplied predicate.

## Evidence

[Adjustment and registry](../../../../finstack-quant/core/src/dates/calendar/business_days.rs), [joint behavior](../../../../finstack-quant/core/src/dates/calendar/composite.rs), [day stepping](../../../../finstack-quant/core/src/dates/date_extensions.rs). Inline tests `modified_following_is_evaluated_on_the_union_calendar`, `canonical_id_normalizes_union_members`, and `test_add_business_days_error_on_all_holidays` exercise dispatch/failure. [Schedule test](../../../../finstack-quant/core/tests/dates/schedule.rs) `nearest_payment_tie_rolls_following` verifies a concrete tie. These are deterministic boundary examples/invariants, not proof that every caller selected the correct legal calendars. [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions) is the conventions reference.

Calendar completeness limits whether a rolled date can be treated as a certified settlement date.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
