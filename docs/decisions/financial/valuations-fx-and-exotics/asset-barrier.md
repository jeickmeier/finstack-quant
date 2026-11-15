# VALUATIONS-FX-AND-EXOTICS-ASSET-BARRIER: Asset barrier analytic, Monte Carlo and log-space PDE algorithms

- Record: `VALUATIONS-FX-AND-EXOTICS-ASSET-BARRIER`
- Decision status: **Accepted 2026-11-14** — complete asset barrier monitoring, pricing, rebate, settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent monitoring, first-passage, rebate, parity and finite-grid comparisons checked. Runtime, numerical, market data and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for asset barrier pricing and settlement

1. **Book the payoff and exact monitoring terms.** Require a European cash
   call or put on a qualified same-currency equity or spot index, holder
   long or short, explicit up/down and knock-in/out, and finite positive
   strike, barrier and quantity. Bind the authorized benchmark, price basis
   and information cut. Continuous monitoring includes its authored start
   and expiry. Discrete monitoring uses only the exact ordered eligible
   observations, including expiry only when listed. Touch is inclusive,
   `S≥H` for up and `S≤H` for down.
   Require qualified elapsed coverage, latch the first eligible hit and
   retain its provenance. An eligible missing observation requires authorized
   resolution. [Observation grids](../valuations-framework/observation-grids.md),
   [barrier classification](../core-conventions/020-barrier-classification.md).

2. **Separate activation, entitlement and payment.** A confirmed hit
   activates knock-in or terminates knock-out. An early unhit discrete
   close leaves knock-out vanilla or determines the deferred knock-in
   rebate. Value a surviving vanilla on the selected compatible dynamics.
   Require terminal fixing only when earned intrinsic needs it. Book any
   rebate as total nonnegative Money in payoff currency, separate from
   option quantity and any trade premium. Knock-out rebate pays at first
   hit or its stated deferred date. Knock-in failure cash is deferred.
   Retain signed unpaid claims until discharge. Eligible same-day unpaid
   cash uses factor one. Partial, failed or overdue payment requires a
   qualified residual on its stated basis.
   [Dated discounting](../core-conventions/016-discounting.md).

