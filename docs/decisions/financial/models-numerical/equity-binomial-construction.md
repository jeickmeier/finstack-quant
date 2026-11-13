# MODELS-NUMERICAL-001: CRR and strike-centered Leisen–Reimer construction

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Source and caller paths inspected. Independent Decimal finite-lattice sums, backward recursions and limit checks were executed. Production pricing, risk and host qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked CRR and Leisen–Reimer lattices

1. **Select the construction explicitly.** Retain
   Cox–Ross–Rubinstein (CRR) and strike-centered
   Leisen–Reimer (LR) as two finite recombining
   laws for vanilla calls and puts. The model request
   selects the law and exercise style. Product
   model selection and defaults remain at their owners.

2. **Use the accepted spot-vanilla units and domain.**
   Require finite positive spot S and strike K,
   signed finite continuous annual discount rate r
   and yield q, and finite nonnegative model
   expiry T and annual lognormal volatility sigma.
   The owner supplies coherent carry, variance
   and discount on the declared model-year clock.
   [MODELS-ANALYTIC-01](../models-analytic/spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results)

3. **Keep model premiums in native units.** Return the
   present premium per unit of underlying payoff in
   its original price scale. Position signs, quantity,
   contract multiplier, Money conversion, FX and
   payment or margin timing belong to the product.

4. **Require a checked literal interval count.** A live
   lattice configuration has positive N. Require
   odd N for LR, including deterministic requests
   using that configuration. Admit both parities for
   CRR. Validate the selected method and count at
   construction and ingress. Remove LR's even-to-odd
   adjustment and unchecked mutable configuration.

5. **Use the indexed equal-step model.** The mathematical
   step is h=T/N and the logical node time at index
   j is jT/N. For represented labels use the
   accepted uniform coordinate profile T*(j/N), exact
   endpoints and a checked increasing sequence. The
   lattice takes its own count and horizon. A supplied
   arbitrary TimeGrid does not define this homogeneous
   law. Financial event placement remains with record 002.
   [CORE-NUMERICS-20](../core-numerics/time-grids.md)

6. **Bound work before building the lattice.** Check
   integer and allocation sizes and the owner-supplied
   positive node-evaluation budget. The complete
   stochastic rollback visits (N+1)(N+2)/2 nodes
   and needs O(N) value storage. Check the complete
   planned work before allocation. The deterministic
   route uses a single state per required time and
   exact expiry needs no lattice. Report invalid or
   excessive work without changing N.

7. **Use exact expiry after input admission.** At T=0,
   return the literal terminal call or put intrinsic
   on S and K. Validate the applicable inputs and
   prepared exercise or payoff state before the
   identity. Expiry and actual contractual exercise
   or settlement are distinct product decisions.

8. **Use the single deterministic path at sigma=0.**
   For T>0 with continuous carry alone the path is
   `S(t)=S*exp((r-q)*t)`. Price European terminal intrinsic
   with the date-zero discount. For permitted early
   exercise take the largest discounted intrinsic at
   all eligible nodes and maturity. Preserve the
   complete scaled payoff. An escrowed diffusion state
   and its prepared dividend payoff belong to record 002.
   No proxy variance, fake two-branch tree or
   small-positive-volatility switch.

9. **Use CRR's reciprocal log moves.** For positive
   sigma and T define `a=sigma*sqrt(h)` and
   `b=(r-q)*h`. Use log moves +a and −a and
   mean growth `exp(b)`. Preserve scale through the
   complete products. This matches the first stock
   moment on each step. Finite-step log variance
   depends on the branch probabilities.

10. **Require strict stochastic support.** For CRR use
    `-a<b<a`. Equality is outside the stochastic
    support contract. For homogeneous inputs the
    equivalent exact predicate is `N*sigma^2 >
    T*(r-q)^2`. Use a checked comparison that resolves
    the sign near a boundary. Reject out-of-support
    carry rather than floor, clip or change methods.
    Unresolved arithmetic is a computation failure.

