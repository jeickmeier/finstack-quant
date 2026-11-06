# VALUATIONS-STRUCTURED-CREDIT-15: Monte Carlo factors, finite names and loss estimation

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: stochastic statistical/instrument pools and full waterfall scenarios.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

The stochastic engine simulates pool behavior and executes the complete waterfall on each path, preserving nonlinear coverage, loss and cash-allocation effects. It does not price cash securitizations with the synthetic-CDO tranche engine in ADR 0031. The comment naming “scenario tree” is not a second tree-pricing mode: the implemented pricing mode is Monte Carlo.

## Algorithm and numerical controls

`num_paths` counts independent estimators and must be at least two. With antithetics enabled, each estimator runs a path and sign-mirrored partner, so 5,000 estimators means 10,000 scenarios. Ordered parallel collection preserves deterministic ordering. Philox uses a deterministic FNV-derived instrument-ID/valuation-date seed and separately salted streams for systematic credit, prepayment, regimes, per-name events and instrument utilization. Renaming an instrument or moving the date therefore changes draws.

Monthly latent factors follow stationary AR(1): `Z1 = epsilon1`, `Zm = phi Z(m-1) + sqrt(1-phi²) epsilonm`, `phi=exp(-kappa/12)`. Zero kappa retains one horizon-persistent factor. Multiple months are aggregated using variance `m + 2 sum((m-k) phi^k)` so the period copula factor remains standard normal. Supported factor dispatch is single factor or two factors; the latter forms a separately streamed prepayment factor with the supplied correlation. Intensity-process mean reversion can set the common-factor persistence.

Prepayment models receive seasoning, refi rate, persistent burnout/regime state and uniforms. Default models receive seasoning and factor; monthly SMM/MDR are clamped to [0,0.50]. Period survival multiplies monthly survivals. Recovery uses a modeled conditional recovery based on the period's mean factor. Copula default specifications can generate finite-name events or conditional/LHP behavior according to pool granularity; individual instrument hazard curves override the deal marginal probability where present.

The explicit `CorrelationStructure` supplies a scalar asset-correlation override; full Matrix/Sectored per-pair dependence is not consumed here. The ordinary public configuration builder starts from `SingleFactor { volatility: 1, mean_reversion: 0 }` and never copies the preset prepayment/default correlation into a `TwoFactor` configuration. Thus the ordinary path shares one factor, even when the preset correlation structure contains -0.30 or -0.20 prepayment/default correlation. The private engine supports a two-factor branch, but support is distinct from selection. Main-engine discounting is deterministic relative curve discounting; stochastic pool “rates” means prepayment/default channel rates, not a Hull-White discount-rate simulation.

PV standard error/confidence interval uses independent estimates, averaging antithetic pairs first. Loss standard deviation uses population variance of scenario losses. Expected shortfall averages the largest `ceil(5% × scenarios)` losses, at least one. Tranche WAL averages only paths with positive principal; reported credit duration is positive-cashflow PV-weighted time, not a bumped credit sensitivity. Clean and dirty MC prices are equal by construction.

## Domain, failures and alternatives

Month-based frequency and contiguous input-order attachment bands are required. Cumulative-loss/timing default curves, severity vectors and NPL resolution are rejected in stochastic pricing. Missing stochastic specifications become deterministic specs. No calibration fit occurs. Supported alternatives include deterministic inputs, different stochastic pool model specs and granularity; deterministic expected flows and the separate OAS engine have materially different assumptions.

## Implementation and validation evidence

- [structured_credit/pricing/stochastic/pricer/engine.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/stochastic/pricer/engine.rs)
- [structured_credit/pricing/stochastic/pricer/config.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/stochastic/pricer/config.rs)
- [structured_credit/types/pricing_methods.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pricing_methods.rs)
- [structured_credit/pricing/simulation_engine/pool_flow_source.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flow_source.rs)
- [family.rs](../../../../finstack-quant/valuations/structured-credit/src/family.rs)

Existing evidence: [structured_credit/unit/stochastic_pricing_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/stochastic_pricing_tests.rs); [structured_credit/unit/per_name_copula_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/per_name_copula_tests.rs); [structured_credit/unit/instrument_pool_stochastic_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/instrument_pool_stochastic_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [Monte Carlo](../../../REFERENCES.md#glasserman-2004-monte-carlo), [Philox](../../../REFERENCES.md#salmon-2011-philox), [Gaussian copula](../../../REFERENCES.md#li-2000-gaussian-copula). These references identify the method family; they do not certify the preset parameters or every implementation branch.

The deterministic limiting-vector assertion and RNG/reproducibility tests are meaningful but do not validate nonlinear tail accuracy, Monte Carlo convergence or preset empirical calibration.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
