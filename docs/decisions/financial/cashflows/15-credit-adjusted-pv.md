# CASHFLOWS-015 — Product-owned credit valuation and qualified PV reporting

- Decision status: **Accepted — complete product-owned credit valuation and qualified PV reporting package, 2026-11-09**.
- Implementation status: **Observed; reconciliation pending**, scoped to cashflow PV aggregation and its actual valuation-export/host consumers.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded exact/high-precision examples and read-only financial/source/consumer reviews are reported below; they do not qualify a complete credit price or cashflow decomposition.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and domain

The observed reporting kernel discounts settlement cash, assigns survival treatment by CFKind and estimates recovery from inferred FIFO funded principal at scheduled repayment dates. Its inputs do not establish whether cash is promised, already expected or dependent on survival/default, nor the recoverable claim or product payment rule. The accepted replacement below removes that generic credit valuation and preserves reporting of values supplied by actual valuation owners. No runtime changes are authorized here.

## Accepted decision: product-owned credit valuation and qualified PV reporting

Accepted on **2026-11-09** after complete whole-record review. Runtime/caller/host
reconciliation and independent complete-path qualification remain pending. Inherit accepted
[core016 dated cash discounting](../core-conventions/016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting),
[core012 monetary admission and arithmetic](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity),
[market04 local conditional probabilities](../core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation),
[012 complete source schedules](12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views)
and [014 exact reporting and separate qualified PV ownership](14-aggregation-and-life.md#accepted-decision-exact-nominal-reporting-and-explicit-repayment-life).
Those contracts remain accepted; this accepted package does not introduce another credit
model, recovery engine, row-contingency configuration or pricing registry.

1. **Remove generic credit valuation from cashflow reporting.** Retire the
   survival-by-kind/payment-date-recovery algorithm rather than repair its
   inferred exposure with more metadata or modes. Cashflows owns construction,
   event semantics and exact reporting. Actual product/model valuation owns
   credit-dependent cash, default/recovery claims and their price attribution.
   Reporting consumes that authority; it does not choose a credit model from
   the presence of a curve or a cashflow kind.

2. **Delete the obsolete entry points and private algorithms.** Remove
   credit_adjusted_cashflow_pvs, the schedule and crate-private pv_by_period
   methods, automatic credit-curve/recovery dispatch, funded_principal and its
   otherwise unused repayment/funding inference, recovery validation,
   default_mass and local survival helpers. Remove dependent defaults/arguments,
   tests, benchmarks and documentation with them. Retain ordinary discounting
   through core016 and aggregation through014; add no compatibility alias or
   replacement generic credit/PV engine.

3. **Require owner-qualified expectation and contingency.** A CashFlow amount
   retains its actual contractual/settlement meaning. CFKind alone cannot tell
   whether its amount is promised, conditional, already probability weighted,
   scenario-realized or a known claim. Do not universally multiply ordinary
   cash by survival, exempt Recovery/AccruedOnDefault from credit, or treat
   ProtectionPayment as survival cash. Product owners select the credit event,
   probability measure and information state before producing qualified expected
   cash or PV. Never weight an already expected amount a second time.

4. **Preserve signed obligations under their actual model.** Negative funding,
   fees, protection and other payments do not universally cancel on the same
   reference default. Select their contingency at the owner; no sign/absolute-
   cash gate may infer it. Plain fixed-input discounting preserves signed cash
   and supported sign reversal under the SAME admitted factors, selection and
   successful bounded-Money arithmetic. This does not promise additivity or
   scaling across different per-row monetary rounding boundaries. Exact
   reporting sums preserve admitted signed PVs. Nonlinear credit,
   collateral/netting and recovery models retain their own position mapping;
   this is not universal price antisymmetry or permission to reverse003 stock.

5. **Do not infer recovery exposure from future repayments.** Complete012
   schedules can have positive residual principal, later funding, early cash,
   PIK, write-downs, premiums and pending settlements. Neither remaining cash
   nor optional tracked opening/through stock proves a recoverable claim or its
   exposure horizon. Remove opening-as-repayments-minus-increases, cash/delta
   fallback, early-capitalization pairing and epsilon solvency/lot truncation.
   Actual funding, capitalization and repayment conventions stay with003/012
   and their product valuation owner.

6. **Keep recovery amount, timing and allocation with the product.** Owners
   select recoverable exposure, claim basis, seniority, recovery/LGD, default
   horizon, settlement delay and any default-time integration or explicit
   payment-date approximation. FIFO matching is not inferred from aggregate
   stock history. The hazard source's recovery_rate is metadata under market04,
   not automatic authority for a product recovery term. No global recovery
   timing option, lot framework or new default allocator is selected here.

7. **Prevent overlap by actual claim authority.** Remove the guard that permits
   estimated recovery merely because no DefaultedNotional row appears. Recovery,
   protection, accrued-on-default and model-integrated claims can overlap with
   no such row, or can be distinct with one. Actual owner decomposition must
   count each entitlement once and distinguish known, expected and contingent
   amounts; invalid or unsupported combinations error without zero-recovery
   substitution. Removing the generic engine does not authorize duplicate
   claims at its consumer.

8. **Use the accepted probability source wherever credit owners need it.**
   Evaluate conditional survival from local integrated intensity,
   `exp(-J(as_of,T))`, and interval default mass through market04's checked
   operations on the retained source clock. Remove `S(T)/S(as_of)`, positive
   absolute-base-survival gates, rounded-endpoint subtraction, clamps and
   default-mass tolerances from the reporting/export path. Rounded absolute
   survival0 does not prove an observed default; actual known-event eligibility
   remains owner information. This restates an accepted dependency, not adoption
   of every downstream credit price or default process.

9. **Keep probability and clock annotations truthful.** A default probability
   annotation requires an explicit owner-defined interval and conditioning
   basis. Do not derive it from previous exported row survival: duplicates,
   currency interleaving or missing dates must not redefine the interval.
   Use owner/source-qualified dated annotations or leave them unavailable under
   the export contract. Remove context-free elapsed-time reconstruction and
   inferred observed-default comments; a displayed year fraction does not
   reconstruct a dated discount factor or product accrual clock.

10. **Reuse core016 literally for ordinary qualified cash.** Owners may discount
    deterministic or already qualified expected dated cash through the existing
    canonical helper, using a one-row call when row PV is needed. Require
    explicit as_of and include_today; never assume dates<=as_of have settled.
    Honor whole-input admission, source-owned dated relative factors, no unused
    source queries and finite strictly positive required discount factors,
    including factors>1. Zero, invalid factors and genuine source failures do
    not become successful zero PV or unit-factor fallbacks. Do not duplicate
    the source clock, discounting or scalar/Money arithmetic in cashflows.

11. **Require qualified native-currency source association.** Actual valuation
    owners choose the discount/funding/CSA and credit source for each native
    claim, plus valuation date, information state and one coherent selected
    model/market snapshot. Do not infer currency from CurveId, use the first
    dependency as authority or fall back to a primary foreign-currency curve.
    Remove automatic HazardRate model-key-to-credit-curve/recovery inference in
    the export path. FX/reporting-currency conversion and scenarios retain
    their accepted owners and explicit inputs.

12. **Retain exact admitted native PV Money.** Ordinary cash discounting follows
    core016's shortest-text Decimal factor admission and bounded Money products;
    model-produced scalar PV reaches Money through core012's explicit checked
    admission. Preserve admitted Money without amount()->f64->Money round trips.
    An owner may have additional model error; Money admission does not certify
    price accuracy. Use014 exact whole-sum addition for already admitted row PV
    totals, with exact-or-error final reporting sums and no implicit ISO rounding.
    Do not conflate model product rounding with exact nominal addition.

13. **Keep one existing period-reporting operation.** Supply owner-qualified
    dated native PV Money to aggregate_by_period under014's same complete
    PeriodPlan/calendar authority, sparse buckets and typed result rules.
    Those amounts are already values: do not discount or survival-weight them
    again. The owner chooses and discloses the attribution/report date; an
    integrated default-time contribution is not automatically cash received on
    scheduled principal repayment. A reporting-window total is only that window,
    not necessarily the complete price. No parallel PV aggregation/result class
    or fiscal calendar authority is added.

14. **Preserve complete source rows and meaningful value attribution.** Keep
    actual012 source cash/events and their identity, signs, dates and exact
    amounts separate from model expected cash/PV attribution. Do not overwrite
    promised cash with probability-weighted cash or fabricate contractual
    Recovery rows, payment dates or residual plugs to make totals match.
    Excluded/noncash rows may remain labelled audit rows; they must not create
    PV contributions or buckets merely through a manufactured zero. Qualify
    selected-row/component association, including legitimate zero values, at
    the actual product export owner.

15. **Require explicit support for product/model cashflow decomposition.**
    Preserve working canonical pricing and the valuation export's required
    native-PV reports. Use existing export validation/pricing-trace admission
    with unsupported as the default until the actual product/model owner
    supplies a meaningful complete attribution. The current default-accepting
    hook plus a generic formula is insufficient. Missing support or failed
    attribution errors explicitly; do not retain a private copy of the deleted
    credit engine, fall back to risk-free/zero values or rescale row values to
    the model price. No new export/pricer framework is required.

16. **Preserve the canonical-price reconciliation boundary.** Keep the export's
    comparison to the SAME selected model result, valuation date, currency,
    source snapshot and applicable scenario/reporting view. A mismatch or
    incomplete decomposition must not publish reconciles_with_base_value=true.
    Agreement of totals is necessary, not proof of correct row allocation.
    The valuation owner qualifies its financial/numerical tolerance; this
    record does not adopt the observed `max(abs(price)*1e-6,1e-6)` threshold or
    unrelated FX/scenario/price definitions.

17. **Reconcile actual Rust and host surfaces together.** Remove obsolete
    schedule pv_by_period/Python/WASM members, credit defaults, wrapper calls,
    declarations, parity entries, examples and float-PV assumptions in one
    implementation slice. Keep existing014 exact aggregation usable for
    owner-qualified PV Money and existing valuation product/report surfaces
    truthful about support and errors. Bindings remain conversions only; add
    no new direct generic host NPV/credit API, backwards-compatibility path or
    configurable row-credit interpretation.

18. **Limit primary attribution and require independent qualification.** The
    verified Hull–White April2000 working paper specifies claim/recovery and
    default-time cashflow assumptions; it does not justify CFKind credit rules,
    inferred opening principal or generic FIFO repayment-date recovery. Cover
    contingency/expectation/sign counterexamples, residual stock and funding,
    overlap, local conditioning after absolute underflow, small default mass,
    strict factor/Money admission, dates/zero/empty/currency/state boundaries,
    truthful attribution and unsupported/reconciliation failures independently.
    Product credit/recovery/integration and current-market qualification remain
    separately owned. This package changes documentation only.

## Independent bounded reference checks

Independent Python Fraction/Decimal calculations on2026-11-09, without importing
production code, checked these scope and formula counterexamples:

- At discount1 and survival0.8, unconditional funding−100 is−100 while a payment
  expressly cancelled on that reference default has expected value−80. A
  default-only protection payment10 has expectation2, not8. An already expected
  amount4 remains4 rather than being weighted again to3.2.
- Principal100 with survival0.8 and recovery0.4 has terminal expected value88
  under that simple model. An additional already expected recovery8 for the
  SAME entitlement would incorrectly raise it to96. A row-kind absence cannot
  establish nonoverlap.
- Opening0, future draw100 and repayment50 can legitimately leave tracked stock50;
  inferred opening `50-100` is−50 and falsely rejects that schedule. Opening100,
  new funding100, repayments100 at T2/T3, recovery0.4, discounts0.9/0.8 and
  survival1/0.8/0.6/0.4 at as_of/funding/T2/T3 give repayment-date recovery27.2
  under FIFO and26.4 under new-lot-first allocation. The same aggregate cash/
  stock history does not select a universal matching convention.
- A large prior integrated intensity can make both absolute survival endpoints
  round to0 while the next local one-year survival remains exp(−0.02). At
  local intensity2^-54, rounded survival is1, yet direct default probability
  is positive and yields recovery approximately2^36 on par2^90 at recovery1.
  The observed absolute1e-12 gate also accepts a doubling from2^-100 to2^-99
  as a zero default mass; those endpoints are not a monotone survival interval.
- For constant intensity0.2, discount rate0.1, par100, recovery0.4 and maturity2,
  deferring recovery to maturity gives
  `40*exp(-0.2)*(1-exp(-0.4))`; default-time recovery under this deterministic
  simple model is `40*(0.2/0.3)*(1-exp(-0.6))`. They differ even with the same
  claim and hazard. No complete-product equivalence follows from a shared
  terminal recovery rate or a matching total.

These are bounded independent references supporting the proposed ownership and
removal choices. They do not execute the current kernel, validate every credit
model, or certify a pricer/export or complete host implementation.

## Observed settlement formula and curve conventions

For future cash at `T`, relative discount is `D(as_of,T)` from `Discounting::df_between_dates`; conditional survival is `Q(T)=SP(T)/SP(as_of)`. Require finite positive base survival and finite future survival in `[0,1]`. At as_of use survival one. No hazard means survival one.

- Noncash kinds, and all dates `<=as_of`, receive zero PV.
- Recovery and AccruedOnDefault receive `cash×D`, without another survival factor.
- Other settlement cash, including negative future funding commitments, receives `cash×D×Q(T)`.

The row API requires one supplied discount factor per flow; every factor must be finite nonnegative (zero is permitted). Historical rows still have discount inputs validated. The period wrapper computes relative discounts for future dates and one for past/on-date rows, then buckets the row PVs using [CASHFLOWS-014](14-aggregation-and-life.md). Schedule convenience lookup takes a discount curve ID and optional hazard ID and automatically takes recovery rate from the hazard curve.

## Observed recovery exposure and allocation algorithm

Recovery input must be finite in `[0,1]` and requires a hazard curve even when rate is zero. Positive estimated recovery is rejected if **any** DefaultedNotional row is present, to avoid mixing explicit default events with inferred recovery. Explicit Recovery rows alone do not trigger this guard.

For positive recovery, process each native currency separately:

1. Identify positive future repayment claims by principal-repayment kind and positive cash. For economic dates on/after valuation, a negative explicit principal delta supplies par amount, positive delta suppresses repayment, and zero/absent delta uses cash. For economic dates before valuation, still-pending repayment uses positive cash amount.
2. Collect future economic principal increases. Explicit positive delta overrides inference; otherwise PIK uses positive amount and Notional/RevolvingDraw uses negative cash magnitude. Separate PIK or zero-cash capitalizations from ordinary advances.
3. Pair an early-paid or zero-cash repayment with same-economic-date capitalization first. The already-paid portion creates no new recoverable claim. A still-future early cash payment receives a lot beginning on its cash date, preventing recovery before capitalization for that paired claim.
4. Infer opening funded principal as remaining positive repayment claims minus future advances/capitalizations. Reject materially negative opening amount using tolerance `max(largest repayment,1)×1e-12`; a complete future principal stream is required.
5. Put positive inferred opening principal in a lot at as_of; append future funding lots in economic-date order. Sort repayments by cash date and allocate each **FIFO** across funding lots. Reject repayments before the required lot's funding date or repayments exceeding remaining lots beyond tolerance.
6. For a repayment at `T` with lots `(tj,Nj)`, add `recovery × D(as_of,T) × Σ Nj[Q(tj)−Q(T)]`. Reject survival increases above `1e-12` over a funded interval; clamp tiny negative default mass to zero. Allocate recovery PV to that repayment's row and reporting period.

Recovery exposure is therefore tied to actual funded intervals and par deltas where available, while the payment-date discount approximation defers every default recovery in a lot to its scheduled principal payment date.

## Alternatives and limitations

Observed branches are no hazard, survival-only with no/zero recovery, explicit post-default-kind cash, or inferred funded-principal recovery. The accepted package removes those generic credit choices and reuses core016/014 only after actual owners qualify their cash or value attribution. It does not choose universal default-time versus payment-date recovery, a recoverable claim basis or funding contingency.

The kernel infers opening recoverable principal from remaining cashflow structure rather than accepting schedule notional. Incomplete principal streams fail or imply different opening exposure. Recovery/protection rows without DefaultedNotional can coexist with estimated recovery; users must ensure they are not overlapping claims. The row-level function does not itself full-validate an enclosing schedule or guarantee currency/curve economic matching.

## Evidence

- [aggregation.rs](../../../../finstack-quant/cashflows/src/aggregation.rs): `credit_adjusted_cashflow_pvs`, `funded_principal`, `default_mass`, `survival_on_date`, `pv_by_period`; [schedule curve dispatch](../../../../finstack-quant/cashflows/src/builder/schedule.rs).
- Inspected `funded_recovery_payment_discount_matches_independent_interval_formula`: explicit exponential relative discount and survival powers establish the payment-date formula independently of the production allocation kernel. Located tests cover forward funding, FIFO amortization/draw/PIK, explicit delta versus cash, early PIK repayment, pending settlements, currency separation and DefaultedNotional guard. [credit_models.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/credit_models.rs) locates explicit default/recovery PV behavior.
- [Hull–White April2000 author working paper](https://w4.stern.nyu.edu/finance/docs/WP/2000/pdf/wpa00021.pdf), section1/equations2–5 defines claim/recovery and default-time loss assumptions; section2/equation9 specifies CDS payoff/accrual integration under stated independence/no-counterparty-default assumptions. Its q(t) is a risk-neutral unconditional default density, not intensity. The [reference entry](../../../REFERENCES.md#hull-white-2000-cds) distinguishes this verified version from the journal citation. It does not certify the observed FIFO approximation or adopt a whole downstream product model.
- Actual production consumers: [schedule dispatch](../../../../finstack-quant/cashflows/src/builder/schedule.rs), [valuation export](../../../../finstack-quant/valuations/core/src/instruments/cashflow_export.rs) and existing Python/WASM schedule methods. The export synthesizes generic row PVs then compares them to a selected price; total agreement alone does not prove attribution. Independent read-only financial, numerical, date/ownership and consumer/primary reviews challenged the full proposal.
- Tests were not executed.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