11. **Compute both CRR weights stably.** Mathematically,
    p_up=(exp(b)−exp(−a))/(exp(a)−exp(−a)).
    For `L(x)=log(-expm1(-x))` use
    `log(p_up)=b-a+L(a+b)-L(2*a)` and
    `log(p_down)=L(a-b)-L(2*a)`, evaluated with
    checked local differences and scaling. Preserve
    both strictly positive branches as a complementary
    pair. Remove the absolute move-width floor and
    subtraction of a rounded up probability from one.

12. **Separate CRR spacing from step reweighting.**
    For existing callers with fixed stock spacing
    and different interval carry, take the actual
    admitted per-step net log growth b and apply the
    same strict support and probability law to the
    fixed spacing a. The reweighting operation cannot
    change the node geometry. Do not admit an unused
    placeholder drift or infer credit economics from
    this stock transition.

13. **Use stable strike-relative LR inputs.** Put
    `w=sigma*sqrt(T)` and
    `m=log(S/K)+(r-q)*T` using the accepted checked
    log-forward geometry. Use the two signed
    ordinates `d1=m/w+w/2` and `d2=m/w-w/2`,
    with scale retained through their actual use.
    Preserve their separation and sign without
    an unnecessary spot ratio or squared volatility.

14. **Select Peizer–Pratt method 2.** With the literal
    odd count N, use
    `c=(N+1/6)/(N+1/3+0.1/(N+1))^2`.
    For ordinate z set `beta=c*z^2` and
    `R=sqrt(-expm1(-beta))`. The larger weight is
    `(1+R)/2` and the smaller is
    `exp(-beta)/(2*(1+R))`. The up weight is
    the larger for z>0 and the smaller for z<0.
    At z=0 both are one half. Preserve the small
    ordinate scale when its square underflows.

15. **Use the paired LR weights for spot and stock
    moments.** Write P(z) for the PP2 up weight
    and Q(z) for its complement. Set
    `p=P(d2), q_down=Q(d2)` and
    `u=exp(b)*P(d1)/P(d2)`,
    `d=exp(b)*Q(d1)/Q(d2)`. The stock-weighted
    branches are `p*u=exp(b)*P(d1)` and
    `q_down*d=exp(b)*Q(d1)`. Thus the first
    moment is exp(b) and the positive stochastic
    branches straddle it strictly.

16. **Form LR movement ratios in checked log scale.**
    Use `-beta-log(2)-log1p(R)` for the small
    log weight. For same-sign ordinates use
    `beta1-beta2=2*c*m` and a factored difference
    of the roots. For beta_B≥beta_A use
    `R_B-R_A=exp(-beta_A)*[-expm1(-(beta_B-beta_A))]
    /(R_B+R_A)` and evaluate the larger-weight
    ratio using `log1p((R_B-R_A)/(1+R_A))`.
    Use the paired branch formulas directly for
    opposite signs. Resolve positive movement and
    rare branch scale before rounding a diagnostic.

17. **Keep LR's strike-specific geometry explicit.**
    For positive volatility and odd N the middle
    terminal nodes of the modeled multiplicative
    underlying bracket its reference strike K.
    The construction depends on the strike.
    Qualify cross-strike prices and finite-difference
    risks as repricings on their selected lattices.
    Same-strike parity refers to the same tree.

18. **Preserve homogeneous recombination in node
    arithmetic.** For step j and i up moves use
    `log(S/K)+i*log(u)+(j-i)*log(d)`. Use
    `log(S/K)+(2*i-j)*a` for CRR. For LR the
    two log ratios relative to b sum to −2*c*m,
    which gives a stable centered form. Keep the
    full node coordinate in scale. The tree need
    not materialize `u/d` or two rounded spot
    levels to identify a near-strike payoff.

19. **Evaluate vanilla intrinsic on the original scale.**
    At a reachable node use its signed log distance
    from strike, stable differences and expm1 for
    the relevant positive payoff. Literal equality
    gives zero. Preserve a positive payoff even
    when separately rounded spot and strike would
    coincide. A genuine zero can occur on a finite
    tree with zero payoff at every eligible state.

