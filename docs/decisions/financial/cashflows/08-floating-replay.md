# CASHFLOWS-008 — Compiled floating coupon replay and settlement

- Decision status: **Accepted — complete parent-bound floating replay package, 2026-11-09**.
- Implementation status: **Observed; reconciliation pending**, scoped to `floating_replay.rs` and its actual builder/bond replay consumers.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded exact examples and read-only state/consumer reviews are reported below; they do not qualify production execution or a model's information set.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and supported use

Reusable compiled coupons separate term-rate observation from accrual-start notional capture. This supports simulation/replay consumers whose principal can change between reset and accrual start. The observed replay state locks one nonnegative float notional for one contractual coupon; the builder separately settles balance segments using `settle_index_rate`. These observed paths require reconciliation with accepted001–007. The accepted replacement below keeps one constant-principal replay profile and one original-parent amount authority.

## Accepted decision: parent-bound replay, exact full settlement and explicit timing ownership

Accepted on **2026-11-09** after complete whole-record review. Runtime/caller/host
reconciliation and independent complete-path qualification remain pending. Inherit accepted
[001's immutable periods and complete day-count context](01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context),
[003's covered economic history and event phases](03-principal-and-emission-order.md#accepted-decision-dated-opening-principal-signed-cash-and-atomic-event-phases),
[005's original-parent amount/allocation authority](05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization),
[006's qualified term observations and exact controls](06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures)
and [007's overnight growth and parent contributions](07-overnight-compounding.md#accepted-decision-resolved-overnight-blocks-cumulative-growth-and-parent-allocation).
Those accepted choices are not reopened. This record selects a replay calculation
boundary; it does not select a stochastic rate model, exercise entitlement,
unpaid-claim lifecycle or schedule-based accrued-interest convention. No runtime
changes are authorized by this documentation acceptance.

1. **Compile the original owned parent once.** Keep CompiledFloatingCoupon as
   an immutable calculation owner using001/005's actual S<E, payment P, nominal
   references, complete day-count segments, source provenance and guarded prefix
   A(t). Known full accrual A_E=0 is valid when its required context is complete.
   Remove redundant public FloatingCouponPeriod/FloatingCouponEconomics input
   authorities and caller-authoritative accrual_factor; derive/cache computed
   data privately from the owned parent. Remove1e-12 context/split matching,
   inferred365/366 denominators and ACT365L proportional substitutes. Never
   reconstruct a composite parent from a single optional cash-row context.

2. **Reuse one observation/control and allocation authority.** Reuse005's
   exact cash_fraction/CouponType,006's exact Decimal controls and qualified
   resolved Term observation, or007's compiled overnight observation schedule.
   Do not accept independently duplicated economic/observation bounds requiring
   approximate matching or public mutation to clear one copy. A private kernel
   view can select Daily/Period application once without duplicating authority.
   Validate the actual index/currency, mode, controls and parent association at
   guarded construction; remove unsupported/irrelevant fields rather than ignore
   them. This package adds no alternative cash/PIK or rate formula.

3. **Bind every state to its immutable compiled parent.** Create state with
   CompiledFloatingCoupon::new_state(); remove unbound public Default. A private
   shared immutable owner reference/identity suffices: no global ID service,
   content hash, registry or nonce is required. Compiled clones share that owner;
   state clones retain the binding with independent mutable progress. Separately
   compiled parents, even structurally equal ones, cannot exchange states.
   Every coupon transition/query checks this binding before zero, empty or
   identity shortcuts, for both Term and Overnight. Reject wrong-owner states
   without changing either state.

4. **Capture exact principal independently of fixing.** capture_notional
   locks one exact nonnegative Money N in the parent currency, including zero.
   N denotes stock at logical actual S after003's same-date phases, asserted
   constant over this evaluated whole parent [S,E). Locking a fixing does not
   freeze principal. Preserve the exact Money value; no Money→f64→Money route.
   Reject a second capture, including an identical value, without replacing the
   first. A zero capture still requires valid currency, owner and capture inputs;
   observation readiness is not a capture prerequisite.

5. **Keep observation progress independent of capture.** new_state initializes
   the empty007 overnight accumulator. It can advance with qualified observations
   before N is captured; Term fixing can likewise lock before capture. Neither
   transition implicitly captures principal. Full monetary evaluation requires
   both captured N and the required completed observation state. Readiness is
   explicit data availability, not proof of publication, payment or lifecycle
   status. Remove ambiguous is_live in favor of explicit get_* observation,
   notional and progress accessors.

6. **Lock qualified Term observations, not a bare rate.** observe_term takes
   the006 resolved observation/value/provenance or an actual model-owner-qualified
   equivalent. Retain IndexId/currency, fixing date, actual index Sx/Ex/alpha,
   observed/projected role and actual source/information phase. Apply006's raw
   quote/model admission and pre/post simple-gross checks; no reset-date scalar
   seed or coupon-clock rescaling. A duplicate lock errors atomically even when
   equal. The source/model owner qualifies whether a value is available; state
   presence, method-call order, a date comparison or a future path array does
   not establish lawful information availability. No generic eligibility engine
   is introduced here.

7. **Make the constant-principal profile explicit.** Capture is an assertion
   about the evaluated whole-parent continuation history, not proof that future
   stock stays unchanged. Scheduled or realized intra-parent stock changes must
   use003's covered balance history through the SAME005/007 original-parent
   authority, or return an unsupported-profile error. Do not compile substitute
   segments, restart overnight growth, re-anchor nonadditive day counts or keep
   the first N while ignoring movements. An instrument may have a separate
   terminating branch whose actual owner applies its adopted termination payoff;
   that does not authorize008 full settlement on a changing-principal history.

8. **Use one read-only full-amount query.** Rename settle to get_settlement.
   Require captured N plus a qualified Term lock, or007 native calculation
   coverage exactly through E with all required final rate/adjustment dependencies
   available. Delegate to005's whole-parent monetary restoration, split and PIK
   admission, using006/007's actual staged rate/growth authority. Constant-N
   identities do not permit skipping required model/growth admission or making
   an unavailable source available. A_E=0 or N=0 does not bypass active-input
   validation. Return one complete checked result or a real error.

9. **Remove arbitrary segment settlement.** Delete public settle_index_rate
   and its supplied constrained/raw f64 pair, f64 notional and unrestricted
   segment-alpha path. It is not a compatibility wrapper. The builder evaluates
   its full covered003 history once through005's parent authority;008's constant
   case supplies one constant interval to that same authority. No per-segment
   financial rounding, independent partial splits or duplicate amount formulas.

10. **Return truthful exact components.** Keep a plain FloatingCouponSettlement
    value with exact Money total/cash and explicit noncash pik_delta (renaming
    pik_amount), retaining parent dates/provenance. Require cash+pik_delta=total
    exactly, nonnegative generated PIK per005, and enabled zero components.
    Negative total is supported only where005's actual split/PIK gate permits
    it, including cash-only negative coupons. An eventual PIK CashFlow has
    amount0 and the explicit economic principal delta at E. Expose an admitted
    Decimal final contractual rate only where truthful; Term raw quote metadata
    retains observed/projected identity, and Overnight has no fabricated single
    reset/index-rate metadata.

11. **A getter does not post an event.** get_settlement is repeatable and
    nonconsuming. It does not reset or mark state Settled, advance actual time,
    pay cash, capitalize PIK, change captured N or declare an unpaid claim.
    Actual003/012/product owners apply each selected cash/PIK event once at its
    own P/E phase. Remove payment-date or max(P,E) retirement shortcuts from
    this calculation boundary. State availability is distinct from event
    application and entitlement.

12. **Separate early-payment knowledge from qualified forecasts.** Do not
    add a blanket P>=S or P>=E rule. An owner can calculate an early-payment
    contractual amount as already known only when the actual captured balance and every required observation,
    final R and007 q dependency are lawfully known in its information phase.
    get_settlement never automatically advances a copy into the future. The
    owner may explicitly evaluate future WEIGHTS through E on a copy when all
    required inputs are already known, including genuinely available lookback/
    cutoff observations; this neither advances economic time nor posts PIK.
    A known logical-S balance may be supplied as a calculation input before
    calendar S without asserting S's event phases have already occurred. At P=S
    the actual owner needs a valid post-capture information/payment phase or a
    clear unsupported error; no nearest-grid-step snap. Unknown dependencies
    remain unavailable even when a simulation stores the complete future path.
    A006-authorized projection or model-owner-authorized scenario can support a
    correctly labelled forecast/scenario calculation, including future weight
    coverage; it does not certify actual contractual cash/PIK as known. Preserve
    that result's projected/scenario role. Actual early posting or termination
    claims require the owner's separate information test, including unresolved
    prior-PIK/start-balance dependencies. The repeatable getter is never that
    posting authorization.

13. **Remove the generic partial monetary method.** Delete accrued_amount,
    its payment-window zero rules, factor-prorating and current-prefix-rate
    amount formula for both modes. Expose007 source-neutral native prefix/
    progress/availability instead.013 and actual exercise/termination owners
    must select partial payoff/entitlement using preserved parent authority.
    A known prefix does not determine final q; even absent explicit caps,007's
    final contractual-rate admission adjustment can depend on H_E. Do not borrow
    unknown future q, cap only the current prefix, compile a shorter parent or
    split an independently rounded partial amount. This removal selects no
    replacement ex-coupon, cash/PIK inclusion or termination payoff policy.

14. **Keep failures locally atomic.** Wrong owner/mode, duplicate capture/
    fixing, missing state, invalid source, numerical admission or failed advance
    leaves prior replay data unchanged. Getter errors do not poison or mutate
    it. Inherit007's monotone transactional advance and frozen observations;
    identical-endpoint queries cannot evade binding/input checks. External
    provider, RNG, callback and cache side effects are not rolled back. Preserve
    the actual failure rather than returning zero or an alternate fixing.

15. **Keep guarded state small and Rust-only.** Use private validated parent/
    state fields, shared immutable data and explicit get_* accessors. Preserve
    bound in-memory clones/checkpoints without adding public state serialization,
    checkpoint schemas, host state classes or a new lifecycle framework. Update
    the existing Rust exports and the documented host parity exclusions as the
    actual public types change. Existing exposed specs retain their005/006/007
    strict ingress/error contracts; no compatibility aliases or fallback path.

16. **Reconcile actual builder and bond consumers together.** Replace builder
    segment settlement and LSMC Default slots, is_live, duplicate descriptors,
    grouped-metadata parent reconstruction, float capture/partial splitting and
    early-payment advancement guards with the authorities above. Use actual
    event dates and source information phases rather than approximate model
    steps. Preserve known-reset lookup errors. Reconcile principal event identity
    instead of principal_delta.is_none(), cash-inferred deltas or PIK cash amounts,
    inheriting003/core015 explicit effects. This does not approve broader LSMC
    rate/discount/hazard models, exercise payoff or termination/ex-coupon policy.

17. **Qualify the complete boundary separately from these examples.** Before
    runtime qualification, independently cover Term/Overnight wrong-owner and
    cloned-state behavior, observation-before-capture, duplicate/failed atomic
    transitions, exact currency/zero/signed amounts and PIK gates, full composite
    ICMA/ACT365L context and legitimate zero A_E. Include P<S/P=S/P<E/P>E actual
    information phases, known versus unknown final q, repeated nonposting queries,
    model/Decimal underflow/overflow and exact agreement of the constant profile
    with the builder's whole-parent amount. The existing tests and independent
    bounded references below do not establish these production guarantees.

## Independent bounded reference checks

On2026-11-09 an inline independent Python Fraction calculation, without importing
or running production code, checked:

- A Term quote4% locked before S does not capture the then-balance100. If a
  preceding coupon adds PIK8 at S, capturing108 with A_E=0.25 gives total1.08;
  cash_fraction0.4 gives cash0.432 and PIKdelta0.648 exactly. Repeated calculation
  does not itself increase N; the actual once-only event would produce108.648.
- A known native prefix H1=0.1 at A1=0.5, with A_E=tau_E=1 and reference cap0,
  has first contribution -0.005 when H_E=0.21 and -0.115 when H_E=0.43. Identical
  prefix knowledge therefore cannot determine the final parent's allocated amount.
- In ideal exact two-day compounded arithmetic with D360, first quote3.6%,
  reference cap5%, N100 and A=tau, second quote7.2% produces H_E=0.00030002 and
  first-day allocated amount0.008887888888…; second quote0 produces H_E=0.0001
  and first-day amount0.01. Actual admitted007 endpoints can change the last
  digits; the future-dependency discriminator remains. This is not a current
  partial-payoff recommendation or a claim about model execution.
- Signed split conservation holds for total-2/cash-only; total-2 with half PIK
  fails nonnegative generated-PIK admission; exact-zero enabled splits conserve0.

Read-only independent reviews checked the state/ownership, information, amount
and consumer boundaries. These checks support the accepted package’s distinctions only;
they are not a production state-machine test, vendor price comparison, model
calibration or contractual adoption evidence.

## Observed compile and state transitions

1. Require accrual end after start, finite positive accrual factor, finite split fractions in `[0,1]`, sum within `1e-12`, and valid rate parameters.
2. Verify accrual factor against the supplied day-count context to relative tolerance `1e-12 × max(|computed accrual|,1)`. For ACT/365L, instead require the factor's implied denominator to be 365 or 366 within `1e-9`; later partial accrual prorates the stored fraction.
3. For overnight observation, require exactly matching accrual endpoints and a constructible accumulator. Daily index bounds in coupon economics must match any explicit economic bound to the observation constraint; then clear economic index bounds to avoid applying them twice. Period constraints are preserved.
4. `observe_term` accepts one finite index fixing only for Term observation. It does **not** capture notional. Repeated fixing errors without replacing the locked value. `capture_notional` accepts one finite nonnegative notional, initializing overnight accumulation if needed; duplicate capture errors.
5. `advance_overnight` requires an overnight coupon and prior notional capture, delegates to the atomic monotone accumulator and preserves the full contractual grid. Term observation cannot be applied to overnight coupons or vice versa.
6. `settle` requires captured notional and a term fixing, or a complete overnight replay. Compute all-in rate with the shared bounds/spread/gearing kernel, then `total=N×accrual_factor×rate`, `cash=total×cash_fraction`, `PIK=total×pik_fraction`. Reject nonfinite results. `settle_index_rate` accepts a supplied constrained/raw index pair and a finite nonnegative segment accrual fraction; it does not itself require that fraction to be at most the full coupon accrual.

The observed state has no parent binding for Term operations; Overnight advance
checks its accumulator schedule identity, while full settlement does not itself
validate a supplied state's parent. The accepted package requires both modes and every
coupon operation to use the same bound owner.

## Observed accrued amount and conventions

`accrued_amount` returns zero on/before accrual start or on/after `max(payment_date,accrual_end)`. Otherwise it accrues through `min(requested_date,accrual_end)`, with contractual calendar/frequency/reference context and termination flag only at the end. ACT/365L preserves its denominator by actual-day proration of the stored full factor. A term coupon requires the locked fixing; an overnight coupon requires accumulator progress exactly through the requested elapsed endpoint, rejecting stale or future replay state. Amount is `N×elapsed_yf×all_in_rate`; this method returns the total coupon amount, without an ex-coupon adjustment or cash/PIK exclusion.

The recorded reset date is metadata: the replay methods do not themselves receive a clock that enforces invocation on that date. The caller orchestrates observation/capture timing. `settle` is observational and can be called repeatedly; it does not consume/reset state. Negative settlement PIK can be returned here; builder emission imposes the separate nonnegative net-capitalization restriction.

## Alternatives and limitations

The observed advice to use builder-style segment settlement or compile suitable
segments is superseded by the accepted original-parent boundary; accepted005/007
already own amount and overnight allocation. A separate principal ledger inside
replay would duplicate003. Schedule-based accrued interest uses different payment/
ex-coupon selection rules in [CASHFLOWS-013](13-accrued-interest.md); its unreviewed
financial choices are not adopted by removing this generic partial method.

## Evidence

- [floating_replay.rs](../../../../finstack-quant/cashflows/src/builder/floating_replay.rs) contains compile, transition, amount and accrued-interest algorithms.
- Located tests there: `term_fixing_precedes_and_does_not_capture_accrual_notional`, `overnight_incremental_state_matches_one_shot_coupon_settlement`, `unfinished_overnight_coupons_cannot_settle`, `overnight_accrued_interest_rejects_stale_and_future_replay_state`, `duplicate_fixing_and_notional_errors_preserve_locked_state`, and ACT/365L denominator examples. These are state/invariant and production-kernel comparisons, not independent pricing-engine validation.
- [builder coupon emission](../../../../finstack-quant/cashflows/src/builder/emission/coupons.rs) constructs duplicate descriptors and calls segment settlement. [Bond LSMC replay](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/replay.rs) creates/reset Default states, captures float stock and calls accrued_amount before independently splitting it. Its early-payment settlement helper advances a clone to E after caller guards; this is not unconditional future access. The actual guards/source qualification require reconciliation, including known_reset error handling. [LSMC build](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/build.rs) and replay also classify principal using missing delta or infer its effect from cash. These are static findings, not executed pricing reproductions.
- The [host parity ledger](../../../../finstack-quant-py/parity_contract.toml) currently excludes these compiled/state/callback APIs as Rust-only. The accepted package adds no host state framework.
- [ISDA 2006 Definitions, section4.9(a)–(d)](https://www.sc.com/en/uploads/sites/66/content/docs/2006-ISDA-Definitions.pdf) distinguishes payment dates from calculation-period endpoints, including Early Payment and Delayed Payment. It does not supply this utility's replay state machine, model information set or partial payoff. [ARRC technical appendices](https://www.newyorkfed.org/medialibrary/Microsites/arrc/files/2020/ARRC-Syndicated-Loan-Conventions-Technical-Appendices.pdf) provide specific observation/payment convention examples, not a universal early-payment availability rule. These primary pointers and accepted006/007 do not authorize borrowing a future path value. No independently adopted replay state machine or error-tolerance standard was located. No production tests executed.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
