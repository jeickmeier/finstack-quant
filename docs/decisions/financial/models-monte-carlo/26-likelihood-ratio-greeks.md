# MODELS-MONTE-CARLO-26: GBM likelihood-ratio delta and vega density scores

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in greeks/lrm.rs and pricer/path_dependent.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

The high-level score estimator supports constant-drift GBM and payoffs whose dependence on S0/volatility is entirely through post-zero path densities. Payoffs explicitly opt in. It rejects time-zero Asian fixings, continuous barrier/lookback corrections, Sobol, antithetics, drift schedules and mismatch between score parameters and process parameters. Positive finite initial spot, volatility and discount factor are required. Every path is captured; budget is four million path points, additionally subject to generic capture limits.

## Algorithm and conventions

Capture discounted final payoffs and all spots on a uniform grid. Reconstruct each transition normal `z_i=(ln(S_(i+1)/S_i)-(r-q-sigma²/2)dt)/(sigma sqrt(dt))`. Delta uses only the first-transition density score `z_0/(S0 sigma sqrt(dt))` for the full path law. Vega sums transition scores `(z_i²-1)/sigma-sqrt(dt)*z_i`, multiplies each undiscounted payoff, reapplies discount, and multiplies by 0.01: reported vega is per one percentage-point volatility move. The low-level `lrm_delta` also supports a terminal density score when passed a terminal shock and full maturity. Each score-weighted payoff enters online mean/variance and Student-t interval independently.

## Failure behavior and alternatives

Low-level delta indexes shocks by payoff length without validation; vega silently zips to the shorter input. High-level capture ensures matching sizes but does not independently check every reconstructed log ratio before updating statistics. A density-only payoff assertion must be true for custom implementations; historical fixed values may be held constant but fresh initial-dependent contract fields cannot be ignored. First-step delta variance can worsen as the grid is refined. Common-random-number finite differences are implemented alternatives; pathwise estimators are not implemented in this slice.

## Evidence

[Score kernels](../../../../finstack-quant/models/src/monte_carlo/greeks/lrm.rs), [capture/reconstruction](../../../../finstack-quant/models/src/monte_carlo/pricer/path_dependent.rs); located tests include `test_lrm_greeks_unbiased_for_path_dependent_payoff`, `lrm_asian_greeks_match_common_random_number_bumps` and contract rejection. These compare methods under a shared GBM setup; an independent Greeks reference dataset is absent. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) is the canonical methodological reference.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
