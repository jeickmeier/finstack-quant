# MODELS-ANALYTIC-25: HW1F event grid, antithetic streams and bank-account estimator

**Decision status:** Accepted (complete package, 2026-11-10).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and caller paths inspected. Independent Decimal, Gaussian and sampling arithmetic checks were executed for the accepted laws. Production pricing, risk and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for exact joint Hull–White paths and sampling

1. **Use the accepted curve-fitted Gaussian state.** A dated
   run takes the qualified centered Hull–White model, its
   origin, active volatility and deterministic curve shift.
   Set `x_0=0` and the log bank account to zero. Use model
   years from the same origin for events and steps. Derive
   the actual short rate from the shift when a caller needs
   it. Remove a separate fitted-path initial-rate override
   and the fitted theta path.
   [MODELS-ANALYTIC-24](rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection)

2. **Step the endpoint and integrated rate jointly.** For
   the actual adjacent times a<b, put `h=b-a`,
   `A=exp(-kappa*h)` and use B from record 23. With
   `Phi(a,b)=integral_a^b phi(t)dt` from record 24, update
   `x_b=A*x_a+eta` and
   `log_bank_b=log_bank_a+B(a,b)*x_a+Phi(a,b)+xi`.
   The centered noises (eta,xi) are jointly Gaussian
   with covariance entries V(a,b), J(a,b) and Q(a,b)
   in state-then-integral order. Brownian increments on
   disjoint intervals supply independent innovations.
   [MODELS-ANALYTIC-23](hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures)

3. **Use the conditional covariance factor.** Sample with
   a lower-triangular root with nonnegative diagonal, so
   `eta=s*Z_state` and `xi=l*Z_state+r*Z_integral`,
   where `s^2=V`, `s*l=J` and `l^2+r^2=Q`.
   Keep both noise loadings for a positive-length segment
   with positive sigma. The two stochastic integrands
   are linearly independent on such a segment. Both
   innovations are exactly zero on intervals with no
   supported volatility.

4. **Build constant-sigma roots with their exact limits.**
   For segment length h, let `z=kappa*h` and
   `E(z)=-expm1(-z)/z` with E(0)=1. The endpoint
   variance is `sigma^2*h*E(2*z)` and the covariance
   is `sigma^2*B^2/2`. The residual integral variance
   is `sigma^2*h^3*[1-tanh(z/2)/(z/2)]/z^2`.
   Use the small-z continuation of the bracketed ratio,
   beginning `1/12-z^2/120+17*z^4/20160`. At kappa=0,
   the exact variances and covariance are `sigma^2*h`,
   `sigma^2*h^3/3` and `sigma^2*h^2/2`, with
   `sigma^2*h^3/12` for the conditional residual.
   Evaluate the complete loadings in scale, with the
   original units retained at large z.

5. **Compose scheduled roots in time order.** Split the
   interval's deterministic preparation at exact sigma
   knots. For a preceding root (s,l,r) followed by a
   segment with root (d,e,f), decay A and loading B,
   put `p=A*s`, `q=B*s+l` and `S=hypot(p,d)`.
   For positive S, take `u=p/S` and `w=d/S`. The new
   root is `(S,u*q+w*e,hypot(r,f,u*e-w*q))`.
   Use scaled orthogonal arithmetic. A true noise-free
   interval has a zero root. Internal coefficient knots
   require neither payoff events nor extra random draws.

6. **Preserve the integral residual.** Avoid subtraction
   of rounded `Q-J^2/V` or `1-rho^2`. A positive
   schedule can have nearly collinear rounded covariance
   entries or individually unrepresentable variances
   while the loadings remain usable. Retain needed scale
   through the step and expose unresolved numerical
   range as a computation error. Remove covariance
   clipping, singularity floors and silent decorrelation.

7. **Keep the exact discount law at sampled times.**
   Conditional on x_a, the expectation of
   `exp(log_bank_a-log_bank_b)` is the conditional
   discount bond from record 24. The Gaussian integral
   contributes Q/2 in the log moment, cancelling that
   term in the deterministic shift integral. With zero
   volatility from the model origin through b, relative
   discounting follows the rebased curve. Retire endpoint
   trapezoid integration and
   reconstruction of exact bank accounts from rate-only
   captures.

8. **State what exact stepping covers.** The joint law at
   the admitted nodes has no time-discretization error
   for the Hull–White endpoint and integrated short rate
   in real arithmetic. Finite precision and pseudorandom
   normals still affect computation. Products own
   approximations for monitoring between nodes,
   exercise, accrual observations and other state
   dynamics.

