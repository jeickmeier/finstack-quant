# VALUATIONS-EQUITY-AND-COMPOSITION-11: Equity rough Heston and rough Bergomi adapters

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-11`.
- **Decision status:** Accepted 2026-11-14 — complete equity rough Heston and rough Bergomi adapter package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent strip, parity-estimator and finite-grid variance and payoff checks completed. Production pricing, replay, risk and host qualification remain pending.

## Accepted decision for checked equity rough-volatility adapters

1. **Select one live European route after lifecycle resolution.** Keep
   `rough_heston_fourier`, `monte_carlo_rough_heston` and
   `monte_carlo_rough_bergomi` as exact model keys. Resolve exercise,
   expiry and outstanding settlement first. Require a positive live
   fixing horizon and reject nonzero cash-dividend detachments through
   the exact fixing phase. Use signed fixing-equivalent carry, with
   the rate from `D(V,E)` on the declared clock and the admitted
   continuous yield or no-distribution basis. For qualified fixed
   cash payment at `P`, compose the fixing-discounted Fourier result
   with `D(V,P)/D(V,E)` or discount the Monte Carlo terminal payoff
   directly with `D(V,P)`. Apply signed quantity once. Admit physical
   settlement only on qualified payoff equivalence.
   [Accepted lifecycle](06-option-lifecycle.md),
   [accepted dated payoff basis](07-black-tree-and-direct-greeks.md).

2. **Share one qualified rough Heston block and keep engine controls
   separate.** Bind the six authored or exact fitted parameters to
   the underlying, pricing measure, normalized fractional kernel,
   model clock and conditioning origin. Require finite positive
   `v0`, `κ`, `θ` and `σv`, with `0<H<1/2` and `−1<ρ<1`.
   Use classical Heston and zero vol-of-vol as qualification limits.
   Retire the unqualified global scalar lookup and unused Black
   volatility dependency. Fourier requires both
   `model_config.rough_heston_fourier` tolerances. Use
   `price_abs_tol>0` in fixing-discounted premium per underlying
   unit and `0<price_rel_tol<1` relative to time value. Delegate
   the bounded Lewis time and frequency refinement, preserve the
   actual numerical failure, and check strict unit-price bounds
   through payment composition. Monte Carlo uses the accepted
   hybrid history and positive-part variance projection, adapted
   left-node spot variance, and direct call and put observations.
   [Accepted Fourier policy](../models-analytic/rough-heston-riccati.md),
   [accepted hybrid simulation](../models-monte-carlo/15-rough-heston.md),
   [El Euch and Rosenbaum](https://doi.org/10.1111/mafi.12173).

3. **Elect one rough Bergomi variance source and align its strip.**
   Choose either an explicit finite nonnegative annual Black
   volatility, with its square evaluated on the model clock, or
   a qualified current-origin unshifted Black absolute-strike
   surface. Use the declared fixing-equivalent carry consistently
   for model ATM forwards and paths. On the surface route use
   the exact source expiries strictly before fixing and append
   fixing once. Query the declared interpolation within actual
   source support at the model ATM-forward strikes. Keep exact
   expiry and strike coordinates and checked `W(u)=u σ(u)^2`.
   Difference adjacent total variances over actual widths, with
   `W(0)=0`, to obtain the piecewise-constant variance density.
   Admit literal zero rates and reject negative or unresolved
   rates. Keep the source identity, clock, original conditioning
   origin and model-forward mapping. The moving-ATM strip is a
   proxy for expected variance. Insert each strict-interior
   selected strip end in the exact union with the base grid
   before plan preparation. Resolve only the elected variance
   source.
   [Accepted forward variance](../core-market/forward-variance.md),
   [accepted rough Bergomi model](../models-monte-carlo/14-rough-bergomi.md),
   [Bayer, Friz and Gatheral](https://doi.org/10.1080/14697688.2015.1099717).

4. **Use bounded put observations for rough Bergomi stochastic-grid
   calls.** Retain adapted Volterra driving noise, sampled-grid
   variance compensation and ongoing history through zero-rate cells.
   Require `ρ≤0` for equity pricing when `η>0` and the variance
   source is active. Under stochastic sampled variance, use the
   bounded put estimator and the deterministic cash shift
   `D(V,P)(F−K)` for calls. Translate the interval by the same
   shift and order endpoints after signed scaling. Require the
   reconstructed unit call to be resolvably positive and at most
   `D(V,P)F`. A failed price gate or unresolved cancellation is
   a precision error with sampling diagnostics. Retire clipping
   and nonfinite-to-zero uncertainty. Direct puts and direct
   payoffs under deterministic sampled variance use their accepted
   estimators. Preserve the ideal-law fixed-grid moment distinction
   between rough Bergomi calls and rough Heston's direct estimators.
   [Accepted rough Bergomi estimators and moment limits](../models-monte-carlo/14-rough-bergomi.md).

5. **Require explicit simulation controls and budget actual history
   work.** Require an explicit base grid, 2 through 10,000,000
   independent estimator groups, pairing, exact Philox seed policy
   and execution mode. Require an aggregate history-work budget for
   each run. Count history-cell visits and state updates across all
   legs on the actual final grid. Preflight model-specific grid,
   stream, layout, allocation and memory limits. Remove hidden
   path and time-step defaults. Execute the declared full budget
   or fail the run. Report actual groups, trajectories, grid, seed
   and stream, standard error and fixed-sample 95% Student-t interval
   with literal sampled zero dispersion preserved. Apply any elected
   positive PV-currency half-width tolerance after the run. Qualify
   time and history bias, variance projection and transform
   convergence separately from sampling uncertainty.
   [Accepted estimation](../models-monte-carlo/01-engine-and-estimation.md),
   [accepted fractional history plan](../models-monte-carlo/13-fractional-noise.md).

6. **Reprice selected inputs and preserve their provenance.** Hold
   the rough parameters fixed for ordinary market repricing. With
   a selected rough Bergomi surface, rebuild the ATM strip at the
   bumped forwards for same-cut spot and carry shocks while holding
   the same surface authority. A frozen-strip partial is a separate
   experiment. Native volatility shocks move only the selected
   scalar or declared exact surface nodes. Check every required
   reprice and qualify the common logical groups across the complete
   Monte Carlo stencil. Compute uncertainty on the within-group
   stencil result. Require a qualified source and conditioning state
   for every cut in a dated risk experiment. Expose active input and
   run provenance, checked raw price, diagnostics and Rust failures
   through the existing Python, WASM and schema paths.
   [Accepted selected-model risk](08-selected-risk-and-implied-vol.md).

## Observed behavior

### Choice and rationale

Rough-volatility engines are explicit European alternatives with strict market parameters. Rough Bergomi bootstraps interval forward variance from ATM-forward total variance; rough Heston uses its own variance parameters. Neither is automatically chosen from the shape of an equity smile.

### Scope and input domain

European equity options without future cash dividends. Rough Heston requires `ROUGH_HESTON_V0/KAPPA/THETA/SIGMA_V/RHO/HURST`; rough Bergomi requires `ROUGH_BERGOMI_ETA/HURST/RHO` plus an unshifted Black strike surface or an explicit implied-volatility override. Shared constructors validate Hurst/model domains.

### Algorithm

Rough Heston Fourier constructs `RoughHestonFourierParams` and calls its European transform pricer with r/q/T. The adapter supplies no numerical override. Rough Heston MC constructs the hybrid discretization on a uniform grid, starts `[S,v0]`, uses serial `McEngine`, Philox and a terminal European payoff, discounts by `exp(−rT)` and attaches positive standard error.

Rough Bergomi: an implied-volatility override yields flat forward variance `xi0=σ²`. Otherwise retain positive surface expiries before maturity and append maturity. At each u sample clamped volatility at `S exp((r−q)u)`, form `W(u)=σ(u)^2u`, then set interval variance `[W(u)−W(previous)]/(u−previous)`. Nonfinite/nonpositive intervals error; the resulting piecewise-constant curve conserves each sampled total variance.

Simulate each path using Riemann–Liouville Volterra increments and their matched Brownian driving normals, a RoughBergomi Euler discretization and shared fractional-path engine. Reuse buffers; update online mean/standard error; discount both at the end. A nonfinite sample standard error is replaced with zero before diagnostics, an observed behavior requiring agreement.

### Selection, controls and conventions

Rough MC path count accepts the instrument override; seed is resolved per instrument. Default rough MC budget is 100,000 paths and 100 time steps; a path override above 5,000,000 is rejected. Rough Bergomi uses an explicit serial path loop, not generic target-confidence stopping/antithetic configuration. Surface sampling is ATM-forward at each expiry, not a strike-specific/local-vol smile evolution.

### Limitations, failures and alternatives

ATM strip construction is not calibration of rough parameters, and a positive total-variance strip does not prove an arbitrage-free smile. Uniform left-grid variance sampling creates discretization error across strip breaks. Parameter/transform failures may surface through nonfinite Money construction rather than an adapter-specific convergence report. Standard Heston and Black are available alternatives, not automatic fallback paths.

### Evidence

[rough Bergomi](../../../REFERENCES.md#bayer-friz-gatheral-2016), [rough Heston](../../../REFERENCES.md#el-euch-rosenbaum-2019), [hybrid scheme](../../../REFERENCES.md#bennedsen-lunde-pakkanen-2017); inline `variance_strip_tests` in [rough_bergomi_mc.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/rough_bergomi_mc.rs) inspect total-variance conservation, invalid surfaces and deterministic-variance refinement. [test_rough_bergomi.rs](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_rough_bergomi.rs) locates simulation cases. These are limit/identity/cross-engine evidence, not independent calibrated-market validation.

## Independent numerical checks

- Exact source pairs `(time,σ)=(.25,.25),(1,.125),(2,.125)`
  produce rates `1/16,0,1/64`. The break-aligned source area is
  `1/32` and coarse left sampling on `[0,1,2]` gives `5/64`.
  With deterministic sampled variance, forward 103, strike 105
  and payment discount .96, the Black call is 6.113389533634.
- With forward 95, strike 100 and payment discount .98, put
  samples `0,3,6` in terminal payoff units give a reconstructed
  unit call of −1.96. The negative estimate fails the price gate.
  In a separate two-cell stochastic-grid quadrature with spot 100,
  strike 102, unit horizon, integrated carry .03, payment discount
  .97, `H=.25`, `η=1.2` and `ρ=−.7`, equal source area .04
  gives call 8.0978771621 on the flat strip and 7.4892817156 on
  the zero-then-positive strip.
- Standalone conditional quadrature for rough Heston with spot
  110, strike 105, horizon 1.2, rates .032 and −.012, payment
  discount .945, `v0=.06`, `θ=.045`, `κ=.9`, `σv=.35`,
  `H=.22` and `ρ=0` gives a call of 17.0728683087 on one cell
  and 17.0278229398 on two. The paid parity difference is
  10.3610422098 on either grid.

The calculations exercise the stated finite-grid and payoff laws.
Repository pricing, host, risk and replay qualification remain open.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [rough_heston_market.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/rough_heston_market.rs)
- [rough_heston_fourier.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/rough_heston_fourier.rs)
- [rough_heston_mc.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/rough_heston_mc.rs)
- [rough_bergomi_mc.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/rough_bergomi_mc.rs)
