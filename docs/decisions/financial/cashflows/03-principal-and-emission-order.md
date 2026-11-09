# CASHFLOWS-003 — Principal events and date-by-date emission order

- Decision status: **Accepted — complete principal-event and emission-order package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, scoped below.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded exact-arithmetic/date examples and read-only source/consumer reviews are reported below; these do not qualify production execution or financial product contracts.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and domain

The builder tracks principal in `Decimal` by economic effective date and emits independently dated cash settlements. Nonnegative opening notional, deterministic draws/repayments, cash amounts different from par changes, pre-issue movements and PIK capitalization are supported. Decimal balance arithmetic limits accumulation drift; no benchmark for the source comment's quantitative drift claim was inspected.

## Accepted decision: dated opening principal, signed cash and atomic event phases

Accepted on **2026-11-08** after complete whole-record review. Runtime/caller/host/schema reconciliation and independent qualification remain pending.
Inherit accepted [001's period/context rules](01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context),
[002's explicit programs and atomic configuration](02-program-compilation.md#accepted-decision-coupon-and-payment-program-compilation),
[exact Money arithmetic](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity),
[core015's cash/principal/date meanings](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata)
and [financial clocks](../conventions/0045-financial-clocks-and-date-boundaries.md).
The stage order and automatic par exchanges below are selected builder operators,
not universal loan, bond, swap or CDS conventions. No runtime is changed here.

The owner installment slot was clarified on **2026-11-08** with
[accepted004](04-amortization.md#accepted-decision-explicit-amortization-laws-live-balance-installments-and-monetary-boundaries):
LinearBetween is stateless, so no installment initialization/update phase remains.
The relative order of coupon, PIK, principal, amortization, fee and redemption
phases is unchanged; the observed source pipeline below is not rewritten.

Generated-coupon component wording was clarified on **2026-11-08** with
[accepted005](05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization):
one parent coupon supplies one admitted nonnegative PIK component; signed
internal calculation terms remain in its owner model rather than becoming
separate principal events. Independently supplied signed003 events are unchanged.

1. **Keep one exact nonnegative outstanding-principal stock.** Initial stock is
   same-currency exact Money, including zero; balances and principal effects use
   its stored Decimal authority throughout. No amount() binary64 round trip,
   display-zero rule, implicit FX, minor-unit rounding, epsilon repair or balance
   floor. Reject a negative initial stock. Cash is signed from the holder/entity
   perspective under0044; changing position exposure does not negate the tracked
   stock or its principal deltas. Signed positions remain with the owning caller.

2. **Require an explicit dated opening balance.** Add required opening_date to
   the canonical principal/build inputs. initial means the stock immediately
   BEFORE all modeled movements on opening_date, not issue-closing stock. This
   is a logical before-event value, not a constructed prior calendar date, so
   the minimum supported Date remains usable. Require
   opening_date<=issue<maturity and opening_date no later than every principal
   economic date and every actual accrual/history interval start required by the
   compiled coupons/fees. The caller asserts that opening stock and supplies all
   subsequent modeled activity. Do not infer its effective date from issue, the
   earliest event, a curve base, cash settlement or a generated coupon. Missing
   opening_date or missing earlier history errors; no default/backfill mode.
   This preserves explicitly supported pre-issue activity and001's legitimately
   adjusted initial accrual dates without claiming unknown historical exposure.

3. **Use one covered economic history before and after issue.** Starting at
   opening_date, process every modeled economic date in ascending order, including
   pre-issue events and any required pre-issue coupon/fee phases, exactly once.
   Apply opening-date events after the declared opening stock. Commit one closing
   balance per economic date; that balance applies on [date,next_date). A coupon
   over [start,end) uses the balance after all start-date phases and excludes
   end-date changes. Coupon generation itself remains001/002-owned. A history
   request before opening_date, a gap in required coverage or malformed history
   errors. Remove balance_segments' live-outstanding fallback and the separate
   on/before-issue replay that can diverge from later phases. A mere earlier cash
   settlement does not require earlier principal history or move stock.

4. **Require actual signed settlement cash on every explicit principal event.**
   Keep PrincipalEventSpec with explicit economic date, payment_date, delta,
   cash and kind, but make cash required Money. For cash-capable kinds emit that
   cash unchanged; positive means received and negative means paid. Cash and
   delta must match the opening currency. Exact zero is valid. Remove Option
   cash, resolved_cash/par inference and classification-dependent sign flipping.
   A discounted draw can explicitly supply cash=-98 and delta=+100 from zero;
   repayment cash=102 with delta=-100 is equally literal. Payment may precede,
   equal or follow the economic date; no generic lag/BDC/order is inferred.

5. **Admit only core015 principal-capable kinds without guessing effects.**
   Permit Notional, Amortization, PrePayment, RevolvingRepayment, RevolvingDraw,
   Pik and DefaultedNotional for this event input; reject coupon/fee/recovery/
   protection/margin kinds rather than hiding stock movements on them. Every row
   carries its explicit signed delta and economic date, including zero. Pik and
   DefaultedNotional require cash exactly zero and payment_date equal to economic
   date. Allow signed corrections; do not infer delta sign or amount from kind.
   Ordinary generated amortization, PIK and write-down rules retain their owners.
   The builder's phase-level nonnegative-stock checks still apply. Preserve
   explicitly supplied zero-cash/zero-delta rows instead of deleting them.

6. **Select the terminal economic/payment dates as one pair.** With active
   coupons require the unique compiled period whose unadjusted end is raw
   maturity; take its actual accrual_end and payment_date together. Missing or
   ambiguous terminal periods error; do not independently maximize dates from
   different periods or silently fall back when an expected coupon is absent.
   For002's deliberate couponless program, select (maturity,maturity). This is
   the builder's explicit generic redemption recipe. Principal economic dates
   must lie in [opening_date,terminal economic date]; the actual terminal date
   alone is the upper bound, including when adjustment moves it before/after raw
   maturity. Remove the additional raw-maturity cap on explicit events. Settlement
   dates are not bounded by that economic interval. Scheduled amortization's
   admitted issue/grid dates remain004-owned and cannot exceed the terminal
   economic boundary. A terminal pair incompatible with required opening/history
   coverage errors without clipping dates or inventing exposure.

7. **Apply explicit same-date principal movements as one exact atomic batch.**
   After that date's admitted generated PIK phase, compute the exact candidate
   B+sum(delta) for ALL explicit events at that economic date and require a
   representable nonnegative final stock. Date-only events declare no intraday
   sequence: insertion order, kind rank, cash date and canonical output sort do
   not select insolvency or interest. For example B=50, delta=-75 and +50 closes
   at25 in either order. Preserve every event row and settlement separately;
   this is stock aggregation, not cash netting or event deduplication. If a
   product requires intraday/prefix constraints it must use its owner model;
   no optional sequencing framework is added to this date-only builder.

8. **Select one disclosed phase order on every economic date.** Compute ALL
   ending fixed/floating coupons against already covered history, then apply
   their admitted generated PIK effects, the explicit principal batch, scheduled
   amortization, periodic-fee evaluation, terminal redemption, and finally commit
   closing stock/history. Accepted004's stateless rule needs no separate
   installment initialization/update phase.
   Issue is not a second initialization/replay exception. A movement at a coupon
   end affects the following interval, not the coupon just computed. A principal
   event or fee date does not create a coupon cut. Reject an unmet phase/history
   dependency instead of retroactively using a new balance for prior accrual.

9. **Keep generated PIK timing explicit and contract-limited.** For this builder,
   capitalize admitted generated coupon PIK at its actual accrual end before
   explicit principal/amortization/redemption, independently of its cash payment
   date. Accepted005 defines one nonnegative admitted generated PIK component
   per original parent coupon, retaining signed internal contributions. Apply
   every admitted component's full checked effect; do not silently discard an
   independently supplied signed principal effect through a positive-only gate.
   Generated amount/allocation eligibility follows005;008 replay-state decisions
   retain their scope. Terminal capitalization participates
   in the amount subsequently amortized/redeemed even when settlement is earlier
   or later. This operator does not represent every contract's capitalization
   date: payment-date capitalization and other timing require their real owner
   workflow, not silently re-dating this primitive or another generic mode.

10. **Make automatic exchanges an explicit par recipe with complete effects.**
    Inherit002's required PrincipalExchange selection. InitialAndFinal emits
    issue cash=-initial with principal_delta=0 and principal_date=issue, since
    that baseline was already counted at opening_date. It deliberately settles
    the declared base stock, not issue-closing stock after pre-issue movements;
    no catch-up funding is inferred. At the terminal phase, positive remaining
    stock B emits exact cash=+B on the selected payment date and delta=-B on the
    terminal economic date, leaving zero stock. Automatic zero exchanges may be
    omitted; supplied zero events under5 remain. None suppresses both automatic
    funding and final redemption/closure; it does not suppress explicit events,
    scheduled amortization or PIK, and may leave a positive residual stock. For
    other funding/redemption cash, select None and supply the actual signed
    cash/delta events, including zero delta for already-counted opening stock.
    Do not add par-event helpers, final-only variants, a second funding amount
    or hidden closure/write-off. Neither mode activates initial stock twice.

11. **Visit all required dates while preserving row identities.** Prepare a
    checked sorted/deduplicated processing-date union for required economic and
    settlement work, including opening, issue, raw maturity, terminal dates,
    coupon/fee boundaries/payments, principal events and owner004 amortization
    dates. A unique queue Date never merges coincident instructions or means
    a settlement-only date changes principal. Every supplied event is emitted
    once, including duplicate equal-valued rows and zero rows, with its original
    cash/economic dates. Emit independent fixed fees once. Canonical final row
    sorting is presentation only; it cannot retroactively change batches/phases.

12. **Use exact checked phase arithmetic and atomic local publication.** Inherit
    core012's exact sums: compute the complete candidate stock including opening
    B and all deltas within that phase, then admit its Decimal result and require
    nonnegative stock after each stock-changing phase. Do not net across phases
    to hide an invalid state. Do not require each
    incidental partial sum or a separately materialized net delta to fit when
    the final exact candidate does. For B=Decimal::MAX with deltas -MAX and +0.1,
    the valid final stock is0.1 even though the standalone net delta cannot be
    represented as Decimal. Validate/reserve complete rows, history and amount
    results before committing a phase, and publish a complete validated schedule
    or error. Check counts, capacities, date arithmetic and aggregation work;
    failures remain truthful currency/domain/range/history/resource causes.
    No partial output, saturating arithmetic, silent precision loss or arbitrary
    financial tolerance. A failed build leaves input instructions unchanged;
    external provider/cache observations are not transactionally rolled back.

13. **Keep amount and lifecycle decisions with their existing owners.** This
    record selects phase placement, not004's amortization laws/rounding/draw
    eligibility or005–009's coupon, fixing, replay and fee amounts/bases. In
    particular, do not adopt the current Notional-only LinearBetween funding
    detector, installment recast or fee opening-balance default by quoting this
    order. Feed the exact explicit batch components to the owning calculation;
    it must not reconstruct eligible funding from cash signs or input order.
    Periodic fees query their own covered accrual interval/point base, never the
    post-amortization live stock as a fallback for earlier exposure merely
    because their phase runs later. Fixed-fee economics remain009;
    representative schedule day count, lifecycle
    admission and balance replay remain012. Per-period001 context is authoritative
    and must not be replaced by the first leg's day count or ACT365F fallback.
    Default/prepayment/recovery engines remain010/011/015 and their callers.

14. **Guard one canonical event/notional/build admission path.** Apply002's
    fallible atomic local updates and complete final-horizon validation to all
    fluent, typed/spec, JSON and restored inputs. Validate every supplied row,
    currency, kind, date and exact amount even if zero or no automatic exchanges
    are selected. Use guarded complete construction/restoration rather than
    public unchecked fields/derived serde producing invalid validated state.
    Keep the incomplete instruction collector distinct from a compiled plan.
    Remove the lossy float-only Notional::par helper; use existing exact Money
    inputs. Require opening_date and signed cash without aliases, omission
    defaults, compatibility parsers, a second event enum or parallel host class.
    Strict lossless numeric/unknown-field/loading rules retain accepted core
    numerical ownership; this accepted package adds no new serialization framework.

15. **Reconcile replay, exports and consumers against the same dated effects.**
    Carry opening_date/initial authority and every principal delta/economic date
    through complete schedules and full event exports. Record012 must reconstruct
    the same closing economic stocks using exact date-level sums, never canonical
    row prefixes, cash amount/date, kind-based inference or skipping a guessed
    funding row. Keep deliberate settlement-cash projections local to their
    actual owner; they must not clear economic metadata on supposedly complete
    event rows. Reconcile valuation cashflow exports and structured-credit opening
    balance reconstruction, plus coupon/fee/accrual/WAL/credit callers. A cash view
    remains actual settlement cash and does not claim full economic history.

16. **Expose the same explicit contract through existing hosts and schemas.**
    Propagate opening_date, required signed cash, exact Money, explicit exchange,
    accepted kinds, paired terminal dates and precise errors through canonical
    Rust construction, specs/JSON, Python/WASM converters, stubs/declarations,
    schemas/examples and the existing parity ledger. Reuse the existing plain
    event-object boundary rather than adding a PrincipalEventSpec host class.
    Reconcile dependent product adapters in the same implementation slice;
    do not infer old cash signs, opening history or missing economic fields at
    a host boundary. Independent cross-host success/error/state/export checks
    are implementation qualification, not established by documentation approval.

17. **Require independent complete-output qualification before financial use.**
    Qualify pre-issue/opening coverage, start/end inclusion, every principal kind,
    signed premium/discount cash, zero/duplicate rows, same-date permutations,
    large/cancelling exact sums, terminal PIK and early/late settlement, None
    residuals, couponless schedules, malformed/overflow/resource failures and
    constructor/restoration/host/replay agreement. Compare contractual fixtures
    only within their actual phase/capitalization conventions. The bounded
    examples below support these accepted definitions; they neither execute the
    production pipeline nor qualify a universal event order, quantitative drift
    bound, product suitability or external vendor equivalence.

## Observed ordered algorithm — not the accepted target

1. Require principal, issue and maturity; require issue before maturity; validate notional/amortization. Sort principal events stably by economic date. Require event delta currency to match opening notional and cash currency to match delta. `Amortization` delta must be nonpositive and `Notional` delta nonnegative. Other flow kinds have no equivalent sign restriction in the fluent method.
2. Determine terminal economic and payment dates from the coupon ending at raw maturity; take independent maximum dates if necessary. Without coupons both are raw maturity. Explicit principal events are bounded above by both raw maturity and the effective terminal boundary. Dated amortization is also checked against issue/raw maturity and the effective terminal boundary. No lower issue bound is imposed on principal events.
3. With default `InitialAndFinal`, emit `-initial` as issue-date Notional funding. Start balance at `initial`; apply all events dated on/before issue once and record their actual historical economic dates, including CDS pre-issue accrual. Then apply issue-date scheduled amortization.
4. Visit the sorted union of issue, maturity, redemption cash date, coupon/fee accrual and payment dates, dated amortization, principal events and LinearBetween start. For dates after issue, execute exactly: **fixed coupons + floating coupons → add positive net PIK → explicit principal events → scheduled amortization → periodic fees → initialize/update LinearBetween installments → terminal redemption → record closing economic balance**.
5. Coupons read the balance history over `[accrual_start,accrual_end)`; an event at an endpoint affects the next interval. PIK at the final accrual boundary is included before amortization/redemption even when BDC pays cash earlier or later. Periodic fees use historical balances; point-in-time fees use the opening balance of their own period.
6. Emit final positive balance as Notional at redemption payment date with `principal_date=terminal economic date`; zero the economic balance. `PrincipalExchange::None` suppresses both automatic issue funding and terminal redemption, while explicit principal events and scheduled amortization still operate.

## Observed signs, controls and precision

An explicit event adds `delta` to economic outstanding. If `cash` is absent, amortization cash is `-delta`, other cash is `delta`. Emission uses positive supplied cash for `Amortization` and negative supplied cash for other kinds. Thus discount/premium cash need not equal par change. Each event carries the explicit delta and economic date. Checked Decimal additions reject overflow and any negative balance immediately; same-day events are applied in insertion order, so a repayment before a funding event may fail even when the day's net delta is nonnegative.

Fixed fees are emitted once before the loop. Representative schedule day count is the first fixed leg's, otherwise first floating leg's, otherwise ACT/365F; flow-specific accrual metadata remains authoritative. Calendar IDs and projected fixing observations are normalized after emission. The completed schedule receives full validation.

## Alternatives and limitations

The accepted package replaces observed insertion-order admission with atomic same-date
stock aggregation while preserving all cash/event rows. It also replaces inferred
history origin and cash signs with explicit inputs. Cash-date stock recognition,
an intraday sequence model, issue-only history, and amortization-before-PIK are
different contracts and are not additional modes. This builder does not run
prepayment/default models; those specifications have separate engines/owners.
[CASHFLOWS-012](12-schedule-lifecycle.md) owns lifecycle replay and must reconcile
to the selected opening/date-batch contract under this accepted package.

## Independent reference examples and ownership limits

Read-only source/consumer reviews checked the orchestrator, phase pipeline,
balance-segment helper, event/spec/host ingress and schedule/valuation consumers.
Bounded Python Decimal/Fraction examples, independent of production helpers,
checked the following accepted targets on2026-11-08:

- With initial100 on1January before events and a +20 movement on5January,
  issue10January has stock120. Automatic issue cash is-100 with delta0;
  it is not another +100 movement or an inferred -120 settlement. At36% ACT360,
  accrual [1January,11January) is100×4×.36/360+120×6×.36/360=1.12.
  An11January movement changes the next interval, not that coupon.
- Opening50 with same-date -75/+50 closes25 under either permutation; genuinely
  negative closing stock errors. Complete exact candidates involving Decimal::MAX
  and cancelling/opposite-scale deltas are tested without binary64 intermediates.
- Ending-coupon PIK2, explicit net principal -55 and scheduled amortization7
  change opening100 to40 before terminal redemption; par cash40/delta-40 closes
  zero regardless of early/late payment. None retains40 without an automatic
  write-off. Signed admitted PIK components are not individually discarded.
- A Preceding terminal30May before raw1June excludes a31May principal movement;
  a Following terminal2June permits a2June movement despite raw1June. Missing or
  ambiguous terminal pairs error; output sorting, cash dates and duplicate/zero
  events do not change the economic batches or delete supplied rows. Missing
  opening history errors rather than borrowing the live balance.

These are small exact arithmetic/date-policy checks, not tests of the production
API, generated coupons, market conventions, precision bounds or host bindings.
The [2006 ISDA Definitions](https://www.sc.com/en/uploads/sites/66/content/docs/2006-ISDA-Definitions.pdf),
§4.9, distinguish payment and period-end dates, including early/delayed payment;
they do not select this builder's whole phase order. The
[HeartFlow/Hayfin credit agreement dated14June2024](https://www.sec.gov/Archives/edgar/data/1464521/000162828025004179/filename7.htm),
§3.10(b), capitalizes elected PIK on its defined Interest Payment Date. That is
a concrete contract-specific timing rule, not evidence that accrual-end
capitalization is universal. Actual product contracts must qualify the selected
operator; no general external normative event-order authority is claimed.

## Evidence

- [orchestrator.rs](../../../../finstack-quant/cashflows/src/builder/orchestrator.rs), [pipeline.rs](../../../../finstack-quant/cashflows/src/builder/pipeline.rs), [principal event definitions](../../../../finstack-quant/cashflows/src/builder/specs/principal.rs), [fluent validation](../../../../finstack-quant/cashflows/src/builder/coupon_api.rs).
- Located formula/regression cases in [audit_regressions.rs](../../../../finstack-quant/cashflows/tests/audit_regressions.rs): `b14_midperiod_draw_accrues_from_its_event_date`, `b13_preceding_final_payment_includes_contractual_pik`, `discounted_draw_roundtrips_principal_independently_of_cash`. [principal_credit_regressions.rs](../../../../finstack-quant/cashflows/tests/principal_credit_regressions.rs) checks pre-issue CDS exposure and amortization funded by PIK/draws.
- Source/consumer inspection also covered [balance segmentation](../../../../finstack-quant/cashflows/src/builder/emission/balances.rs), [schedule replay](../../../../finstack-quant/cashflows/src/builder/schedule.rs), [valuation event exports](../../../../finstack-quant/valuations/core/src/instruments/cashflow_export.rs) and [structured-credit flow reconstruction](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_flows.rs).
- No external normative source for general event ordering was established. Production tests/builds/generators were not run. Independent bounded reference checks do not establish production conformance. This review implements no runtime changes; reconciliation of the accepted package remains pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