3. **Use the checked analytic law on a qualified continuous window.**
   Require an entire remaining continuous window, an eligible opening
   observation, qualified history and a compatible constant lognormal law.
   An expressly elected flat proxy can match terminal factors. Bind the
   selected expiry-strike volatility on its declared clock. Price the
   no-rebate option and separately entitled rebate using the actual payment
   basis. Use the first-passage discount law for immediate hit rebates.
   Literal zero volatility uses the exact deterministic crossing, including
   terminal equality.
   [Continuous barrier premiums and cash](../models-analytic/barriers-and-rebates.md#accepted-decision-for-continuous-barrier-premiums-and-cash),
   [Reiner and Rubinstein](https://digicoll.lib.berkeley.edu/nanna/record/86304/files/b120984374_C044481811.pdf?registerDownload=1&version=1&withMetadata=0&withWatermark=0).

4. **Use exact events and dated carry in simulation.** Use checked exact
   GBM with qualified datewise carry. In a continuous window test its own
   opening node, then bridge subsequent monitored cells, with exact knots
   at relevant carry and discount breaks. Sample first passage for an
   immediate rebate. Discrete monitoring uses the listed observations.
   Heston uses a complete sourced QE-M parameter block and explicit checked
   grid. Continuous Heston requires a selected and separately qualified
   integrated-variance bridge approximation. Require explicit work, seed
   and sampling controls. Report actual work and sampling uncertainty
   separately from bridge and time-step error. Qualified plain GBM can
   use serial Sobol with separately addressed crossing uniforms.
   [Barrier simulation](../models-monte-carlo/18-barrier-monitoring.md),
   [Heston QE-M](../models-monte-carlo/09-heston-qe.md).

5. **Select a checked log-spot PDE with explicit grid controls.** Admit a
   qualified constant-coefficient Black–Scholes law or an expressly elected
   flat proxy. Require declared bounds, grid sizes and work limits. The
   exact continuous knock-out barrier is an absorbing edge with its dated
   rebate value. Discrete monitoring spans both sides, projects only at
   authorized events and uses the actual next eligible observation for
   the remote hit-side boundary. Start with two full backward-Euler
   intervals before Crank–Nicolson and restart damping after discrete
   projections. Compute knock-in as matched-grid vanilla less zero-rebate
   knock-out plus separately dated no-hit cash. Reject unresolved parity.
   Qualify mesh and truncation error separately for price and Greeks.
   [1D PDE time march](../models-numerical/theta-time-march-and-thomas.md),
   [Broadie, Glasserman, and Kou](https://www.columbia.edu/~mnb2/broadie/Assets/bgk_mf.pdf).

6. **Keep risk and export on the resolved basis.** Hold authorized history
   and selected dynamics fixed. Treat a live trigger-crossing shock as
   scenario P&L where a smooth Greek is unavailable. Use actual shock
   widths, the active lognormal volatility source where applicable and
   qualified paired draws for simulation risk. Retire duplicate `rho` and
   reprice each actual curve once across its DV01 roles. Horizon theta
   uses authorized or expressly qualified hypothetical events. Crossing a
   continuous window requires qualified coverage or first passage.
   Export known intrinsic and rebate as separate signed
   `DerivativeSettlement` rows with provenance. Complete cash export is
   unavailable when contingent. Use the same Rust state through the
   existing Python and WASM instrument envelopes.
   [Option risk](../valuations-framework/option-sensitivities.md),
   [horizon theta](../valuations-framework/carry-theta-breakeven.md),
   [cashflow export](../valuations-framework/cashflow-export.md).

## Independent calculations

- A deterministic log-linear spot path is `106` at time `0.2` and `115`
  at time `0.4`, with continuous monitoring starting at `0.2` and up
  barrier `110`. First passage occurs at `0.290906595934`. The log-linear
  discount curve on this cell has factors `0.991` and `0.975` at its
  endpoints. The PV of a USD `400` rebate paid at first passage is
  USD `393.47807184`, versus USD `390` if paid at the right endpoint.
- With spot and strike `100`, up barrier `115`, rate `0.04`, yield
  `0.015`, volatility `0.25`, one monitoring time `0.40`, terminal fixing
  `0.90`, deferred payment `1.10`, unit quantity and rebate `2.50`,
  conditional Gaussian integration gives knock-out value `5.7316762478`
  for immediate rebate and `5.7191677799` for deferred rebate. The
  separate damped-Crank–Nicolson implementation on uniform log grids of
  width `±2` around the exact barrier gave `5.4737690839` at 201 spot
  nodes by 200 time steps and `5.6995870852` at 1,601 by 1,600 for the
  immediate case. This illustrates finite-grid sensitivity at the
  observation projection.
- With spot `105`, down barrier `92`, put strike `104`, time `0.72`,
  rate `0.042`, yield `0.017`, volatility `0.24`, quantity `850` and
  expiry-to-payment factor `0.997`, continuous no-rebate knock-out and
  knock-in PV are USD `247.12874866` and `5,653.87771241` by independent
  reflection-density integration. Their sum matches the dated vanilla
  PV of USD `5,901.00646106` to the shown precision.

## Observed behavior

### Choice, rationale and scope

The inspected asset barrier supports continuous analytic pricing, GBM and
Heston Monte Carlo, and a one-dimensional log-space Black–Scholes PDE.

### Algorithm and conventions

Resolve expiry DF, instrument-time effective rate, spot, optional dividend yield and active expiry/strike volatility. Recorded knock-out pays only an unpaid at-expiry rebate; at-hit rebate is already settled. Recorded knock-in becomes vanilla in analytic/GBM/PDE paths. Expiry requires breach state and, after expiry, an explicit expiry fixing; the realized claim is retained. Total cash rebate is scaled separately from quantity; MC converts it to per-unit rebate.

Analytical value calls the shared barrier and rebate formulas. GBM uses a daily date-derived cumulative drift `-ln DF(date)-q t`; the monitoring helper builds continuous/discrete payoff grids. Heston uses strict parameters, QE discretization, the same drift schedule, initial variance and observed breach state; 100,000 default paths, 252 steps/year, minimum ten steps and instrument-derived seed. Continuous crossing treatment belongs to the shared payoff, with volatility/state inputs provided here.

PDE evolves log spot with diffusion `σ²/2`, convection `r-q-σ²/2`, reaction `-r`. Continuous knock-out domain ends exactly at the barrier, with Dirichlet boundary equal to rebate (at-hit) or rebate discounted from expiry. Other boundaries use zero/linear asymptotes. The grid is sinh-concentrated near strike, width `max(5σ√t,0.5)`; discrete monitoring instead uses a wider domain around barrier/spot/strike with width `max(7σ√t,1)` and concentration 0.1. Two Rannacher implicit steps precede Crank–Nicolson.

Discrete PDE merges uniform times and observation times, runs backward, resets hit nodes to rebate only at observation events and restarts Rannacher smoothing after each projection. Knock-in uses same-grid vanilla minus zero-rebate knock-out plus survival-rebate value; parity below `-1e-6×max(abs(vanilla),1)` errors, tiny negatives clamp to zero. Continuous parity extends the grid while preserving knock-out nodes. Interpolate solved value at log spot.

Generic finite-difference Greeks and curve DV01/key rates reprice the selected model. Theta carries observed state using held spot at crossed observations; it does not infer an actual historical market path.

### Selection and controls

Continuous monitoring defaults to `BarrierBSContinuous`; discrete to GBM MC. Explicit alternatives are `PdeCrankNicolson1D` and Heston MC. Analytical pricing rejects discrete monitoring. The PDE uses internal default 200 space points and 100 time steps, rather than instrument tree settings.

Strike, barrier, quantity and fixing are positive; rebate finite/nonnegative and currency matched; discrete dates sorted/nonempty/no later than expiry. Shared GBM defaults are listed in the Asian record. Heston path count must be valid. PDE numerical parameters are fixed private fields in the registered default.

### Limitations, failure and fallback

Analytic/PDE constant effective rates differ from datewise-drift MC on nonflat curves. Continuous equity barriers lack an explicit monitoring-start field/history requirement; missing breach state can therefore mean unseasoned even if the economic trade is seasoned. The retained post-expiry claim, paid-rebate treatment and fixed PDE domain/grid are observed implementation choices, not independently qualified lifecycle or convergence guarantees.

### Alternatives and tradeoffs

Analytic formulas are efficient for flat continuous coefficients; PDE and MC support explicit discrete schedules. Local-volatility/time-dependent PDE and stochastic-rate barrier models are absent. Heston volatility does not make its continuous-crossing approximation exact.

### Implementation evidence

- [finstack-quant/valuations/exotics/src/exotics/barrier_option/heston_mc_pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/heston_mc_pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/barrier_option/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/barrier_option/pde_pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/pde_pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/barrier_option/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/barrier_option/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/types.rs)

- [finstack-quant/valuations/exotics/tests/barrier_option/test_day_count_basis.rs](../../../../finstack-quant/valuations/exotics/tests/barrier_option/test_day_count_basis.rs): Instrument/curve day-count regression comparisons.
- [finstack-quant/valuations/exotics/tests/barrier_option/theta_observed_state.rs](../../../../finstack-quant/valuations/exotics/tests/barrier_option/theta_observed_state.rs): Theta equals held-spot explicit observed-state repricing; internal consistency, not external market-path evidence.

Canonical reference index. [Reiner Rubinstein](../../../REFERENCES.md#reiner-rubinstein-1991), [Andersen QE](../../../REFERENCES.md#andersen-2008-heston-qe), [Press](../../../REFERENCES.md#press-numerical-recipes).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
