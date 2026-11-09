# CORE-CONVENTIONS-010: Published TBA settlement dates and explicit class selection

- Decision status: **Accepted — complete published-settlement package, 2026-11-07**.
- Implementation status: **Partly reconciled 2026-11-08**: items 1, 2, 4, 6, 7, 10 (fixed-date tests) and 11 landed, and TBA/dollar-roll require an explicit class (12, per owner resolution O2); items 8 and 9 (data pin and admission) remain. Scoped to SIFMA functions in `dates/imm.rs` and `generated/sifma_settlements_generated.rs`.
- Validation status: **Evidence inspected/located; independent qualification pending**. Documentation, source-date spot-checks and independent CSV/search/calendar examples passed; production Rust/host tests and full source-table qualification were not executed for this review.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

The observed API separates exact maintained SIFMA dates from a business-day-index estimate, but also defaults/infer classes and collapses absent or invalid stored dates into None. The accepted package below makes published dates and explicit class selection canonical, removes the unqualified estimator and defines required data, search, provenance and host boundaries.

## Accepted decision: published settlement dates, classes and coverage

Accepted on **2026-11-07** after whole-record review. This complete SIFMA
settlement package requires runtime/data reconciliation and independent
qualification. Inherit the accepted
[004 frozen-data/error boundary](004-holiday-engine.md#accepted-decision-holiday-engine-and-data-boundaries),
[005 SIFMA trading-calendar purpose](005-jurisdiction-calendars.md#accepted-decision-jurisdiction-calendar-purposes-and-coverage),
[007 date arithmetic](007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions)
and [009 exact date/search principles](009-contractual-roll-dates.md#accepted-decision-unadjusted-roll-dates-and-product-boundaries).
The observed implementation remains separate below.

1. **Require explicit settlement class everywhere in the shared API.**
   Retain A/B/C/D and canonical wire/display labels `a`/`b`/`c`/`d`.
   Text parsing may retain outer-whitespace/ASCII-case normalization for
   those labels; reject unknown classes. Remove the enum's Class-A Default,
   Python's omitted/None-class default and the separate WASM default-class
   branch/export. Require a class for both exact lookup and strict-next
   search. Explicit A remains valid; unknown/omitted input must not become
   A, D or another class.

2. **Make exact lookup a required published-date operation.** Change
   `sifma_settlement_date(month, year, class)` to the existing checked
   `Result<Date>` contract. Validate the exact requested month/year/class,
   then return the maintained settlement cell for that key. Missing local
   row/class coverage is an explicit data-unavailable error; invalid stored
   dates are data-validation errors, not missing cells. Remove `.ok()` and
   None/undefined success paths rather than adding a parallel optional
   financial lookup. Error context identifies the class, month/year and
   applicable data edition/coverage. Do not estimate, change class, skip
   dates or substitute another calendar when required data are unavailable.

3. **Use the published Settlement cell unchanged.** The
   [SIFMA source table](https://www.sifma.org/resources/general/mbs-notification-and-settlement-dates)
   publishes separate Notification and Settlement rows. This API selects
   Settlement only; it neither returns notification dates nor derives them
   by subtracting days. Do not re-roll a published settlement or require a
   successful `sifma` holiday-calendar query to return it. That calendar is
   a bond-trading recommendation under 005, not the MBS settlement table.
   A conflicting maintained calendar/source value requires data review,
   not silent calendar repair or replacement of the published cell.

4. **Select the next date strictly for the supplied class.** Extend
   `next_sifma_settlement(date, class)` to return checked `Result<Date>`.
   It means the earliest required published monthly settlement strictly
   after `date`; equality advances to the following month's date. Query
   the current month only when a date in it could still qualify. If its
   known settlement is not later, query the immediately following month.
   At calendar month-end, every current-month date is already nonqualifying,
   so an absent current cell may be bypassed without inventing its value.
   A required missing current/following cell errors; do not jump over an
   unknown gap to a later known row and call that the next settlement.
   Remove the Class-A-only path and arbitrary 14-month loop. These monthly
   semantics need at most the current and following cells, not an open-ended
   search. Use checked month/year transitions; Date::MAX has no later date
   and errors without an irrelevant data lookup.

5. **Describe actual maintained coverage and gaps accurately.** The
   audited CSV contains 36 monthly rows: Class A for 2025–2027 and B/C/D
   for 2026–2027, with 108 populated and 36 blank class cells. A blank
   2025 B/C/D cell means unavailable in this maintained dataset; it is not
   evidence that SIFMA never published that date. Preserve per-class gaps
   instead of claiming every class is covered between the first/last row.
   No local values after December 2027 are presently maintained. Do not
   interpret missing data as no settlement in that month. Future dates
   already present in a verified published edition remain published dates,
   rather than projections merely because they are later than valuation.

6. **Remove the unqualified business-day-index estimator.** Remove
   `estimated_sifma_settlement_date`, its 9/11/14/16 ordinal controls,
   exports, tests and suitability/calibration claims, without a compatibility
   alias or renamed projection wrapper. It is not a published date rule:
   January 2026's 11th business day under the observed calendar is January
   16, while the source's Class-B settlement is January 20. Neither an
   authored example nor the source's assertion of calibration establishes
   accuracy for pricing or settlement. Beyond maintained published coverage,
   a projection/scenario owner must explicitly supply or adopt its own
   documented date assumptions. This package creates no replacement model
   and no automatic fallback from required published data.

7. **Remove agency-substring/term classification from core.** Remove
   `SifmaSettlementClass::from_agency_term` and host factories/examples.
   Agency text containing `GN`, every 15Y label and the catch-all D branch
   do not establish actual security eligibility. The
   [DTCC product/class description](https://dtcclearning.com/products-and-services/fixed-income-clearing/mbsd/mbsd-overview/sifma-and-allocations.html)
   identifies distinct product groups; D is a named product class, not a
   safe unknown-program bucket. The owning TBA/dollar-roll workflow must
   require a class or use an explicitly adopted mapping from validated
   product metadata and applicable dated class definitions before invoking
   this API. Do not introduce a speculative agency mapping registry or
   adopt every product's current inferred-class behavior here.

8. **Freeze source identity and class coverage with the maintained data.**
   Record the official source URL/artifact, publication/edition information,
   retrieval date, content hash, extraction choice and maintained class/month
   ranges/gaps alongside the CSV/generated reference-data ownership. Freeze
   amendments and prior editions needed for reproducible results; a source
   URL alone is not a data pin. The owning market/replay state records the
   edition on which selected dates depend. Historical information-cutoff
   workflows must use an edition available under their own accepted as-of
   policy; a current table alone does not prove historical availability.
   Keep the minimal data/provenance record with the existing reference-data
   process, without adding a provider registry or runtime network fetch.

9. **Validate data admission and deterministic generation.** Retain
   the exact six-column header, unique year/month keys, at least one
   populated class per row, valid month/day integers and deterministic
   sorting/emission. Validate years and populated cells against the actual
   canonical Date domain, not merely Python's permissive calendar arithmetic
   or a generic positive-year restriction. Cross-check declared class
   coverage and every imported value against the pinned Settlement source
   rows. Reject malformed/duplicate/out-of-domain data or wrong source-row
   selection; do not silently drop, repair or fill cells. Generator/source
   equality proves deterministic derivation, not independent correctness
   of the imported financial dates.

10. **Use reviewed publication refreshes and request-driven readiness.**
    Core reference-data maintainers review official schedules/amendments,
    update maintained data and provenance, and regenerate checked-in output
    and affected independent fixtures together. Routine builds/renderers
    consume frozen inputs and never refresh from today's web page. Validate
    source extraction independently and test fixed coverage/equality/gap
    cases. Do not fabricate dates to satisfy the existing rolling twelve-
    month test or make fixture results depend on the current wall clock.
    No universal twelve-month future-data guarantee is adopted: an owning
    operational workflow may separately require a dated readiness horizon,
    while this API requires complete data for its actual request.

11. **Preserve exact host inputs and explicit failures.** Keep all
    financial lookup/search logic in Rust. Require the same class on
    Rust/Python/WASM surfaces; remove default-class helper branches and
    optional-result declarations. Reject Boolean, fractional, nonfinite,
    invalid month/class and out-of-Date-range year/epoch inputs before
    interpreting coverage. Preserve data-unavailable versus malformed-input
    context through the existing structured error mapping. Native-host date
    conversion failures are explicit; no clipping, undefined-result or
    implicit UTC-instant conversion. Reconcile declarations, schemas where
    affected, parity contracts, examples and source documentation together.

12. **Reconcile owning financial consumers and qualify their actual uses.**
    Reconcile the TBA class selection and published-date resolver, plus
    dollar-roll or other callers sharing them, against their own approved
    security/contract conventions. Preserve the distinction between a
    published regular TBA date, explicit contract settlement and a projected
    scenario date. Do not adopt a pricing, notification, allocation, funding
    or nonstandard-settlement rule merely because the date lookup succeeds.
    Independent publication extraction, required-gap/equality/month-end/date-
    limit cases and host failures remain with the
    [existing follow-ups](../FOLLOW_UPS.md#core-conventions). Runtime, data
    refresh and independent production qualification remain open.

## Boundary examples for the accepted package

These accepted boundaries use the current maintained CSV and selected
independent published-date/calendar checks. They do not refresh the dataset,
execute production Rust/host tests or qualify an operational settlement workflow.

| Request | Required result |
| --- | --- |
| January 2026 exact lookup, explicit A/B/C/D | January 14 / 20 / 22 / 27 from Settlement cells. |
| January 2026 Class B source Notification versus Settlement | January 15 is notification; return January 20 for this API. |
| January 2027 exact lookup, explicit A/B/C/D | January 14 / 19 / 21 / 25, as published future dates in the inspected source. |
| January 2025 A / B in the current dataset | January 14 / data-unavailable error; B is not inferred from A or an estimator. |
| A valid 2030 class/month with no maintained cell | Data-unavailable error, without asserting SIFMA will never publish a date. |
| Strict next from `2026-01-14`, Class A / B | February 12 / January 20, respectively. |
| Strict next from `2026-01-20`, Class B | February 17; equality does not retain January 20. |
| Strict next from `2025-12-31`, Class B | January 20, 2026; the missing December B cell cannot contain a later date. |
| Strict next from `2025-12-15`, Class B | Data-unavailable error: the missing December cell could contain the first qualifying settlement. |
| Strict next from `2027-12-22`, Class D | Data-unavailable error for the required January 2028 cell; no estimate or gap skipping. |
| A known published cell with unavailable unrelated holiday-calendar coverage | Return the cell without a holiday-calendar query or re-roll. |
| Date::MAX strict-next request | Explicit no-later-date error, without querying irrelevant settlement data. |
| Missing/unknown class, Boolean/fractional numeric input or invalid requested year | Explicit input/conversion error before coverage lookup. |
| Malformed imported date, duplicate month key or populated cell beyond Date's domain | Reject data admission/generation; never collapse it into a missing-date success. |
| The observed January 2026 Class-B ordinal estimate | January 16 differs from published January 20; the estimator is removed, not used as fallback. |

## Observed algorithm, selection and conventions

`SifmaSettlementClass` has A/B/C/D and no default; every lookup takes an explicit class. The former `from_agency_term` string/term heuristic was removed on 2026-11-08; the class is never inferred from agency text or term.

`sifma_settlement_date` rejects an out-of-range year, then linearly finds `(year,month)` in the generated table, selects the class column and constructs the date. A missing row or class cell is an `InputError::NotFound` naming the class, month and the class's maintained coverage; an invalid stored day is a validation error. The table contains Class A for 2025 and all four classes for 2026–2027. `next_sifma_settlement(date, class)` returns the current month's cell when it is later than `date`, otherwise the following month's cell. Equality advances, a required missing cell errors rather than being skipped, the current cell is not consulted on a month's last day, and `Date::MAX` errors without a lookup.

The former business-day-index estimator `estimated_sifma_settlement_date` was removed on 2026-11-08, with no replacement projection.

## Observed limitations, failure and alternatives

Exact local data coverage is finite and includes unmaintained class cells in 2025; there are no maintained cells after 2027. The source/generator describes blank cells as unpublished without an attached edition/coverage record proving that status. Calling `next_sifma_settlement` does not select a class from an instrument; the caller supplies it. The accepted package removes estimation/default/classification paths rather than adding a richer generic mapping or projection framework.

## Evidence

[Lookup/classification/estimate](../../../../finstack-quant/core/src/dates/imm.rs), [generated table](../../../../finstack-quant/core/src/generated/sifma_settlements_generated.rs), [maintained CSV](../../../../finstack-quant/core/data/sifma_settlements.csv). Inline tests `sifma_published_cells_are_returned_unchanged`, `sifma_missing_cells_are_not_found_errors`, `sifma_next_is_strict_and_class_specific`, `sifma_next_errors_on_required_gaps_without_skipping`, `sifma_next_at_date_max_errors_without_lookup` and `sifma_class_a_2025_uses_published_dates` are fixed-date stored-value and algorithm examples with no wall-clock dependence. The generator consistency check passed; Rust tests and external publication refresh were not run. [SIFMA TBA Good Delivery](../../../REFERENCES.md#sifma-tba-good-delivery) is the canonical reference entry.

The [reference-data generator](../../../../finstack-quant/core/build/generate_reference_data.py) validates CSV shape, unique keys and calendar month/day values, but does not establish the imported source edition, class-coverage record or canonical Date year range. The separate [TBA resolver](../../../../finstack-quant/valuations/fixed-income/src/tba/types.rs) and the dollar roll require an explicit `settlement_class` and propagate the core lookup error for a missing published date.

On 2026-11-07, spot-checks of January 2026, January 2027 and March 2027
matched all four maintained Settlement cells to the official page. Independent
CSV coverage, required-cell/search and calendar-arithmetic examples passed.
These checks did not refresh maintained/generated data, establish the historical
source edition for every cell or qualify every published class/month.

## Follow-up tracking

Implementation/data reconciliation and independent qualification are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
