# CASHFLOWS-005 — Parent-coupon amounts and cash/PIK allocation

- Decision status: **Accepted — complete parent-coupon cash/PIK package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, scoped below.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded exact/rational/date-policy examples and read-only amount/context/host/consumer reviews are reported below; they do not qualify production execution or contractual coupon suitability.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and scope

The observed implementation computes coupon amounts on each constant-economic-balance interval, with cash and PIK components emitted separately. Fixed coupons use checked Decimal multiplication; floating settlement uses finite-checked f64. The accepted whole-parent calculation/allocation rules are below, with observed behavior retained separately. Fixed-rate step selection belongs to [CASHFLOWS-002](02-program-compilation.md), floating rates to [CASHFLOWS-006](06-floating-rate-projection.md), and overnight increments to [CASHFLOWS-007](07-overnight-compounding.md).

## Accepted decision: parent-coupon accrual, exact splits and capitalization

Accepted on **2026-11-08** after complete whole-record review. Runtime/caller/host/schema reconciliation and independent qualification remain pending.
Inherit accepted [001's original periods/context and composite metadata rules](01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context),
[002's explicit windows and settlement selection](02-program-compilation.md#accepted-decision-coupon-and-payment-program-compilation),
[003's dated history and economic phases](03-principal-and-emission-order.md#accepted-decision-dated-opening-principal-signed-cash-and-atomic-event-phases),
[004's amortization and monetary boundaries](04-amortization.md#accepted-decision-explicit-amortization-laws-live-balance-installments-and-monetary-boundaries),
[core012's exact monetary admission and bounded transforms](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity)
and [core015's cash/principal/metadata meanings](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata).
The parent-prefix weighting, final amount-rounding stage, cash-first allocation
and nonnegative generated PIK profile below are explicit builder choices, not
universal loan/bond/swap conventions. This review changes no runtime.

The term-rate handoff was clarified on **2026-11-08** with
[accepted006](06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures):
use its already admitted contractual Decimal R directly.006 owns raw-index
admission and bounded contractual-rate restoration;005 separately owns the
whole-parent Money restoration, without an R-to-f64-to-Decimal round trip.

The overnight handoff was clarified on **2026-11-09** with
[accepted007](07-overnight-compounding.md#accepted-decision-resolved-overnight-blocks-cumulative-growth-and-parent-allocation):
use its fixed full-parent clock bridge and exact residual-adjusted contributions,
with its explicit model-growth admission and reference-cap limitations.005 still
restores the complete parent amount and allocates cash/PIK once.

1. **Make the original contractual coupon the amount/allocation unit.** Retain
   each001/002 compiled parent period's identity, raw/actual endpoints, original
   nominal context, stub status, selected rate/observation definition and cash/
   PIK selection. Balance changes, ICMA nominal pieces and observation/rate
   contributions are internal calculations, not separate coupons. Compute one
   total C and allocate it once per parent, emitting at most one enabled cash
   row and one enabled PIK row for that parent, including legitimate zeros.
   Do not combine distinct parents merely because payment, reset or economic
   dates match. Preserve signed internal contributions in the owner model;
   they are not independently admitted Money/PIK events later silently netted.
   Separately supplied003 events retain their individual identities/effects.

2. **Use covered economic stock over the parent's actual half-open interval.**
   Require003 history covering [S,E). Starting stock includes start-date phases;
   end-date changes affect the next coupon. Split at actual stock changes, not
   at settlement dates, zero-net principal batches or redundant equal-stock
   history entries. Keep001-required nominal/context contributions internally
   without creating extra settlement rows, rate resets, capitalization dates or
   financial rounding. Missing earlier history, malformed stock or incomplete
   period context errors; remove the live-balance fallback. Nonnegative stock
   and currency follow003; purchase price/cash sign is never the accrual base.

3. **Select original-parent cumulative accrual increments for coupon weights.**
   For fixed/term coupon allocation define A(t) by001's accepted direct
   period_accrual(parent,S,t), always anchored at ORIGINAL actual start S with
   frozen full parent context; A(S)=0 after complete validation. Admit each finite
   nonnegative prefix value once through core012's declared model-to-Decimal
   boundary, cache it for that endpoint, then use exact Decimal differences
   w(s,e)=A(e)-A(s). Require monotone prefixes and nonnegative representable
   weights; errors are not clipped, epsilon-repaired or recalculated against a
   different origin. The admitted weights telescope exactly to admitted A(E).
   This does NOT redefine core day-count formulas or001's direct [s,e] query.
   Re-anchoring direct day counts at every balance change is removed because
   accepted 30/360 variants and OneOne are not generally partition-additive.

4. **Keep the weighting consequences and zero domain explicit.** Prefixes keep
   original ICMA nominal references, the full ACT365L denominator context and
   the actual terminal fact only when the queried endpoint is that original
   terminal endpoint. Never infer context from a scalar factor or cash date.
   OneOne deliberately gives its whole unit to the first nonempty balance
   interval and zero to later intervals: it is not proportional daily accrual.
   Other legitimate zero weights/full fractions remain valid. Zero does not
   manufacture index/rate zero or excuse invalid active inputs. Projection or
   annualization that separately needs a positive denominator remains006/007's
   supported-domain decision and must fail when unavailable. Do not insert the
   current overnight (0,0) rates simply because a segment's weight is zero.

5. **Restore Money once from the complete parent calculation.** Fixed annual
   decimal rate r stays exact; for fixed/term contributions calculate the exact
   sum of admitted B_i*r_i*w_i before a single final bounded Decimal restoration
   C=Q(sum). For a term coupon r_i is the owner's eligible all-in annual rate;
   use006's already admitted contractual Decimal R directly, without another
   model conversion;006 owns raw-index admission and its bounded rate stage.
   Never convert existing principal to f64. Accepted007 supplies the actual signed
   overnight contributions through its fixed full-parent bridge/model-admission
   rules and original observation provenance. Apply that formula through the same
   parent amount authority, not fabricated segment annual rates, direct synthetic
   projection clocks or re-anchored partial coupons.007 owns its selected
   compounding/bound allocation; actual replay/entitlement/reporting state still
   retains008/012/013 ownership. Use the
   core012 nearest-even greatest-representable-scale profile for the COMPLETE
   exact admitted sum; internal products/contributions need not individually fit
   Money if the final sum does. Exact zero cancellation is valid; final overflow
   or nonzero-to-zero underflow errors. No per-piece monetary restoration or
   binary64 amount sum. The required fused evaluator is reconciliation work,
   using existing numerical dependencies/one core authority, not an existing
   qualified helper or a new public arbitrary-precision mode.

6. **Keep one independent exact split fraction.** Retain explicit Cash, Pik and
   Split, but Split stores only exact Decimal cash_fraction c in [0,1]; derive
   the PIK share as exact1-c. Cash is the endpoint c=1 and Pik is c=0. Remove the
   redundant pik_fraction input/storage, both1e-9/1e-12 sum gates, f64 split
   state, automatic normalization and old two-field aliases. This represents
   every exact valid split without two competing authorities. Reuse accepted
   exact Decimal host/wire admission and one canonical validated operation.
   Explicit settlement selection and removal of Cash defaults are inherited002,
   including unused/shadowed payload validation before allowed omission.

7. **Allocate the stored parent total once, with an explicit residual rule.**
   For stored C compute cash=Q(C*c) once under core012, then PIK=exact(C-cash).
   At c=0/1 preserve the exact endpoints. Require exact cash+PIK=C, currency and
   sign-consistent allocations; an unrepresentable required residual errors.
   Cash receives bounded computational rounding and PIK its exact residual;
   do not independently round both economic shares or promise separately nearest
   PIK rounding. For C!=0, every strictly positive selected share must have a
   nonzero allocated amount: a cash transform underflow or a positive PIK share
   lost to a zero residual errors. Exact C=0 and zero-share endpoints are valid.
   This is a specified numerical allocation, not a penny-rounding repair.

8. **Require nonnegative generated PIK per original parent coupon.** Use the
   SAME rule for fixed, step-selected, term and overnight coupons: after complete
   parent amount/allocation, PIK must be nonnegative. Negative cash-only coupons
   remain valid. Preserve signed internal rate/balance contributions, including
   negative pieces and exact cancellation, without rejecting a piece before
   evaluating its parent. A negative parent PIK errors even if another coupon
   closing that day could offset it; no cross-parent/net-date eligibility gate.
   Generated net de-capitalizing coupons are outside this selected profile.
   Core015/003's explicit signed PIK corrections remain supported separately;
   they do not silently authorize a different generated-coupon policy.

9. **Emit enabled parent components faithfully, including exact zeros.** An
   enabled cash share emits one cash row at the actual parent payment date;
   positive means received and negative paid under0044. An enabled PIK share
   emits one noncash row with amount EXACTLY zero and principal_delta equal to
   its admitted PIK amount. A strictly zero share emits no corresponding row.
   Enabled zero-total cash/PIK components retain their parent identity and
   truthful zero metadata; do not drop a selected PIK-only zero coupon or use
   a display threshold. Fixed cash uses Fixed or the ORIGINAL parent's Stub
   classification; floating cash uses core015 Floating, not FloatReset.
   Balance/nominal/rate subdivisions never create stub labels or cash principal
   fields. Do not put capitalized interest in settlement amount.

10. **Keep capitalization at the actual whole-parent end.** PIK date and
    principal_date both equal parent actual accrual end, independently of its
    cash settlement date. All coupons ending there use pre-end history before
    their admitted PIK effects enter003's exact stock-changing phase, ahead of
    explicit principal/amortization/redemption. Hand exact principal effects to
    that phase, including zero; remove f64 PIK totals and positive-only dropping.
    No capitalization at internal nominal/balance cuts and no intra-parent
    interest-on-new-PIK. Payment-date capitalization, election-dependent rates
    and different contractual timing need their actual owner workflow; neither
    Split nor Pik infers such economics. This preserves003's chosen timing.

11. **Retain the actual parent calculation authority, not a false slice.** Keep
    the immutable parent context, original anchor, covered balance intervals,
    admitted prefix values/weights, actual rate/growth provenance, ordered signed
    contributions, total and split with the existing owner coupon model. Carry
    the information through complete schedule/state handoffs where needed for
    amount/replay/accrual queries; a deliberately projected cash view need not
    claim complete provenance. Reuse001's existing composite policy: attach
    optional core015 CashFlowAccrual only when one genuine complete calculation
    slice/context truthfully represents the row. In particular a prefix weight
    A(e)-A(s) must NOT be labeled direct day_count(s,e), and a composite coupon
    must not carry one invented ICMA reference pair. Omit false optional details;
    queries needing them use the owner authority or error. No new generic
    accrual-mode enum, snapshot registry or alternative persistence framework.

12. **Keep scalar rate, factor and observation metadata honest.** A row may
    report its truthful full parent accrual measure, including known zero, and
    its full annual coupon quote when one such quote actually applies. Do not
    scale that quote by the cash share, infer a weighted rate from composite
    cash/PIK amounts, or copy a full fraction onto every internal contribution.
    Preserve original parent stub identity and distinguish raw projected index
    from all-in rate. Reuse a genuine single term reset/index only where the
    owner's observation contract satisfies core015; do not manufacture one
    representative reset/projected index for an overnight/composite coupon.
    Metadata availability is explicit, not established by a cash date or a
    nonzero factor; allocation/partial-accrual formulas need owner provenance.

13. **Keep rate selection and financial controls with their owners.** Fixed
    rates are signed exact annual Decimal quotes; step selection remains002's
    raw-period-start rule and its window/cadence effects. Floating fixing/curve
    lookup, explicit cutoff, forward/index tenor, transformations, gearing,
    bounds, fallback and overnight growth allocation remain006/007/008. Preserve
    real source/validation/calculation errors. Balance/nominal pieces do not
    reset an observation or borrow missing market history. Selecting a cash/PIK
    share does not add a PIK rate premium, interest floor, election condition or
    interest-on-interest model. A product requiring them must provide its actual
    selected rates/owner contract, not rely on the names Cash/Pik/Split.

14. **Select no implicit financial rounding of coupon payments or principal.**
    Inherit core012/004 precision authority. Preserve sub-minor-unit amounts,
    the stated once-per-parent/once-per-cash computational rounding stages and
    exact PIK residual. Do not apply currency/display scale, generic monetary
    tolerance, automatic financial mode, fractional normalization or an epsilon
    floor to coupon/PIK eligibility. Contracts needing settlement-unit rounding
    and corresponding principal allocation need an owner-qualified explicit
    schedule/economic event path. This package adds no hidden rounding stage or
    second generic rounding policy and makes no universal pricing-error claim.

15. **Use one guarded canonical coupon/split ingress and nested wire shape.**
    Validate fixed/floating/step specs and payment selections through shared
    Rust authority for local exact fields, then full period/owner admission.
    Remove unchecked From/restoration paths and decimal integer/float coercions
    that bypass accepted exact admission. Nest schedule explicitly in the
    Fixed/Floating/StepUp wire specs to match their existing composition, replacing
    flattened Raw* duplicates/partial decimal adapters. Keep one strict nested
    schema with unknown-field rejection and exact rates/step tuples/fraction;
    no flat legacy parser, old pik_fraction input or omission-as-Cash fallback.
    Constructor validation/derived complement are real operations, not parity
    literal exclusions; provide actual counterparts over the existing plain-value
    WASM shape without adding a new class or independent host calculations.

16. **Bound work and publish complete atomic results.** Validate the full parent
    definition, history/context, selected rates/observations, split, monotone
    prefix weights, complete monetary expression and PIK eligibility before
    appending its rows/effects. Check interval/count/capacity and transient exact
    arithmetic work. Follow accepted deterministic input-validation/sorted-parent
    execution order; arbitrary traversal, callback completion or output sorting
    cannot hide failures or change valid amounts. Stage all same-date coupon
    results before003's PIK phase; no one
    coupon's new PIK feeds another closing coupon. Return a complete plan/schedule
    or its actual domain/currency/range/underflow/source/resource error. Inherit
    local builder/build failure atomicity; do not claim rollback of external
    market/cache observations or reproducibility of changing provider inputs.

17. **Reconcile the actual monetary, host and downstream paths together.** Share
    this one whole-parent amount/allocation authority between fixed/floating
    emission, compiled settlement and replay; remove float notional/amount/split
    state and duplicate tolerance/formula paths at monetary boundaries. Apply
    canonical specs/JSON, Python/WASM, stubs/declarations, schemas/examples and
    parity changes in the same implementation slice. Reconcile term-loan, bond
    hazard/LSMC and Merton split adapters without silently broadening their
    owner-specific model domains. Statements/structured-credit/accrued/credit
    consumers must read actual PIK principal effects/economic dates instead of
    cash amount/sign/date. Preserve known zero and original parent/provenance;
    lifecycle, partial accrued interest, ex-coupon entitlement and reporting
    formulas stay008/012/013/014/015-owned. A flattened row cannot re-create
    missing parent-origin calculation authority; required queries must error.

18. **Require independent complete-parent qualification.** Cover original versus
    directly re-anchored synthetic counts, OneOne, AFB/leap/ICMA/365L context,
    partial-prefix monotonicity, changing principal and admitted zero fractions,
    signed/internal cancelling growth, complete-expression overflow/underflow,
    one-field split endpoints/exact conservation/both-share underflow, negative
    cash and per-parent PIK failure, enabled zero components and coincident
    independent parents. Qualify whole-output cash/delta/date/context agreement,
    state/host/consumer handoffs and unsupported-query errors against independent
    references, with contractual rate/rounding/election/timing fixtures where
    relevant. Bounded examples below support accepted definitions, not executed
    production, all-market applicability or a universal accuracy certificate.

## Observed algorithm and conventions — not the accepted target

1. Find balance at accrual start from the last history entry on/before start, otherwise use the supplied fallback. Split only at actual balance changes strictly before accrual end; same-balance date entries do not split coupons.
2. For each `[s,e)` segment compute contractual day-count fraction `α(s,e)` with the parent period's calendar, frequency, reference coupon and terminal context. This is not a generic actual-day weighting approximation. ACT/365L preserves the full-period 365/366 denominator.
3. Fixed total is `C = B × r × α`, with checked Decimal products before f64 output. `r` is an annual decimal rate. Classify fixed cash as Stub only when the original full period is irregular; balance subdivisions do not independently create stubs.
4. Resolve Cash to `(cash,pik)=(1,0)`, Pik to `(0,1)`, or explicit Split. Each Decimal fraction must lie in `[0,1]`, and their sum must differ from one by no more than `1e-9`; fractions are not renormalized.
5. Emit cash whenever cash fraction is positive, including zero and negative cash amounts. Fixed cash uses Fixed/Stub; floating cash uses FloatReset and carries reset date. Amount is `C × cash_fraction`.
6. PIK is excluded from settlement cash but increases principal at the parent period's economic end, independently of payment date. Fixed negative PIK segment amounts are rejected. Floating segments may be signed when allocating bounds; the **net PIK for the whole period** must be finite and nonnegative. Nonzero signed PIK rows all carry the same final economic capitalization boundary.
7. Store per-flow accrual start/end, day count, calendar, nominal reference period, terminal flag, rate, accrual factor and, for floating rows, raw index rate. These fields drive later accrued interest and balance replay; they are not reconstructed from payment dates when present.

## Observed selection, failures and rationale

Coupon split is explicit or cash by default; a payment program can override it. The builder adds aggregate positive PIK before explicit principal movements and amortization. Negative cash coupon rates are supported; de-capitalizing a net PIK coupon is not. Overflow or nonfinite settlement results fail. Source comments justify segmenting by economic principal changes rather than adjusted payment dates.

`CouponType::split_parts` accepts sum error up to `1e-9`, while `CompiledFloatingCoupon::compile` requires `1e-12`. A split accepted for a fixed coupon can fail for a floating coupon without economic changes.

## Alternatives and ownership limits

The accepted package selects parent-prefix weights instead of the observed direct
day-count-per-balance-slice operator, one parent amount/allocation instead of
independently rounded piece cash/PIK rows, and one independent split fraction.
Cash-date stock, time-prorated replacement weights, negative generated net PIK,
financial penny allocation and dual input/algorithm modes are not added.
OneOne's opening-interval weighting and the retained nonnegative generated PIK
profile are explicit supported-domain limitations; actual contracts must adopt
their applicable operator rather than infer conformity from this record.

## Independent reference examples

Read-only source/context/API/consumer reviews and bounded independent
Fraction/Decimal/date-policy examples on2026-11-08 checked these accepted targets,
without production helpers:

- US30/360,29January→28February2025, split at31January: direct subperiod counts
  are2+28=30 while the full parent count is29. The selected prefixes give weights
  2/360 and27/360. With annual rate0.36 and principal100→101 at the cut, exact
  mathematical amount is0.2+2.727=2.927; the observed direct-slice operator gives
  3.028. Constant100 remains2.9 under the prefix partition, before the declared
  model-to-Decimal/computational rounding. OneOne weights are1 then0, explicitly.
- Complete contribution sum [3,3,3,3,-10]×1e-28 is2e-28. Allocating once at
  c=0.5 gives cash1e-28/PIK1e-28; independently rounded piece allocations can
  incorrectly produce a negative PIK net. Internal signed pieces remain retained.
- Stored C=3e-28,c=0.5 gives cash2e-28 and PIK1e-28, conserving3e-28. C=1e-28,
  c=0.5 errors on cash underflow; c=0.9 errors because the positive PIK share
  would disappear into zero residual. C=0 and c=0/1 remain valid endpoint cases.
- C=10^28+1,c=1e-28 gives bounded cash1+1e-28 but an unrepresentable exact PIK
  residual10^28-1e-28; error without independent residual rounding. A complete
  signed product sum can succeed despite an unrepresentable individual product;
  final true nonzero-to-zero or out-of-range C still errors.
- Negative internal contributions with positive parent C can support positive
  PIK; a negative parent C with a positive PIK share fails, even if another parent
  has offsetting PIK. Cash-only negative C remains signed cash. Enabled zero
  cash/PIK rows remain distinct, with PIK amount0/delta0/date=parent economic end.

These are small exact/admitted-number and date-policy checks. They do not execute
production schedules or establish exact real day-count/model inputs, every
projection domain, all-data monotonicity, host conformance or contract suitability.
The [2006 ISDA Definitions](https://www.sc.com/en/uploads/sites/66/content/docs/2006-ISDA-Definitions.pdf),
§4.16, describe named day-count conventions; they do not select this project's
prefix/allocation/rounding operators. The
[HeartFlow/Hayfin agreement dated14June2024](https://www.sec.gov/Archives/edgar/data/1464521/000162828025004179/filename7.htm),
§3.10(a)–(b), specifies a PIK margin increment and payment-date capitalization.
It demonstrates why this generic split cannot infer contractual rate premiums
or universal capitalization timing. The chosen003 accrual-end operator retains
its expressly stated product-qualification boundary.

## Evidence

- [balance_segments](../../../../finstack-quant/cashflows/src/builder/emission/balances.rs), [coupon emission](../../../../finstack-quant/cashflows/src/builder/emission/coupons.rs), [CouponType](../../../../finstack-quant/cashflows/src/builder/specs/coupon.rs), [compiled settlement](../../../../finstack-quant/cashflows/src/builder/floating_replay.rs).
- Inspected formula/regression evidence: `bounded_split_pik_capitalizes_net_coupon_on_one_contractual_boundary` in [compiler_regressions.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/compiler_regressions.rs), which explicitly expects a negative segment and nonnegative net PIK. Located `b14_midperiod_draw_accrues_from_its_event_date` and ACT/365L balance-segment cases in [audit_regressions.rs](../../../../finstack-quant/cashflows/tests/audit_regressions.rs).
- General convention pointers: [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions), [ICMA](../../../REFERENCES.md#icma-rule-book). Independent accuracy/provenance evidence for the complete split-PIK lifecycle was not located.
- Consumer/API inspection includes [schedule accrued interest](../../../../finstack-quant/cashflows/src/accrual.rs), [Python coupon factories](../../../../finstack-quant-py/src/bindings/cashflows/builder/specs/coupon.rs), [term-loan split selection](../../../../finstack-quant/valuations/fixed-income/src/term_loan/types.rs), [structured-credit PIK consumption](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_flows.rs) and [statement period flows](../../../../finstack-quant/statements/src/capital_structure/period_flows.rs).
- Production tests/builds/generators were not run. The whole-parent fused amount evaluator, exact shared split/state, original-parent provenance and consumer reconciliation remain implementation work; the complete package is accepted, with runtime reconciliation and independent qualification pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
