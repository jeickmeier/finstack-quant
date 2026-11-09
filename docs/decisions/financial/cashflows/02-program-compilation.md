# CASHFLOWS-002 — Coupon and payment program compilation

- Decision status: **Accepted — complete coupon/payment-program compilation package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, scoped below.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded program/date examples and read-only compiler/API/host/reference reviews are reported below; these do not qualify production compilation, market observations or financial product contracts.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice, rationale and domain

The builder resolves coupon/payment programs into independent adjacent windows across the issue-to-maturity horizon. This supports fixed, floating, fixed-to-floating, margin-step, fixed step-up, cash, PIK and split coupons. Source comments explicitly warn that window boundaries change the generated coupon grid. This is a deliberate implementation rule, not evidence that an off-cycle contract should adopt it.

## Accepted decision: coupon and payment program compilation

Accepted on **2026-11-08** after whole-record review. Inherit accepted
[001's period/grid/context contract](01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context),
[exact Money authority](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity),
[cashflow metadata](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata),
[financial clocks](../conventions/0045-financial-clocks-and-date-boundaries.md)
and [bounded strict admission](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics).
Runtime/caller/host/schema reconciliation and independent qualification remain
pending; observed source behavior is retained separately below.

1. **Adopt one explicit independent-window compilation contract.** Every
   supplied coupon or payment-window boundary is a contractual schedule cut.
   Sort/deduplicate issue, maturity and those boundaries; each adjacent atomic
   interval receives its own accepted001 grid using the selected coupon's
   conventions. Do not continue a global original roll across a cut. Even a
   same-value Cash-on-Cash override or equal-rate adjacent window remains active
   and can move earlier/later coupon dates, nominal references, fixed-step
   activation and owner-selected amortization cadence. Do not coalesce it away.
   This is the chosen program operator, not a universal amendment/PIK-toggle
   convention. An amount-only amendment preserving original coupons needs its
   actual owner workflow; merely aligning dates does not prove clamped nominal
   phase/reference identity. No second global-grid or pro-rata mode is added.

2. **Replace sticky deferred mutation errors with fallible atomic updates.**
   CashFlowBuilder remains an incomplete instruction collector, but configuration
   methods return Result and validate their local payload/order before changing
   state. Remove pending_error/first-error poisoning and no-op-after-error paths.
   Compound helpers prepare/validate/check capacity for the complete addition,
   then commit once; rejection leaves prior instructions unchanged. principal()
   validates/replaces its own amount/dates atomically without erasing programs.
   Keep coupon-before-principal and principal-before-coupon setup: defer only
   checks requiring the final horizon or complete program until build. build
   takes a shared view and does not rewrite input state. A caller must handle
   each rejected update; there is no hidden error history, reset method or old
   fluent compatibility wrapper.

3. **Resolve exact half-open windows against the final horizon.** Require
   issue<maturity. Windows use unadjusted contractual dates, not delayed cash
   settlement dates or as_of. Symbolic Issue/Maturity bounds resolve at build; explicit
   dates remain literal if principal dates are replaced. Every active coupon/
   payment window must be positive and contained in [issue,maturity]. Reject
   reversed, out-of-range and explicitly empty windows without clipping.
   Only helper-created Issue→issue heads and maturity→Maturity tails may be
   omitted, after their supplied payloads pass validation. Dated helper steps/
   switches may equal either horizon endpoint, with those literal empty-edge
   rules; repeated/decreasing dates and dates outside the closed horizon error.
   These rules do not constrain separately owned pre-issue principal/fee events.

4. **Validate every supplied instruction before selection or omission.**
   Validate local types, exact rate/spread units, full schedule choices, split
   payloads and ordered steps before dropping empty helper edges, selecting
   overrides or ignoring unused rate steps. A shadowed/unused invalid payload
   is not harmless. Resolve provided calendar IDs strictly, while querying
   calendar coverage and market observations only for actual required dates.
   Delegate split conservation/tolerances and signed PIK economics to005/008;
   this package does not adopt the current1e-9/1e-12 discrepancy. An incomplete
   raw builder is not a validated contract. Complete specs, program compilation
   and consuming boundaries must enforce the same adopted requirements.

5. **Require exact single-coupon coverage or an explicit couponless program.**
   Across the horizon, every atomic interval must have exactly one coupon
   definition. Reject gaps, overlaps, nested/identical coupon definitions and
   duplicate full-horizon legs even if their payloads match. Sort for validation,
   without input-order economic priority or silent deduplication. Couponless/
   principal-only or fee-only programs remain valid when no active coupon or
   payment windows remain after payload validation, symbolic resolution and only
   the permitted helper empty-edge omission. Principal and fee emission still
   follow their owners. Active payment instructions without coupon coverage error.
   A requested coupon leg cannot disappear as an empty successful build.

6. **Validate the entire payment-window family before choosing winners.**
   Permit disjoint/touching half-open windows and proper containment, including
   one shared endpoint; reject identical windows and every crossing partial
   overlap, even if a deeper window shadows the full intersection or all splits
   match. Validate globally with a deterministic sorted interval/stack pass or
   equivalent complete method, not only against the current winner. For valid
   windows the unique innermost containing window supplies the split; outside
   overrides use that coupon's explicit base split. Input order cannot change
   valid economics or hide invalid crossing windows. No last-wins tie, priorities,
   repair or generic interval framework.

7. **Revalidate each atomic grid and preserve its whole context.** Each cut
   must satisfy that interval's selected001/core stub and Natural/EOM/IMM anchor
   rules. A Natural off-cycle short stub may be valid while the same cut makes
   an EOM/IMM regular anchor or None-stub grid invalid; fail without snapping,
   changing stubs, adding both-end stubs or borrowing a neighboring/global grid.
   Preserve each window/period's actual nominal references, raw/adjusted endpoints,
   explicit termination facts and immutable context. Remove inherited CDS front-
   accrual extension/clipping and frequency/stub overrides. Enforce declared
   continuous-leg actual coverage and retain legitimate shared payment/reset
   dates; no global date uniqueness check or collapsed-period removal.

8. **Define fixed step-up as raw-period-start rate selection.** Retain explicit
   initial_rate and strictly increasing step_schedule. For each generated raw
   period start s, select the latest step with date<=s, otherwise initial_rate.
   An off-cycle step does not itself cut or prorate that period and may have no
   effect before maturity. Validate every date/rate, including unused entries,
   within the closed instrument horizon. Rates are exact annual decimal quotes;
   do not infer percentages, monotonic increases or nonnegative cash rates from
   the method's name. PIK/sign constraints remain005-owned. Equal selected-rate
   grouping is an internal optimization that preserves periods/context and
   every contractual cut. Remove the unsupported universal step-up-market claim.
   Another supplied window cut can change starts and therefore step activation.

9. **Keep floating margin and fixed-to-floating switches effective from date.**
   float_margin_steps uses the explicit base spread until the first date, then
   each supplied decimal-bp spread from its date to the next, finally maturity.
   Each change creates coupon windows and therefore independent grids under1;
   it is not the fixed-step operator in8. Empty steps mean one full-horizon base
   floating definition. fixed_to_float creates explicit fixed-before/floating-
   from windows, with its two complete specs and the endpoint rules in3.
   Validate both specs even if one has no active interval. Make MarginStepSpec's
   wire/member name from_date rather than ambiguous date; reconcile all hosts.
   No schedule phase, reset history, fixing eligibility or rate is interpolated
   automatically across a switch. Projection/observation economics remain006/007.

10. **Make payment-step UNTIL semantics explicit and remove the Cash tail.**
    Rename payment_split_program to payment_splits_until and PaymentStepSpec.date
    to until, without aliases. Each pair supplies its split over the interval
    from the prior boundary (issue initially) up to that date, exclusively.
    Selection follows raw accrual/program intervals: a coupon ending at until
    receives the left split even if its cash payment occurs on/after that date.
    After the last supplied date, add no implicit override: each remaining
    coupon uses its own explicitly chosen base split. Empty steps add nothing.
    A Cash tail requires an explicit Cash window/step covering it. Repeated or
    decreasing dates fail, endpoint empty heads follow3, and all surviving
    payment boundaries still cut/rebuild grids under1. This is distinct from
    margin effective-FROM semantics, not an enum/host-specific default.

11. **Require explicit financial base settlement and principal exchange.**
    Fixed, floating and step-up coupon definitions must supply Cash/Pik/Split;
    remove CouponType Default and serde/host omitted-as-Cash behavior. Require
    PrincipalExchange explicitly for the build, removing InitialAndFinal Default
    and serialization omission of that choice. None is deliberate, not inferred
    from an IRS/product name; definitions/emission signs and principal ordering
    remain003-owned. Empty instruction-list defaults remain literal no instructions.
    Other unreviewed rate/fee controls are not approved by these two selections;
    their owning006/007/009 packages still determine their supported defaults.

12. **Transport an explicit observation cutoff through one build path.**
    Use one canonical build operation accepting optional market and optional
    owner-supplied as_of, propagated identically from fluent, typed build-spec
    and JSON/host paths. Any active floating definition requires as_of; purely
    deterministic fixed builds need none and do not query unused market data.
    Remove build_as_of/alternate behavior and curve-base-date cutoff inference;
    do not add a new build-context framework. as_of does not clip the contractual
    horizon or prove intraday/publication eligibility. The precise same-day,
    historical/future fixing and fallback rules remain006/007/0045-owned, not
    newly adopted here. Inactive validated specs need no observation queries.

13. **Preserve actual calendar and market failures.** Apply accepted001's
    explicit fixing-calendar requirement and no schedule-calendar fallback.
    Resolve only market data actually required by active instructions under
   006/007, preserving missing versus invalid/unsupported/computation/poison
    causes. Remove .ok() conversions that erase a real source failure before
    fallback. Supplying an explicit fallback does not authorize swallowing every
    error; eligibility remains its owner's rule. Freeze actual resolved inputs/
    calendar context under existing ownership for one build; no live current-data
    substitution, implied FX usage or new market snapshot/validator registry.

14. **Keep fee and principal processing roles separate from coupon cuts.**
    Compile fee periods over their explicit owner horizon/conventions separately;
    fee dates and principal events do not create coupon-program cuts merely by
    entering the processing date queue. That queue may deduplicate dates but
    must visit all coincident coupon/fee/principal events. The selected coupon
    boundaries are exposed faithfully to any owner choosing them for amortization;
    this package does not approve its installment counts, same-day funding/PIK
    order, fee pre/post-issue eligibility or final redemption timing. Those remain
   003/004/005/009-owned. Representative schedule day-count/reporting metadata
    and couponless defaults remain012-owned; period001 context is authoritative.

15. **Preserve exact Money after ingress throughout the shared build path.**
    Inherit012: initial principal, explicit principal events, state initialization
    and returned redemption must not detour through amount() f64 views or a float-
    reconstructed Money. Use stored Decimal/currency and checked accepted monetary
    arithmetic; no display-zero suppression, implicit rounding or FX. Keep explicit
    economic delta versus cash under core015. Actual principal lifecycle/PIK/
    amortization formulas remain003–005; calling the common compiler does not
    adopt their observed alternatives or certify its current monetary accuracy.

16. **Use one strict specification-to-builder authority.** Keep typed enum
    dispatch into the same Rust configuration/compile/project path as fluent
    calls. Guard complete spec construction/restoration/publication; reject
    unknown fields, duplicates, noncanonical dates, lossy Decimal/numeric inputs
    and malformed flattened specs under accepted024/025. Validate original JSON
    before Value materialization loses information. A schema pass does not run
    financial semantics or make mutable raw fields safe. Keep active inputs,
    selection and qualified producer metadata honest; no compatibility parser,
    separate host compiler, wire-default recovery or alternate financial path.

17. **Bound work and return a complete schedule or error.** Check cut-count,
    aggregate window/period/nominal-reference/event allocations and integer
    arithmetic before reserving/appending; use available fallible reservations.
    Retain001's per-grid final-date ceiling without treating auxiliary dates as
    output or selecting a new financial horizon cap. Validate payloads in supplied
    instruction order, then geometry/selection by deterministic sorted boundaries;
    valid results do not depend on incidental map/instruction ordering. Publish
    only a fully admitted local plan/result, never a repaired gap, partial schedule
    or success after a swallowed error. Failed builds leave builder instructions
    unchanged. External provider/cache observations are not rolled back; equivalent
    frozen inputs, not merely repeated live requests, define reproducibility.

18. **Reconcile current APIs and independently qualify the chosen operator.**
    Update all fallible builder/helper calls, canonical build/cutoff signatures,
    required settlement/exchange fields, UNTIL/FROM names, strict specs, actual
    compiler/projector callers and existing Python/WASM/stubs/schema/declarations/
    facade/parity/examples together. No old pending-error/build_as_of/helper/field
    aliases or new host classes/math. Independently qualify raw re-anchoring and
    nominal phase, redundant cuts, order-independent nesting/crossing rejection,
    payload-before-shadow validation, helper endpoints/tails, period-start step
    sampling, exact-money/cutoff/error transport and complete-output behavior.
    Reference examples/source reviews are not executed production, provider replay
    or market-product qualification. This review changes no runtime, tests,
    generators, schemas, fixtures, hosts or financial defaults in code.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Quarterly Natural/ShortFront Jan15,2025→Jan15,2026, no cuts | Raw ends Apr15/Jul15/Oct15/Jan15. |
| Same input with supplied Cash-on-Cash payment cut May1 | Raw windows Jan15/Feb1/May1 then May1/Jul15/Oct15/Jan15; Apr15 is replaced even though split values match. |
| Quarterly EOM/ShortFront Jan31,2025→Jan31,2026 with May1 cut | Revalidate the left atomic anchor; reject its off-EOM regular end, without changing the grid/stub. |
| Crossing A=[Jan1,Jul1), B=[Apr1,nextJan1), masked by C=[Apr1,Jul1) | Reject all insertion orders before winner selection. |
| Equal payment windows with equal or different payloads | Reject duplicate windows; no last-wins rule. |
| Properly nested override, supplied in any order | Same unique innermost split; outside uses the coupon's explicit base split. |
| Empty payment_splits_until on a base PIK coupon | No overrides; PIK remains, with no added cut. |
| UNTIL(d,PIK), then no further override | PIK on [issue,d); remainder uses each coupon's explicit base split. |
| Coupon accrues up to UNTIL(d,PIK), cash settles after d | Left-window PIK selection remains; do not filter by cash payment date. |
| No coupons, only a valid UNTIL(issue,PIK) empty helper head | Omit that head after validation; no active coupon/payment instruction remains. |
| Fixed step May1 inside an uncut Apr15→Jul15 raw period | Old rate for that period; new rate begins at the first generated raw start>=May1. |
| A supplied window cut creates a raw May1 start | That fixed step now activates May1; the cut changes both dates and rate exposure. |
| Margin FROM(d,new_spread) / payment UNTIL(d,split) | New margin on [d,next); supplied payment split on [prior,d). |
| Switch exactly at issue/maturity | Validate both specs, omit only the helper-generated empty edge; retain the active full-horizon leg. |
| Invalid payload on a shadowed or dropped interval | Error before omission/selection; no hidden invalid configuration. |
| No coupon/payment instructions | Valid couponless program, subject to separately owned principal/fee admission. |
| Active floating program with absent as_of | Input error; no curve-base knowledge-date inference. |

## Observed algorithm

1. Builder methods accumulate instructions; the first deferred error remains authoritative. `principal()` replaces principal/dates without clearing that error. Full-horizon bounds resolve only during build, allowing principal and coupon setup in either order.
2. Resolve every window and discard only the special empty issue/maturity edge pieces. Require all remaining windows to be positive and inside `[issue,maturity]`.
3. Sort and deduplicate issue, maturity, every coupon boundary and every payment boundary. For each adjacent interval, require exactly one coupon piece covering the whole interval. Missing coverage or overlapping coupon pieces fail.
4. Select the narrowest currently encountered containing payment window, otherwise the coupon's own split. Partial-overlap checking compares only with that current winner: an innermost window can mask crossing outer windows, so rejection can depend on insertion order. Equal windows follow the containment branch and a later supplied matching window replaces the earlier one. No distinct economic rationale for this tie rule was located.
5. Generate a fresh schedule for each interval using [CASHFLOWS-001](01-schedule-periods.md). A short-front off-cycle cut changes earlier payment anchors in its window. Only the initial CDS window may extend front accrual before issue; subsequent windows clip the first accrual start at the switch to avoid double accrual.
6. Fixed step-up rates use the latest step date `<= unadjusted period start`. Step dates must strictly increase. An off-cycle rate step waits until the next period start and does not split that period. Adjacent equal-rate periods are grouped. Payment-program cuts still partition the horizon before this grouping.
7. Floating windows resolve the fixing calendar override or inherit their payment calendar; convert and validate rate controls. Check duplicate payment dates across the complete coupon program.

`float_margin_steps` uses the base spread until the first date, then each new spread from its date. `payment_split_program` is different: each supplied split runs **until** its date; cash covers the final remainder. Both reject repeated or decreasing dates. `fixed_to_float` explicitly splits at the switch date. An empty coupon and payment program produces principal-only cashflows; a payment program without coupon coverage errors.

## Observed selection, wire defaults and failures

`CashflowScheduleBuildSpec::build` dispatches every JSON leg/program variant to the same Rust builder, then calls `build(market)`. The JSON entry point has no explicit observation-date input; curve base dates supply fixing cutoffs. Principal exchange defaults to initial-and-final, lists to empty, and coupon split to cash. Unknown JSON fields are rejected, including flattened coupon input guards. Schema generation is a structural description and does not execute every economic validation.

Fee periods compile separately across the whole horizon; they do not cut coupon windows. Fixed fees after raw maturity are rejected; fees before issue are permitted. See [CASHFLOWS-009](09-fees.md).

## Observed alternatives

Supported alternatives are explicit coupon windows, fixed step-up on a retained grid, and floating-margin windows. A global anchored coupon grid with pro-rata intra-period rate changes is not the implemented window model.

## Evidence

- [compiler.rs](../../../../finstack-quant/cashflows/src/builder/compiler.rs), [coupon_api.rs](../../../../finstack-quant/cashflows/src/builder/coupon_api.rs), [json.rs](../../../../finstack-quant/cashflows/src/json.rs).
- Compiler unit tests locate default cash, nested override, partial-overlap rejection and out-of-horizon rejection. [Compiler regressions](../../../../finstack-quant/cashflows/tests/cashflows/builder/compiler_regressions.rs) locate `step_up_uses_contractual_start_under_both_adjustment_directions`; [principal program tests](../../../../finstack-quant/cashflows/tests/cashflows/builder/principal_events.rs) locate margin-step order/boundary cases. These are implementation regression evidence, not independently sourced contract examples.
- [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions) supplies a general schedule-convention reference; no external standard establishing this program-partition policy was identified in the owned source.
- Actual [builder lifecycle](../../../../finstack-quant/cashflows/src/builder/orchestrator.rs),
  [projection/state handoffs](../../../../finstack-quant/cashflows/src/builder/pipeline.rs)
  and [coupon specifications](../../../../finstack-quant/cashflows/src/builder/specs/coupon.rs)
  identify sticky errors, exact-money detours, required cutoff transport and
  unadopted split defaults. Current JSON has no observation-date argument, and
  payment_split_program([]) appends a full-horizon Cash override. Located tests
  exercise branches/fixtures, **not executed** for this review.
- The [2006 ISDA Definitions](https://www.sc.com/en/uploads/sites/66/content/docs/2006-ISDA-Definitions.pdf),
  sections4.9–4.13, distinguish payment, period-end, business-day adjustment and
  calculation-period roles. [Official Strata schedule documentation](https://strata.opengamma.io/schedules/)
  describes explicit periodic rules/stubs. Neither supplies authority for this
  application's window regeneration, nested precedence, helper-tail or sampled-
  step contract. Those are the explicit project choices proposed above, not
  claimed universal market conventions.

- Independent **2026-11-08** standard-library date/Decimal checks passed for
  the uncut and May1-cut original-anchor grids, changed fixed-step activation,
  aligned-cut nominal-phase differences, six permutations of masked crossing
  windows, 24 valid nesting permutations with invariant innermost winners,
  duplicate/touching intervals, an off-EOM cut and exact-money float loss. The
  interval reference checks compare all pairs, independently of a future sorted
  stack implementation. Four read-only source/caller/reference reviews checked
  the complete staged package. These are bounded accepted targets, not production
  compiler/provider/state execution or financial-product qualification.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
