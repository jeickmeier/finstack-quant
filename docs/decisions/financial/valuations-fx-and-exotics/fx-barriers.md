# VALUATIONS-FX-AND-EXOTICS-FX-BARRIERS: FX barrier monitoring, rebate state and analytic/Monte Carlo pricing

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-BARRIERS`
- Decision status: **Accepted 2026-11-14** — complete FX single-barrier monitoring, pricing, rebate and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation remains pending.
- Validation status: Independent checks cover exact eligible observations, retained rebate, path carry, deterministic first passage and continuous in/out parity. Production valuation, observations, simulation and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked FX barrier monitoring and settlement

1. **Apply the exact monitoring contract.** Require the
   ordered pair, long or short side, positive base notional,
   fixed strike and barrier, and explicit up/down knock-in
   or knock-out. Continuous monitoring includes the stated
   start and expiry. Discrete monitoring checks only its
   ordered eligible observations, including expiry only if
   listed. Up touches at `S >= H` and down at `S <= H`.
   Bind the exact benchmark and information cut. Require
   qualified elapsed coverage, latch the first eligible hit
   with its provenance and require authorized resolution
   for missing due history.
   [Observation rules](../valuations-framework/observation-grids.md) ·
   [Inclusive barrier touch](../core-conventions/020-barrier-classification.md).

2. **Resolve barrier state separately from payment.** A
   confirmed knock-in leaves the accepted vanilla option
   and any earned delivery claim. A confirmed knock-out
   ends the option and retains its unpaid rebate. When
   discrete monitoring ends early with no hit, the
   surviving knock-out becomes vanilla and the failed
   knock-in can owe deferred cash. Use one total
   quote-currency rebate. A knock-out rebate can pay at
   first hit or on the stated deferred date. A knock-in
   no-hit rebate is deferred. Verify settlement status
   independently of the hit. Carry known unpaid cash
   through its actual due date, including after expiry.
   [FX vanilla lifecycle](fx-vanilla-delta.md#accepted-decision-for-checked-fx-vanilla-pricing-and-delta-conventions) ·
   [Barrier cash rules](../models-analytic/barriers-and-rebates.md).

3. **Admit the analytic route on a compatible continuous
   window.** Use the checked Reiner–Rubinstein law on a
   full remaining continuous window with qualified no-hit
   history, an eligible opening observation and
   constant-coefficient lognormal dynamics. A
   terminal-factor flat proxy requires explicit selection.
   Use a modeled FX coordinate that preserves the
   contractual barrier. A fixed delivery lag can use a
   qualified deterministic payoff transform that
   preserves the delivery-date vanilla value. Price the
   option and rebate separately. Use first-passage
   discounting for immediate hit cash. An incompatible
   analytic request requires explicit selection of an
   eligible model.
   [Checked analytic law](../models-analytic/barriers-and-rebates.md).

4. **Use dated GBM and exact observation knots for Monte
   Carlo.** Bind exact event and curve knots and use
   dated base and quote discount factors for interval
   carry. Map simulated cash FX to the actual benchmark
   for each eligible observation. Keep expiry exercise
   and delivery discount distinct. An explicitly selected
   continuous path route requires the checked crossing
   law on each monitored cell. For a future start, test
   the opening observation and bridge only subsequent
   monitored cells. Sample first passage for an immediate
   at-hit rebate and discount the payment once.
   [Shared barrier simulation](../models-monte-carlo/18-barrier-monitoring.md) ·
   [Broadie, Glasserman and Kou](https://www.columbia.edu/~mnb2/broadie/Assets/bgk_mf.pdf).

5. **Keep risk and export on the resolved basis.** Use
   explicit simulation controls and report actual work
   and uncertainty. Preserve the selected model,
   qualified history and paired draws under shocks. A
   live trigger-crossing move is scenario P&L where a
   smooth Greek is unavailable. Use dated theta with
   qualified events. Export known rebates as
   `DerivativeSettlement` and earned delivery in its
   native currencies. Unresolved contingent cash has no
   complete contractual export. Rust owns the existing
   Python envelope and typed WASM routes.
   [Paired simulation risk](../models-monte-carlo/25-finite-difference-greeks.md) ·
   [Dated theta](../valuations-framework/carry-theta-breakeven.md) ·
   [Cashflow classification](../core-conventions/015-cashflow-semantics.md).

## Observed behavior

### Choice, rationale and scope

FX single barriers support analytical continuous-monitoring Reiner–Rubinstein pricing and GBM Monte Carlo with explicit monitoring grids. Observed breach state determines whether the remaining claim is a vanilla, a rebate or a live barrier.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

### Algorithm and conventions

Resolve positive FX spot (explicit scalar ID or matrix), terminal-implied domestic/foreign rates, and strike volatility. Before simulation, use recorded breach: knocked-in becomes vanilla GK; knocked-out at-hit rebate is already paid and worth zero; at-expiry rebate remains `rebate×D_d`. At expiry explicit breach state controls intrinsic/rebate; after expiry zero is returned.

Analytical price is `barrier_price(BarrierParams(S,K,H,t,r_d,r_f,σ),type,callput)` plus the timing-specific `barrier_rebate`. Both kernels are audited with the models closed-form family. MC simulates GBM drift `r_d-r_f` and volatility `σ`; constructs continuous/date-restricted `BarrierOptionPayoff`, includes rebate per base unit and total notional, and discounts the payoff to expiry. At-hit rebate requests pass the domestic rate into the payoff.

MC uses merged path settings, instrument-derived seed, and the monitoring grid. Diagnostics expose standard error, estimator/simulated path counts, seed, actual time grid and antithetic/Sobol/bridge settings. Risk bumps the active spot source and volatility source, uses central delta/gamma and mixed vanna/volga repricing, and separates domestic/foreign curve rho from generic DV01.

### Selection and controls

Default selection follows the instrument monitoring/model contract; the family registers both `FxBarrierBSContinuous` and `MonteCarloGBM`. Direct `compute_pv` uses MC. Analytical pricing rejects discrete dates and future-start monitoring; MC builds the requested window/date grid.

Continuous monitoring requires a start date; seasoned monitoring requires `observed_barrier_breached`. Discrete dates are inspected for elapsed monitoring. The MC engine handles path randomness/continuous crossing treatment; local configuration is not evidence of a converged barrier probability. Rebate money is divided by notional for per-unit payoff construction.

### Limitations, failure and fallback

Flat terminal-implied domestic/foreign rates are used in FX barrier paths; a nonflat curve path is not preserved as a time-varying drift here. Monitoring inclusion, paid-at-hit state and rebate units follow the observed lifecycle above. Seeded reproducibility does not establish accuracy near barriers or convergence on sparse discrete schedules.

### Alternatives and tradeoffs

Continuous analytic pricing is fast within its domain; discrete MC is supported without pretending a continuity correction is exact. Time-dependent-rate PDE/local-volatility or stochastic-rate FX alternatives are not implemented by these local pricers.

### Evidence

- [finstack-quant/valuations/fx/src/fx_barrier_option/metrics.rs](../../../../finstack-quant/valuations/fx/src/fx_barrier_option/metrics.rs)
- [finstack-quant/valuations/fx/src/fx_barrier_option/pricer.rs](../../../../finstack-quant/valuations/fx/src/fx_barrier_option/pricer.rs)
- [finstack-quant/valuations/fx/src/fx_barrier_option/types.rs](../../../../finstack-quant/valuations/fx/src/fx_barrier_option/types.rs)

- [finstack-quant/valuations/fx/tests/fx_barrier_option/lifecycle.rs](../../../../finstack-quant/valuations/fx/tests/fx_barrier_option/lifecycle.rs): Observed-state, timing and expired-claim regressions.
- [finstack-quant/valuations/fx/tests/fx_barrier_option/greek_sources.rs](../../../../finstack-quant/valuations/fx/tests/fx_barrier_option/greek_sources.rs): Active spot/volatility source sensitivity assertions.
- [finstack-quant/valuations/fx/tests/fx_barrier_option/vanna_convention.rs](../../../../finstack-quant/valuations/fx/tests/fx_barrier_option/vanna_convention.rs): Mixed derivative normalization regression.

References for the original audit are [Reiner and Rubinstein](../../../REFERENCES.md#reiner-rubinstein-1991) and [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo).

## Independent numerical checks

- A discrete up barrier at 1.25 observes January 10
  at 1.24 and January 12 at 1.25. The different
  January 11 level 1.27 is outside monitoring. The
  first eligible hit is January 12 at equality. An
  unpaid USD 20,000 rebate due January 13 has PV
  USD 19,940 at factor .997.
- At zero volatility, cash FX 1.20 with intermediate
  quote/base factors .96 and .99 gives 1.2375 at the
  first knot. Expiry quote and base factors .95
  give 1.20. For an up-and-out call with barrier
  1.22, strike 1.18, base amount 100,000 and USD
  1,000 immediate rebate, monitoring the intermediate
  knot gives PV USD 960. Expiry-only monitoring
  leaves a call worth USD 1,900. Continuous first
  passage on the qualified log-linear first cell
  gives rebate PV USD 978.310729941.
- With cash FX 1.20, down barrier 1.10, strike 1.21,
  expiry .5, quote/base flat rates .04 and .015,
  volatility .20 and EUR 1 million notional, use
  expiry-to-delivery quote/base factors .995 and .998.
  The transformed strike is 1.206362725451.
  Reflection-density integration gives no-rebate
  knock-in and knock-out PVs of USD 8,773.779631
  and 62,349.463866. Their sum USD 71,123.243497
  matches the corresponding delivery-date vanilla.

## Follow-up tracking

Implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