20. **Use one discounted continuation.** Roll back the
    conditional weighted sum of the two child
    values with the continuous step discount
    `exp(-r*h)`. Keep the smaller probability
    and its mathematical complement as one
    normalized pair. Use stable scaled or log
    arithmetic through the complete sum and
    discount. Signed r can give a factor above
    one. General callback, short-rate and OAS
    discounting rules belong to record 003.
    [Tree discounting and induction](tree-discounting-and-induction.md)

21. **Apply the holder decision at eligible nodes.**
    European exercise pays terminal intrinsic.
    For permitted early exercise compare intrinsic
    with the discounted continuation and take the
    larger value. The American lattice represents
    exercise at its qualified nodes. Bermudan
    eligibility uses the prepared owner event
    indices. Record 002 owns dated eligibility,
    cash dividends and same-node cash ordering.
    [Dividends and exercise](dividends-and-lattice-exercise.md)

22. **Use one requested finite lattice per result.**
    Report errors for invalid controls, unsupported
    stochastic support and unresolved required
    arithmetic. Keep the selected construction,
    interval count and finite-tree payoff. Remove
    probability repair, numerical payoff flooring
    and hidden refinement or fallback.

23. **Check the complete premium.** Return a finite
    nonnegative value, carrying needed scale
    through rollback and early-exercise comparison.
    A positive premium lost to zero is a computation
    error. A valid zero payoff is exact. Require
    representability of requested diagnostics
    independently of the premium. Reuse checked
    scalar primitives without requiring detached
    factors to fit when their complete use fits.

24. **Reconcile the live equity, commodity and
    futures paths.** Propagate exact zero cases,
    method-specific count checks and coherent
    rate/carry inputs through their Rust owners.
    Preserve qualified zero-carry lognormal
    futures geometry and the applicable discount
    convention. Replace commodity ratio-overflow
    and short-horizon carry substitutes with
    checked complete-rate preparation.

25. **Reconcile shared CRR and existing host surfaces.**
    Route convertible fixed-spacing and step-drift
    use through the same checked transition. Keep
    survivor drift, recovery and credit cash with
    the convertible owner. Update actual Python
    and WASM product pricing and risk, controls,
    schema and parity. Odd counts apply only when
    LR is selected. Lattice runtime state remains
    Rust-only.

26. **Keep finite-grid accuracy with the chosen use.**
    Use declared premium-error tolerances in
    original per-unit price units for the intended
    exercise, maturity, moneyness and volatility
    regimes. Qualify positive tail prices separately.
    A successful rollback computes the selected
    finite tree. Continuous-time approximation
    quality and risk convergence require their own
    evidence.

27. **Qualify actual PV and risk configurations.**
    Carry the declared method and grid through
    each sensitivity experiment. Qualify any
    owner-selected difference between official PV
    and risk grids, including the listed futures
    path. Product owners select dated discount and
    volatility adapters, defaults, exercise and
    settlement.

