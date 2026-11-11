# CASHFLOWS-012 — Canonical schedules, balance replay and explicit cash views

- Decision status: **Accepted — complete validated schedule, balance and cash-view package, 2026-11-09**.
- Implementation status: **Observed; reconciliation pending**, scoped to schedule construction/state, principal replay, provider views, composition and actual consumers.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded exact/date examples and read-only source/consumer reviews are reported below; these do not qualify production execution or instrument payoff completeness.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and domain

The observed implementation preserves classified native-currency rows, explicit economic principal deltas and flow-owned accrual metadata. Its public provider methods normalize the raw schedule to a valuation date and expose settlement cash without paying PIK or default-state movements as cash. The accepted contract below removes that implicit normalization. There is no FX conversion in these operations. Source comments motivate keeping economic balance dates distinct from cash settlement dates.

## Accepted decision: validated complete schedules, explicit balances and separate cash views

Accepted on **2026-11-09** after complete whole-record review. Runtime/caller/host
reconciliation and independent complete-path qualification remain pending. Inherit accepted
[003's dated exact principal and producer phases](03-principal-and-emission-order.md#accepted-decision-dated-opening-principal-signed-cash-and-atomic-event-phases),
[005's original-parent calculation authority](05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization),
[core012's exact Money rules](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity)
and [core015's literal cash/principal/date semantics](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata).
This package keeps one source-faithful schedule and removes implicit residual
rewriting. It selects no new rebasing, position, ledger, pricing or entitlement
framework. No runtime changes are authorized here.

1. **Publish one immutable resolved schedule.** CashFlowSchedule retains
   canonical classified rows and truthful metadata, with an explicit optional
   dated principal opening. Resolve amortization in its producer: remove the
   inception Notional.amort recipe and representative day_count from resolved
   schedule state and constructors. Neither is a second calculation authority
   for already-resolved rows. Keep the builder's input recipe and each actual
   coupon/fee parent's own day count/context with their owners. Replace the
   schedule's representative get_notional by get_opening; no global day-count
   fallback is selected for reporting or accrual.

2. **Declare principal opening and finite coverage explicitly.** Some opening
   contains opening_date, exact nonnegative Money balance and through_date, with
   opening_date<=through_date. It means stock immediately BEFORE all modeled
   opening-date movements, with closing queries covered through through_date
   inclusively. Do not derive either boundary from issue, maturity, first/last
   flow, settlement date or a curve date. The producer asserts coverage and all
   included activity; a deliberately constant continuation can explicitly use
   the maximum supported Date. Builder003's raw opening<=issue/event rules remain
   unchanged; its resolved output supplies its actual finite modeled coverage.

3. **Make absent principal context truthful.** None means an untracked native
   cash/event collection, not zero stock and not USD/first-row-currency stock.
   It may retain valid multicurrency cash and explicit principal events, but
   offers no scalar nonnegative-principal proof; balance/credit operations
   needing that proof error. Some requires every NONZERO principal effect in
   its single currency and covered date range, without implicit FX or ignored
   foreign activity. Foreign fees/coupons and explicit zero effects remain
   valid after core015 row validation, including dates outside principal
   coverage when they cause no movement. Never downgrade invalid Some to None.

4. **Guard every publication and restoration path.** from_flows and serde/
   JSON/host constructors validate the whole candidate before publishing it:
   complete core015 rows, exact monetary state, declared metadata, canonical
   shape and all applicable tracked-stock checks. Remove sorting-only serde,
   public/default metadata repair and from_flows_unchecked publication. Internal
   staging buffers are not public validated schedules. validate rechecks the
   same contract, not a separate permissive mode. Validate full input before
   empty/date-filter shortcuts, including invalid past rows. None validates its
   declared native-event scope without claiming an unknown principal history.

5. **Keep canonical ordering a presentation rule.** Retain the documented
   payment-date/kind-group ordering with complete deterministic ties over the
   admitted row data. Use canonical kind labels and accepted numeric/wire
   identities rather than native enum ordinals or Decimal encoding bytes.
   Preserve duplicate and zero rows and distinct original parent identities;
   no deduplication, cash netting or monetary quantization. Equal-date output
   order supplies no intraday economic sequence, event phase or coupon base.

6. **Replay only explicit principal effects.** Use core015 principal_delta
   and principal_date; cash, CFKind, issue metadata and funding magnitude never
   determine delta. Remove the approximate funding-row skip, sign/kind inference,
   payment-date fallback and first-flow anchor. Opening-funded inception cash
   has explicit delta0 under003; a zero-opening actual draw can have cash-98
   and delta+100. PIK/write-down cash remains0, while actual explicit delta
   moves stock. Preserve signed corrections; no cash-as-principal shortcut.

7. **Compute exact atomic closing balances by economic date.** For a tracked
   schedule, group every admitted nonzero delta at each economic date and compute
   exact B+sum(delta), then require representable nonnegative Money. Internal
   partial sums need not be independently representable or nonnegative; no
   insertion/kind-rank-dependent rejection. Remove all EPSILON tolerances,
   negative-balance warnings-as-success, zero floors and minor-unit cleanup.
   Generic flattened replay certifies date-level closing stock only.003 producers
   separately validate their declared intermediate phases; sorted rows cannot
   reconstruct that phase authority or repair its absence.

8. **Unify checked closing queries.** outstanding_by_date and
   outstanding_at_dates use this one replay authority and get_opening, not issue
   metadata. A query on a movement date includes that date's entire closing batch.
   Date queries must be nondecreasing, preserving duplicates and requested order;
   reject dates outside [opening_date,through_date]. Carry stock between admitted
   events only inside that declared interval. Validate required context before
   empty results. The logical opening balance is available through get_opening,
   without inventing a prior calendar date at Date::MIN.
    outstanding_by_date returns one opening-date point containing its complete
    closing batch, followed by one point for each later date with nonzero input
    effects, including a net-zero batch. No artificial coupon/payment-date or
    through-date points. A tracked schedule without movements therefore returns
    [(opening_date,balance)], while None remains unavailable.

9. **Remove fictional per-row and unguarded balance views.** Delete
   balances_by_flow and the f64 outstanding_on helper. A serialization row does
   not identify a unique before/after phase, and a changing-balance coupon has
   no universal scalar notional. Remove their half-minor-unit display cleanup,
   negative flooring and unknown-history defaults. Existing exports may expose
   correctly labelled date-level closing queries or actual producer-owned
   calculation data; do not silently relabel either as per-row balances.

10. **Remove automatic residual normalization.** Delete normalize_public and
    its blanket-provider use. cashflow_schedule returns the complete validated
    source schedule unchanged within its declared scope, retaining original
    cash/economic dates, opening, rows and parent/source context. Do not rebase
    opening, zero past deltas, synthesize future Notional rows, clear recipes
    during a read or discard history by cash date. No new residual anchor/type/
    rebasing helper is added. An owner needing a real snapshot must construct
    it from its actual authority and explicit coverage, not infer settlement
    or prior balance history from a cash projection.

11. **Use one explicit native settlement view.** Add canonical
    get_settlements(cutoff,include_cutoff) on the schedule, requiring the caller's
    same-day choice. Select cash-capable kinds by core015's exhaustive whitelist,
    preserving signed Money, native currency, zeros, duplicates and order; exclude
    noncash PIK/write-downs by kind, not amount/delta heuristics. Filter only cash
    dates >=cutoff or >cutoff as explicitly selected. Replace dated_flows and
    provider dated_cashflows with this authority; provider get_settlements takes
    source curves/as_of plus the separate cash cutoff/inclusion and delegates.
    No date netting, FX, reporting conversion or NPV inclusion policy is implied.

12. **Keep source knowledge, cash selection and economic activity distinct.**
    Provider as_of remains a source projection/fixing-information request; cutoff
    is a cash-view parameter. Neither proves cash was paid, principal moved,
    a forecast became observed or a claim was retired. A later cutoff changes
    neither source observations nor the complete schedule. Balance queries still
    replay economic dates, including past-cash/future-principal effects. A cash
    view cannot be fed back as a tracked history without explicit owner rebuild.
    Sources must retain truthful supplied history/provenance or report missing/
    unsupported data, not regenerate unknown history from current curves.

13. **Require explicit representation without incomplete success.** Remove
    Contractual Default and retire Placeholder as a successfully published
    financial schedule: an unmodeled contingent payoff returns a real unsupported
    error. Contractual and Projected remain explicit source declarations.
    NoResidual is allowed only for an empty represented event scope explicitly
    asserted by the owner. A complete schedule containing historical rows remains
    Contractual/Projected even when its settlement view is empty. Neither empty
    cash nor NoResidual proves zero stock, zero instrument value, realized
    settlement or complete modeling of unrepresented claims. Do not infer tags
    from filters or combine them by precedence.

14. **Remove generic date heuristics unrelated to the declared operation.**
    Issue and maturity remain contract metadata, distinct from principal opening,
    coverage, accrual and actual payment. Remove the interest-before-issue ban,
    first-flow funding fallback and100-year warning/date addition. Actual core015
    reset/context rules and001/003/009 producer date restrictions still apply.
    Known supported pre-issue or early cash is not invalid merely by a generic
    issue comparison; all dates retain their own validated financial roles.

15. **Rebuild changed schedules atomically through the canonical constructor.**
    Remove update_flows, try_update_flows, retain_flows, push_flow, with_notional
    and with_representation public bypasses. Owners stage changed rows, opening
    and metadata together and publish a fully validated replacement; failed work
    leaves the original immutable object intact. Affected parent/source context
    must be preserved or recomputed by its actual owner, not silently cleared or
    reconstructed from flattened rows. This needs no new transaction framework;
    external provider/callback effects remain outside local rollback.

16. **Remove ambiguous generic scaling.** Delete scale_amounts, without a
    replacement compatibility wrapper. Cash-direction changes, economic quantity
    resizing and principal stock are different owner operations. A negative
    position does not negate003's economic stock or its delta semantics. Actual
    product owners generate their correct signed rows and, when resizing, all
    applicable monetary/calculation authority through accepted rules. Independently
    rounding scaled opening/deltas can destroy proportional closure; no unproved
    generic sizing identity or implicit financial rounding is adopted here.

17. **Restrict generic merge to compatible untracked native collections.**
    Make merge_cashflow_schedules fallible, with no caller-supplied combined
    notional or representative day count. Require nonempty inputs, all opening
    contexts None, homogeneous explicit Contractual or NoResidual representation
    and equal global metadata after admitted canonicalization. Concatenate all
    rows without netting/deduplication and publish through the canonical validator.
    Reject tracked or Projected composition in this generic helper; actual
    owners rebuild those using their real combined principal/projection/parent
    authority. Matching scalar amounts do not establish shared or independent
    exposure. No metadata-to-None conflict repair, representation precedence,
    synthesized opening, FX scalar balance or hidden market-context merge.

18. **Retain truthful metadata without calculation fallbacks.** Keep actual
    issue/maturity, descriptive calendars/commitment and source observations with
    their admitted meaning; they do not replace principal coverage or flow-owned
    context. Calendar sets use deterministic canonical identities. A commitment
    is optional exact nonnegative Money, never inferred stock; actual facility
    calculations remain owner-defined. Preserve006/market12 qualified observation
    identity/provenance;016 owns materialization. Do not normalize conflicting
    projected observations or silently select one during collection merge. This
    record does not select new fixing, accrual, WAL or credit-PV algorithms.

19. **Simplify adapters without inventing economic inputs.** Make from_flows
    the classified-row authority, removing the redundant classified wrapper and
    notional_hint/first-currency/USD construction. Dated amount adapters require
    an explicit supported nonprincipal cash-capable kind, metadata and opening
    choice; they cannot invent principal delta/date, noncash state, a reset or
    accrual calculation from an amount/date pair. Principal events use complete
    classified rows. Empty untracked streams have no fabricated currency.

20. **Reconcile actual consumers, preserving working product generation.**
    Update IRS/basis-swap/FX/XCCY sources for real owner-signed rows and composite
    context when scaling/merge paths are removed. Replace export/table balance
    columns based on balances_by_flow with actual owner-defined information or
    unavailable values. Reconcile bond/term-loan outstanding helpers, statements
    replay, provider callers assuming all rows future and known source prefilters.
    Preserve actual coupon/fee parent authority under001/005–009. Unsupported
    snapshots/compositions must error, not drop previously represented cash or
    pretend a future-only list is a principal history.

21. **Reconcile exact host/wire/state paths together.** Update existing Rust,
    JSON/schema, Python/WASM constructors/accessors, provider views, stubs,
    facade/parity, examples and export/UI consumers for explicit opening/coverage,
    representations, removed recipe/day-count/helper fields and required cash
    cutoff inclusion. Validate restoration fully; remove obsolete aliases and
    constructor literal exclusions where Rust validates. Decimal/Money authority
    stays exact through ingress/egress. Remove DataFrame ingestion from from_flows;
    exact classified rows are its reconstruction input. DataFrame export and JS
    numeric views remain presentation, not canonical restoration. Use existing classes/plain values, without a new
    ledger, component registry or public checkpoint/state framework.

22. **Qualify representation, economics and source completeness separately.**
    Independently cover full construction/restoration and failed replacement,
    exact same-date batches, opening/range limits, native/mixed currency modes,
    pending cash/future economic activity, cutoff equality and zeros, unsupported
    representations/merges, scaling-removal consumers and source/context fidelity
    across hosts. Preserve original configuration/evidence. Generic validation
    proves its declared row/stock scope, not phase sequencing, asset payoff
    completeness, market calibration or fair value.013–016 retain their separately
    stated accrued-interest, aggregation, recovery-PV and fixing decisions.

## Independent bounded reference checks

On2026-11-09 independent inline Python Decimal/date calculations, without
importing production functions, checked the selected boundaries:

- Opening50 and same-date deltas-75/+50 close25 in either input order. A repayment
  100 on day1 followed by a draw100 on day2 from50 instead has an invalid earlier
  closing balance, even if a later cash cutoff would hide those rows.
- Opening0, funding cash-98/delta+100, then redemption cash102/delta-100 gives
  stock100 then0; opening-funded100 with funding delta0 does not double count.
- Repayment delta-100 economically30June, cash100 on5July: a1July cash view
  retains100, while the unchanged tracked schedule closes0 on1July. If cash
  arrives25June but principal moves30June, a27June cash view omits it while the
  full schedule still closes100 on27June and0 on30June. No synthetic row or
  delta rewrite is needed; source rows remain unchanged.
- At a cutoff equal to30June, include_cutoff true/false selects that day's cash
  explicitly; an economic closing query includes all30June movement in either
  cash-view choice. Valid zero-cash principal-capable rows remain in the cash
  view, while PIK/write-down rows are excluded by classification.
- A true USD residual0.004 remains0.004 rather than display-zero. Opening100
  and the current helper's scaled redemption200 close-100 if opening is left
  unchanged. Even all-component nearest-even scaling can break proportional
  closure: opening10e-28 and repayments5e-28 each, factor0.5, yield stored
  opening5e-28 and repayments2e-28 each, leaving1e-28. This is why no generic
  sizing law is claimed by deleting scale_amounts.
- Some(USD opening) rejects a nonzero EUR principal effect; an EUR fixed fee
  remains native cash. None permits a valid native collection but makes no
  scalar balance proof. Queries outside declared opening/through coverage must
  fail, independent of cash dates or whether the collection is empty.

These are bounded arithmetic and contract examples, not executed production
mutation/provider/host tests or full financial-model qualification.

## Observed canonicalization and validation

Rows sort by payment date, then economic kind rank: interest; fees; amortization; prepayment; defaulted principal; recovery/default accrual; PIK; notionals/revolving movements; margin/collateral; other. Ties use currency, exact Decimal amount, reset/economic dates, accrual factor, rate, detailed accrual metadata, principal delta, kind discriminant and Decimal representation. This guarantees deterministic serialization beyond economic equality.

`from_flows` validates each row then sorts; it does **not** perform full notional/principal validation. Serde sorts but is structural; `validate` separately checks notional, rows, sorted dates, principal balances and interest payments before issue. Flows over 100 years after issue warn rather than fail. Principal negative-balance rejection tolerance is `max(abs(initial)×1e-8,1e-6)`; replay itself warns below `-1e-9`. Mutators and scale/merge operations can require a later explicit validation. `try_update_flows` can leave earlier rows modified on callback failure and does not reach its final sort in that case.

## Observed balance reconstruction

1. Start with the schedule's initial notional. Order rows by `get_balance_date()` then canonical order. Explicit `principal_delta` overrides inferred movement.
2. Skip exactly one inferred initial funding row: negative Notional, no explicit delta, cash date equal to funding anchor, and magnitude equal to initial principal within relative `1e-12` with scale floor one.
3. Otherwise infer `+amount` for PIK; `−amount` for amortization, prepayment, defaulted principal, Notional, revolving draw or repayment; other kinds leave balance unchanged. Use checked Money arithmetic and emit closing balance per economic date.
4. `outstanding_by_date` and `balances_by_flow` require explicit issue metadata. `outstanding_at_dates` requires sorted queries and returns the closing balance including movements on the query date. `outstanding_on` separately floors its lookup result at zero and assumes the supplied path is sorted.
5. `balances_by_flow` records before/after principal; for nonmoving accrual rows it attaches balance held strictly before accrual end. It rounds displayed residues below half a currency minor unit to zero. That display cleanup does not redefine the underlying replay/validation tolerance.

## Observed public residual algorithm

`CashflowProvider` is blanket-implemented and always calls `raw_cashflow_schedule(...).normalize_public(as_of)`:

- Rebase opening principal using economic movements strictly **before** `as_of` in the initial-notional currency. A movement exactly on `as_of` remains in the residual stream.
- If that past movement's cash is still pending (`cash_date>=as_of`), preserve cash but set its explicit principal delta to zero, preventing double balance movement.
- If cash settled before `as_of` but its economic date is still future/on-date, create a zero-cash Notional row retaining the future principal delta and economic date.
- Clear the inception amortization recipe to None; retained rows already resolve remaining movements. Remove cash rows dated strictly before `as_of`, retain on-date rows and resort. Original issue/maturity metadata is retained.
- Settlement view includes only `is_cash_settlement` kinds and excludes zero-cash rows carrying explicit deltas. PIK and DefaultedNotional are state, not cash. Provider's dated view directly uses normalized settlement rows; schedule's public `dated_flows` additionally calls native-cash validation.

Full scalar-principal validation is unsuitable for multiple principal currencies. `dated_flows` validates structure/dates but skips single-balance validation when more than one nonzero principal currency is present. Raw adapters without a notional hint use zero in the first flow's currency, or USD when empty; they do not infer face principal from cash.

## Observed merge and scale choices

Merge concatenates/sorts rows, unions calendars, normalizes projected observations and keeps commitment/issue/maturity only when every input agrees. Representation precedence is Projected, Placeholder, Contractual, then NoResidual. Caller supplies combined notional and representative day count; merge does not prove economic consistency. `scale_amounts` scales cash and explicit principal deltas, not opening notional or amortization recipe, and resorts after sign changes. Caller must set compatible notional before asking for balance-based analytics.

## Alternatives and evidence

Cash-date-only filtering would lose pending economic movements; scalar validation across currencies would invent FX economics. Both differ from implementation.

- [schedule.rs](../../../../finstack-quant/cashflows/src/builder/schedule.rs), [traits.rs](../../../../finstack-quant/cashflows/src/traits.rs), [JSON validate/dated adapters](../../../../finstack-quant/cashflows/src/json.rs).
- Located invariant/regression tests in schedule source for exact canonical ordering, structural serde, mixed-currency merge, normalization before/after economic/cash dates, and same-day opening balance. Traits tests locate `public_provider_owns_the_complete_cashflow_lifecycle` and `native_settlements_support_multiple_principal_currencies`.
- No external standard dictates this internal representation. Tests were not executed.
- [IRS composition](../../../../finstack-quant/valuations/rates/src/irs/cashflow.rs) and [basis swaps](../../../../finstack-quant/valuations/rates/src/basis_swap/types.rs) use signed scaling/merge; [cashflow export](../../../../finstack-quant/valuations/core/src/instruments/cashflow_export.rs) consumes per-row balance pairs. [Python schedule bindings](../../../../finstack-quant-py/src/bindings/cashflows/builder/schedule.rs) and [WASM schedules](../../../../finstack-quant-wasm/src/api/cashflows/schedule.rs) expose the current state/views. These are actual reconciliation paths, not claims that their product or host behavior was executed.
- The accepted deletion of implicit normalization is a library ownership choice informed by accepted003/core015 date and principal meanings. It is not an externally mandated settlement convention and introduces no residual/rebasing API. The independent checks above do not certify current production execution.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
