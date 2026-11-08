# CORE-CONVENTIONS-008: Anchored coupon grids, stubs, payment/fixing lags and failure policy

- Decision status: **Accepted — complete schedule-generation package, 2026-11-07**.
- Implementation status: **Observed; reconciliation pending**, scoped to all four files under `dates/schedule/`.
- Validation status: **Evidence inspected/located; independent qualification pending**. Documentation and independent anchor/stub/grid/event-date examples passed; production Rust/host tests were not executed for this review.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

The observed schedule keeps its supplied/generated date grid unadjusted and computes payment/fixing dates separately. Its default spec is monthly, no stubs, no adjustment/calendar, no EOM, no IMM, zero payment lag, no fixing series and Strict error policy. These observed defaults and fallback paths are not independently adopted. The accepted package below defines the generic grid's role, explicit selections, stub/roll boundaries, event dates and failure/state contract.

## Accepted decision: schedule grids, events and failure boundaries

Accepted on **2026-11-07** after whole-record review. This is the complete
schedule-generation package; runtime reconciliation and independent qualification
remain open. Inherit the accepted
[002 coupon-context](002-coupon-context-day-counts.md#accepted-decision-coupon-context-day-counts),
[004 calendar-data](004-holiday-engine.md#accepted-decision-holiday-engine-and-data-boundaries),
[006 rolling](006-business-day-adjustments.md#accepted-decision-rolling-and-joint-calendar-conventions)
and [007 arithmetic](007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions)
contracts. Observed behavior remains separately recorded below.

1. **Define the result as a raw unadjusted period grid with separate events.**
   Keep supplied start/end dates exactly once and generate only interior
   boundaries. Do not business-day-adjust this grid inside the generic builder.
   Payment and fixing dates remain parallel per-period series. The consuming
   product decides whether raw grid dates also define its actual accrual
   boundaries and applies any separately selected accrual adjustment. Do not
   describe unadjusted accrual as universal: the
   [March 4, 2009 North American CDS specification](https://www.cdsmodel.com/assets/cds-model/docs/Standard%20CDS%20Contract%20Specification.pdf)
   adjusts intermediate accrual dates Following while retaining unadjusted
   maturity. This historical example establishes the need for separate roles,
   not adoption of that entire product specification.

2. **Require explicit financial selections and consistent admission.**
   A complete `ScheduleSpec` requires `start < end`, a valid positive 007
   frequency, a stub policy, one grid mode and a payment business-day convention.
   Remove the implicit monthly/None-stub/no-convention financial defaults.
   Retain only Natural, EndOfMonth and quarterly IMM as mutually exclusive
   grid choices; replace conflicting mode booleans with that single selection.
   A mode selector must not reset frequency/stub or silently switch another
   choice. An incomplete builder may collect inputs, but cannot publish a
   valid specification or schedule until required choices are supplied.
   Retain the declared operation defaults of zero extra payment lag and no
   fixing series. Validate complete specifications at construction,
   deserialization and execution, including directly mutated/restored values;
   reject unknown fields, missing required choices and incompatible settings.

3. **Generate every regular date from the original anchor.** Natural uses
   the exact checked calendar arithmetic in 007: D/W are calendar days,
   M/Y use direct target-month clamping. The regular anchor is start for
   None/back stubs and end for front stubs. Compute `anchor + n*frequency`
   from that anchor; never step from a previously clamped/snapped date.
   Preserve the frequency's admitted count/unit and use widened checked
   products rather than the source's assumption that every count fits `i32`.
   A regular interval is defined by successive grid indices, not equal
   elapsed-day lengths or inferred annual payment counts. Front and back
   generation may select different Natural grids after month clamping;
   alignment is always tested against the chosen original anchor/direction.

4. **Adopt the five literal stub policies.**

   | Stub | Selected rule |
   | --- | --- |
   | None | Generate forward and require end to be an exact point on the selected grid. A term shorter than one frequency is not an exception. |
   | ShortBack | Generate forward from start; close the residual final period at end. |
   | LongBack | Use that forward grid, merging a real final residual with the immediately preceding regular period. |
   | ShortFront | Generate backward from end; open the residual first period at start. |
   | LongFront | Use that backward grid, merging a real initial residual with the immediately following regular period. |

   Long policies leave a schedule exactly aligned to their own anchor/direction
   unchanged. If no interior
   boundary exists, an allowed stub gives `[start,end]`; there is no adjacent
   period to merge. The directional/merge distinction is also described by
   [Strata's short/long stub definitions](https://strata.opengamma.io/apidocs/com/opengamma/strata/basics/schedule/StubConvention.html),
   but the strict None rule above is this project's selected boundary.
   Do not add implicit both-end stubs, smart thresholds or guessed stub dates.

5. **Make EOM an explicit calendar-month-end grid.** Permit only M/Y
   frequencies. Require the regular anchor side to be calendar month-end:
   start for None/back policies, end for front policies. Snap calculated
   interior rolls to the actual last calendar day of their target month.
   None requires both endpoints aligned and an exact frequency-phase hit.
   Preserve an off-grid endpoint only on the declared stub side; reject
   an off-grid regular anchor instead of silently creating another stub or
   snapping the supplied endpoint. This corrects the observed forced-EOM
   behavior from a midmonth anchor. It is distinct from last-business-day EOM
   and from an inferred/conditional preference for EOM.

6. **Use the same strict anchor/stub contract for quarterly IMM.** The
   lattice is the third Wednesday of March, June, September and December.
   Require an M/Y frequency whose month count is a positive multiple of
   three. The regular anchor side must itself be on that lattice; its month
   determines the frequency phase for 6M, 9M, 1Y and larger intervals.
   None requires an exact aligned end. Only the declared stub side may be
   off-grid. Remove the implicit short stub at the opposite endpoint and
   the search for a replacement first/last IMM anchor. Do not turn a required
   date/helper failure into a successful two-endpoint schedule via `.ok()`.
   These are unadjusted grid dates, not exchange-specific expiries or
   settlement dates; those selectors remain with [009](009-contractual-roll-dates.md)
   and the consuming product.

7. **Remove the generic CDS mode and its automatic start substitution.**
   Remove `cds_imm`, `cds_imm_mode` and their implicit quarterly/ShortBack
   defaults. A product supplies its actual raw accrual start, end, frequency
   and stub after selecting its contractual regime; Natural generation
   already produces a quarterly twentieth grid from explicit twentieth
   anchors without a second schedule algorithm. Trade date, protection
   effective date and accrual start must not be conflated. Any prior-roll
   accrual, quarterly/semiannual maturity selection, adjusted accrual or
   terminal-day inclusion remains with the product and 009's date helpers.
   [QuantLib's distinct TwentiethIMM/OldCDS/CDS/CDS2015 rules](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/time/dategenerationrule.hpp)
   also show why a generic prior-twentieth shift is insufficient to certify
   every CDS regime. This package does not adopt one of those product recipes
   or add a replacement CDS mode/compatibility alias.

8. **Resolve calendars for their actual uses.** Require the payment
   convention explicitly, including deliberate Unadjusted. A calendar may
   be absent only when Unadjusted and both requested lags need no business-day
   queries. A positive payment or fixing lag requires an explicit calendar,
   including explicit `weekends_only` where intended. A provided identifier
   is always resolved strictly, independently of whether payment adjustment
   seeks an open date; unknown IDs cannot be ignored for Unadjusted/zero
   operations. Do not classify unused dates for an identity operation.
   Retain one supplied calendar for this builder's payment adjustment and
   both lags. Products needing distinct calendars compose their own explicit
   workflow rather than silently treating those roles as interchangeable.
   Required coverage/data errors follow 004/006, including joint calendars.

9. **Adopt payment adjustment followed by payment lag.** For each raw
   period end `e`, compute `shift_business_days(adjust(e, convention), lag)`
   with the selected calendar. The extra lag is an integer `0..=i32::MAX`;
   reject negatives, use 006's start-excluded counting, and perform no extra
   post-lag roll. Zero retains the adjusted endpoint. This ordering is this
   generic operation's selected recipe, not a universal product convention.
   Preceding rules may legitimately put payment before the raw end; do not
   impose an unconditional payment-on/after-end condition.

10. **Keep fixing lag distinct from payment adjustment.** Absent lag
    produces no fixing series; configured zero produces one unchanged raw
    period start per interval, even if closed. For a positive integer lag
    through `i32::MAX`, subtract that many business days directly from each
    raw start using 006's start-excluded rule. Reject negatives; do not
    pre-roll the start or reuse the adjusted/delayed payment endpoint.
    Actual reset dates, in-arrears rules, observation shifts, publication
    eligibility and information cutoffs retain their product/0045 owners.

11. **Use necessary date comparisons and one explicit resource ceiling.**
    Keep at most **100,000 final grid dates including both endpoints** as
    a fixed resource/admission limit, not a financial maturity convention.
    Apply it to every mode, stub and restored result, without truncation.
    A removed long-stub boundary or temporary comparison anchor must not
    incorrectly consume final-output capacity. Use exact widened arithmetic
    to prove when a hypothetical comparison roll lies beyond the supported
    Date domain and hence beyond the actual endpoint; a permitted stub may
    finish at that endpoint without materializing the irrelevant roll.
    An aligned end needs no successor. None still requires an exact grid
    hit. Invalid arithmetic and required returned/reference/calendar dates
    remain errors; this is not a catch-all date-error suppression policy.

12. **Return a complete schedule or an error.** Remove
    `MissingCalendarWarning`, `GracefulEmpty`, the redundant one-choice
    error-policy control and their fallback warning carriers/accessors.
    Equal/reversed ranges, missing choices/calendars, invalid grids/lags,
    required data/date failures and resource errors cannot return empty or
    partial contractual schedules. Remove monotonicity/drop repair and its
    log-only success. A generated raw grid must be strictly increasing,
    with both endpoints exactly once; duplicate/backward anchors are errors.
    Deliberate generation of an endpoint once is not repair of malformed
    output. Do not sort, silently drop dates or substitute another policy.

13. **Validate immutable result shape without reconstructing economics.**
    A valid Schedule has 2–100,000 strictly increasing raw grid dates,
    exactly `N = dates.len()-1` payment dates and either zero or N fixing
    dates. Preserve period index and legitimate payment/fixing collisions;
    never deduplicate them. Require all three arrays on canonical wire state,
    including an empty fixing array when the series is omitted, and reject unknown fields,
    impossible dates, malformed lengths and empty/fallback results. Remove
    missing-payment/fixing defaults. Keep result fields private/validated
    with ordinary accessors rather than mutable invariant bypasses. Apply
    the same boundary to JSON, pickle and host construction. Restoration
    validates stored dates/shape; it neither rebuilds against today's
    calendar nor proves provenance or consistency with a missing source
    specification. Rebuilding uses the fully selected specification and
    004–006's frozen calendar-edition requirements in the owning state.

14. **Keep nominal references available for explicit day-count composition.**
    Consumers needing ICMA use the same original anchor, frequency and
    selected roll to supply full nominal reference periods, including a
    boundary removed when merging a long stub. Split an accrual span into
    contained slices with exact coverage and sum the accepted 002 primitive.
    Do not use a shortened actual stub as its own regular denominator or
    infer references/frequency from payment dates or elapsed days. A nominal
    reference outside the actual schedule may still be required; if that
    date cannot be represented, the requested accrual calculation errors
    even when the raw grid itself can validly terminate without that date.
    This package adds no new day-count formula or inferred context framework.

15. **Reconcile canonical and host contracts together.** Keep generation
    and validation in Rust. Propagate explicit selections, removed modes/
    policies/warnings, strict state admission, lag units and retained period
    indices through Python/WASM, schemas, parity contracts and examples.
    Remove inherited monthly/mode defaults, stale conditional-EOM prose and
    unchecked JSON/pickle construction. Respect native-host date domains and
    centralized error mapping. Reconcile CDS and other financial callers
    against their owning conventions; do not replace removed inference with
    another automatic product recipe. Implementation and independent
    date/calendar/host qualification belong to the
    [existing follow-ups](../FOLLOW_UPS.md#core-conventions).

## Boundary examples for the accepted package

These independent calendar examples express the accepted contract; they are
not executed production tests or vendor qualification. Unless stated otherwise,
the grid is Natural, payment is explicit Unadjusted with zero extra lag and
the fixing series is absent.

| Request | Required result |
| --- | --- |
| Natural `1M`, None, `2025-01-31 → 2025-04-30` | January 31, February 28, March 31, April 30; no clamping drift. |
| Natural `3M`, `2025-01-15 → 2025-09-15` | ShortBack: Jan15, Apr15, Jul15, Sep15. LongBack: Jan15, Apr15, Sep15. ShortFront: Jan15, Mar15, Jun15, Sep15. LongFront: Jan15, Jun15, Sep15. None: error. |
| A grid exactly aligned to the selected Long policy's own anchor/direction | Every regular boundary retained. |
| Natural `1M`, `2025-01-31 → 2025-04-30`, front versus back generation | Back grid: Jan31, Feb28, Mar31, Apr30. ShortFront: Jan31, Feb28, Mar30, Apr30. LongFront merges the genuine initial residual: Jan31, Mar30, Apr30. |
| `1M`, `2025-01-15 → 2025-01-20` | None errors; an allowed stub returns the two endpoints. |
| EOM `1M`, None, `2025-02-28 → 2025-04-30` | February 28, March 31, April 30. |
| EOM `1M`, None/ShortBack from `2025-01-15` | Error: regular start is off-grid; do not snap it or create an implicit initial stub. |
| EOM `1M`, ShortFront, `2025-01-15 → 2025-03-31` | January 15, January 31, February 28, March 31. |
| Quarterly IMM `3M`, None, `2025-03-19 → 2025-12-17` | March 19, June 18, September 17, December 17. |
| Quarterly IMM `6M`, ShortBack, `2025-03-19 → 2026-01-15` | March 19, September 17, January 15; phase follows the actual start. |
| Quarterly IMM ShortBack from `2025-01-15`, or EOM with `1W` | Error for an off-grid regular anchor or incompatible frequency, respectively. |
| Explicit Natural `3M`, None, `2024-12-20 → 2025-12-20` | Dec20, Mar20, Jun20, Sep20, Dec20; no automatic trade-date interpretation or start backshift. |
| End Saturday `2025-01-11`, Following, weekends-only, payment lag1 | Monday January 13 is the adjusted end; payment is Tuesday January 14. |
| Start Sunday `2025-01-12`, weekends-only, fixing lag1 / lag0 / absent | Friday January 10 / unchanged Sunday January 12 / no fixing series. |
| Explicit Unadjusted plus positive business lag | Valid only with a supplied calendar; do not ignore a calendar-only lag request. |
| Daily grid `2025-01-03 → 2025-01-06`, Following, weekends-only | Three separate period payments, all January 6; retain all three rows. |
| Equal/reversed dates, missing choices, unknown supplied calendar, negative lag or malformed restored shape | Explicit error; no empty/fallback schedule or dropped rows. |
| 100,000 / 100,001 final raw dates | Admitted / resource error, regardless of generation direction or removed temporary anchors. |
| Valid final endpoint at Date::MAX with an unnecessary successor outside the domain | An allowed stub/aligned end may finish without that successor; a required unrepresentable ICMA reference still errors. |

## Observed algorithm

Generate each roll as original anchor plus `n * tenor`, then apply Natural, EndOfMonth or ThirdWednesday roll. Do not iterate from a previously clamped date. ShortBack generates forward from start and appends end. No-stub uses that path but requires an exact grid hit at end. ShortFront generates backwards from end and prepends start. LongFront removes the first interior anchor only if the start is off-grid; LongBack merges the last irregular period by inspecting the next-after-next roll. Preserve contractual start/end even when off EOM. Enforce at most 100,000 anchors with checked roll-index/date arithmetic. Afterwards keep only strictly increasing dates, dropping duplicates/backwards values and logging the drop.

Standard IMM mode defaults quarterly/ShortBack; requires whole-quarter frequency, and no-stub endpoints on quarterly third Wednesdays. It selects the first roll on/after start for forward generation, or last roll on/before end for front stubs, preserving external endpoints. If there is no internal roll, return `[start,end]`.

The IMM boundary lookups use `.ok()` before selecting that two-endpoint result,
so lookup failures are not distinguished from absence of an interior roll.

CDS IMM mode also defaults quarterly/ShortBack, but snaps a non-roll start to the strictly previous quarterly 20th, retaining front accrual. It then invokes ordinary Natural generation with supplied end/frequency/stub. Unlike standard IMM, it does not separately reject non-quarter frequencies or off-roll maturities after caller overrides. EOM is rejected for day/week tenors and either IMM mode; both modes cannot coexist. Setter calls switch off the other mode.

For each accrual period end, apply business-day convention, **then** add payment lag in business days. For each period start, fixing lag produces `start - lag business days`; zero lag retains start without adjustment, and absent lag omits the fixing series. Negative lags error. Positive lags require a resolved calendar. Calendar resolution is attempted only when both convention and calendar ID are present; an ID alone does not enable lag calculation.

## Observed selection, conventions and failure

`ScheduleSpec::new` permits equal endpoints, but generation rejects `start >= end`; Strict therefore errors and GracefulEmpty can return an empty schedule with warning. Public-spec deserialization validates order and mutually exclusive modes; more checks occur on build.

Strict propagates errors. MissingCalendarWarning skips a missing adjustment calendar and records a warning; a positive lag still errors without a calendar. GracefulEmpty converts errors inside `build_impl` to empty date series plus error warning, but outer checks for incompatible EOM/modes and reversed endpoints still error. Monotonic deduplication logs rather than adding a `ScheduleWarning`; payment-date collisions are retained because accrual periods remain distinct.

The public Schedule fields and plain derived Deserialize admit unchecked result
arrays; missing payment/fixing arrays default empty. Shape validation and canonical
restoration are not established by a valid result from the builder alone.

## Alternatives and tradeoffs

The observed API offers five stub choices and three error policies. The accepted package retains the stub choices with literal anchor/phase rules, removes fallback policies and automatic CDS start substitution, and validates raw grids separately from product-owned accrual adjustment. Different payment/fixing calendars, both-end stubs and full product regime selectors are outside this generic builder's selected scope; their absence must not be hidden through inferred defaults or repaired dates.

## Evidence

[Builder/lag/fallback ordering](../../../../finstack-quant/core/src/dates/schedule/builder.rs), [generation](../../../../finstack-quant/core/src/dates/schedule/generation.rs), [spec defaults](../../../../finstack-quant/core/src/dates/schedule/spec.rs), [public schedule/warnings](../../../../finstack-quant/core/src/dates/schedule/mod.rs). [Integration tests](../../../../finstack-quant/core/tests/dates/schedule.rs) include anchored month-end drift, all stub variants, finite date boundaries, quarterly IMM validation, payment-lag-after-adjustment, fixing T-minus, and unadjusted CDS 20ths. These primarily assert constructed expected grids and invariants. [ISDA](../../../REFERENCES.md#isda-2006-definitions) and [ICMA](../../../REFERENCES.md#icma-rule-book) are relevant convention references, not confirmation of every override combination.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
