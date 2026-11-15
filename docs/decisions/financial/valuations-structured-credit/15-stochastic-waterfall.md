# VALUATIONS-STRUCTURED-CREDIT-15: Monte Carlo factors, finite names and loss estimation

- **Decision status:** Accepted 2026-11-15 — complete stochastic waterfall, factor and collateral laws, reproducible sampling and path estimation package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent factor, finite-name loss, draw alignment, PV uncertainty, tail-mass and WAL examples checked. Production calibration, convergence, nonlinear tail accuracy and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for stochastic waterfall pricing and estimation

1. **Run the contractual waterfall on every trajectory.** Select stochastic
   pricing explicitly. Direct and registered pricing resolve one Rust request.
   Initialize each trajectory with the same qualified opening state and run
   the authorized collateral, claims, coverage, call, loss and terminal rules
   on that trajectory. Use positive dated relative discount factors from the
   selected deterministic curve. Identify the projection measure and assumption
   provenance. A physical forecast or stress run gives a PV under its stated
   assumptions. Market pricing requires qualified inputs on that basis.
   OAS and synthetic tranche pricing retain their separate routes.
   [Accepted dispatch](01-dispatch-and-assumptions.md#accepted-decision-for-structured-credit-dispatch-and-assumptions).

2. **Prepare the actual contractual grid and validate the route.** Bind
   collection, payment, exposure, event and terminal dates, including authorized
   stubs and live recovery tails. Remove the stochastic `tree_steps` horizon
   override and the positive-performing-balance shortcut. Validate waterfall
   authority directly and use derived loss-order bands for reporting, including
   qualified shared bands for pari passu groups. Reject active original-balance
   loss targets, timing curves or severity vectors on incompatible stochastic
   routes. Expected delinquency migration requires a qualified path-entry rate.
   Unresolved NPL workouts, modeled CMBS balloon splits and active stochastic
   card-trust behavior require separate qualification.
   [Accepted pool bands](02-pool-normalization-and-statistics.md#accepted-decision-for-collateral-normalization-and-pool-statistics) ·
   [Accepted terminal cash](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash).

3. **Elect factor persistence and coupling.** On the dated model-month grid
   start credit at a standard-normal draw. For finite annual `κ >= 0`, set
   `φ = exp(−κ/12)` and evolve `Z_m = φ Z_(m−1) + sqrt(−expm1(−κ/6)) ε_m`.
   At `κ = 0`, retain the initial draw throughout. Select a shared
   credit/prepayment factor or a second stationary driver `E` with the same
   persistence and explicit `r` in `[−1,1]`, forming
   `Z_p = r Z_c + sqrt((1−r)(1+r)) E` with exact endpoints.
   For a copula period, standardize `Σ_m w_m Z_m` by
   `sqrt(Σ_ij w_i w_j φ^|i−j|)` with actual model-month exposure weights.
   A separately selected recovery-factor average retains its own scale.
   Resolve draw demand from all active consumers, including recovery and
   utilization. Separate prepayment factor coupling, copula correlation,
   recovery and utilization loadings. Reject unsupported matrix or sector
   dependence and conflicting or unused parameters.
   [Accepted pool dependence](../models-and-valuations-credit/08-prepayment-and-pool-correlation.md#accepted-decision-for-checked-prepayment-and-pool-correlation).

4. **Elect the default, prepayment and recovery laws.** Use the actual
   intersected monthly exposure for conditional-rate survival compounding,
   with the asset's applicable seasoning, refinancing authority, burnout and
   regime state. Remove generic 50% monthly caps and deal-average shock
   rescaling. A qualified named hazard supplies `−expm1(−integrated hazard)`
   on the actual exposure. A selected period copula uses one reference
   marginal per eligible name from the qualified hazard or base default law.
   Evaluate checked Gaussian, Student-t or random-factor-loading conditioning
   once per period with the prescribed common mixing and literal probability
   endpoints. Elect `PerName` with a qualified roster of distinct default
   identities or `ConditionalFraction` with expected defaulted par conditional
   on the factors. The marginals describe the reference period law. Persistent
   factors and survival selection change the multiyear survival distribution.
   Recovery follows instrument or claim authority, expressly adopted hazard
   metadata, then the qualified deal or path source. Sample recovery only on
   authorized realized defaults and keep gross par, vintage, recovery amount
   and receipt date distinct.
   [Accepted pool default laws](../models-and-valuations-credit/07-pool-defaults.md#accepted-decision-for-checked-pool-default-rates-and-period-copula-resolution) ·
   [Accepted contractual collateral](08-instrument-collateral.md#accepted-decision-for-instrument-collateral-and-liquidity).

5. **Run one checked fixed sampling budget.** Require the explicit independent
   count `N >= 2`, pairing selection, and exact `u64` seed or explicitly
   selected qualified shared derivation. Remove automatic 5,000 estimators,
   automatic antithetics, minimum-50 repair and implicit ID/date seed.
   Preflight count, physical path multiplication, host representation,
   work and address capacity. Use the accepted Philox profile with versioned
   stable addresses by estimator, model date, original identity and role.
   Skips and survivor compaction preserve corresponding draws. Reflect
   corresponding normals and complement open regime uniforms. Reuse the
   prescribed period copula mixing within the pair. Preserve logical output
   order across execution modes. Report the first logical path failure with
   its actual operation and date. Publish the complete successful run atomically.
   [Accepted Philox and seed rules](../models-monte-carlo/02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

6. **Compute matching PV, prices and uncertainty.** Discount each eligible
   signed note settlement cashflow once at the explicit entitlement cut.
   Include eligible funding and draw cash, exclude noncash capital effects,
   and reconcile deal PV to same-path note PVs. Use single-leg PVs or
   checked equal-weight pair means for sample standard error and the
   fixed-sample 95% Student-t interval. A constant realized sample has a
   point interval. The interval describes sampling uncertainty under the
   selected model, with approximate coverage for general waterfall payoffs.
   Report both independent counts and physical trajectory counts. On the
   matched deal or note basis, dirty price is `100 × mean PV / current face`
   when face is positive. Leave it unavailable at zero face. Compute clean
   only with matching accrued cash.
   [Accepted PV estimators](../models-monte-carlo/01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

7. **Use unaveraged trajectories for credit-loss statistics.** Measure
   nominal incremental note principal loss attributable to credit,
   including qualified final extinguishments and authorized reversals
   attributable to the measured loss. Exclude opening booked loss,
   repayments and noncredit call effects. Keep par-preserving collateral
   loss separate. Sum matching note losses within each trajectory before
   calculating deal statistics. Report mean loss and population standard
   deviation on all equally weighted physical paths. Require explicit
   finite confidence `0 < c < 1`. Use the shared checked empirical mass
   geometry to average exactly the worst `1−c` mass, including the
   appropriate fractional boundary observation. Paired legs remain dependent.
   The tail statistic carries no sampling interval.
   [Accepted empirical tail geometry](../models-and-valuations-credit/10-portfolio-loss-and-correlated-recovery.md#accepted-decision-for-checked-portfolio-loss-and-correlated-recovery) ·
   [Expected shortfall with discrete losses](https://arxiv.org/abs/cond-mat/0104295).

8. **Report repayment-conditioned WAL and honest availability.** Compute
   ACT/365F from valuation to actual eligible principal payment, weighted
   by positive repaid par and using the elected same-day entitlement cut.
   Exclude write-downs, PIK, funding and premiums. Average contributing
   path lives equally and report the count and fraction. With no
   contributing paths, WAL is unavailable. Remove the `credit_duration`
   label for positive-cashflow PV timing. Reconcile complete results,
   currency, units, availability, shape and provenance across Rust, the
   registry, Python, WASM and stored state. Records 16, 17 and 20–22 retain
   [utilization attribution](16-revolver-utilization-and-option-cost.md#accepted-decision-for-revolver-utilization-and-draw-spread-attribution),
   [OAS](17-oas-scenarios.md#accepted-decision-for-structured-credit-oas-and-settlement-spread-fitting),
   [quote inversion](20-quotes-and-inverse-pricing.md#accepted-decision-for-structured-credit-settlement-quotes-and-inversion),
   [bumped risk](21-risk-bumps-and-scenarios.md#accepted-decision-for-structured-credit-risk-and-scenarios)
   and [reporting](22-diagnostics-and-reporting.md#accepted-decision-for-structured-credit-diagnostics-and-reporting), respectively.
   [Accepted reporting and weighted life](../cashflows/14-aggregation-and-life.md#accepted-decision-exact-nominal-reporting-and-explicit-repayment-life).

## Independent calculation checks

- Month weights 0.5 and 1, `φ=0.5` and factors −1 and 0.25 give weighted
  sum −0.25, variance 1.75 and standardized factor −0.188982. The raw
  weighted recovery signal is −1/6 if that mean is selected. Conditional
  monthly default rates 2% and 4% on those fractions compound to 0.0496485.
- Two conditionally independent names with face 20 and 80, conditional
  default probabilities 10% and 30% and full LGD give loss outcomes
  0, 20, 80 and 100 with probabilities 0.63, 0.07, 0.27 and 0.03.
  Conditional-fraction mean loss is 26. With 25 of junior protection,
  that path gives senior loss 1 while the finite-name expected senior
  loss is 17.10. Two Gaussian periods with the same persistent factor,
  `ρ=1` and reference marginal 20% give cumulative default 20%, while
  independent periods would give 36%.
- A surviving name B uses its own −0.7 normal even when original name A,
  assigned +0.9, has already defaulted. At `ρ=0` and `p=0.5`, B
  defaults on that leg and survives with +0.7 on the reflected leg.
- Antithetic PV legs (70, 130) and (80, 100) give pair means 100 and 90,
  mean PV 95 and SE 5. At 95% loss confidence, 22 physical legs with
  losses 100, 20 and twenty zeros give ES about 92.73. Whole-observation
  rounding gives 60. Repayments of 20 at year one, 80 at year three,
  zero, and 100 at year two produce conditional WAL 2 years on three of
  four paths. The ratio using pooled repayment weights is 2.3 years.

## Observed implementation and scope

The observed engine simulates pool behavior and executes the complete
waterfall on each trajectory. The scenario-tree label refers to the
Monte Carlo implementation. ADR 0031 describes the synthetic tranche engine.

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

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