9. **Use one checked grid with an explicit horizon.**
   Use the exact union of the origin, terminal time,
   required event coordinates and any explicit extra
   sampling knots from the owner. Use actual adjacent
   stored times and CORE-NUMERICS-20's exact lookup.
   Require finite nonnegative coordinates in horizon
   and checked resource admission. Remove implicit
   monthly and minimum-step subdivisions and terminal
   epsilon logic from the Hull–White driver.
   [CORE-NUMERICS-20](../core-numerics/time-grids.md)

10. **Preserve event identities and order.** A product
    supplies its chronological fixing, observation,
    exercise and payment events and the order of any
    ties. Distinct records at one time use the same
    state and dispatch in that order. Dispatch requested
    time-zero and terminal events at their actual nodes.
    Distinct represented times remain distinct. An
    extra sampling knot has no financial event unless
    the owner requests one.

11. **Check the run before starting work.** A stochastic
    price run needs a positive horizon, at least one
    payoff event, valid admitted model and complete
    controls. The zero grid can express a deterministic
    time-zero case with no random step. Check actual
    counts, conversions, step/stream/leg products,
    stream address limits and requested allocations.
    Prepare common interval coefficients once. Propagate
    allocation and numerical failures without partial
    successful estimates.

12. **Expose only required checked path quantities.**
    Each simulated leg starts with fresh payoff state.
    Give event consumers the exact node, centered state
    and checked requested observations. Derive a short
    rate only for consumers that need it. Preserve
    matching state and log bank for exercise or required
    captures. Payoff state cannot rewrite the model
    coordinates, bank account or event ordering.

13. **Accumulate the bank in log units.** Store the
    integrated annual decimal rate in model years.
    At the origin the account is one. Use checked
    ordered accumulation with adequate scale and
    precision. For relative discounts retain the
    required interval information when subtracting
    rounded prefix logs would erase it. Exponentiate
    only for a requested account, discount or complete
    discounted amount. Honor each requested result's
    range independently.

14. **Discount at the actual cash payment time.** Under
    the money-market measure the path PV of cash C
    payable at b is `C*exp(-log_bank_b)`. From a to
    b, use the same-path factor
    `exp(log_bank_a-log_bank_b)`. Use checked signed
    scaling and carry the complete PV through any
    unrequested out-of-range factor. A simulated fixing
    uses record 24's path projection and keeps its
    correlation with the bank. Apply discounting once.

15. **Require complete payoff accounting.** Carry dated
    pending cash to its payment event and require
    necessary path observations. Remove missing-state
    rate or unit-bank defaults, deterministic-curve
    substitution for missing simulated discounts and
    final-node settlement of earlier pending cash.
    Report bad currency tags, missing observations,
    inconsistent event state or failed arithmetic.
    Aggregate each completed leg's time-zero PV in the
    requested currency using accepted Money boundaries.
    [CORE-CONVENTIONS-012](../core-conventions/012-money-decimal-precision.md)

16. **Report deterministic work literally.** When the
    owner resolves a deterministic cash-only outcome,
    it may evaluate its checked dated curve PV directly.
    An exact outcome represented in estimate fields
    has zero sampling error, point bounds and zero
    simulated paths. A completed simulation can also
    have constant observations. Its counts still
    reflect its actual work.

17. **Require explicit simulation controls.** The plain
    driver takes the independent base-stream count,
    effective u64 seed and antithetic selection. Require
    at least two pricing groups. Remove the generic
    embedded HW path-count/seed/pairing defaults, path
    floors and exercise-only fields from plain pricing.
    Product policies can supply these inputs explicitly.
    Regression, training and exercise controls stay
    with their owners.

18. **Preserve the resolved seed.** A literal seed,
    including zero, is valid and is used as given.
    An owner may explicitly choose the existing stable
    instrument-and-scenario derivation and pass its
    result to the driver. Use the existing documented
    UTF-8 instrument bytes, delimiter and scenario
    FNV-1a derivation when selected. Do not silently
    replace a literal seed or infer a scenario. The
    derivation does not promise collision-free labels.

19. **Pin the Philox draw assignment.** Use the shared
    Philox4x32-10 profile with effective seed as key
    and zero-based base index as substream ID. Start
    the counter at zero. Each positive grid interval
    consumes one four-word block, making two open
    52-bit midpoint uniforms and one checked reflected
    Box–Muller pair in state-then-integral order. Keep
    that draw assignment at deterministic intervals.
    Zero-time dispatch consumes no draw. Reject failed
    draws and prevent counter wrap.
    [CORE-NUMERICS-17](../core-numerics/pseudorandom-and-bridge.md)