28. **Use independent pricing and numerical evidence.**
    Compare European rollback with independent
    finite-binomial sums and analytic premiums.
    Compare early-exercise regimes with an independent
    qualified reference. Exercise odd and even counts,
    strict CRR support, negative rates, exact zero
    boundaries, tiny positive time value and LR
    tails. Qualify actual product, risk and host paths.
    [Lattice references](../../../REFERENCES.md#checked-crr-and-leisenreimer-lattices)

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| CRR, one step, S=K=100, r=q=0, T=1, sigma=2⁻⁶⁰ | The call is about 4.33681e-17. Both detached binary64 moves round to one. |
| CRR, one step, r=sigma=0.25, q=0, T=1 | Growth coincides with the up move. The strict stochastic support test rejects the boundary. |
| CRR, S=100, K=110, r=−0.03, q=0.015, sigma=0.20, T=1.2, N=25 | Direct binomial summation and rollback give a call of 3.47312085862. On the same tree the European put, put with two early exercise nodes and all-node American put are ordered by their opportunity sets. |
| LR, S=200, K=100, r=q=0, sigma=0.04, T=1, N=5 | The up probability rounds to one. The complementary weight is about 8.15859e-25, and the positive finite-tree put premium is about 3.03838e-71. |
| LR, S=K=100, r=0.05, q=0, sigma=0.20, T=1, N=101 | Direct binomial sum and rollback both give a call of about 10.4505493366. |
| Deterministic call S=100, K=60, r=0.20, q=0.10 with opportunities at 0, 2 and 4 years | The discounted intrinsic values are 40, 41.65387255 and 40.07226676. The interior opportunity is optimal on that grid. |

These Decimal checks cover the stated finite-lattice formulas and
payoff boundaries. Production and independent complete-path
qualification remain open.

## Observed implementation and evidence

### Choice, scope and rationale

The engine supports multiplicative recombining vanilla lattices, with caller-supplied constant continuously compounded rate and continuous yield. `OptionType::intrinsic` is max(S−K,0) or max(K−S,0); `ExerciseStyle` defaults to European. Spot and strike are positive price levels, volatility is annual decimal lognormal volatility, and time is years. LR is documented as improving convergence; that motivation is observed, not an approved accuracy guarantee.

### Algorithm and selection

For N steps, dt=T/N. CRR uses u=exp(σ√dt), d=1/u, p=(exp((r−q)dt)−d)/(u−d). Its helper rejects u−d<1e−14 and probabilities outside [0,1]. `with_drift` reconstructs probabilities with the supplied annual drift and dt; it is not a separate interpolation method.

LR uses d1,d2 from the shared Black–Scholes kernel and c=(N+1/6)/(N+1/3+0.1/(N+1))². The inversion is PP(z)=1/2+sign(z)√(1−exp(−cz²))/2. Set p=PP(d2), p′=PP(d1), u=exp((r−q)dt)p′/p, d=exp((r−q)dt)(1−p′)/(1−p). The implementation evaluates these ratios in log space with `expm1`/`ln_1p` branches to avoid cancellation in deep tails. The constructor rounds a positive even N upward; pricing rejects an even LR grid created by mutating public fields. At σ=0 both factors equal exp((r−q)dt), with p=1/2.

The [equity caller](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/black.rs) explicitly chooses LR and defaults to 201 steps. This is a caller default, not a global engine default. There is no automatic convergence refinement or fallback to CRR. Discounted induction is [record 003](tree-discounting-and-induction.md); dividends and exercise are [record 002](dividends-and-lattice-exercise.md).

### Failure behavior and alternatives

Pricing requires finite positive T and N>0; LR checks finite positive factors. The shared `OptionMarketParams::validate` requires strictly positive σ and allows T=0, whereas tree pricing allows σ=0 and rejects T=0. Supported CRR/LR are alternatives; adaptive refinement or alternative tree constructions would be new work.

### Evidence

[binomial_tree.rs](../../../../finstack-quant/models/src/trees/binomial_tree.rs) owns construction, including `calculate_parameters`; [tree_framework.rs](../../../../finstack-quant/models/src/trees/tree_framework.rs) owns `EvolutionParams`; [types.rs](../../../../finstack-quant/models/src/types.rs) owns payoff and validation. Inline `test_golden_crr_atm_vs_black_scholes` uses the numeric 10.4506 anchor (500 steps, 0.1% tolerance); `test_golden_lr_odd_converges_faster` allows 0.05 absolute price error at 101 steps. Other convergence tests compare the repository BS implementation, and bit pins only detect changes. [Black–Scholes reference](../../../REFERENCES.md#black-scholes-1973) supplies the analytical comparator, not LR adoption evidence.

The [equity input adapter](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/inputs.rs)
and [commodity option](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/types.rs)
invoke shared option validation before tree pricing. The commodity
carry helper uses short-horizon and ratio fallbacks.
The [listed futures option](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/types.rs)
uses distinct default grids for its official PV and risk routes.
The [convertible credit tree](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/credit_tree.rs)
uses fixed CRR spacing with interval-specific reweighting.
The [independent reference fixtures](../../../../finstack-quant/models/tests/production_equity_audit.rs)
include QuantLib comparisons and a test for the current LR
even-count adjustment.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
