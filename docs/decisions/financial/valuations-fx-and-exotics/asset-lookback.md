# Lookback extrema, continuous formulas and monitored GBM paths

- Record: `VALUATIONS-FX-AND-EXOTICS-ASSET-LOOKBACK`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

Fixed- and floating-strike lookbacks use recorded extrema plus future path extrema. Continuous formulas and GBM MC with continuous/discrete monitoring are supported.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

For fixed call the terminal payoff is `Q max(max_observed-K,0)`; fixed put uses `Q max(K-min_observed,0)`. Floating call is `Q max(S_T-min_observed,0)` and floating put `Q max(max_observed-S_T,0)`. A fixed-strike terminal observation enters the extremum only when expiry is monitored; floating payoff always uses terminal spot. After expiry require an explicit terminal fixing, and retain the realized claim.

Analytical pricing combines recorded extremum with current spot (`max` or `min`) and calls the four dedicated Conze/Goldman-style lookback kernels at effective rate `-ln DF/t`, dividend yield, and volatility at strike or current spot for floating strike. Missing continuous-history extremum defaults to current spot; nonfinite/invalid output is checked before constructing money.

MC builds uniform continuous steps or a required-date discrete grid, uses a date-derived GBM drift schedule, seeds the payoff with the supplied historical extremum (otherwise ±infinity), and simulates fixed/floating lookback payoffs. Continuous monitoring enables the shared payoff's Brownian-bridge extremum treatment with `σ`; discrete paths inspect only observation nodes. Discount at expiry. Generic finite differences provide delta/gamma/vega/vanna/volga; rho uses discount-only parallel curve repricing; DV01 uses combined parallel curve repricing, with key-rate variants.

## Selection and controls

Continuous monitoring defaults to `LookbackBSContinuous`; discrete dates default to GBM MC. Explicit analytical pricing rejects discrete monitoring. The relevant extremum is max for fixed call/floating put and min for fixed put/floating call.

Elapsed discrete dates require the relevant observed extremum. Strike is required for fixed lookbacks; extrema/spot/fixing/quantity follow positive domains. Shared GBM config and instrument seed apply. Volatility remains a single expiry/strike (or spot) quote, not a dynamically sampled smile.

## Limitations, failure and fallback

Continuous lookbacks have no explicit history-start requirement, so omission of an extremum means an unseasoned current-spot assumption. Analytical constant-rate and MC datewise-drift paths differ on nonflat curves. Monotonicity tests do not establish continuous-extremum sampling convergence or an adopted post-expiry residual-claim contract.

## Alternatives and tradeoffs

The analytic formulas avoid time-grid bias in their domain; discrete MC represents an actually discrete payoff. Using discretely sampled extrema for continuous claims would change price and is not equivalent. Multi-factor/stochastic-volatility lookbacks are not registered here.

## Evidence

- [finstack-quant/valuations/exotics/src/exotics/lookback_option/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/lookback_option/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/lookback_option/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/lookback_option/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/lookback_option/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/lookback_option/types.rs)

- [finstack-quant/valuations/exotics/tests/lookback_option/seasoning.rs](../../../../finstack-quant/valuations/exotics/tests/lookback_option/seasoning.rs): Recorded-extremum monotonicity and spot-clamping identities; invariant evidence rather than an independent full formula grid.

Canonical reference index: [Conze Viswanathan](../../../REFERENCES.md#conze-viswanathan-1991); [Goldman Sosin Gatto](../../../REFERENCES.md#goldman-sosin-gatto-1979); [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
