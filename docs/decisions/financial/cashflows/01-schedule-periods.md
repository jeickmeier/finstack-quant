# CASHFLOWS-001 — Contractual periods, calendars and day-count context

- Decision status: **Accepted — complete contractual-period/calendar/day-count package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, scoped to the owning files below.
- Validation status: **Production evidence inspected/located, not executed**. Independent date/fraction examples and read-only grid/context/caller/reference reviews are reported below; these do not qualify production schedules or financial product conventions.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and scope

The observed implementation compiles cashflow dates from contractual anchors using the core schedule builder, then enriches economic accrual dates, cash payment dates and optional reset dates. It currently supports plain month/day/week grids, third-Wednesday IMM and CDS twentieth grids. This record owns the cashflows-layer transformations; date arithmetic and holiday definitions belong to core. Source comments motivate preserving contractual reference periods and separating cash settlement from interest accrual. The observed preset table below is not adopted financial authority; the complete accepted package removes those factories.

## Accepted decision: contractual periods, calendars and day-count context

Accepted on **2026-11-08** after whole-record review. Inherit accepted core
[actual-day conventions](../core-conventions/001-actual-day-counts.md#accepted-decision-actual-day-conventions),
[coupon context](../core-conventions/002-coupon-context-day-counts.md#accepted-decision-coupon-context-day-counts),
[30/360 termination](../core-conventions/003-thirty360.md#accepted-decision-synthetic-day-count-conventions),
[calendar data](../core-conventions/004-holiday-engine.md#accepted-decision-holiday-engine-and-data-boundaries),
[rolling](../core-conventions/006-business-day-adjustments.md#accepted-decision-rolling-and-joint-calendar-conventions),
[date arithmetic](../core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions),
[schedule grids](../core-conventions/008-schedule-generation.md#accepted-decision-schedule-grids-events-and-failure-boundaries)
and [single-slice cashflow metadata](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata).
The accepted formulas, grid rules and single-slice boundary are not reopened.
Point15's scoped clarification to core015's metadata endpoint wording for
explicitly adjusted accrual was accepted on2026-11-08 and recorded there in place.
Runtime/caller/host/schema reconciliation and independent qualification remain
pending; observed behavior is retained below.

1. **Keep the cashflows layer responsible for contractual period enrichment.**
   Core owns raw date grids, arithmetic, holiday data and day-count kernels.
   This layer binds an explicit grid to actual economic accrual, payment/reset
   dates and complete reusable coupon context. A successful period build does
   not approve a bond, swap, CDS, overnight observation regime or market-default
   recipe. Program-window grid selection belongs to record002; coupon amounts,
   PIK, replay, accrued-interest reporting and aggregation retain their owners.
   Do not silently select those economics through a date helper.

2. **Require complete explicit schedule conventions with checked state.**
   Retain ScheduleParams as the cashflow convention type. Require frequency,
   day count, stub, the same single Natural/EOM/QuarterlyIMM selector as core008,
   payment BDC and the accrual-adjustment choice. Remove end_of_month/RollRule
   combinations and hidden serde/default choices. Keep the already accepted
   operation defaults of zero extra payment lag and no reset series; Some(0)
   reset remains distinct from None. Use private guarded state, immutable get_*
   views and checked complete construction/restoration/updates; invalid updates
   change nothing. Validate date-independent controls then date-dependent
   anchors/context at execution. Missing required fields, unknown fields,
   unsupported frequencies and invalid direct/restored state error consistently.

3. **Remove all ten generic and market-named preset factories.** Delete
   quarterly_act360, semiannual_30360, annual_actact and the USD/EUR/GBP/JPY
   swap/bond/Treasury presets across current surfaces. Their labels do not
   establish complete or authoritative conventions, and selecting one currently
   hides several unrelated financial choices. Existing callers/examples must
   construct all conventions explicitly. Product owners may adopt their actual
   documented recipes in their own review; do not relocate these unqualified
   bundles unchanged, add a preset registry, retain aliases or infer choices
   from currency/product names. Their exported availability is acknowledged;
   removal is an explicit API decision, not a claim they are dead code.

4. **Remove generic CDS grid overrides and prior-roll accrual.** Inherit core008:
   delete CdsImm, effective_frequency/effective_stub and automatic quarterly/
   ShortBack substitution, prior-twentieth start changes and compiler clipping
   associated with that generic mode. Preserve the supplied start/end/frequency/
   stub. A CDS owner supplies actual accrual start, terminal date treatment and
   selected regime; Natural can express explicit quarterly twentieth anchors.
   A twentieth grid is not complete CDS protection/accrual/settlement semantics.
   No replacement generic CDS mode or compatibility alias is selected.

5. **Retain the original nominal grid and its period identities.** Delegate
   Natural/EOM/IMM dates and literal stub rules to accepted core008, passing
   explicit Unadjusted/no-event choices when only its raw grid is needed.
   Preserve original anchor, direction, frequency phase, grid indices and removed
   long-stub nominal boundaries in private compiled metadata. Classify regular
   coupons/stubs from that provenance, never from a local forward-OR-backward
   tenor step between already clamped endpoints. No regularity/frequency inference
   from adjusted duration or payment dates. EOM anchor rules apply to every
   day count. Retain the 100,000 final grid-date ceiling including endpoints;
   auxiliary reference/comparison dates are not final output dates, while their
   required arithmetic, progress and allocation remain checked.

6. **Resolve calendars for explicit roles without fallback.** Require an
   explicit payment/accrual schedule calendar whenever its BDC, lag or selected
   day count needs business-day queries, including Bus252. Absence is allowed
   only when those roles require no queries. A supplied ID always resolves
   strictly; identity operations need not classify unused dates. Preserve the
   existing shared schedule calendar/BDC for the selected accrual/payment recipe.
   Make the fixing calendar explicit when positive reset lag needs it in both
   standalone and builder paths; add that existing builder capability to the
   period request without a schedule-calendar fallback. Resolve supplied fixing
   IDs even at zero lag, without rolling an unused date. Required calendar data/
   coverage failures propagate. Context stores the actual selected calendar
   authority; partial queries cannot silently switch to another/current provider.

7. **Keep raw nominal and actual accrual boundaries separate.** With the
   explicit accrual-adjustment flag false, actual accrual equals raw boundaries;
   with true, apply the selected schedule BDC/calendar once to each shared anchor
   and share that result between neighboring periods. Preserve raw endpoints
   regardless. Require strictly increasing actual intervals and reject collapse/
   reversal instead of dropping or repairing periods. Within a declared continuous
   accrual leg, assembly must preserve exact economic coverage across adjacent
   windows; reject unintended gaps/overlaps without imposing a new global raw
   grid. Record002 owns intended program/leg coverage and independent windows.
   A product needing different terminal/intermediate adjustment rules supplies
   its explicit owner workflow; the boolean does not certify CDS conventions.

8. **Keep payment adjustment then nonnegative business-day lag.** For each raw
   period end e use shift_business_days(adjust(e,BDC),lag), with integer lag
   0..=i32::MAX, start-excluded counting and no extra post-lag roll. Zero keeps
   the adjusted end. Preceding can put payment before the raw accrual end; do
   not impose payment>=raw_end universally. Delete global unique-payment-date
   validation, including compiler-window checks. Distinct periods/components
   may share payment dates and retain their order/identity/context; no automatic
   merge, deduplication or changed principal timing. A deduplicated processing
   date queue must still visit every period/event on that date.

9. **Define this operation's reset anchor and identity explicitly.** Reset lag
   is measured from the period's actual contractual accrual_start. None omits
   the reset; Some(0) retains that start exactly, including a closed date. A
   positive integer through i32::MAX subtracts business days directly using the
   explicit fixing calendar, with no pre-roll or post-lag BDC. Remove the redundant
   BDC argument from compute_reset_date and reconcile every caller. Do not reuse
   the core raw-start fixing series when adjusted accrual changed the chosen
   anchor. Preserve duplicate resets with period identity. Require reset<=payment
   for the supported single-observation period/row; reject incompatible selected
   dates without moving them. In-arrears/multi-observation/index-publication
   eligibility remains with records006/007 and accepted0045, not this lag recipe.

10. **Compose ICMA from the complete actual nominal reference sequence.**
    Inherit core002's contained formula days(slice)/(F*days(reference)), with
    F=12/m only for admitted m=1,2,3,4,6,12-month nominal frequencies, with1Y
    equivalent to12M. Retain
    every required reference from the original grid, including outside a stub
    or adjusted coupon boundary; intersect the actual accrual request with those
    nominal intervals and cover it exactly once. Sum contained contributions;
    no gap/overlap, nearest-reference guess, local clamped stepping or core
    extrapolation. Long coupons and adjusted spillover may need several slices.
    Allow ICMA with the supported IMM intersection 3M/6M/12M using those actual
    nominal dates; remove the obsolete blanket IMM ban. A required reference
    outside the supported Date domain errors even if the raw coupon fits. This
    is one inherited formula across explicit grids, not product adoption.

11. **Use one ACT/365L authority and remove factor-based prorating.** Inherit
    core002's actual-days divided by366 when the original full contractual
    calculation period ends in a leap year, otherwise365, for every frequency.
    Preserve that actual full period across partials/balance segments/replay;
    payment year and a shortened reporting segment never choose its denominator.
    Remove prorated_actual_accrual and its two ACT365L-only production uses;
    evaluate each contained query directly with the retained full context.
    Remove replay admission that merely checks a stored factor implies either
    365 or366, and missing-context fallback to segment endpoints. Broader replay/
    accrued-interest economics remain with records008/013. Do not generalize
    actual-day linear allocation to ICMA, 30/360, OneOne or other nonadditive
    conventions; required contractual context cannot be reverse-engineered from
    a stored year fraction.

12. **Carry explicit true/false termination status where required.** Reuse
    accepted core003's optional end_is_termination_date context fact. Require
    explicit status for 30E/360 ISDA at the final actual accrual endpoint of the
    requested schedule/period; the owner supplies whether that endpoint is actual
    contractual termination. Ordinary interior endpoints are explicitly false.
    A partial slice receives true only when it reaches that owner-designated
    terminal actual endpoint. A program-window end, last available period,
    raw-end equality with params.end or delayed payment is not sufficient
    evidence. Reconcile standalone requests and the compiler's global maturity
    knowledge without a new termination-policy framework or default false.

13. **Return fully computed immutable periods with owned context.** Make
    SchedulePeriod private/validated with immutable get_* views and retained
    nominal/full-period/termination/calendar context. Require raw and actual
    start<end, supported event dates and finite nonnegative accrual fractions;
    computed zero is valid and never means uncomputed. Internal skeletal state
    cannot escape. Change period_accrual to take only that period and start/end,
    removing independently supplied conventions that can reinterpret it. Validate
    containment/context before equal-date zero; unsupported/missing context is
    an error. Keep no mutable aliases or public bypass. This type currently has
    no canonical serde/host state contract; do not invent a new restoration or
    persistence framework. Existing ScheduleParams wire paths remain guarded.

14. **Make the single-period helper obey the same grid contract.** Retain
    build_single_period as a convenience over the same validated build_periods
    path, succeeding only when that selected grid/stub produces exactly one
    actual coupon period. Remove independent enrichment that silently ignores
    interior boundaries or an incompatible None-stub/frequency choice. It is
    neither an explicit-list override nor a second schedule algorithm. A custom
    coupon outside this admitted grid needs its owner's explicit context path;
    it cannot be smuggled through ignored controls or inferred references. Keep
    no unchecked/legacy alternate helper or duplicated host date implementation.

15. **Preserve usable day-count metadata through emission and queries.**
    Reconcile period compilation, principal-balance splits, coupon emission,
    replay and accrued-interest consumers, not just the scalar full fraction.
    Accepted core015 permits one complete contained context per accrual slice.
    Split internal ICMA accrual contributions at nominal boundaries, retaining
    the actual original coupon/event/payment identity and stub label. Each slice
    carries its nominal pair; ACT365L slices retain the original full calculation
    period. Clarify core015's word "unadjusted": slice start/end describe the
    actual calculation endpoints, including explicitly adjusted accrual; raw
    nominal reference dates stay separate. This does not change the selected
    adjustment flag, day-count formula or single-context limit. The existing
    core015 record carries this accepted clarification in place.
    A composite cash row omits optional single-slice metadata when no one context
    truthfully represents it, retaining genuine calculation segments in the
    existing owner model as core015 requires. Queries needing them obtain that
    context or fail. No mandatory new cash rows, intermediate financial rounding
    or PIK/principal event dates arise from an internal context split. Amount
    construction/aggregation/rounding stay with records003/005/012 and accepted
    Money authority. Never emit one unusable reference or copy the whole coupon
    fraction onto every segment.

16. **Check numerical/resource failures without repair or partial schedules.**
    Count dates/days/indices with exact checked integers before floating division;
    sum ICMA contributions with accepted checked ordered accumulation and require
    finite nonnegative results. A zero fraction can be legitimate; downstream
    division may separately require positivity. Apply the core final-grid cap
    per generated grid and checked aggregate period/component/nominal-reference
    allocation/progress across the request, without a new financial horizon cap.
    Required date/context/data/resource errors propagate; no .ok()-as-regular,
    denominator floor, alternate day count, clipped query, fabricated zero or
    empty/partial successful output for a requested period/grid build. An absent
    coupon request in a deliberately couponless/principal-only program remains
    record002's separate case. Preserve accepted input/lookup/computation
    error meanings. Program compilation publishes only after all intended periods
    and context have passed; atomic financial state remains with its owner.

17. **Reconcile actual product and host consumers in one later slice.** Update
    ScheduleParams/BuildPeriodsParams/grid/reset/period-query signatures and all
    Rust tests/examples, existing Python/WASM schedule factories/constructors,
    schemas/declarations/facades and parity ledger. Remove preset/RollRule/CDS/
    legacy field spellings without aliases. Reconcile actual bond/IRS/term-loan/
    TRS/CDS/tranche/calibration/fee consumers and single-reset cashflow admission,
    preserving their separately reviewed product recipes. No new host period
    class, binding math, preset registry or automatic convention migration.
    Record002 still owns program precedence/cuts, record005 coupon calculations,
    records006/007 index observations, record008 replay and record013 reporting.

18. **Qualify original grids, context and caller handoffs independently.**
    Cover all explicit core modes/stubs, month-clamp phase, long-stub nominal
    boundaries, adjusted spillover, date-domain edges, collisions, reset-zero/
    fixing-calendar differences, legitimate zero fractions and terminal-window
    distinctions. Require complete nominal-slice and actual-period metadata in
    partial/replay/emitted views, exact resource/error behavior and all existing
    host parity paths. Independent date/fraction examples and primary-reference
    review are not vendor comparison, executed production tests or certification
    of the removed presets. This review changes no runtime, test, generator,
    fixture, schema, host or financial default in code.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Natural1M ShortBack Jan30→Mar15,2025 | Nominal Jan30/Feb28/Mar30; final stub uses15/(12×30), not a local Mar28 reference. |
| Natural1M ShortBack Jan31→Mar30,2025 | Feb28→Mar30 remains a stub;30/(12×31), not1/12. |
| Natural1M ShortFront Jan31→Apr30,2025 | First stub Jan31→Feb28 uses28/(12×29), retaining backward Apr30 phase. |
| Long first Nov15,2024→Jul15,2025 with Jan15/Jul15 nominal phase |61/(2×184)+1/2, with two complete contained contexts. |
| ICMA nominal Jul4,2025→Jan4,2026, actual end Jan5 |1/2+1/(2×181), using the next nominal interval for the extra day. |
| Daily May2→May5,2025; weekends-only Following, raw accrual | Three positive periods may all pay May5; retain all identities. |
| Same daily schedule with adjusted accrual | Collapsed actual intervals error; do not remove them. |
| ACT365L Jun30→Dec31,2024; payment Jan2,2025 |184/366; any partial keeps the same full-period denominator. |
| 30E/360 Jan30→Jan31 | Computed zero is a valid result, not missing/uncomputed state. |
| Reset None / Some(0) at a closed actual accrual start | Omitted / exactly that start, subject to supported reset<=payment admission. |
| ThirtyE360Isda ending at an interior window | Explicit ordinary-end status; never infer termination from the local window end. |
| Single-period request whose canonical grid contains several coupons | Error; no ignored interior boundaries. |

## Observed algorithm and conventions

1. Require start before end and nonnegative payment/reset business-day lags. Resolve calendars strictly; unknown identifiers fail. `CdsImm` overrides requested frequency with quarterly and requested stub with short-back. Other rolls preserve the requested tenor and stub.
2. Delegate anchor generation to `Schedule::builder`. Front stubs anchor backward from the end; back stubs anchor forward from the start. Apply EOM to the nominal grid. Reject EOM with IMM/CDS IMM. Reject ACT/ACT ICMA with third-Wednesday IMM; ICMA EOM requires a month-end regular-grid anchor.
3. Business-day-adjust each shared anchor once. With `adjust_accrual_dates=false`, keep raw accrual endpoints; with `true`, use adjusted endpoints and reject collapsed/reversed intervals. Cash payment is the adjusted end plus `payment_lag_days` business days. Lags apply after business-day adjustment. Distinct periods may not have the same payment date, including across program windows.
4. Preserve raw endpoints as `unadjusted_start/end`. Stub classification uses those raw endpoints, not holiday-shifted durations: plain periods accept either a forward or backward tenor step; IMM requires valid IMM dates separated by the tenor's months; CDS requires consecutive CDS dates.
5. Reset with zero lag is exactly accrual start, without an additional adjustment. Positive lag subtracts business days from accrual start using the fixing calendar, then applies the specified BDC. The standalone period API uses its schedule calendar for resets; floating emission can use a distinct fixing calendar.
6. Build day-count context with frequency, calendar, reference coupon period and termination flag. ICMA regular periods use their raw period; back stubs use the next nominal coupon, front stubs the preceding one; CDS uses the surrounding quarterly twentieth dates. ACT/365L uses the full actual coupon interval to select its denominator. Partial accrual retains that full-period context. ThirtyE/360 ISDA's termination flag applies only when the subinterval reaches the actual terminal accrual endpoint.

Rates and year fractions are decimal; payment and reset lags count business days. `period_accrual` rejects subintervals outside their contractual period. `prorated_actual_accrual` computes `full_yf × subinterval_actual_days / full_period_actual_days` and validates finite nonnegative fractions.

## Observed selection and controls

`build_periods`, `build_single_period` and the builder compiler share these rules. Schedule constructors default to short-front, no EOM and no payment lag. The bond presets leave accrual dates unadjusted; swap presets adjust them. The inspected preset table is:

| Preset | Tenor / day count | BDC / calendar | Payment lag |
|---|---|---|---:|
| Generic quarterly | 3M / ACT/360 | Modified Following / weekends_only | 0 |
| Generic semiannual | 6M / 30/360 | Modified Following / weekends_only | 0 |
| Generic annual | 1Y / ACT/ACT | Following / weekends_only | 0 |
| USD SOFR swap | 3M / ACT/360 | Modified Following / usny | 2 |
| USD corporate bond | 6M / 30/360 | Following / usny | 0 |
| USD Treasury | 6M / ACT/ACT ICMA | Following / usny | 0 |
| EUR €STR swap | 1Y / ACT/360 | Modified Following / target2 | 2 |
| EUR government bond | 1Y / ACT/ACT ICMA | Following / target2 | 0 |
| GBP SONIA swap | 1Y / ACT/365F | Modified Following / gblo | 0 |
| JPY TONA swap | 1Y / ACT/365F | Modified Following / jpto | 2 |

## Observed limitations and alternatives

`ScheduleParams::validate` checks calendar and payment lag only; full grid/context validation happens during compilation. `build_single_period` enriches the supplied single interval rather than constructing a multi-period IMM/CDS schedule. Negative payment lags and coincident adjusted payments are rejected rather than combined. Possible alternatives, not implemented here, include merging coincident payments and preserving one global grid through program switches; the latter is explicitly different from [CASHFLOWS-002](02-program-compilation.md).

The local forward-OR-backward regularity test and adjacent-reference construction
lose original nominal phase after month clamping. Current ICMA context is one
pair and relies on a core extrapolation path removed by accepted002. ACT365L
replay can admit either implied365/366 denominator without proving the original
full-period end-year. Standalone period queries receive fresh independent params
and infer termination from the local end, while compiler emission separately
tracks actual maturity. SchedulePeriod fields are public and0 marks uncomputed
fractions even though valid conventions can compute zero. Required caller/context
reconciliation is distinct from broader unreviewed program/coupon/replay policy.

## Evidence

- Owning algorithms: [date_generation.rs](../../../../finstack-quant/cashflows/src/builder/date_generation.rs), [periods.rs](../../../../finstack-quant/cashflows/src/builder/periods.rs), [schedule presets](../../../../finstack-quant/cashflows/src/builder/specs/schedule.rs).
- Located regression/invariant tests: `adjusted_accrual_boundaries_are_shared_between_consecutive_periods`, `payment_lag_applies_after_business_day_adjustment`, `explicit_and_generated_periods_reject_collapsed_accrual_bounds` in the date-generation source; `short_front_icma_stub_uses_ending_regular_coupon` in periods; [roll-rule integration tests](../../../../finstack-quant/cashflows/tests/cashflows/builder/roll_rule.rs).
- Repository reference anchors: [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions), [ICMA Rule Book](../../../REFERENCES.md#icma-rule-book). These are bibliographic pointers, not a checked clause-to-implementation conformance certificate.
- No market-vendor comparison was executed. The source-level branch inspection is complete for this scope.
- Actual [compiler](../../../../finstack-quant/cashflows/src/builder/compiler.rs),
  [coupon emission](../../../../finstack-quant/cashflows/src/builder/emission/coupons.rs),
  [floating replay](../../../../finstack-quant/cashflows/src/builder/floating_replay.rs)
  and [accrued-interest context](../../../../finstack-quant/cashflows/src/accrual.rs)
  identify the single-reference/full-period handoffs. The two production
  prorated_actual_accrual calls are ACT365L-only. No production Rust/host tests,
  builds, generators or reference-vendor runs were executed.
- [ISO15022 A006/A009](https://www.iso20022.org/15022/uhb/mt527-33-field-22a.htm)
  and the [official Strata ICMA description](https://strata.opengamma.io/day_counts/#actualactual-icma)
  provide nominal-period context; accepted core002 already selects the project's
  exact formulas and ACT365L variant. The historically dated
  [March4,2009 North American CDS specification](https://www.cdsmodel.com/assets/cds-model/docs/Standard%20CDS%20Contract%20Specification.pdf)
  distinguishes adjusted intermediate accrual, unadjusted maturity and terminal
  inclusion, showing why a twentieth grid alone is not a full CDS recipe. These
  references do not approve any of the ten source preset bundles or every product.
- Independent **2026-11-08** standard-library Python date/Fraction checks passed
  for original-anchor clamping/phase, all three short-stub fractions, long-coupon
  and adjusted-spill references, full-context365L partitioning, three coincident
  weekend payments versus collapsed adjusted accrual, zero/positive reset and
  terminal-February distinctions. Four read-only source/caller/reference reviews
  checked the staged package. These are bounded reference examples, not executed
  production algorithms, vendor fixtures or contract-level qualification.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
