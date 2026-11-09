# CORE-CONVENTIONS-009: Unadjusted roll dates and product expiry boundaries

- Decision status: **Accepted — complete roll-date package, 2026-11-07**.
- Implementation status: **Observed; reconciliation pending**, scoped to roll-date functions in `dates/imm.rs`. Item 7 (generic IMM option-expiry removal) is reconciled as of 2026-11-08; the other items remain pending.
- Validation status: **Evidence inspected/located; independent qualification pending**. Documentation and independent Gregorian/lattice/search examples passed; production Rust/host tests and product-date qualification were not executed for this review.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

Shared helpers compute unadjusted Gregorian dates. Some source names and descriptions imply broader futures settlement, option expiry and CDS maturity conventions than their formulas establish. The accepted package below retains precise calendar selectors, removes generic option-expiry inference and separates product/regime selection from date arithmetic.

## Accepted decision: unadjusted roll dates and product boundaries

Accepted on **2026-11-07** after whole-record review. This is the complete
roll-date package; runtime reconciliation and independent qualification remain
open. Inherit the accepted
[004 calendar-data](004-holiday-engine.md#accepted-decision-holiday-engine-and-data-boundaries),
[006 adjustment](006-business-day-adjustments.md#accepted-decision-rolling-and-joint-calendar-conventions),
[007 date-arithmetic](007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions)
and [008 schedule](008-schedule-generation.md#accepted-decision-schedule-grids-events-and-failure-boundaries)
boundaries. The observed implementation remains separately described below.

1. **Keep these primitives date-only and unadjusted.** Returned dates are
   explicit points on a Gregorian lattice, without a holiday calendar,
   business-day adjustment, settlement lag, trading session, time zone or
   information cutoff. A closed date still belongs to its unadjusted lattice.
   Do not infer a trade date from today's clock or describe every IMM point
   as the legal settlement/expiry of every futures, option or CDS contract.
   Product owners select those meanings and any required published data.

2. **Retain third-Wednesday and third-Friday construction for every month.**
   `third_wednesday(month, year)` and `third_friday(month, year)` accept any
   of the twelve months, not just quarterly months. For target weekday `w`,
   the day is `15 + ((w - weekday(year,month,15)) mod 7)` with a nonnegative
   remainder; it lies in 15–21. Continue using the shared nth-weekday kernel
   with valid-date/error semantics, rather than parallel host arithmetic.
   Support the actual proleptic Gregorian Date domain, including year zero
   and negative years where represented. Arithmetic availability outside
   a product's historical life is not evidence that the contract existed.

3. **Retain strict searches and make their comparisons explicit.**

   | Operation | Selected candidate set | Comparison |
   | --- | --- | --- |
   | `next_imm(date)` | Third Wednesday of Mar/Jun/Sep/Dec | Earliest candidate strictly `> date`. |
   | `next_cds_date(date)` | Twentieth of Mar/Jun/Sep/Dec | Earliest candidate strictly `> date`. |
   | `prev_cds_date(date)` | Twentieth of Mar/Jun/Sep/Dec | Latest candidate strictly `< date`. |
   | `next_third_friday(date)` | Third Friday of every month | Earliest candidate strictly `> date`. |

   A date already on the selected lattice advances to the next candidate
   or retreats to the prior candidate; it is not returned unchanged. Search
   the selected months in chronological/reverse-chronological order across
   years. Keep strict and inclusive meanings distinct rather than shifting
   the supplied date by an incidental day or changing equality semantics.

4. **Keep predicates as exact lattice-membership checks.** `is_imm_date`
   is true only in Mar/Jun/Sep/Dec on Wednesday with day 15–21.
   `is_cds_date` is true only on the twentieth of those months. A valid
   off-grid Date returns false; invalid host input errors rather than
   becoming false. Membership proves neither an open business day nor an
   instrument's eligibility, settlement or quoted tenor.

5. **Retain two explicitly selected inclusive semiannual lattices.**
   Keep the existing canonical helpers with these limited meanings:

   | Operation | Selected candidate set | Comparison |
   | --- | --- | --- |
   | `prev_cds_semiannual_roll(date)` | March/September twentieth | Latest candidate `<= date`. |
   | `next_semiannual_cds_maturity(date)` | June/December twentieth | Earliest candidate `>= date`. |

   Equality returns the input. The June/December helper is a selector for
   that explicitly chosen maturity lattice, not every standard CDS maturity.
   The March/September helper does not determine a first premium accrual
   date, step-in or cash-settlement date. Correct documentation to state
   both the lattice and inclusivity; names alone are insufficient selection.

6. **Remove universal CDS-regime and tenor-composition claims.** Do not
   adopt `March/September floor + tenor -> June/December ceiling` as a
   general trade-to-maturity algorithm. The
   [official CDS model's December 2015 table](https://www.cdsmodel.com/news.html)
   includes March/September maturities for 3M/9M, alongside June/December
   whole-year points; it describes a single-name quoting change similar to
   indices, not a new simultaneous rule for every index and contract.
   Regime/version, trade/reference date, tenor eligibility, maturity,
   accrual start, protection start, settlement and terminal-day inclusion
   retain their product/calibration owners. Accepted 008 removes automatic
   schedule start substitution; these helpers do not restore that inference.
   Reconcile existing callers against their own stated conventions and
   independent dated fixtures, without silently substituting a different
   CDS regime or accepting a past maturity for a positive live quote tenor.

7. **Remove the generic IMM option-expiry helpers.** Remove
   `imm_option_expiry` and `next_imm_option_expiry`, including exports,
   bindings and examples, without aliases or a renamed convenience wrapper.
   The observed third-Wednesday-minus-five-calendar-days formula computes
   a Friday anchor but does not establish a generic option expiry. For
   example, [CME rule 460A01.J.1](https://www.cmegroup.com/rulebook/CME/V/450/460A/460A.pdf),
   printed page 4, specifies SOFR option termination with scheduled-holiday
   and underlying-market-opening exceptions. A nominal date and a generic
   Preceding roll cannot encode all of that product's rules. Product owners
   can use the retained third-Wednesday and checked day arithmetic when their
   actual convention calls for that anchor. Do not add a generic exchange
   expiry service or certify a rulebook solely from this calendar formula.

8. **Keep third Friday independent of listed-option expiry.** Retain
   `third_friday` and strict `next_third_friday` as unadjusted monthly
   primitives. April 2025 returns April 18 even though it is Good Friday;
   [OCC's dated 2025 processing notice](https://infomemo.theocc.com/infomemos?number=56330)
   places standard expiration processing on April 17. Do not silently
   holiday-adjust the primitive or equate its date with expiration, exercise
   cutoff, last trading, cash settlement or an index's opening/closing fixing.
   The applicable product/venue and dated rules select those outcomes.
   This package does not adopt one universal preceding-business-day policy.

9. **Use checked inputs and genuine finite-domain failure.** Require
   an exact integer month in 1–12 and a year whose requested Date exists;
   validate invalid/out-of-range scalar years before arithmetic. Direct
   third-weekday requests return their specified month/year's date without
   probing irrelevant neighboring years. Searches return the first/last
   qualifying represented candidate or an explicit existing error when
   none exists in the requested direction. Use checked year transitions
   and date construction; do not wrap, saturate, clamp to Date::MIN/MAX,
   return None, fabricate a date or impose an unrelated financial horizon.
   Required failures propagate through callers rather than `.ok()` or
   `unwrap_or(as_of)` fallback. Predicates require only valid-date membership,
   without unused holiday-data queries.

10. **Remove obsolete replacement-anchor searches after 008.** Its
    strict regular-anchor contract removes the need for the private
    `imm_on_or_after` and `imm_on_or_before` schedule searches. Remove them
    when reconciling their obsolete caller path; do not expose a second
    public inclusive IMM family or retain dead helpers for compatibility.
    Keep only the shared internal machinery needed by retained operations.

11. **Preserve exact host domains and canonical behavior.** Financial
    formulas stay in Rust. Python/WASM use the same retained names,
    membership rules, strict/inclusive comparisons and required failures.
    Retain checked integer month/year/epoch-day input conversion; reject
    Boolean, fractional, nonfinite or out-of-range numeric values rather than
    lossy coercion. Correct four-digit-only year prose and claims that all host
    native date types share Rust's range. A mathematically valid Rust date
    outside a native host date domain produces an explicit conversion error,
    not clipping or a global restriction on the Rust primitive. Epoch days
    remain signed civil days under 007; no implicit UTC instant conversion.
    Remove obsolete exports and reconcile declarations, parity, schemas
    where affected and examples in the same implementation slice.

12. **Keep product reconciliation and evidence separately owned.** This
    package adopts no pricing, calibration, historical CDS regime, exercise
    or settlement algorithm. Reconcile material CDS calibration/option and
    schedule callers, including short-tenor maturity composition and hidden
    boundary fallback. Independently qualify date extremes, equality,
    year transitions, selected lattice membership and all host errors;
    qualify actual product dates against the applicable dated rulebook or
    contract at their owner. SIFMA functions sharing `imm.rs` retain
    [010](010-sifma-settlement.md)'s separate scope. Runtime work and
    independent qualification belong to the
    [existing follow-ups](../FOLLOW_UPS.md#core-conventions).

## Boundary examples for the accepted package

These independent Gregorian examples express the accepted contract. They are not
executed production tests, exchange-expiry qualification or acceptance of a
CDS trade-to-maturity recipe.

| Request | Required result |
| --- | --- |
| `third_wednesday(February,2025)`; its `is_imm_date` | February 19; false because February is not a quarterly IMM month. |
| `third_wednesday(March,2025)` / `third_friday(March,2025)` | March 19 / March 21. |
| `next_imm(2025-03-19)` | June 18, 2025; strict equality does not retain March. |
| `next_cds_date(2025-03-20)` / `prev_cds_date(2025-03-20)` | June 20, 2025 / December 20, 2024. |
| March/September floor of March 20 / March 19, 2025 | March 20, 2025 / September 20, 2024. |
| June/December ceiling of June 20 / June 21, 2025 | June 20 / December 20, 2025. |
| June/December ceiling of September 20, 2028 | December 20, 2028, solely as the selected lattice operation. |
| `next_third_friday(2025-03-21)` | April 18, 2025, without Good-Friday adjustment. |
| `next_third_friday(2025-12-25)` | January 16, 2026. |
| A valid quarterly twentieth falling on a closed day | `is_cds_date` remains true; the primitive makes no openness assertion. |
| Retained direct third-weekday requests in the minimum/maximum represented year | Return that month's valid third weekday; no irrelevant adjacent-year probe. |
| Inclusive June/December ceiling on the last represented December 20 | Return that date; strict `next_cds_date` from it errors because no later quarterly twentieth exists. |
| March/September floor on the first represented March 20 | Return that date; a required earlier candidate below the Date domain errors. |
| Month 0/13, fractional/nonfinite host numeric input or an invalid requested year | Explicit input/conversion error, without normalization or fabricated date. |
| Existing composition for `2015-12-22 + 3M`: Sep20 floor + 3M then Jun/Dec ceiling | Produces December 20, 2015 by arithmetic; it is not adopted as the quoted CDS maturity. The official dated table gives March 20, 2016 for that tenor/regime. |

## Observed algorithm and actual selection

Third Wednesday/Friday uses the nth-weekday kernel. Quarterly months are March, June, September, December. `next_imm`, `next_cds_date` and `prev_cds_date` are strictly after/after/before the supplied date. Internal IMM on-or-after/on-or-before helpers are inclusive. `is_imm_date` requires quarterly month, Wednesday and day 15–21; `is_cds_date` requires quarterly month and day 20.

`prev_cds_semiannual_roll` returns March/September 20 on or before input. `next_semiannual_cds_maturity` returns June/December 20 on or after input. A generic month scan iterates candidate months in direction order, moves years as needed and errors at representable date boundaries.

The former `imm_option_expiry(month,year)` (third Wednesday minus five calendar days) and `next_imm_option_expiry` were removed on 2026-11-08 under item 7. `next_third_friday` searches all months strictly after input. Neither moves a Good Friday or other closure; caller-specific holiday adjustment is separate.

## Observed conventions, controls and limitations

Date granularity is one calendar day. Inclusive and exclusive variants are intentional and can shift a trade on roll day by a full cycle. No automatic pre-/post-Big-Bang or historical contract-regime selector exists in these primitives. `third_wednesday` alone permits nonquarterly months while `is_imm_date` does not.

The source's universal June/December post-2015 maturity claim is broader than
the official tenor table. [CDS calibration's composition](../../../../finstack-quant/calibration/src/build/cds.rs)
unconditionally applies the two semiannual helpers, while
[CDS-option start selection](../../../../finstack-quant/valuations/credit/src/cds_option/types.rs)
can replace a failed prior-roll search with `as_of`. These are material
consumer reconciliation boundaries, not evidence of an adopted product recipe
or an executed pricing defect reproduction.

## Alternatives and tradeoffs

Retaining date primitives avoids embedding an exchange or historical CDS regime in the common calendar layer. The accepted package removes misleading option-expiry helpers rather than introducing a generic expiry framework; actual product expiry and tenor-to-maturity mappings remain with their owning conventions and evidence.

## Evidence

[Owning code and inline tests](../../../../finstack-quant/core/src/dates/imm.rs), including `prev_semiannual_roll_selects_mar_or_sep_on_or_before`, `semiannual_maturity_snaps_to_jun_or_dec`, `next_third_friday_remains_unadjusted_on_good_friday`, and date-boundary failures. [Schedule tests](../../../../finstack-quant/core/tests/dates/schedule.rs) exercise actual IMM caller behavior. [ISDA CDS model](../../../REFERENCES.md#isda-cds-standard-model) is relevant background; the inspected code does not establish an independent exchange rulebook citation for every option-expiry helper.

These date formulas alone do not certify a trade's contractual maturity or exercise deadline.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