20. **Mirror the entire innovation pair.** The antithetic
    partner shares its base substream and negates both
    ordered normals at each step. It has a fresh payoff
    and model state. Payoff branches do not shift the
    shared random address sequence. A parameter-only
    bump with the same declared seed and grid retains
    the same draw assignment. A changed grid can
    change that coupling.

21. **Preserve logical run order.** Use fixed-count
    simulation, ordered base-stream and leg results and
    stable error attribution. If parallel execution is
    used, apply the accepted deterministic partition
    and reduction rules. Preserve the actual resolved
    seed, pairing and grid/step settings in replay
    information. Target/build and math settings are
    part of the reproducibility boundary.
    [ADR 0043](../../standards/0043-deterministic-ordering-and-monte-carlo-execution.md)

22. **Use one observation per pricing base group.**
    Use the scalar PV for an unpaired base or a
    stable checked midpoint of both PVs for a pair.
    Validate complete groups and every finite
    currency-qualified leg. Preserve the midpoint at
    extreme scale. Use accepted ordered checked
    statistics with at least two groups. Plain Monte
    Carlo uses every completed base group.

23. **Retain the explicitly labelled 95% sampling interval.**
    Call the accepted core Student-t mean interval with
    alpha=0.05, n equal to the pricing group count
    and n−1 degrees of freedom. The standard error
    is sample SD divided by sqrt(n). Constant eligible
    samples give zero estimated dispersion. Preserve
    required precision, error and interval bounds.
    The interval describes sampling uncertainty and
    generally has approximate coverage for nonlinear
    payoffs. It excludes model, calibration, grid and
    exercise-policy error.
    [CORE-NUMERICS-09](../core-numerics/sample-statistics.md)

24. **Keep training with the exercise engine.** An
    exercise owner may select training and pricing
    groups. Partners stay in the same group, and
    only the designated pricing groups enter reported
    sampling statistics. Record 26 owns issuer policy,
    split selection and sparse continuation behavior.
    Held-out sampling uncertainty is conditional on
    an independently frozen training policy when that
    design is used.
    [MODELS-ANALYTIC-26](hw1f-issuer-lsmc.md)

25. **Report actual estimator counts and work.** Record
    independent pricing base groups, the legs contributing
    to them, and total simulated legs when training or
    other work makes that count different. Validate
    counts and finite results on published state and
    restoration. Use the existing result and metadata
    surfaces where applicable. Preserve LMM's separate
    terminal-measure scale and controls when reconciling
    shared grid and pair helpers.

26. **Reconcile cash and exercise consumers.** Route
    callable range accrual, Snowball, TARN and the
    separate HW Bermudan implementation through the
    actual joint bank path and dated events. Use
    relative bank discount for continuation or exercise
    cash at its stated time. Preserve owner-specific
    holder or issuer decisions and coupon ordering.
    Reconcile the MBS OAS discount path and qualify
    its payment timing and explicit spread separately.

27. **Reconcile correlated paths and existing hosts.**
    Revolving credit keeps its simulation origin and
    credit/utilization driver ownership. Qualify the
    joint covariance with those drivers before claiming
    exact path discounting there. Reconcile existing
    Python and WASM product controls, diagnostics,
    numeric representation and error mapping with
    canonical Rust. Keep low-level joint path preparation
    and payoff callbacks Rust-only.

