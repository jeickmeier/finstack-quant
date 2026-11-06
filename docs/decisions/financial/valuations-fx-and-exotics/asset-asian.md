# Asset Asian seasoning, exact geometric control and GBM/Heston alternatives

- Record: `VALUATIONS-FX-AND-EXOTICS-ASSET-ASIAN`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

Asset Asian options combine supplied historical fixings with dated future observations. Geometric pricing, arithmetic moment matching, GBM simulation with a geometric control, and Heston simulation are separate supported algorithms.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Validate a nonempty ordered fixing schedule and require observations on/before valuation. Accumulate realized sum, positive-fixing log sum and count. Expired/fully fixed payoff uses the realized average, with spot fallback only when count is zero. Asset Asian paths retain the realized claim for nonpositive remaining time, unlike the commodity Asian post-expiry zero branch.

Geometric pricing forms `μ=[historical_log_sum+Σ(log S+carry(t_i)-σ²t_i/2)]/n` and `v=σ²Σmin(t_i,t_j)/n²`; use the exact lognormal average-price formula with expiry DF. `carry(t_i)=-ln DF_i-q t_i`, so every fixing uses its own curve date. Arithmetic TW uses `K*=(nK-H)/m`, scale `m/n`, and the shared two-moment kernel at actual future fixing times. For nonpositive effective strike the call reduces to expected full-average intrinsic using datewise forwards; otherwise the shared TW kernel derives a constant effective rate from expiry DF.

GBM MC merges all future fixing times into the time grid; times must map within `1e-10`. A cumulative drift schedule preserves datewise curve forwards. Arithmetic payoff `X` and geometric control payoff `Y` are simulated using the same seed/grid; capture paired final values, compute online means/variances/covariance, and apply `mean(X)-β[mean(Y)-E(Y)]` with the shared control-variate coefficient. The control expectation is the seasoned geometric formula above, scaled identically. Geometric MC directly averages payoffs.

Heston MC resolves a strict preconfigured Heston parameter set, starts `[spot,v0]`, uses QE steps and Philox, and includes exact fixing times. It multiplies fixing spots by `exp[-ln DF_i-r_eff t_i]` to restore datewise forwards. Defaults are 100,000 paths, 252 steps/year and minimum ten steps; positive path overrides and instrument seeds apply. Its returned stderr is attached as a measure.

Registered delta/gamma/vega/vanna/volga are generic finite-difference repricing; rho is the same parallel-combined 1bp curve calculator as DV01. Theta observation handling records the held valuation spot for newly crossed fixing dates.

## Selection and controls

Geometric averaging defaults to `AsianGeometricBS`; arithmetic to `AsianTurnbullWakeman`. Explicit registry alternatives are GBM and Heston MC. Direct instrument value dispatches the two analytic choices. Algorithm keys do not imply cross-model calibration equivalence.

Shared GBM defaults: 100,000 estimator paths, 252 steps/year, minimum eight steps, seed derived from instrument/scenario; parallel enabled, antithetic/Sobol/Brownian bridge initially off. Active quote/surface supplies constant expiry/strike GBM volatility. Heston parameters do not silently fall back to the Black surface.

## Limitations, failure and fallback

The positive-effective-strike TW branch uses flat terminal-implied drift while geometric, GBM and the linear seasoned branch preserve datewise curve forwards. The control-variate coefficient is estimated on the pricing sample and uses two separate seeded runs; this construction alone does not establish estimator bias or accuracy. Asset Asian residual settlement semantics are observed behavior, not an adopted lifecycle contract.

## Alternatives and tradeoffs

Exact geometric is supported; arithmetic MC is supported at computational cost. Moment matching is an approximation, and a calibrated stochastic-volatility model changes assumptions rather than merely improving numerical precision. Conditional-expectation or independent-training control variants are not implemented locally.

## Evidence

- [finstack-quant/valuations/exotics/src/exotics/asian_option/heston_mc_pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/heston_mc_pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/asian_option/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/asian_option/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/asian_option/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/types.rs)

Delegated implementation dependencies:

- [finstack-quant/models/data/defaults/pricer_defaults.v1.json](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json)
- [finstack-quant/valuations/core/src/instruments/pricing/heston_market.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/heston_market.rs)

No separate independent numerical benchmark was established in this slice. Inline tests in the owning implementation provide only the assertions present there; they are not proof of accuracy outside those cases.

Canonical reference index: [Kemna Vorst](../../../REFERENCES.md#kemna-vorst-1990); [Turnbull Wakeman](../../../REFERENCES.md#turnbull-wakeman-1991); [Andersen QE](../../../REFERENCES.md#andersen-2008-heston-qe); [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
