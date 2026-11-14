# VALUATIONS-BONDS-04: Rates-only lattice valuation, event mapping and exercise

**Decision status:** Accepted — complete checked rates-only bond tree and exercise package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, fit, convergence, market and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked bond rates trees and exercise

1. **Elect and qualify one rates-only lattice.** When record 03
   selects Tree, require a typed election of scalar Hull–White,
   constant-volatility Black–Derman–Toy (BDT) or constant-parameter
   Black–Karasinski (BK). Bind the calibration discount source
   explicitly with the required role, denomination, origin and
   coverage. A bond discount binding can serve this role when
   explicitly designated and qualified. Preserve the security,
   settlement financing, projection and make-whole role bindings.
   The bond profile fixes
   Continuous base discounting for Hull–White and Simple base
   discounting for BDT and BK. Require finite nonnegative sigma,
   annualized per square root of an ACT/365F model year. Sigma
   is absolute decimal rate volatility for Hull–White and log-rate
   volatility of the selected Simple quote for BDT and BK. In
   inverse model years, Hull–White admits finite `κ≥0`, BDT has
   `κ=0` by construction and BK requires finite `κ>0`. Use one
   complete authored block or an explicitly identified, compatible
   fitted block under the shared provenance rules, with matching
   model, source roles, currency, origin, clock and units. Reject
   partial or incompatible active controls, implicit model and step
   defaults, discount-ID scalar lookups, option implied volatility
   as a short-rate parameter and scheduled Hull–White volatility on
   this bond route. Ho–Lee remains a separate model kernel.
   [Bond model selection](03-pv-models.md#accepted-decision-for-checked-bond-pv-selection-and-fractional-recovery-of-par),
   [shared fit identity and controls](../valuations-framework/model-resolution.md),
   [Hull–White](../models-numerical/hull-white-event-grid-and-affine-bonds.md),
   [BDT](../models-numerical/bdt-lognormal-curve-fit.md) and
   [BK](../models-numerical/black-karasinski-trinomial.md).

2. **Admit only fully resolved rates-only holder claims.** Use
   record 01's cash, coupon parents, principal changes, position
   cuts and vested rights. A stochastic rates-only tree admits
   coupons fixed or fully known at the cut, and deterministic
   contractual principal changes whose cash and state can be
   replayed exactly. An unresolved floating reset is admissible
   only at exact zero sigma when qualified deterministic forward
   projection, required known fixings and the same contractual
   accrual rules supply every amount needed. Conditional cash,
   principal changes, default or reference-curve dynamics that
   need additional state are outside this route. A source that
   supplies only a cash strip cannot infer missing accrual or
   exercise terms. Lower a return floor only through record 05's
   admitted deterministic process. An irrevocable prior election
   with wholly resolved proceeds and entitlements is a dated
   claim under the selected pricing route. Unresolved earlier
   notice or election is missing state.
   [Schedules and holder rights](01-schedule-and-holder-cashflows.md#accepted-decision-for-checked-bond-schedules-and-holder-cash),
   [return floors](05-return-floors.md),
   [joint coupon and state replay](06-joint-coupon-replay.md) and
   [joint exercise policy](07-lsmc-policy.md).

3. **Use exact, authorized exercise opportunities.** Require
   explicit discrete decision dates or the complete set of
   eligible dated opportunities from a contractually authorized
   window, including its boundary inclusion, calendar and
   eligibility rules. Treat valuation-day exercise as live only
   when the prepared phase and information cut leave it open.
   For a live option, admit unconditional full redemption, with
   decision, economic principal effect and proceeds payable on
   the same eligible date. Resolve all required eligibility and
   conditions before admission. Future advance-notice decisions,
   deferred exercise proceeds, partial redemption and continuously
   timed exercise require distinct contracts and are unavailable
   here. Remove the generic endpoint, cash-date and month-end
   sampling rule. A later adjusted contractual hold payment
   does not extend the exercise window. Keep notice, legal
   redemption and actual cash dates distinct in the input
   even when a supported profile requires coincidence.
   [Bond time and settlement](02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks)
   and [filed notice and holder-entitlement provisions](https://www.sec.gov/Archives/edgar/data/1219601/000119312513010869/d465899dex42.htm).

4. **Fit and roll on one exact dated grid.** Use ACT/365F from
   the prepared model origin, with required fit, work and
   capacity controls under the selected kernel. Include exact
   nodes for live decisions, actual payments and the economic,
   coupon and fixing events needed by this claim. Derive the
   pricing horizon from those required events, including
   lagged cash. Preserve multiple event identities at the
   same node. For Hull–White take an explicit final event grid
   or the exact union of the requested seed grid and mandatory
   nodes. A supplied final grid must already contain them. BDT
   and BK take literal positive uniform count `N` and positive
   horizon, with required dated event placement proved by the
   integer equality `N*d=j*D` for elapsed days `d`, horizon days
   `D` and node index `j`. Retain the true event date for source
   lookup and the kernel's canonical `T*(j/N)` label for the
   admitted index. Use the same integer geometry to identify
   exact dates at uniform knots without scheduled events. Reject
   missing or out-of-domain events, overflow and resource or fit
   failures. Remove node snapping, step-count search, cash splitting
   and compensating discount adjustments. Evaluate targets on actual
   dates using the qualified source's native clock. At genuinely
   off-date steps, interpolate the authorized adjacent-day native
   coordinate and evaluate the source there. Replay origin-relative
   unspread factors with the owner's explicit fit tolerances before
   pricing. BDT and BK require strictly decreasing positive targets
   even at zero sigma. Exact zero sigma retains the chosen model's
   one-state limit, grid, discount law and exercise policy. A qualified
   origin-only contractual payoff with no later model events is
   evaluated directly after validation, without fitting a future
   interval. Use checked rollback and propagate required input and
   arithmetic errors.
   [Exact TimeGrid](../core-numerics/time-grids.md),
   [dated source clocks](../models-analytic/rate-clock-and-projection.md),
   [Hull–White event grids](../models-numerical/hull-white-event-grid-and-affine-bonds.md),
   [BDT](../models-numerical/bdt-lognormal-curve-fit.md) and
   [BK](../models-numerical/black-karasinski-trinomial.md).

5. **Identify cash and principal by their contractual event phase.**
   Determine which prior and co-dated balance changes apply before
   exercise. Use that remaining economic principal `N_ex` before the
   exercise itself and before any ordinary final redemption it
   replaces. Earlier amortization remains applied. Counterfactual
   ordinary principal payable on hold stays in continuation even
   when paid at a later adjusted date. Pay a vanilla clean
   percentage `p` as `N_ex*p/100 + A`, with finite positive `p`
   and eligible positive outstanding principal. `A` is the accrual
   actually payable to this holder on redeemed principal. Determine
   `A` from the governing terms, original coupon parent and
   entitlement. The signed market settlement-accrued amount has a
   separate purpose. Pay co-dated unconditional cash once outside
   the comparison. Any detached or vested later cash payable on
   both branches enters both at the same conditional value, or is
   carried separately through the same pricing basis. Retain each
   claim exactly once. Require qualified co-dated ordering, balance,
   cash identities and payable redemption amounts before rollback.
   [Bond schedules and principal](01-schedule-and-holder-cashflows.md#accepted-decision-for-checked-bond-schedules-and-holder-cash)
   and [settlement accrued](02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks).

6. **Admit one explicit deterministic-reference make-whole law.**
   Use this profile for issuer calls only, when the terms authorize
   full benchmark payments discounted before subtracting accrued.
   Require the contractually qualified benchmark and its currency,
   explicit terminal horizon and complete promised payment strip
   for the redeemed principal. Use eligible strictly future
   principal and interest on actual dates. Exclude same-day cash
   and independently retained claims. Resolve final principal,
   terminal coupon and vested claims from the governing terms.
   Require the expressly designated reference curve and signed
   annual continuous reference spread `s_ref` on its full native
   clock. Compute
   `P_ref=Σ Cash_i*D_ref(E,T_i)*exp(−s_ref*(c_ref(T_i)−c_ref(E)))`,
   using the checked source-relative log-factor law. Here
   `E` is the actual redemption date and `T_i` the benchmark
   payment date. Pay `max(F,P_ref−B)+A`, where `F` is the
   contractual clean floor on redeemed principal, `B` is the
   contract's accrued deduction and `A` is the holder's separately
   payable accrued. When `B=A`, the equivalent dirty redemption
   is `max(F+A,P_ref)`. Reference cash and factors are deterministic
   across rate nodes. Keep benchmark spread, tree OAS and hazard in
   their distinct roles. Use separate qualified source bindings for
   benchmark and base calibration. A shared curve identity is
   admissible when it qualifies for both designated roles, and a
   shock to that identity affects both. A quoted Treasury yield with
   nominal semiannual discounting, accrued stripped from the next
   coupon before discounting, an unmodeled benchmark observation
   date or a synthetic par-call strip without qualified cash are
   outside this formula. Reject make-whole puts. Share payable
   strike preparation with workout pricing where the contractual
   profile qualifies. Workout selection and stochastic benchmark
   models have their own owners.
   [Workout yield](11-workout-yield.md),
   [joint policy](07-lsmc-policy.md),
   [filed PV-before-accrued terms](https://www.sec.gov/Archives/edgar/data/72971/000119312522055987/d273152d424b2.htm)
   and [filed next-coupon adjustment](https://www.sec.gov/Archives/edgar/data/1451505/000145150510000083/exhibit4_1.pdf#page=17).

7. **Resolve the holder put before the issuer call.** At an
   eligible shared decision phase, take the maximum payable put
   and minimum payable call among same-side alternatives on the
   same terms and entitlement basis. Reject incompatible rights,
   including a payable put above the call or unsupported priority.
   Given hold continuation `H`, first choose the put if its
   payable amount strictly exceeds `H`. Then choose the call
   only if that post-put value strictly exceeds
   `C + N_ex*f/10000`, where `C` is the selected payable call
   and `f` is an explicitly selected finite nonnegative
   threshold in cents per 100 par. Zero elects frictionless
   exercise. Use strict comparisons with no tolerance band.
   On exercise, pay the applicable contractual amount and retain
   shared cash under clause 5. Friction is behavioral and pays
   no cash. With positive friction, continuation can increase
   through the threshold and cause a downward jump in node value.
   OAS inversion and finite-difference risk require their own
   eligibility and boundary treatment.
   [Shared model controls](../valuations-framework/model-resolution.md),
   [OAS inversion](12-spread-quotes.md),
   [curve risk](15-curve-risk.md) and
   [embedded-option risk](18-embedded-option-risk.md).

8. **Apply one explicitly quoted OAS through rollback.** Use
   the bond's required independent quote-compounding field.
   Bond Tree admits signed annual decimal OAS quoted as
   Continuous or nominal Semiannual Periodic under the checked
   node discount law. Continuous shifts `log D` by `−s*h` on the model
   interval. For Periodic convert the node's base quote to an
   equivalent annual continuous rate `c` on that interval
   and shift `log D` by `−2*log1p((s/2)*exp(−c/2))*h`,
   subject to the strictly positive shifted periodic base. Apply
   the same law to the one-state limit. A supplied exact zero
   preserves calibrated base factors and still requires a valid
   quote basis. OAS changes continuation and can change exercise
   decisions. Use unspread transition weights and curve fit,
   keep contract benchmark discounting and projection independent
   of the OAS, and apply no spread across a same-day cash phase.
   Use the same rollback with rights removed for an expressly
   requested straight comparator. Preserve chosen model, prepared
   dates, sources, terms other than the removed rights, and OAS
   experiment. Separately qualify supported settlement-model
   views under record 02. Record 12 owns inversion and quote
   acceptance, and records 15 and 18 own risk and option outputs.
   [Checked tree discounting and OAS](../models-numerical/tree-discounting-and-induction.md),
   [bond quote basis](12-spread-quotes.md#accepted-decision-required-quote-compounding)
   and [contingent settlement](02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks).

9. **Report the selected economics and require checked host parity.**
   Report effective model, rates/source/clock roles, fitted block
   provenance, grid geometry, curve replay and exercise policies,
   and label deterministic benchmark projection and threshold
   effects. Qualify fit separately from option-price and boundary
   convergence. An exact dated grid gives a discrete time and
   state approximation, with no blanket price bound. Expose
   contractual scheduled flows as scheduled claims. A selected
   Tree valuation decomposition or cash trace is available
   only when the model owns it and it reconciles to the price
   and cut. Preserve distinct model price and settlement-quote
   mark outputs. Reconcile Rust preparation and callers, forms,
   schema/state restoration and existing Python and WASM routes
   to the same admission and result contract. Validate complete
   input and restored state and return typed errors or defined
   unavailability. Identify the failed model, binding, event
   date or node where applicable. Bindings convert types and errors.
   [Model provenance](../valuations-framework/model-resolution.md),
   [cashflow export](../valuations-framework/cashflow-export.md) and
   [callable-bond model overview](0028-callable-bond-model-selection.md).

## Observed behavior

### Choice and controls

Public bond trees use `model_config.tree_model`, defaulting to `hull_white`, with a complete explicit or market-resolved swaption-family mean-reversion/scalar-volatility pair keyed by the selected tree discount curve. [Shared scalar resolution](../valuations-framework/model-resolution.md) owns positive/finite and missing/partial-pair rules; the explicit deterministic-zero exception below is bond-specific. The shared resolver rejects `hw1f_sigma_schedule`; piecewise Hull–White sigma is not supported by this bond tree route. BDT requires explicit `bdt_sigma`; BK requires explicit `bk_sigma` and positive `bk_mean_reversion`. Foreign BDT/BK settings are rejected for an incompatible tree selection; option `implied_volatility` is not a short-rate calibration input. Explicit `hw1f_sigma = 0` requires positive mean reversion. HW sigma is annual absolute rate volatility; BDT/BK sigma is annual relative short-rate volatility; mean reversion is per year. Public steps default 100. Private `TreePricerConfig::default` instead uses Ho–Lee sigma 0.01/200 steps; it is not public native dispatch. HW/Ho–Lee use continuous tree compounding, BDT/BK simple. An optional tree discount-curve ID controls calibration and this valuation clock. Source rationale is common model inputs for PV, OAS, option decomposition and vega, including the straight comparator leg; model adoption remains pending. Missing required HW inputs do not substitute Ho–Lee or a default volatility. Rates-only calibration errors propagate; [joint rates-credit calibration](../models-numerical/rates-credit-marginal-calibration.md) has separate fallback behavior and is not covered by that rejection statement.

### Algorithm

1. Lower deterministic return floors to call rights. Build the canonical cashflow schedule and exercise candidates: window endpoints, all schedule cash dates inside the window and month ends. HW adds mandatory cash/exercise times to its nonuniform grid. BDT/BK select a step count from requested to `max(requested,min(4·requested,1000))` minimizing the worst distance of exercise endpoints from integer grid nodes; invalid endpoint time conversions are ignored in this alignment heuristic.
2. Fit the selected model to the discount curve using the owning models-crate tree. BDT/BK explicitly require acceptable reported calibration quality. Stochastic rates-only floating coupons are rejected; zero-volatility trees allow preprojected floating coupons. Zero-volatility valuation uses exact mandatory-event-grid discount ratios.
3. Map a cashflow on an exercise date to the nearest grid point; other cashflows split linearly across adjacent points. Convert each allocated amount by `D(actual event)/D(grid point)` so deterministic PV is preserved. Grid-point dated DFs use log interpolation between surrounding calendar dates. Cash and principal redemption are tracked separately; OAS corrects event-to-grid timing as `exp(−s(event_time−grid_time))`.
4. Option strikes use outstanding after economic balance events, adding maturity principal redemption back for maturity exercise. Clean strike is outstanding times price percentage; add accrued at exercise. Make-whole clean strike is `max(contractual floor, Σ future CF·D_ref(ex,T)·exp(−spread·τ)−accrued)`. Put make-whole terms are not consumed by valuation. Same-date overlapping calls take the minimum strike; puts take the maximum. A put above a call errors.
5. Roll back discounted continuation, first taking the maximum with holder put, then capping at issuer call only if value exceeds `call + outstanding·call_friction_cents/10000`; the friction raises the decision threshold but is not paid. Add current coupons/cash outside that exercise comparison. Terminal principal participates in exercise while other terminal cash is added once. Continuous OAS is a per-step extra factor; noncontinuous quotes convert at each node's short rate.

### Limits, alternatives and located evidence

Discrete candidate grids and cash allocation approximate continuous exercise. Rights whose distinct dates collide on a uniform node can interact; only same-date incompatible barriers are explicitly rejected. Make-whole reference cash is deterministic in rates-only trees, unlike stochastic rates-credit conditional valuation. Daily exercise, explicit notice dates, stochastic index projection and state-dependent reference-curve dynamics would be distinct additional methods.

Owning code: [resolver](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/config.rs), [preparation/route](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/tree_pricer.rs), [valuation and exercise](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/bond_valuator.rs). [Tree tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/tests.rs) include missing HW parameters, market-scalar resolution, explicit BDT selection, window versus endpoint exercise, accrued redemption, deterministic FRNs and OAS round trips; [tree calibration tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/validation/tree_calibration.rs) locate curve-fit/convergence evidence; [exercise-grid tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/exercise_grid.rs) compare candidate approximations with daily grids in selected fixtures; [BK tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/validation/black_karasinski_tree.rs) include limiting comparisons. Evidence is not a blanket error bound. References: [HW 1990](../../../REFERENCES.md#hull-white-1990-pricing-ird), [HW numerical procedures](../../../REFERENCES.md#hull-white-1994-numerical-procedures).

## Independent arithmetic and qualification

- A three-day horizon with three uniform steps admits day 1 at
  index 1 by integer equality. Two steps miss it. With unit
  discount factors, a day-1 call at 100, day-2 coupon of 120
  and day-4 principal of 100 give callable value 100. Splitting
  the coupon between the day-1 and day-4 nodes puts 80 before
  the exercise comparison and produces 180. Both allocations
  produce the same straight value of 220.
- With prior outstanding 1,000, co-dated senior amortization
  of 200, coupon 50 and exercise at 90% of remaining principal,
  the holder receives `200+50+720=970` when the issuer calls.
  A terminal hold principal of 100 paid later at relative DF
  .99 is worth 99 at the decision date. An eligible immediate
  holder put at 100 wins.
- A reference payment of 110 at factor .95, zero benchmark
  spread, clean floor of 100 and accrued deduction and payable
  accrued of 3 produce redemption of 104.50. Reducing the
  next coupon by 3 before discounting produces 104.65 under
  that distinct clause. A clean floor of 105 gives redemption
  of 108 under the accepted formula.
- With issuer strike 100 and outstanding 100, a threshold of
  50 cents per 100 holds at continuation 100.50 and pays
  100 at continuation 100.51. In a two-year tree with unit base
  factors, this call at year 1 and final cash of 110 give
  target price 91.50 at continuous OAS of about 8.8831% and
  9.2071%, on different decision branches. On two half-year intervals
  with nominal semiannual base rates 10% and 40%, a 20%
  nominal semiannual OAS discounts a unit payment by
  `1/(1.15*1.30)=200/299` through nodewise rollback.

Qualify the accepted package with independent bullet and option
benchmarks, dated binding and fit replay, zero-volatility and
domain limits, notice and co-dated entitlements, delayed
terminal principal, make-whole floors and benchmark shocks,
spread and friction boundaries, and convergence across exact
admissible grids. Verify selected-model risk, exports and
host/state restoration on the same prepared contract.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