28. **Qualify the law and the full pricing paths.**
    Compare scheduled covariance roots and composition
    against independent integrals, including Ho–Lee,
    no-noise and near-collinear cases. Check analytic
    discount moments, discounted conditional bonds
    and caplets, actual cash timing and event ordering.
    Verify both mirrored shocks, seed and grid replay,
    complete group counts and confidence arithmetic.
    Use documented tolerances and independently qualify
    actual product, risk and host results.
    [Exact joint path references](../../../REFERENCES.md#exact-joint-hullwhite-paths-and-sampling)

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| Ho–Lee, flat curve 3%, sigma 0.02, one five-year interval | Expected unit discount under the exact joint law is 0.860707976425. Endpoint trapezoids using the exact endpoint rate law give 0.855345327307. |
| Kappa 0.18, sigma (0.006, 0.022, 0.011, 0.016) at (0, 0.4, 1.7, 3.1), interval [0,6] | Independent moment composition gives V/J/Q approximately 0.000616263377 / 0.001748557452 / 0.009557282117. The prepared lower root has coefficients 0.024824652598 / 0.070436331195 / 0.067793844591. |
| Ho–Lee, sigma 1 only on the first 2⁻⁶⁰ years of [0,1] | V, J and Q round to the same binary64 value. The true residual standard deviation is about 2.33189916663e-28. |
| Constant 5% short rate, next reset at 1, cash due at 1.25 | Payment discount is 0.939413062813. Discounting at the reset gives 0.951229424501. |
| Antithetic legs [1,1] and [3,3] | Pair observations are 1 and 3, giving mean 2, standard error 1 and 95% half-width about 12.7062. Treating all four legs as independent gives standard error about 0.57735. |

These examples check the formulas and bookkeeping. The five-year
single-step trapezoid is an illustrative comparison. Production and
end-to-end qualification remain pending.

## Observed implementation and evidence

### Scope and input domain

Rate-dependent payoffs implementing Payoff; supplied HW1F process parameters, initial rate, positive increasing event times and currency. Payoff returns time-zero path value using supplied bank-account state.

### Choice and rationale

Exact marginal HW rate stepping is combined with trapezoidal bank-account integration. Antithetic partners are one estimator for standard-error purposes; this does not make the integrated-rate discount exact.

### Algorithm

For each event gap, subdivide n=max(min_steps,ceil(12gap)) and hit event exactly; min_steps floors1. Optional terminal segment extends to maturity. For each base path use Philox(seed).substream(path_id); antithetic replays identical stream with z sign reversed. Step short rate with ExactHullWhite1F, update bank by exp[.5(rstart+rend)dt], expose r and bank, invoke payoff.on_event then callback. Collect payoff.value at end. Average complete antithetic pairs before OnlineStats; with OOS mode only odd base streams contribute; construct MoneyEstimate from independent estimator stats.

### Selection and controls

Embedded JSON defaults:10000 base paths,seed42,antithetic=true,min_steps4,basis_degree2; oos_lsmc=false in Rust config. Overrides floor supplied paths1 and always derive seed from instrument ID/scenario (default base), replacing configured seed. At least2 independent pricing estimators required; multiplicity only1/2 and complete pairs enforced. `accumulate_bank_factors` exposes the same trapezoid accumulator separately.

### Conventions

Rate decimal per year; bank starts1, represents accumulated money-market account. Callback payoffs own conversion to PV; harness aggregation uses scale1, avoiding another discount. Paths count base streams; simulated paths double with antithetics.

### Limitations, failure and fallback

Trapezoid bank integration retains time-discretization bias. OOS mode on nonexercise MC still discards half streams. Low-level bank accumulator trusts path/grid length compatibility. Input process/r0/payoff finite-value contracts rely partly on downstream components. Standard error measures sampling error, not grid/parameter/model error.

### Alternatives and tradeoffs

Exact joint short-rate/integral sampling would reduce discount discretization error but is not used here. Independent or antithetic streams are supported; QMC is not selected by this harness.

### Implementation and validation evidence

[mc.rs](../../../../finstack-quant/models/src/rates/hw1f/mc.rs), [bank_account.rs](../../../../finstack-quant/models/src/rates/hw1f/bank_account.rs), [config.rs](../../../../finstack-quant/models/src/rates/hw1f/config.rs) and [embedded defaults](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json). Constant-rate bank exponential and pair-aggregation/singleton tests establish deterministic/statistical bookkeeping; full stochastic grid convergence is separate evidence. Reference: [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo).

[Bermudan holder valuation](../../../../finstack-quant/valuations/rates/src/swaption/pricing/monte_carlo_lsmc.rs)
reconstructs bank factors from stored endpoint rates.
[Snowball](../../../../finstack-quant/valuations/exotics/src/exotics/snowball/pricer.rs)
and [TARN](../../../../finstack-quant/valuations/exotics/src/exotics/tarn/pricer.rs)
have next-reset and final-node pending-cash handling, and their
[shared estimate helper](../../../../finstack-quant/valuations/exotics/src/exotics/hw1f_note.rs)
can report configured path counts for a deterministic result.
[Callable range accrual](../../../../finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/pricer.rs)
has fallback state and discount paths.
[MBS OAS](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/metrics/mc_oas.rs)
uses a left rate and a payment-delay proxy.
[Revolver path pricing](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_pricing.rs)
uses rate interpolation and trapezoids. Its
[joint credit-line step](../../../../finstack-quant/models/src/monte_carlo/discretization/credit_line.rs)
needs separate correlation qualification.
[LMM Bermudan](../../../../finstack-quant/valuations/rates/src/swaption/pricing/lmm_bermudan.rs)
shares event-grid and pair helpers under its terminal measure.

## Follow-up tracking

Implementation and validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
