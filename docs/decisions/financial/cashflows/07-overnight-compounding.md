# CASHFLOWS-007 — Overnight observation grids, compounding and balance allocation

- Decision status: **Accepted — complete overnight observation/growth/allocation package, 2026-11-09**.
- Implementation status: **Observed; reconciliation pending**, scoped to the cashflows overnight engine and builder emission.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded calendar/exact/numerical examples and read-only formula/state/host/consumer/reference reviews are reported below; they do not qualify production execution or contractual loan/swap equivalence.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and scope

The observed engine compiles business-day observations once and replays overnight fixings with calendar-day carry weights. It serves the cashflow builder and reusable bond replay. The source identifies a separate valuations engine for swap-style products, with different endpoint and projection-interval behavior. The [shared valuations record](../valuations-framework/overnight-projection.md) documents that difference. The accepted replacements below select this engine's explicit contract without asserting economic interchangeability; observed algorithms remain separate.

## Accepted decision: resolved overnight blocks, cumulative growth and parent allocation

Accepted on **2026-11-09** after complete whole-record review of the package proposed on 2026-11-08. Runtime/caller/host/state reconciliation and independent qualification remain pending.
Inherit accepted [market02's actual index factor queries](../core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates),
[market12's exact observed identity](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings),
[006's qualified observation selection and exact rate controls](06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures),
[005's parent context/amount/allocation authority](05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization)
and001–004's actual periods, covered stock and phase/date rules. The fractional
partial-shift geometry, fixed full-parent bridge and compound-RATE allocation
below are explicit builder operators. They are not universal compounded-loan,
unpaid-interest-balance, swap or reporting conventions. No runtime changes here.

1. **Require explicit overnight inputs and a positive parent interval.** Keep
   the existing SimpleAverage, CompoundedInArrears with lookback, ObservationShift
   and RateCutoff variants as separately selected methods; Simple is the term
   path and errors here. Do not imply arbitrary combinations of those variants.
   Require actual parent S<E, explicit observation-business calendar for block/
   offset geometry, explicit
   Act360 or Act365F index basis D=360/365, and explicit Daily/Period index-bound
   placement. Remove arbitrary positive numeric basis, coupon-day-count fallback,
   method/placement defaults and market-named preset helpers. The parent coupon's
   own day count remains005's distinct A(t). ONE explicit observation calendar
   governs original blocks and lookback/shift stepping. Check its association
   and the basis against the selected index contract; actual index value-period,
   publication and payment-calendar roles remain separately resolved.006 exact Decimal controls, explicit
   positive gearing and spread-placement choice remain authoritative. Reject
   irrelevant controls instead of ignoring them; empty elapsed PREFIXES are
   valid states, not zero-length contractual coupon schedules.

2. **Resolve each actual index observation independently of its weight.** Keep
   IndexId/currency, fixing/value date, actual effective/maturity Sx,Ex, positive
   index alpha and source/eligibility provenance with the existing observation
   data. Apply006/market12 publication-qualified exact fixing or permitted
   projection selection per distinct observation. A genuine Friday overnight
   interval may be Friday→Monday with ax=3/D; project that actual interval through
   market02, never Friday→Saturday merely because it is one calendar day or
   through a clipped coupon/checkpoint endpoint. Lookback/shift/cutoff can reuse
   that annual quote with different selected weights but cannot redefine its
   underlying value interval. No discount-curve substitute, CurveId fixing key,
   curve-base eligibility inference or generic rate fallback.

3. **Build full business-day interest blocks and preserve leading accrual.**
   Use consecutive full calendar blocks [b,nextBusiness(b)) under the explicit
   index calendar, intersected with actual [S,E). If S is closed, the containing
   block begins at the preceding business date; retain only its actual overlap
   for interest weight while using its genuine preceding fixing. Do not snap
   endpoints or discard weekend-only/holiday-only parents. Full block geometry
   remains available even when the coupon clips its beginning/end. Query the
   preceding/next business dates and source coverage actually needed, including
   beyond the coupon for a genuine quote maturity; do not probe an unused prior
   business date for a business-day start with zero offsets. Missing required
   outside-parent calendar/date/source data errors rather than shortening a quote.

4. **Define lookback without moving interest weights.** In-arrears lookback k
   shifts each original block's observation date back k fixing business days,
   retaining the original block's actual calendar-day overlap with [S,t). Zero
   is identity. A looked-back fixing retains its own full resolved Sx/Ex/ax,
   even when those dates span a different number of days from the current
   interest block. Do not shift the coupon's economic stock or silently add an
   observation-weight shift. SimpleAverage uses the unshifted block weights;
   this package does not add a combined average/lookback variant.

5. **Use paired full blocks for observation shift and its partial extension.**
   Pair every original [b,c) with [shift(b,-kBD),shift(c,-kBD)). A prefix's overlap
   x actual calendar days in that ORIGINAL full block receives shifted weight
   x*(shifted block days)/(original block days). Keep this weight as an exact
   integer-derived rational; no rounded integer days or fabricated fractional
   fixing date. Shifted real block start selects the actual quote, whose full
   index interval remains unchanged. The full business-endpoint case agrees
   with shifting both endpoints; proportional partial/nonbusiness-endpoint
   weighting is this project's explicit extension. Zero shift reduces to normal
   carry. Do not subtract business days independently from every calendar
   checkpoint, which can collapse a weekend's distinct dates. Every positive
   actual overlap has positive weight if its required full blocks are valid;
   remove the old collapsed-shift prefix/parent artifacts, without date repair.

6. **Keep cutoff as repeated quotes over separate factor blocks.** Count k
   from the final compiled parent block slices, including a retained leading
   partial block. k=0 is identity; require k<slice_count so one preceding uncut
   fixing remains. Replace only each final block's observation with that preceding
   uncut quote. Preserve every block/weight and factor boundary: two locked
   one-day factors still compound separately, unlike one two-day simple factor.
   Cutoff is fixed for the ORIGINAL parent; a partial replay does not move it
   earlier. Do not infer calendar-day cutoff counts, freeze the entire short
   coupon from an invented pre-parent quote, merge equal fixing dates or add
   lookback/shift combinations unsupported by the chosen variant.

7. **Expose source-neutral native growth before final rate controls.** Let w_i
   be the accumulated observation-day weight in each full block and tau(t)=
   sum(w_i(t))/D. SimpleAverage's native growth is H(t)=sum(r_i*w_i(t)/D);
   compounded methods use H(t)=product(1+r_i*w_i(t)/D)-1, with one simple factor
   per original block, including a clipped carry. Maintain raw growth and the
   actually used base growth separately. Daily index bounds replace each r_i
   before aggregation; Period leaves the base raw and bounds its final annualized
   index once. All-in bounds/spread/gearing apply after aggregation. Do not apply
   Daily index bounds a second time. Actual observation quote domains follow006;
   compounded raw/used factors must additionally remain positive. SimpleAverage
   is additive and does not acquire a compounded positive-gross constraint on its
   full aggregate merely because the same result type is reused.

8. **Declare a checked binary64 MODEL replay with stable numerical limits.**
   Admit raw quotes once under006/core012 and retain exact Decimal Daily controls;
   the private exact clamp feeds one explicit finite correctly rounded binary64
   model view for aggregation. This model view may round a smaller contractual
   increment away; it is not an exact monetary calculation or financial rounding.
   Evaluate rate/weight products and positive factors scale-safely, checking the
   actual pre/post-model factor domain. Use compensated ordered sums for simple
   growth and log1p/expm1 or a qualified stable product-increment formulation for
   compounding, with completed-prefix state separated from the active carry.
   Do not divide out rounded factors. Mixed-sign log cancellation, a false zero
   from rounded logsum, or a known nonzero prefix update lost to an unchanged
   value must be resolved by the checked numerical kernel or fail Computation.
   A rounded zero alone does not prove exact cancellation. Require finite
   requested growth/rate results, no nonzero-to-zero or forbidden zero-gross
   boundary loss, and truthful failure in unresolved regimes. A later cap cannot
   rescue invalid/unrepresentable required raw model results. No universal
   relative-error guarantee or huge exact per-path product fallback is asserted.

9. **Freeze each quote and make checkpoints independent of factor partition.**
   Cache one qualified raw quote per distinct resolved identity/date/source
   version within this accumulator/path, including cutoff duplicates. The callback
   receives immutable actual observation data, not a clipped weight slice that
   could change its projection interval. Retain completed factors/sums plus one
   active ORIGINAL block; extend that block's simple factor using the same quote
   and its full partial weight. Commit it once when complete. Do not multiply
   checkpoint pieces, refetch a changed quote, divide out a rounded prior factor
   or merge distinct cutoff blocks. Given the same admitted/frozen inputs,
   endpoint results follow the same canonical completed-block order regardless
   of intermediate checkpoints; validate numerical resolution rather than
   claiming arbitrary callbacks or independently rounded computations agree.

10. **Keep empty-prefix growth separate from unavailable annualization.** At S,
    validated state has weight0, growth0 and compounded gross1, with no quote
    callback. Annualized raw/base rates are unavailable, not fabricated zeros;
    use Option for those views. For positive weight, annualization is H/tau
    under the checked model profile. Rename projected_rate to source-neutral
    raw/base growth/rate terms, because callbacks may supply observed, projected
    or simulated data. Advance monotonically in [S,E], allowing same-endpoint
    identity after validation; another schedule's accumulator errors. Clones
    retain the same immutable schedule and frozen quote state. A full valid
    parent has positive total observation weight; legitimate005 coupon-alpha0
    does not imply an empty observation window or authorize index zero.

11. **Use one fixed full-parent bridge for amount allocation.** Let A(t) be
    accepted005's original-parent cumulative coupon measure, A_E=A(E), and let
    tau_E be the full observation weight/D. Choose kappa=A_E/tau_E ONCE, not
    A(t)/tau(t) at each checkpoint. Let H(t) be the admitted USED base growth
    (Daily-bounded or raw for Period). Determine final contractual Decimal R
    from the full annualized base using006's exact complete control/restoration
    authority, with Daily index bounds cleared after their earlier application.
    Reuse that algebraic kernel, not its single-term observation selector or a
    fabricated published whole-period fixing. Aggregate-growth domains remain7's
    explicit simple-versus-compounded domains.
    Set q=R-g*H(E)/tau_E using the SAME admitted H authority as the contribution
    differences. For each actual principal interval [s,e), the per-unit amount
    contribution is d=g*kappa*(H(e)-H(s))+q*(A(e)-A(s)). Keep kappa/q/differences
    as exact transient expressions, not independently rounded Decimal rates.
    q includes spread placement, final bounds and all selected admission/rate
    restoration adjustments.005 restores sum(B_i*d_i) to Money once, then splits
    the original parent once. For constant B this is algebraically B*R*A_E at
    the admitted authority. No division by segment alpha or gearing, synthetic
    segment annual rate, zero-weight rate fabrication or restarted compounding.

12. **Select compound-RATE allocation, not an unpaid-interest balance model.**
    The changing-principal law in11 assigns cumulative rate-growth increments
    to003's live principal. It does not add accrued unpaid interest to that stock,
    automatically settle interest on principal repayment, or implement compound-
    balance/trading allocation conventions. With per-block growth0.1/0.2 and
    B100→200, prefix-rate allocation gives54 while a separate unpaid-interest
    balance recurrence gives52. This difference is intentional and requires
    actual product adoption. A contract needing compound-balance accrual or
    proportional accrued-interest paydown must use its real owner model; do
    not claim every syndicated loan or noncumulative-rate method is equivalent.
    No additional selectable interest-balance framework is introduced here.

13. **State exactly what period bounds constrain and when they are known.**
    Period index/all-in bounds constrain the full REFERENCE annual rate R used
    in11, with the residual adjustment distributed over the parent's A(t).
    Under changing B they do not universally cap each contribution, effective
    weighted cash rate or total coupon. For growth0.1/0.2, equal coupon weights
    0.5/0.5, B100→200 and final reference cap0, contributions -0.06/+0.06 can
    still produce positive cash6. Do not repair that outcome or claim an actual
    weighted-cash cap; such a contract needs its owner-specific payoff. R/q are
    completed-parent quantities. A partial replay returns native growth/state,
    not a future final bound adjustment or a cap applied to every prefix.
    Missing future observations cannot be consumed to price earlier exercise or
    reporting as if already known.008/013 and actual pricing owners must select
    their partial/termination payoff rules separately; unavailable required
    completed-parent information errors rather than a look-ahead shortcut.

14. **Make monetary handoff admissions explicit.** Cache each required native
    model growth prefix through core012's shortest-text Decimal admission once,
    preserving the same endpoint values for all exact differences and final q.
    This selected boundary can error for a nonzero1e-31 growth even when a large
    notional could make its Money product representable; no large-B rescue or
    alternate precision path. Native MODEL replay can still expose that finite
    nonzero growth; a monetary query cannot bypass its admitted-input domain.
    Final model annualization/raw-index admission and bounded Decimal R remain
    distinct006 stages;005 owns final whole-parent Money and cash/PIK allocation.
    Preserve no implicit currency/minor-unit/financial rounding. A_E=0 gives
    kappa0 and known zero coupon after full active validation, not an excuse for
    missing rates. Store actual parent/observation geometry and admitted values
    so later required queries do not infer this law from flat cash rows.

15. **Validate exact geometry and bound work without arbitrary31-day rules.**
    Lookback/shift/cutoff controls admit integers0..=i32::MAX, then actual calendar,
    date, cutoff and resource viability; remove the schema-only maximum31.
    Check auxiliary lead/reach arithmetic in sufficiently wide integers rather
    than saturating+1, unwrap-to-MAX or imposing an accidental MAX-1 boundary.
    Require no more than100,000 compiled observation blocks per parent, inclusive,
    and checked aggregate work/capacity under002; unique quote cache is bounded
    by those blocks. Store full ordered blocks and integer-derived rational
    weights, with qualified model views where requested. Do not allocate a
    cutoff entry for every calendar day merely to map partial queries; locate
    their original block. Reject missing coverage, invalid intervals/weights,
    contradictory bounds and unsupported controls before identity/zero shortcuts.
    No sorting/repair of invalid canonical state or nearest-business approximation.

16. **Keep one Rust-owned schedule/state boundary and honest product scope.**
    Use validated immutable compiled schedules and schedule-bound accumulator
    construction; all public active inputs satisfy this same contract. Use
    canonical get_* observation/result accessors without old aliases. Retain
    source-neutral callback/cloneable replay APIs as Rust-only, documenting the
    existing host exclusion; update actual Python/WASM method/spec operations,
    schemas/stubs/declarations/examples/parity without new host state classes.
    Reconcile the separate valuations overnight engine's actual source/identity/
    publication boundaries in the same coherent implementation work. Its Draft
    endpoint snapping, full-coupon lockout, discount fallback, sensitivities and
    product/model decisions are not adopted by sharing an enum or this record.
    Actual product boundary construction may differ explicitly; no hidden
    caller-dependent compounding or claim that the two engines are interchangeable.

17. **Preserve provenance and atomic complete work.** Stage quote/cache/state
    updates and005 amount handoffs before commit; failed advance/build leaves
    prior local state unchanged and returns its actual source/domain/numerical/
    range/resource cause. Callback/RNG/provider/cache effects outside this state
    are not rolled back; reproducibility needs the actual owner-frozen inputs.
    Preserve observed/projected/simulated roles, full actual index intervals,
    raw/used growth and method/bound/clock choices with the parent. Do not label
    simulations published or fabricate one reset/index rate for a composite
    coupon. A reference R that does not describe each changing-balance
    contribution stays in owner provenance, not a misleading generic row-rate
    field. Original coupon identity, enabled zero rows, signed internal terms
    and nonnegative generated-parent PIK follow005; native observation cuts do
    not create principal capitalization dates.016 retains fixing materialization
    and scenario knowledge;008/012/013 retain state/lifecycle/entitlement/reporting.

18. **Qualify the whole selected operator independently.** Cover ordinary and
    leading carry, holidays, full/fractional shift blocks, lookback/cutoff limits,
    actual value-period projection and publication phases, repeated quote/cache
    identities and every valid checkpoint ordering. Compare raw/used simple and
    compounded growth, near-zero/factor/cancellation failures, stage admissions,
    full-parent bridge/cash conservation, changing-B compound-rate versus balance
    differences, reference-bound limitations and no-look-ahead behavior. Require
    state/error atomicity, resource failures and actual host/product/consumer
    agreement against independent rate/calendar/contract fixtures. Bounded
    examples below support accepted definitions, not production conformance,
    all-input numerical accuracy or universal loan/swap equivalence.

## Observed observation algorithm and variants — not the accepted target

1. Require ordered accrual bounds and an overnight method; `Simple` is the term-rate path and is rejected here. Zero-length periods can compile, but positive periods must yield a positive observation interval.
2. SimpleAverage and CompoundedInArrears use accrual endpoints as weight endpoints. In-arrears lookback moves each observation backward by the specified number of business days while retaining original weights.
3. ObservationShift moves both observation-window endpoints and the mapping of every partial accrual date backward by `shift_days` business days. Both observations and weights therefore move. Reject positive accrual/partial windows whose shifted observation interval is empty.
4. Compile business-day observations. Each fixing receives a simple-interest weight covering that business day and following nonbusiness days. A nonbusiness-day start uses the preceding business-day fixing (plus any lookback) for the leading slice. It is retained even when the whole coupon is a weekend stub.
5. RateCutoff leaves weights unchanged and replaces the last `cutoff_days` observation-slice dates with the preceding slice's observation date. The count is of compiled slices, not calendar days. Cutoff zero is unchanged; cutoff at least the number of slices is rejected because no preceding fixing remains.

Lookback/shift counts must fit the business-day arithmetic's signed integer range. JSON Schema marks these controls with a maximum of 31, but the runtime compiler does not enforce 31; it enforces representability/calendar viability. The valuations engine has a different short-coupon cutoff domain: a representable cutoff can freeze the whole coupon to a preceding pre-accrual fixing, while cashflows requires at least one uncut compiled observation. The shared enum does not imply identical accepted cutoff windows.

## Observed rate aggregation and numerical rules

With raw fixing `ri`, weight `di` actual days and basis `D`, compounded growth is `G=∏(1+ri di/D)` and annualized index is `(G-1)D/Σdi`. SimpleAverage uses `Σri di/Σdi`. Builder index basis must be ACT/360 or ACT/365F, selected by `overnight_basis` or inherited coupon day count. The public accumulator accepts any finite positive numeric basis. Published fixings stay in index convention. Projected observations use one-calendar-day curve rate `rate_period(t,t+1day) × curve_yf/(1/D)`; a weekend carry does not project a multi-day fixing.

Raw and constrained accumulators are maintained separately. Daily index constraints floor/cap each observation before compounding; Period constraints bound the final annualized raw rate. Spread, gearing and all-in constraints are applied after index aggregation. Require finite raw observations and positive finite simple-interest factors for compounded methods; reject nonfinite aggregated rates. Negative rates are allowed within the positive-factor domain.

## Observed partial replay and changing principal

An accumulator belongs to the exact compiled schedule instance. Advancement is monotone and bounded by contractual end, and clones state before work so errors do not partially mutate it. If a checkpoint splits a weekend carry, the next advance removes the prior partial factor and replaces it with a longer factor using the **same cached fixing**. It does not compound the pieces separately or refetch a changed fixing. A zero-weight prefix returns rate zero.

Builder emission preserves one cumulative clock across balance segments. Let `αj` be contractual accrual from coupon start through segment end and `Rj` its replayed annualized rate. Segment index increment is `(Rj αj − Rj−1 αj−1)/segment_yf`; zero-year-fraction segments produce `(0,0)` while still advancing the observation clock. This weights the compound increment by the segment's actual outstanding balance.

For period-level index or all-in constraints, replay prefixes without those period bounds. At full coupon end compute the constrained all-in rate minus its unbounded affine counterpart, divide that adjustment by gearing, and add the resulting index adjustment to every segment. Settlement then uses affine spread/gearing with bounds cleared. Thus the final period-bound adjustment is distributed uniformly over contractual accrual time, while raw compounded increments retain their timing. Daily index bounds remain inside the cumulative products. This is a substantive allocation choice for changing principal, not merely a cap/floor implementation detail.

## Alternatives and ownership limits

Supported alternatives are weighted simple average, lookback, observation shift, cutoff and daily/period index constraints. Restarting compounding at each balance change, capping every prefix or dropping weekend-start accrual are not this engine's behavior.

The accepted package replaces one-calendar-day projection, calendar-checkpoint business-
day subtraction, variable-prefix reannualization and divide-out/re-add replay.
It selects one proportional paired-block shift extension and one fixed-bridge
compound-rate allocation without adding alternate geometry/interest-balance
modes. Contractual weighted-cash caps, accrued-interest settlements and partial
exercise payoffs remain with their actual product/reporting owners. Numerical
model and monetary admission limits are explicit supported-domain limits, not
permission to silently replace values or certify every finite input.

## Independent reference examples

Read-only source/calendar/state/host/consumer reviews and bounded independent
calendar/Fraction/Decimal/numerical examples on2026-11-08 checked these accepted
targets without production helpers:

- P(Fri)=1,P(Sat)=0.9999,P(Mon)=0.9994 and D360 imply actual Friday→Monday
  index rate360/4997, whereas Friday→Saturday gives360/9999. The actual quote
  can then carry over the selected full/partial weight; no clipped query defines it.
- Friday rate0.36,D360 has Saturday/Sunday/Monday prefix factors1.001/1.002/1.003.
  Checkpoints replace the active factor; multiplying pieces gives1.003003001.
  Two locked1-day factors with per-day increment0.1 give1.21, not merged1.2.
- With shift1 and weekend-only calendar, original Fri10January→Mon13January2025
  pairs with Thu9→Fri10. Saturday/Sunday/Monday prefixes carry1/3,2/3,1 shifted
  day. A Sat11→Sun12 parent has1/3 shifted day using that SAME real Thu9 quote,
  not an empty shifted window or an interpolated publication.
- Two native increments0.1/0.2 give growth prefixes0.1/0.32. With A_E=tau_E=1,
  B100→200, g1 and no controls/restoration difference, contributions0.1/0.22
  produce54; a compound-balance recurrence produces52. Reference cap0 adds
  -0.16 to each equal-weight slice, producing -0.06/+0.06 and weighted cash6.
- Using admitted endpoint authorities, exact kappa/q algebra telescopes to
  R*A_E for constant B, including differing source/coupon clocks, Daily versus
  Period bounds and final-rate admission adjustments. Known A_E0 remains0.
- Stable native model replay retains tiny1e-28/360 growth that naive1+x loses,
  while007's later Decimal growth admission may truthfully reject it. Severe
  opposite-sign log cancellation and lost nonzero increments must not become
  manufactured zero. Empty prefixes have growth0 and unavailable rate views.

These are small calendar/policy and exact/model reference checks, not executions
of production schedules or a global numerical certificate. The
[ARRC syndicated-loan technical appendices](https://www.newyorkfed.org/medialibrary/Microsites/arrc/files/2020/ARRC-Syndicated-Loan-Conventions-Technical-Appendices.pdf),
Appendices2–3, distinguish observation-weight conventions and compound-rate versus
compound-balance treatment of changing principal. The
[New York Fed reference-rate methodology](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates)
and [Bank of England SONIA methodology](https://www.bankofengland.co.uk/markets/sonia-benchmark/sonia-key-features-and-policies)
describe business-day overnight value periods and calendar-day carry. They do
not prescribe this project's fractional partial-shift extension, fixed clock
bridge, reference-bound allocation, numerical profile or all product payoffs.

## Evidence

- [overnight.rs](../../../../finstack-quant/cashflows/src/builder/overnight.rs), [projection helpers](../../../../finstack-quant/cashflows/src/builder/rate_helpers.rs), [segment-rate allocation](../../../../finstack-quant/cashflows/src/builder/emission/coupons.rs), [compounding controls](../../../../finstack-quant/cashflows/src/builder/specs/coupon.rs).
- Inspected independent arithmetic examples in [overnight_replay.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/overnight_replay.rs): explicit weekend product weights and leading carry with one fixing invocation. Located partial-shift, cutoff, daily/period bounds and error-atomicity tests there. Inspected `overnight_period_bounds_use_final_rate_and_uniform_accrual_adjustment` and daily-bound formulas in [compiler_regressions.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/compiler_regressions.rs); these validate the chosen allocation, not an external market standard.
- Reference pointers: [ARRC SOFR guide](../../../REFERENCES.md#arrc-sofr-users-guide), [BoE SONIA](../../../REFERENCES.md#boe-sonia-key-features). Clause-level reconciliation, external published-rate fixtures and cross-engine parity across holiday starts were not established by this audit.
- Production tests/builds/generators were not run. The complete package is accepted; resolved quote intervals, paired-block geometry, stable replay, exact shared handoffs and actual product/host/state reconciliation remain implementation and independent qualification work.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
