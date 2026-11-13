# MODELS-MONTE-CARLO-14: Rough Bergomi adapted variance and log-Euler spot

- Decision status: **Accepted — complete checked rough Bergomi spot and pricing package, 2026-11-11**.
- Implementation status: The dedicated spot scheme and equity pricing route exist. Checked grid dynamics, variance normalization and estimator reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent moment, grid, scaling and parity examples reviewed. Runtime, host and product qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked rough Bergomi spot and pricing

1. **Select the named rough Bergomi domain.** Require typed
   `0<H<1/2`, finite `eta>=0`, finite `rho` in `[-1,1]`
   and finite positive initial spot, including subnormals.
   `eta` scales a level in model-years to the power `H` into
   a dimensionless log-variance change. `rho` couples the
   stock and Volterra Brownian drivers. Keep one immutable,
   validated model and restore it through the same checks.
   Replace the nonrough-Hurst warning with rejection. Retain
   literal zero `eta` and both correlation endpoints.
   [Bayer, Friz and Gatheral](../../../REFERENCES.md#bayer-friz-gatheral-2016).

2. **Qualify the forward-variance source at its origin.** Use
   the accepted checked `ForwardVarianceCurve` with explicit
   point or ending-interval semantics. `xi(t)` is nonnegative
   annual variance density in squared decimal returns per
   model year, associated with the underlying, pricing measure
   and conditioning information. A zero-history run starts
   at that qualified origin. An older curve shifted to the
   valuation date cannot supply a new conditional curve by
   itself. Require an owner-qualified current-origin source.
   This package supplies no generic mid-path restart.
   [Accepted forward variance](../core-market/forward-variance.md#accepted-decision-nonnegative-variance-sources-and-explicit-model-time-semantics).

3. **Use one model clock and explicitly selected carry.** Use
   finite signed continuously compounded flat `r,q` with
   `C=(r-q)*h`, or the accepted checked cumulative log-carry
   schedule with its increment `C` over the actual interval.
   Require coverage and qualified agreement of model, carry
   and variance clocks. Dated source queries use their native
   mappings. Retagging a time axis alone cannot convert a
   variance density or the Volterra normalization and `eta`.
   Carry defines the model forward; discounting belongs to
   the payoff's actual settlement and measure.
   [Accepted carry and scaled spot](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths).

4. **Require exact variance breaks on the actual grid.**
   Use the accepted stored knots and actual adjacent widths.
   Require every strict-interior rate change of an ending-
   interval source as an exact node, including transitions
   between zero and positive rates. The owner can explicitly
   form an exact union with its base grid and required events
   before preparing the plan. Use the new interval's value
   at an exact ending-interval break. Point-linear sources
   retain left sampling, with quadrature error even when
   all source knots are present. Admit checked irregular grids.
   No epsilon snapping, nominal width substitution, hidden
   subdivision or automatic step-count change inside the scheme.
   [Accepted time grids](../core-numerics/time-grids.md).

5. **Use record 13's adapted driver and sampled variance.**
   Use one grid- and Hurst-bound RL plan for the exact stored
   intervals. At the left node `i`, take its generated level
   `Y_i` and same-loading variance `v_i`. Evaluate the
   variance rate `V_i=xi(t_i)*exp(eta*Y_i-eta^2*v_i/2)`.
   Use `v_i` from the sampled grid for the ideal unit-mean
   Gaussian multiplier. The level and its variance are
   available before the current shock. The receiving level
   first affects the next interval. Remove public injection
   slots, level differencing and accumulation, and fBM
   alternatives from the named model.
   [Accepted fractional driver](13-fractional-noise.md#accepted-decision-for-checked-fractional-noise-and-volterra-drivers).

6. **Compute the full left-frozen integrated variance in scale.**
   Use `I=h*V_i`. Combine source rate, width, log multiplier
   and the complete drift/noise terms before range checks
   that depend on the final result. Preserve signed sums,
   the scaled `eta^2*v_i/2` term and needed square roots.
   An isolated exponential or variance rate need not be
   materialized. A requested positive variance or spot must
   remain positive and finite after rounding. Reject unresolved
   range or arithmetic with estimator, leg, node and operation
   context. No variance floor, exponent cap, clipped shock,
   partial result or dropped failing path.

7. **Use a checked log-Euler spot update and genuine leverage.**
   Put `Z_s=rho*Z_drive+sqrt((1-rho)*(1+rho))*Z_independent`
   and `S_b=S_a*exp(C-I/2+sqrt(I)*Z_s)`. Use the Brownian
   driver assigned to the current cell. At `rho=+1` or `-1`,
   the independent loading is exactly zero. Keep the three
   ordered normal slots and antithetic signs from record 13.
   Check the complete scaled spot update and commit it once.
   Spot is the raw model coordinate. Retire the generic
   zero drift/diffusion placeholders as a usable pairing.

8. **Preserve literal zero and the deterministic-variance limit.**
   With `xi(t_i)=0`, set `I=0` and apply carry alone after
   validating required state and draws. Continue RL history
   and admit later positive variance. At literal `eta=0`,
   use `I=h*xi(t_i)` without a stochastic multiplier.
   The terminal lognormal law then has total variance
   `sum_i h_i*xi(t_i)`. This equals the source area for
   flat and break-aligned ending-interval sources. A point-
   linear source still uses left quadrature. At the origin
   `Y_0=v_0=0` and the multiplier is one. Keep the draw
   profile at deterministic intervals. An admitted origin-
   only raw path has no step or draw.

9. **Separate finite-grid forward preservation from the continuum.**
   Under ideal independent Gaussian draws the adapted spot
   update has conditional mean `S_a*exp(C)`. The terminal
   grid forward is therefore `S_0*exp(sum_i C_i)`. This
   holds at either sign of `rho` and establishes a first
   moment only. For the named equity pricing route require
   `rho<=0` when `eta>0` and the variance source is active
   over the priced horizon. Raw finite-grid scenarios retain
   both signs. A coarse grid with no stochastic left cell
   cannot waive the pricing admission rule. The deterministic
   limits allow either sign. The restriction addresses the
   strict-local-martingale behavior of the positive-correlation
   continuum model under the cited regularity conditions.
   [Continuum martingale reference](../../../REFERENCES.md#rough-bergomi-adapted-steps-and-moment-limits).

10. **Recognize the finite-grid moment limitation.** With
    `eta>0` and positive variance sampled at any left node
    after origin, the ideal grid terminal spot has infinite
    moments of every order `p>1`. At such a node, condition
    on previous draws except the preceding cell's independent
    RL residual. The left spot is then fixed and the variance
    is a positive constant times an exponential of that
    residual. Integrating the current spot shock gives the
    conditional factor `exp(p*(p-1)*I/2)`. Its expectation
    over the residual diverges. Later steps preserve the
    conclusion by conditional Jensen and the first-moment
    identity. The result covers both correlation endpoints
    and the ordinary unbounded call payoff, including a
    nonnegative antithetic pair. Finite Philox support and a
    finite observed sample cannot qualify the ideal-law
    call variance or its standard error.

11. **Use bounded put observations for stochastic-grid calls.**
    The existing European put directly samples `(K-S_T)^+`.
    For a call on an active stochastic grid, estimate the
    same bounded unit put and add the deterministic parity
    adjustment `F_T-K` in payoff units, or `D*(F_T-K)` in
    PV units. Use the qualified grid forward and the same
    payoff observation, discount and settlement. Apply signed
    quantity and discount once. Compute uncertainty on the
    bounded independent observations or complete pair means.
    Translate the interval by the parity shift, scale it
    with the position and order the endpoints. Preserve sampled zero
    dispersion. Return a precision failure for unresolved
    cancellation or failure of the product's price acceptance
    gate. No clipping or synthetic standard error. Direct
    terminal payoffs with deterministic sampled variance use
    their ordinary finite-variance estimator. Other unbounded
    payoffs need their own qualified inference method.
    [Accepted estimation](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

12. **Reconcile raw, priced and host entry points.** Require
    `rough_bergomi_euler` explicitly in the raw route, with
    named spot output and the shared checked path plan. Use
    explicit estimator count, grid construction, seed,
    antithetic and execution controls for live pricing. Use
    the accepted per-estimator Philox substreams and checked
    aggregation instead of the private sequential loop and
    nonfinite-to-zero uncertainty. Carry effective plan and
    estimate provenance through existing Rust, Python and WASM
    raw and pricing surfaces, schemas, declarations and parity.
    Low-level stepping stays Rust-owned.
    [Raw simulation owner](28-capture-and-simulation-contracts.md).

13. **Keep dated lifecycle and market construction with the product.**
    Resolve existing exercise, expiry and settlement before
    preparing live paths. The existing equity route is for
    European claims without future cash dividends. Resolve
    the active flat variance override or the qualified
    ATM-forward strip without querying unused strike quotes.
    Use consistent forward, clock and carry inputs and the
    native discount for the actual settlement. Align strip
    breaks under clause 4. The equity owner selects curve
    provenance, strip edge and fit policy, valuation and risk
    controls, and actual price tolerances. The ATM-implied
    strip remains a proxy for expected variance.
    [Equity rough-volatility adapters](../valuations-equity-and-composition/11-rough-volatility-adapters.md).

14. **Qualify the model, estimator and product independently.**
    Check Brownian leverage, sampled variance normalization,
    exact limits, zero-to-positive breaks, conditional forwards,
    covariance, tail and parity behavior, scaled failures,
    actual routes and replay. Compare the deterministic limit
    against lognormal pricing with the actual integrated
    variance and compare stochastic prices under explicit grid
    refinement against independent rough Bergomi evidence.
    Sampling intervals describe the selected bounded estimator
    on the grid. Grid, tail, finite-normal, input-fit and
    between-node monitoring effects require separate evidence.

## Observed behavior and limitations

The current constructor requires positive eta and warns at
nonrough Hurst values. The step uses the left Volterra level,
continuum compensation and an injected Brownian driver. It
accumulates fractional differences in workspace. The raw path
accepts generator alternatives and the priced route builds a
separate uniform grid. It can cross interval-variance breaks.

The equity adapter uses a flat terminal-rate equivalent to
build intermediate ATM forwards, can resolve an unused strike
quote and estimates the direct European payoff using one
sequential stream. A nonfinite sample standard error is
replaced with zero. The moment and parity analysis below is
for the proposed ideal Gaussian grid law.

## Independent examples and qualification

| Example | Expected result or limitation |
| --- | --- |
| Ending intervals `(.2,.01),(1,.09)` at `eta=0` | The exact area is `.074`. Left sampling on `[0,.5,1]` gives `.05`. Including the exact `.2` break restores `.074`. |
| Point-linear variance from `.01` at zero to `.09` at one | The source area is `.05`. `N` equal left steps give `.05-.04/N`, including with both source knots present. |
| `xi=2^-1074`, width `.5`, log multiplier `745`, carry and shock zero, spot `100` | The complete integrated variance is about `.8752732970` and spot about `64.55603055`. The isolated exponential overflows. |
| `eta=2^512`, `Y_i=2^-512`, `v_i=2^-1024` | The complete log multiplier is `.5`, although the isolated square of eta overflows. |
| Two unit intervals, `H=.25,eta=1,xi=.04` | The first independent RL residual enters the second left log-variance with coefficient `1/3`. The first receiving spot does not depend on that residual. Integrating the second spot normal and then the residual gives infinite terminal second moment. |
| `K=100`, forward `110`, discount `.95` | The call parity adjustment is `9.50` in PV units. Put sampling variance and the paired-mean standard error are preserved by the deterministic shift. |
| Grid `[0,.5,1]`, `H=.25,eta=1.2,rho=-.7,xi=.04,S0=100,K=102`, total carry `.03`, discount `.97` | Conditional Gaussian quadrature gives put PV `7.08378736861` and parity call PV `8.09787716210`. This benchmark uses the same declared grid law and independent integration. |

## Evidence

Located tests cover adaptedness, state reset, spot and variance
means and the small-eta lognormal limit. Loop equality reuses
the same model law. [Process](../../../../finstack-quant/models/src/monte_carlo/process/rough_bergomi.rs),
[stepper](../../../../finstack-quant/models/src/monte_carlo/discretization/rough_bergomi.rs),
[raw non-Markov tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests/non_markov.rs),
[equity pricing route](../../../../finstack-quant/valuations/equity/src/equity_option/rough_bergomi_mc_pricer.rs),
[equity input resolution](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/inputs.rs),
[equity tests](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_rough_bergomi.rs),
[primary references](../../../REFERENCES.md#rough-bergomi-adapted-steps-and-moment-limits).

## Relationship to the accepted forward-variance boundary

[CORE-MARKET-17's complete accepted package](../core-market/forward-variance.md#accepted-decision-nonnegative-variance-sources-and-explicit-model-time-semantics)
requires a nonnegative variance-rate source with strict point/interval semantics,
checked queries and explicit variance-time/underlying/measure/conditioning
qualification. Reconcile genuine zero without an avoidable 0*exp(overflow),
without discarding future positive values or fractional history. Exact source
interval areas do not certify the left-sampled simulation grid across breaks;
keep adaptedness and qualify the actual grid/quadrature/path bias. Frozen source
reuse is not a new conditional curve or a qualified history-free restart.
Record 17 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's full driver,
compensation, dynamics, simulation or pricing accuracy.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
