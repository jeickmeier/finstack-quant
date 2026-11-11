# MODELS-ANALYTIC-26: Issuer-call LSMC continuation, sample split and sparse-regression fallback

**Decision status:** Accepted (complete package, 2026-11-10).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and caller paths inspected. Independent exact cash, rank and stopping-policy checks and a Gaussian callable-bond benchmark were executed. Production pricing, risk and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked issuer-call policy training and pricing

1. **Value the holder's cash under issuer minimization.** The
   issuer chooses between exercise and continuation to minimize
   the holder's expected present value. Use the qualified
   curve-fitted Hull–White model and joint state and log-bank
   paths. Apply position signs and portfolio perspective at
   their owners. Holder exercise has its own maximizing
   policy.
   [MODELS-ANALYTIC-25](hw1f-monte-carlo-discounting.md#accepted-decision-for-exact-joint-hullwhite-paths-and-sampling)

2. **Require exact ordered decisions.** Admit one finite
   decision per qualified exercise time, strictly increasing
   and mapped to its exact event node. Preserve the ordered
   observation, payment and decision labels at a tied node.
   An owner-resolved time-zero or terminal decision is eligible
   when its information and cash ordering are explicit.
   Reject duplicate, unordered and off-grid decisions.
   [CORE-NUMERICS-20](../core-numerics/time-grids.md)

3. **Take eligibility from decision-time information.** The
   product supplies live status and exercise eligibility
   after the events available to that decision. A temporarily
   ineligible live path continues and can qualify later.
   Irreversible termination ends later rights. Exercise
   amounts, including zero and contractually admitted signed
   amounts, do not substitute for eligibility. Apply owner
   rules for lockouts, same-day cutoffs and prior call status.

4. **Use the qualified same-node settlement contract.** The
   selected kernel admits an exercise branch whose exclusive
   redemption is payable at the decision time and known at
   that decision. Notice, later effectiveness or delayed
   redemption requires an owner-qualified branch model and
   lies outside this same-node contract. Require resolved
   live, called-but-unpaid or settled state when past decisions
   affect entitlement.

5. **Use ordinary valuation when there is no live call.**
   Use the checked non-exercise simulation for a live note
   with no remaining call opportunity. Already-resolved cash
   uses its actual outstanding schedule. A deterministic
   valuation reports its actual work. The issuer-policy
   training route requires a nonempty qualified decision set.

6. **Represent actual dated cash on both branches.** The
   payoff supplies each branch's obligations and payment
   times. Preserve cash already paid, common scheduled cash,
   and the obligations extinguished or replaced at the call.
   Remove the default `value_after` and the subtraction of
   two completed time-zero PVs as a cash partition. A bullet
   with no common coupon supplies that fact explicitly.

7. **Exclude only genuinely common cash from comparison.**
   Cash survives both choices when its entitlement, amount
   rule and payment time are unchanged under the applicable
   branches and later policy. Keep it once in realized PV.
   A same-day coupon can be common when the owner specifies
   that order. Accrued coupon paid earlier upon call is
   branch-dependent cash.

8. **Compare the actual call cash in decision units.** For
   a qualified immediate call at node i use `K_i`, the
   full call-exclusive cash payable there in the note
   currency. For callable range accrual this includes the
   redemption amount and accrued coupon paid on call, with
   continuation cash represented on its own due date.
   Remove the synthetic retained terminal accrual and
   conditional bond adjustment from realized call cash.

9. **Use the realized continuation under later fitted rules
   as the training response.** For decision time t_i,
   sum the continue-exclusive dated cash under the already
   fitted later decisions, using each same-path discount
   `exp(log_bank(t_i)-log_bank(t_pay))`. This is a
   response in decision-time currency units. A same-node
   continue-exclusive payment has discount one. Fit its
   conditional expectation from the admitted current state.

10. **Call only on strict holder-cost improvement.** For an
    eligible state, call when `K_i < C_hat_i`, where
    `C_hat_i` is the fitted continuation in the same units.
    Continue at equality. A call replaces the extinguished
    cash with the actual exercise settlement and preserves
    all common cash. Apply the identical rule on training
    rollback and frozen-policy replay.

11. **Keep cash and predictions checked.** Use the accepted
    log-bank and Money scaling, ordered accumulation, finite
    checks and currency validation. Retain complete scale
    for response, comparison and PV, including close and
    signed values. Preserve missing snapshots, bad path
    state and unresolved arithmetic as errors. No synthetic
    zero exercise value, unit bank or continuation clipping.

12. **Require a fixed decision-state basis.** The product
    declares the finite, fixed-width feature row at each
    decision, with one intercept. Capture it using only
    information available at that decision. Validate lengths,
    finiteness and allocation before fitting or evaluation.
    Preserve the schema and ordering on training and
    pricing paths.

13. **Include payoff-relevant path history.** For callable
    range accrual, use the relevant current Hull–White state
    and accrued in-range observations in the decision state
    and basis where they vary. Carry eligibility and accrued
    cash separately from rate. The product qualifies the
    resulting conditional approximation across its decision
    states.
    [Callable range accrual owner](../valuations-fx-and-exotics/callable-range-hw.md)

14. **Remove redundant time terms from the fixed-date fit.**
    A selected univariate short-rate polynomial uses literal
    degree 0 through 3 with features `1,z,...,z^degree`
    on its stated state coordinate. Each decision has its
    own fit. Remove `t*z` and `t*z^2`, which duplicate
    columns at a fixed date. Drop the implicit degree default
    and threshold bucketing. Product-specific features are
    explicit.

15. **Use the intercept for a structurally common decision
    state.** If the decision information is known to be
    common across all paths, use a declared intercept-only
    fit with the checked training mean. This includes a
    qualified time-zero decision. Identical sampled feature
    rows alone cannot establish the structural case. A pricing
    path with different information cannot use a policy
    declared for a common state.

16. **Scale the admitted regression deterministically.** Keep
    the intercept equal to one. Scale each nonconstant
    feature column by its training maximum absolute value
    and scale the response by its maximum absolute value.
    An all-zero response stays zero. A zero non-intercept
    column fails the fit. Preserve the exact chosen scales
    with the coefficients and apply them to pricing rows.
    Restore predictions in decision-time units using checked
    scaled products and sums.

17. **Use checked full-rank least squares.** Use one bounded
    singular-value decomposition of the scaled design with
    machine epsilon as its convergence tolerance and an
    explicit positive iteration limit. Require finite factors,
    positive largest singular value and a strictly positive
    smallest-to-largest singular-value ratio above the
    caller's finite relative tolerance in (0,1). Solve on
    all admitted directions. Propagate convergence and
    output failures without normal equations, pseudoinverse
    truncation or ridge repair. The intercept-only case uses
    the checked mean.

18. **Fail sparse live fits.** For p fitted columns, require
    at least max(2,p) training base groups with eligible
    observations, sufficient rows and full numerical rank.
    Zero observed eligible rows cannot prove contractual
    ineligibility. Exclude structurally unavailable decisions
    in the owner's schedule. Report a sparse, malformed or
    rank-deficient fit with its date and actual coverage.
    Remove the magic extra-row threshold and the fallback
    that compares exercise with the same path's future cash.

19. **Require explicit disjoint training and pricing groups.**
    Use record 25's effective seed and pairing and separately
    supplied positive training and pricing base counts.
    Training uses the first base IDs and pricing the following
    disjoint IDs. Validate total counts and at least two
    pricing groups. Keep both antithetic legs in the same
    partition. Remove implicit in-sample pricing, parity
    splitting and default exercise controls from this route.

20. **Fit on eligible training rows only.** Each eligible
    training leg contributes its own equally weighted
    feature and response row. Antithetic legs can occupy
    different states or have different eligibility. Count
    each base group once for the coverage gate. Pricing rows
    are unavailable to fitting, scaling, basis selection or
    any choice among policies.

21. **Roll the training decisions backward.** Start from the
    no-further-exercise cash. At each earlier date, construct
    responses using the later frozen training rules and fit
    that date's continuation. Update the training path
    cash to reflect that date's selected branch. Use ordered
    resource-checked state and cash capture. Training
    outcomes never enter the reported pricing mean.

22. **Freeze and replay forward.** After training, freeze
    each decision's feature and cash conventions, transforms
    and coefficients with the actual model/event identity.
    Replay chronologically on the independent pricing
    groups, applying the first eligible call selected by
    the policy and settling the corresponding cash.
    Use only each pricing path's state available at its
    decision. Reject mismatched policy state or input.

23. **Report policy and work diagnostics honestly.** Report
    eligible training groups and fit failures by decision,
    actual training and pricing work and pricing estimators.
    Form pricing means and the 95% sampling interval from
    complete record 25 pricing base groups. Do not combine
    training outcomes with pricing observations. Preserve
    stable replay order and existing currency-tagged result
    boundaries.

24. **State the value being estimated.** Conditional on a
    fitted admissible policy, independent pricing estimates
    that policy's holder PV. Its population value is at
    least the issuer-optimal holder PV under the same
    model, information and decision set. The realized
    sample estimate may fall on either side. Sampling
    uncertainty is conditional on the fitted policy.

25. **Remove the asserted in-sample and held-out bracket.**
    Finite training, feature choice and policy approximation
    can affect results. In-sample fitting has no generally
    valid bias direction for this estimator. Assess policy
    suboptimality and any bound on the optimal liability
    through independent evidence.

26. **Reconcile the actual callable range-accrual route.**
    Use the owner's same-day observation order, exact
    index periods, earned accrual and redemption schedule.
    Preserve surviving terminal coupon and principal
    without duplicating call cash. Reconcile rate risk
    through the owner's declared policy treatment and
    record 25's draw coupling.

27. **Reconcile existing host and shared boundaries.** Carry
    checked controls, currency, actual counts and policy
    diagnostics through existing Python and WASM product
    routes. Keep internal exercise callbacks and fitted
    policy state in Rust. Reuse checked regression or
    statistics machinery where applicable. Generic LSMC
    and the holder Bermudan engine retain their distinct
    exercise and policy contracts.
    [Generic exercise](../models-monte-carlo/23-lsmc-exercise.md),
    [regression machinery](../models-monte-carlo/24-regression-policies.md)

28. **Qualify the policy and cash independently.** Compare
    a single-call bond with the analytic bond-option
    decomposition and multi-date policies with a small
    independent stopping benchmark. Exercise actual
    accrued-state dependence, same-date and prior cash,
    date validation, sparse and rank failure, signed
    call values and held-out replay. Qualify actual risk
    and host paths and record the limits of the fitted
    policy.
    [Issuer-call references](../../../REFERENCES.md#checked-issuer-call-policy-training-and-pricing)

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| One decision, call cost 10, identical current information, future continuation 0 or 30 with equal probability | The issuer-optimal holder value is 10. Always continuing gives 15. Using the realized future to choose gives 5. |
| Fixed exercise time 1.25 with six distinct rates | The existing quadratic and cubic designs have exact ranks 3 of 4 and 4 of 6. The cubic without fixed-time products has rank 4 of 4. |
| Same rate, call principal 100, unearned continuation value 100.50, conditional final bond 0.95 | With accrued 5, call 105 is below continuation 105.25. With accrued 15, continuation 114.75 is below call 115. |
| Immediate principal 100 and accrued 10, path discounts 0.98 to decision and 0.90 to maturity, conditional bond 0.95 | Actual call cash has PV 107.80. The synthetic accrued-coupon adjustment plus retained terminal accrual gives 107.49 on this path. |
| Common cash 6 at discount 0.99 and 7 at discount 0.98 | Call-exclusive 105 at discount 0.98 gives total 115.70. Continue-exclusive 120 at discount 0.90 gives total 120.80. Each branch includes the two common payments once. |
| Flat 3% discount curve, kappa 0.05, sigma 0.02, 100 face due at year 5 callable at 96 in year 2 | The straight bond less the bond call gives 85.0525230976. Independent Gaussian integration of the lesser of call cost and conditional bond value agrees. |

Exact rational arithmetic verifies the discrete fixtures. The
single-call benchmark compares analytic option decomposition and
Gaussian quadrature. Production and complete-path qualification
remain pending.

## Observed implementation and evidence

### Scope and input domain

Callable rate payoffs implementing ExerciseBoundaryPayoff. Exercise values are undiscounted issuer call amounts. The `value_after` contract is time-0 pathwise PV of cashflows strictly after the exercise date; coupon-bearing payoffs must override it accordingly. Its default returns the full payoff PV and is valid only for a bullet payoff paid after every exercise date. The actual minimization is not a generic holder-option exercise rule.

### Choice and rationale

Backward regression chooses issuer exercise when call amount is below predicted continuation. Optional split-sample fitting holds out odd base streams. Sparse regression uses realized future continuation pathwise, introducing look-ahead.

### Algorithm

Simulate full paths and record r,bank,exercise amount,inactive flags. Save pre-exercise PV=full PV−value_after. Work backwards; train only active paths with positive exercise amount (and even base streams under OOS). Regress target=(current PV−pre PV)×bank on payoff-supplied basis using shared least-squares solver when active count>num_basis+2. For every active path with positive amount, if amount<predicted continuation replace PV=pre PV+amount/bank. If samples insufficient, compare amount directly to realized (PV−pre PV)×bank instead. Aggregate pair means; held-out pricing streams alone contribute under OOS.

### Selection and controls

Default in-sample (oos=false). Antithetic partners stay together via p/multiplicity parity. Standard ShortRateBasis degree<3 gives[1,r,r²,tr]; degree≥3 gives[1,r,r²,r³,tr,tr²]; configuration degree is consumed by product payoff construction, not automatically by this harness. Exercise times only checked for nonempty and membership in events within1e−12; mapping scans in provided order.

### Conventions

Cashflows on or before exercise remain in PV when issuer calls only when the payoff implements the `value_after` contract correctly. The default makes pre-exercise PV zero, so it does not preserve intermediate or same-date coupons. Exercise basis has rate units and fixed-time redundant columns (tr proportional r); shared least-squares regularization/rank behavior matters. The result uses the pricer's configured currency. The harness passes that currency to payoff methods, extracts numeric amounts and does not validate their returned Money currency tags; payoff implementations must honor the requested currency.

### Limitations, failure and fallback

Unsorted/duplicate exercise_times are not rejected though one-pass event mapping assumes ascending unique entries; some states can remain default. Sparse fallback uses future outcomes even on held-out paths, so OOS is not an independent fitted policy in that branch. Module prose claims in-sample upper bias and OOS lower complement for callable PV; this direction is not established for the implemented issuer-minimization and must not be adopted as a certified bracket.

### Alternatives and tradeoffs

Holder exercise max rule would be another product-specific method and is not selected. More training paths or explicit sparse-fit failure could avoid look-ahead; cross-fitting/dual bounds are unimplemented alternatives.

### Implementation and validation evidence

[lsmc.rs](../../../../finstack-quant/models/src/rates/hw1f/lsmc.rs), [exercise.rs](../../../../finstack-quant/models/src/rates/hw1f/exercise.rs) and [config.rs](../../../../finstack-quant/models/src/rates/hw1f/config.rs). Inline noexercise_equals_discounted_par and coupon preservation tests check pathwise accounting; neither independently qualifies exercise optimality or bias direction. Reference: [Monte Carlo exercise](../../../REFERENCES.md#glasserman-2004-monte-carlo).

[Callable range-accrual pricing](../../../../finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/pricer.rs)
uses the accrued count and a conditional bond to adjust call
value while retaining terminal-dated accrued cash on the path.
The difference from actual redemption PV is conditionally
mean-zero at an adapted call under the correctly fitted path
law. The path cash differs and can affect earlier continuation
regressions.
It supplies rate-only continuation features. The
[call provision](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/bermudan_call.rs)
supplies call dates, par percentage and lockout.
[Shared least squares](../../../../finstack-quant/models/src/monte_carlo/pricer/lsq.rs)
uses SVD with a singular-value cutoff.
The [holder Bermudan path](../../../../finstack-quant/valuations/rates/src/swaption/pricing/monte_carlo_lsmc.rs)
has its own exercise logic.
[Python](../../../../finstack-quant-py/tests/test_valuations_pricing.py)
and [WASM](../../../../finstack-quant-wasm/src/api/valuations/pricing.rs)
exercise existing product pricing routes.

## Follow-up tracking

Implementation and validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
