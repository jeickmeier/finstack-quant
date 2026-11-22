# VALUATIONS-EQUITY-AND-COMPOSITION-10: Equity Heston Fourier and Monte Carlo adapters

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-10`.
- **Decision status:** Accepted 2026-11-14 — complete equity Heston Fourier and Monte Carlo package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent Fourier, payment, deterministic-variance and standalone QE-M checks completed. Repository pricing, Philox replay, risk and host qualification remain pending.

## Accepted decision for checked dated Heston Fourier and Monte Carlo pricing

1. **Select the live European engine explicitly after lifecycle resolution.**
   Keep `heston_fourier` and `monte_carlo_heston` as exact equity
   model keys. Resolve exercise, expiry and outstanding settlement first.
   Require a finite positive live model horizon to fixing and reject
   eligible cash-dividend detachments through its actual fixing phase.
   Admit physical settlement only on qualified equivalence to the
   modeled payoff. Unsupported terms and pricing failures propagate
   through the selected route.
   [Accepted lifecycle](06-option-lifecycle.md),
   [accepted live payoff basis](07-black-tree-and-direct-greeks.md).

2. **Use the same scoped risk-neutral inputs and actual payment basis.**
   Resolve one coherent authored or exact fitted `(κ,θ,σv,ρ,v0)`
   block on the declared clock with underlying, roles and provenance.
   Require finite `κ>0`, `θ>0`, `σv≥0`, `−1<ρ<1` and
   `v0≥0`. Feller status is diagnostic. Retire the unqualified
   global scalar lookup and the unused Black volatility lookup and
   advertised Black-surface dependency. On fixing horizon `T`,
   derive `r=−log D(V,E)/T` from the original dated factor and
   use the elected signed continuous yield or no-distribution basis.
   The terminal carry matches these inputs without claiming
   intermediate term-structure fidelity. For a qualified fixed cash
   payment at `P`, compose the Fourier fixing-discounted unit price
   with `D(V,P)/D(V,E)`. The Monte Carlo engine applies
   `D(V,P)` directly to the checked terminal position payoff on
   this basis. Apply signed quantity once.
   [Accepted model resolution](../valuations-framework/model-resolution.md),
   [accepted Heston PDE adapter](09-equity-pde.md),
   [Heston](https://doi.org/10.1093/rfs/6.2.327).

3. **Use the bounded shared Fourier policy.** Require an explicit call
   or put and use the scalar `heston_price` route with its internal
   tail enclosure, composite order-16 Gauss–Legendre refinement,
   cumulative 1,048,576-node budget and direct OTM time-value gates.
   The dated adapter adds no integration knobs. Check the unit premium
   and its corresponding financial bounds on the actual settlement basis
   before position scaling. Keep range, unresolved computation and
   convergence failures distinct. At exactly zero vol-of-vol use the
   checked Black expectation with exact integrated deterministic
   variance. Every positive vol-of-vol uses the stochastic transform.
   [Accepted bounded Fourier policy](../models-analytic/heston-fourier-integration.md).

4. **Require a complete fixed-budget QE-M run.** Require a literal
   positive uniform interval count, 2 through 10,000,000 independent
   estimator groups, ordinary or antithetic sampling, an exact Philox
   seed or explicitly elected derivation, and execution mode. Require
   an explicit maximum trajectory-step budget. Compare the checked
   product of groups, legs per group and intervals against that budget
   before allocation or draws. Preflight the actual grid, draw,
   stream and allocation requirements. Remove the implicit 100,000
   paths, 252 steps per year, ten-step minimum and implicit
   instrument-derived seed. Use the accepted QE-M conditional variance
   and same-law martingale correction with ordered spot and variance
   innovations. At exactly zero vol-of-vol keep the explicitly
   requested simulation, using the exact deterministic-variance
   transition with stochastic spot and the same innovation layout.
   A failed required path fails the run.
   [Accepted QE-M scheme](../models-monte-carlo/09-heston-qe.md),
   [accepted Philox streams](../models-monte-carlo/02-random-streams.md),
   [Andersen](https://ssrn.com/abstract=946405).

5. **Report fixed-sample uncertainty and qualify time-step bias
   separately.** Compute the checked mean, standard error and fixed
   95% Student-t interval from complete independent group observations
   in signed position PV currency. Each antithetic pair contributes
   one group mean. Keep a literal zero sample standard error and
   point interval for a constant realized sample. Preserve uncertainty
   as unavailable where no simulation ran. A rare-payoff sample can
   yield zero on every group. Apply any elected positive currency
   half-width tolerance after the complete run. Check actual path
   payoff support. Use exact expectation bounds to qualify the
   estimator and discretization, preserving a valid finite sample
   mean and its diagnostics if it crosses an expectation bound.
   Retire clipping, omitted-zero diagnostics and early stopping.
   [Accepted estimation and discounting](../models-monte-carlo/01-engine-and-estimation.md).

6. **Reprice risk on the selected engine and retain provenance.**
   Use the checked raw selected price. Hold the fitted or authored
   block fixed for ordinary market repricing. Keep Heston parameter
   shocks, recalibration and a defined inactive Black-only shock
   distinct. Monte Carlo finite differences require common logical
   groups across every leg of the actual stencil and uncertainty from
   the within-group stencil result. Qualify the two-state coupling
   and any changed-grid experiment. Retain actual group and
   trajectory counts, grid, exact seed and stream, execution and
   pricing sources in existing typed results. Reconcile Rust, Python,
   WASM and schema entry points. Independently qualify Fourier
   resolution, Monte Carlo bias and sampling, and repriced risk.
   [Accepted selected-model risk](08-selected-risk-and-implied-vol.md).

## Observed behavior

### Choice and rationale

The Heston adapters use caller-provided market parameter scalars; no equity-option calibration is performed here. Fourier and QE Monte Carlo are two explicit implementations of the same European stochastic-variance model with different numerical controls.

### Scope and input domain

Live European equity options without future discrete dividends. Required unitless Heston parameters are resolved by the strict shared market helper; lifecycle values resolve first. Ordinary spot/rate/q and volatility-resolution inputs are still collected, even when the option surface volatility is not used as the Heston variance parameter.

### Algorithm

Fourier: collect expiry-equivalent r/q and spot, resolve strict Heston parameters, invoke shared `heston_price` with no adapter-specific integration config, then multiply the unit price by signed quantity.

MC: construct `HestonProcess` and `QeHeston`, use a uniform grid with `max(round(T×steps_per_year),10)` steps, initialize `[S,v0]`, simulate a European terminal intrinsic payoff with quantity included and apply `exp(−rT)` discounting. Use Philox seeded by model configuration/instrument identity. Return mean PV; attach `mc_stderr` only when positive.

### Selection, controls and conventions

Registry keys `HestonFourier` and `MonteCarloHeston` are explicit alternatives. Path count accepts `model_config.mc_paths`; the constructor uses 100,000 paths and 252 steps/year before overrides. The generic engine configuration controls parallel execution; no adapter-specific antithetic, confidence target or calibration override is applied here.

### Limitations, failures and alternatives

A full volatility surface is not fitted by selecting Heston. Flat expiry-equivalent rates and q do not reproduce pathwise term structures. Standard error measures sampling uncertainty, not QE bias or parameter error; shared mathematical parameter validation is not a market-fit certificate. Fourier offers a deterministic alternative; default integration and MC budgets do not establish an accuracy bound.

### Evidence

[Heston](../../../REFERENCES.md#heston-1993), [Andersen QE](../../../REFERENCES.md#andersen-2008-heston-qe); [test_alt_models.rs](../../../../finstack-quant/valuations/equity/tests/equity_option/test_alt_models.rs) and inline [heston_mc.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/heston_mc.rs) locate cross-engine and input-failure cases. The Heston-MC limiting-case comparison in `test_alt_models.rs` is slow/ignored in the ordinary suite, and some comparisons reuse library closed forms. Underlying process/transform algorithms are owned by the models audit; no parameter calibration or new simulation was run. Cross-engine agreement is not a complete independent certification.

## Independent numerical checks

- Standalone damped Fourier quadrature with spot 115, strike 102,
  fixing horizon 1.25 years, rate .03, yield −.015 and
  `(κ,θ,σv,ρ,v0)=(1.8,.05,.65,−.55,.03)` gives a unit call of
  22.4250352088 at the fixing discount. The actual payment discount
  .91 gives 21.1865659358. At exactly zero vol-of-vol, the paid
  deterministic-variance Black result is 20.8255323382.
  [Carr and Madan](https://doi.org/10.21314/JCF.1999.043).
- A standalone QE-M calculation uses spot and strike 100, one year,
  rate .03, yield .01, payment discount .95 and
  `(κ,θ,σv,ρ,v0)=(1.5,.04,.5,−.7,0)`. With 30,000 antithetic
  groups the observed call PV is 6.00063382 with one interval and
  5.82312240 with 64 intervals. The independent Fourier reference
  is 5.82169890. The one-interval 95% half-width of .02538
  illustrates sampling precision alongside a .17893 deviation
  from the reference. The standalone stream is distinct from the
  accepted Philox profile.

Repository runtime, stream replay, risk and host suites remain unrun for
this review.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [heston_mc.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/heston_mc.rs)
- [heston_fourier.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/heston_fourier.rs)
