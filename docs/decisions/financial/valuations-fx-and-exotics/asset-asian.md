# VALUATIONS-FX-AND-EXOTICS-ASSET-ASIAN: Asset Asian seasoning, exact geometric control and GBM/Heston alternatives

- Record: `VALUATIONS-FX-AND-EXOTICS-ASSET-ASIAN`
- Decision status: **Accepted 2026-11-14** — complete asset Asian fixing, dated carry, pricing, simulation, settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent dated carry, Asian premium, arithmetic approximation, control-variate and horizon calculations checked. Runtime, market data, simulation and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for asset Asian pricing and settlement

1. **Book the payoff and the exact observation slots.** Require European
   fixed-strike cash settlement, call or put, explicit holder long or short,
   finite positive strike and quantity, selected arithmetic or geometric
   averaging and a qualified same-currency cash equity or spot index. Bind
   the live spot and observations to the same price and adjustment basis.
   Require a nonempty ordered fixing schedule with equal weight per slot
   and an explicit automatic payoff rule. Separate final observation,
   determination and payment, including qualified phases on a shared date.
   Consume exact authorized fixings at the qualified information cut.
   A same-day fixing becomes usable upon eligible publication. Missing
   eligible or unresolved authority errors. A pending observation needs a
   qualified projection with strictly positive model time.
   [Fixing authority](../core-market/scalar-series-and-fixings.md),
   [CBOE FLEX Asian contract precedent](https://www.sec.gov/files/rules/sro/cboe/2015/34-75425.pdf).

2. **Share the actual dated forwards and checked analytic kernel.** Elect no
   distributions or a qualified deterministic continuous yield. Prepare each
   forward from spot, yield to the fixing and its actual dated financing
   factor. Use an independent factor to actual payment. Discrete cash
   distributions require a qualified fixing law. Match the Asian volatility
   quote and preserve total variance when mapping to the model clock. Use
   `asian_geometric_bs` for exact geometric pricing and
   `asian_turnbull_wakeman` for the arithmetic two-moment fit. Keep the known
   arithmetic contribution separate and use the exact support and deterministic
   branches. Reject conflicting model selection and unresolved arithmetic.
   [Equity carry](../valuations-equity-and-composition/02-spot-and-carry.md),
   [checked Asian kernel](../models-analytic/asian-moments.md#accepted-decision-for-discrete-asian-pricing).

3. **Use exact event nodes in GBM simulation.** Apply exact lognormal
   transitions with dated carry increments. Bind each observation to its
   exact stored node and preserve slot multiplicity at qualified tied
   model times. Apply a qualified per-slot deterministic scale when tied
   slots have different dated forwards. Elect plain GBM for either
   average. Use explicit work, seed and sampling controls. Plain GBM may
   use the admitted serial Sobol profile with descriptive replicate
   diagnostics.
   [Asian payoffs](../models-monte-carlo/17-vanilla-and-asian.md),
   [whole-path Sobol](../models-monte-carlo/03-randomized-qmc.md).

4. **Fit the geometric control for arithmetic pricing on independent groups.**
   Observe the arithmetic and geometric payoffs on each shared path. Match
   the exact geometric expectation to the history, fixing slots and weights,
   dated carry, volatility, payoff terms and independent payment factor.
   Fit the coefficient on an explicit independent Philox pilot and freeze
   it for disjoint pricing groups. Each antithetic pair contributes one
   group. Report uncertainty from the adjusted pricing observations and
   actual pilot work separately. A failed fit errors.
   [Checked control variates](../models-monte-carlo/27-control-variate.md#accepted-decision-for-checked-single-control-monte-carlo-pricing).

5. **Use explicit classical Heston QE-M on the dated basis.** Require a
   complete sourced pricing-measure parameter block, explicit checked grid,
   fixed work budget, seed, pairing and execution mode. Feed dated carry
   increments into the martingale-corrected step. Use the exact integrated
   variance limit at zero volatility of variance. Report actual work and
   sampling uncertainty. Qualify time-step error separately.
   [Checked Heston QE-M](../models-monte-carlo/09-heston-qe.md),
   [Andersen](https://www.risk.net/journal-of-computational-finance/2160370/simple-and-efficient-simulation-of-the-heston-stochastic-volatility-model).

6. **Retain fixed cash through discharge.** After authorized observations and
   contractual determination establish the amount, apply quantity and side
   once and value signed cash on its actual due date. Include eligible unpaid
   same-day cash at factor one. Partial, failed or overdue payment needs a
   qualified residual. Resolve known amounts before live spot or volatility.
   Export one signed `DerivativeSettlement` with observation provenance.
   Complete cash export is unavailable while the payoff is contingent.
   Use the same Rust-owned state through the existing Python and WASM
   instrument envelopes. [Cashflow export](../valuations-framework/cashflow-export.md).

7. **Measure risk on the selected pricing model and state.** Delta and
   gamma use actual spot with its transmission through the dated carry.
   Lognormal volatility Greeks use the active source and one annual
   volatility point (`0.01`) as reporting increment. Heston keeps its
   own parameter basis and its Black volatility Greeks are unavailable.
   Use qualified logical common draws and paired uncertainty for stochastic
   reprices. In controlled reprices freeze the independently fitted
   coefficient and recompute the matching geometric expectation for each
   scenario. Retire the duplicate `rho` registration. Parallel and bucketed
   DV01 reprice each active curve once across carry and payment roles.
   Horizon theta uses qualified closing observations or an expressly
   elected hypothetical fixing scenario, with eligible signed cash.
   [Option risk](../valuations-framework/option-sensitivities.md),
   [horizon theta](../valuations-framework/carry-theta-breakeven.md).

## Independent calculations

- With spot `100`, known fixing `88`, future fixing times `0.25` and
  `0.75`, financing factors `0.997` and `0.958`, continuous yield `0.015`,
  volatility `0.29`, strike `100`, quantity `750` and payment factor
  `0.925`, the dated forwards are `99.9254786830` and `103.2163929657`.
  The geometric call PV is USD `2,049.59967514`. The arithmetic fit is
  USD `2,496.09020121` versus `2,493.65472540` by conditional Gaussian
  integration of the arithmetic payoff. Rebuilding both forwards from
  the one-year expiry factor `0.944` raises the fit to USD `2,611.71232849`.
- With spot `100`, known fixing `92`, future times `0.25` and `0.75`,
  financing factors `0.995` and `0.947`, yield `0.01`, volatility `0.30`,
  strike `100`, quantity `500` and payment factor `0.938`, independent
  antithetic pilot and pricing samples estimated the control coefficient
  at `1.16013752`.
  The held-out standard errors on this sample were USD `17.34` plain and
  `1.40` controlled. The independent experiment used 4,096 pilot and
  16,384 pricing groups with separate Python random streams. This checks
  the estimator construction in the example. Runtime Philox and route
  qualification remain pending.
- For a deterministic call with known fixing `80`, projected fixing `120`,
  strike `95`, quantity `500` and payment factor `0.98`, opening PV is
  USD `2,450`. An authorized hypothetical fixing at `120` and the eligible
  payment give cash USD `2,500` and closing PV zero. Horizon theta is
  USD `+50` on this scenario.

## Observed behavior

### Choice, rationale and scope

The inspected implementation offers geometric pricing, arithmetic moment
matching, GBM simulation with a geometric control, and Heston simulation.

### Algorithm and conventions

Validate a nonempty ordered fixing schedule and require observations on/before valuation. Accumulate realized sum, positive-fixing log sum and count. Expired/fully fixed payoff uses the realized average, with spot fallback only when count is zero. Asset Asian paths retain the realized claim for nonpositive remaining time, unlike the commodity Asian post-expiry zero branch.

Geometric pricing forms `μ=[historical_log_sum+Σ(log S+carry(t_i)-σ²t_i/2)]/n` and `v=σ²Σmin(t_i,t_j)/n²`; use the exact lognormal average-price formula with expiry DF. `carry(t_i)=-ln DF_i-q t_i`, so every fixing uses its own curve date. Arithmetic TW uses `K*=(nK-H)/m`, scale `m/n`, and the shared two-moment kernel at actual future fixing times. For nonpositive effective strike the call reduces to expected full-average intrinsic using datewise forwards; otherwise the shared TW kernel derives a constant effective rate from expiry DF.

GBM MC merges all future fixing times into the time grid; times must map within `1e-10`. A cumulative drift schedule preserves datewise curve forwards. Arithmetic payoff `X` and geometric control payoff `Y` observe one simulated path per leg. The engine accumulates discounted paired moments and applies `mean(X)-β[mean(Y)-E(Y)]` with the shared control-variate coefficient. The control expectation is the seasoned geometric formula above, scaled identically. Geometric MC directly averages payoffs.

Heston MC resolves a strict preconfigured Heston parameter set, starts `[spot,v0]`, uses QE steps and Philox, and includes exact fixing times. It multiplies fixing spots by `exp[-ln DF_i-r_eff t_i]` to restore datewise forwards. Defaults are 100,000 paths, 252 steps/year and minimum ten steps; positive path overrides and instrument seeds apply. Its returned stderr is attached as a measure.

Registered delta/gamma/vega/vanna/volga are generic finite-difference repricing; rho is the same parallel-combined 1bp curve calculator as DV01. Theta observation handling records the held valuation spot for newly crossed fixing dates.

### Selection and controls

Geometric averaging defaults to `AsianGeometricBS`; arithmetic to `AsianTurnbullWakeman`. Explicit registry alternatives are GBM and Heston MC. Direct instrument value dispatches the two analytic choices. Algorithm keys do not imply cross-model calibration equivalence.

Shared GBM defaults: 100,000 estimator paths, 252 steps/year, minimum eight steps, seed derived from instrument/scenario; parallel enabled, antithetic/Sobol/Brownian bridge initially off. Active quote/surface supplies constant expiry/strike GBM volatility. Heston parameters do not silently fall back to the Black surface.

### Limitations, failure and fallback

The positive-effective-strike TW branch uses flat terminal-implied drift while geometric, GBM and the linear seasoned branch preserve datewise curve forwards. The control-variate coefficient is estimated on the pricing sample. The [shared control-variate record](../models-monte-carlo/27-control-variate.md) owns coefficient fitting and adjusted uncertainty. Asset Asian residual settlement remains an observed lifecycle choice.

### Alternatives and tradeoffs

Exact geometric is supported; arithmetic MC is supported at computational cost. Moment matching is an approximation, and a calibrated stochastic-volatility model changes assumptions rather than merely improving numerical precision. Conditional-expectation or independent-training control variants are not implemented locally.

### Implementation evidence

- [finstack-quant/valuations/exotics/src/exotics/asian_option/heston_mc_pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/heston_mc_pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/asian_option/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/asian_option/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/asian_option/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/types.rs)

Delegated implementation dependencies:

- [finstack-quant/models/data/defaults/pricer_defaults.v1.json](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json)
- [finstack-quant/valuations/core/src/instruments/pricing/heston_market.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/heston_market.rs)

The original audit inspected source and inline assertions. The independent calculations above cover the stated cases. Production qualification is pending.

Canonical reference index. [Kemna Vorst](../../../REFERENCES.md#kemna-vorst-1990), [Turnbull Wakeman](../../../REFERENCES.md#turnbull-wakeman-1991), [Andersen QE](../../../REFERENCES.md#andersen-2008-heston-qe), [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
