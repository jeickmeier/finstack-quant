# Credit models and credit valuation methodologies

- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: **131 Rust source files** in models credit, correlation and liquidity plus the credit valuation family. The [coverage ledger](coverage.json) assigns every file once: 108 algorithm, 18 support, 5 test.
- Decision status: **Draft — observed implementation, adoption pending** for all 20 records. Implementation is observed in the defined source paths; discrepancies are recorded explicitly.
- Validation status: existing source/test evidence inspected or located, **not executed**. A formula test, roundtrip or production-kernel comparison does not establish independent accuracy or empirical validity.
- Registry tables and shared numerical/calendar/recalibration dependencies are linked in the records; their owning algorithms are covered by the corresponding neighboring methodology slices. This slice covers the credit callers, selection and financial consequences.

## Governing and overview ADRs

These ADRs retain their stated scope and decision status. They are separate
from the detailed algorithm records and source counts below.

| ADR | Decision status | Scope |
| --- | --- | --- |
| [0030](0030-cds-survival-and-integration.md) | Draft | CDS survival integration and valuation conventions |
| [0031](0031-tranche-loss-model-selection.md) | Draft | Synthetic tranche loss and copula selection |

## Inventory

Each row links a detailed record with formulas, domains, controls, conventions, fallbacks, alternatives and evidence.

| Calculation / supported instruments | Implemented method and material variants | Actual selection | Decision record | Implementation / validation | Scope and evidence limits |
|---|---|---|---|---|---|
| Distress scores for issuer ratios | Altman variants; logistic Ohlson; probit Zmijewski; fixed zones | Explicit score function / request variant | [MODELS-AND-VALUATIONS-CREDIT-01](01-financial-distress-scores.md) — Draft | Observed; [Coefficient, boundary and invalid-input tests](01-financial-distress-scores.md#evidence), not executed | Population fit; threshold rationale |
| PD cycle conversion and rating scales | Gaussian TTC/PIT; annual mean; fixed floor; bins; nearest WARF | Explicit parameters/ID; embedded default scales | [MODELS-AND-VALUATIONS-CREDIT-02](02-pd-scales-and-rating-data.md) — Draft | Observed; [Transform roundtrip and registry/scale cases](02-pd-scales-and-rating-data.md#evidence), not executed | Assumption versus empirical data; central-PD constraints |
| Rating migration and default paths | Schur/log-series generator; repair; Pade exponential; Gillespie | Direct generator or matrix inversion; explicit seed/tolerance | [MODELS-AND-VALUATIONS-CREDIT-03](03-rating-migration.md) — Draft | Observed; [Reconstruction, stochasticity and simulated frequency cases](03-rating-migration.md#evidence), not executed | Embeddability domain; repair budgets |
| Structural PD, calibration and spreads | Terminal/first-passage/jump/CreditGrades; equity/CDS inversions; hazard export | Explicit dynamics/barrier; terminal diffusion calibration | [MODELS-AND-VALUATIONS-CREDIT-04](04-structural-default.md) — Draft | Observed; [Formula, inversion and simulation cases](04-structural-default.md#evidence), not executed | NaN conversion; Poisson tails; approximate CDS; EDF interpretation |
| PIK-linked hazard and issuer exercise | Leverage maps; threshold/logistic; nested common-stream equity comparison | Explicit map/exercise variant | [MODELS-AND-VALUATIONS-CREDIT-05](05-hazard-and-toggle.md) — Draft | Observed; [Branch, monotonicity and deterministic-stream cases](05-hazard-and-toggle.md#evidence), not executed | Monthly barriers; absent-state substitutes; fitted policy |
| LGD, EAD and recovery allocations | Dilution curves; CCF; discounted workout; downturn; beta; priority pro rata | Explicit models and registry defaults | [MODELS-AND-VALUATIONS-CREDIT-06](06-recovery-and-lgd.md) — Draft | Observed; [Conservation, moments and formula cases](06-recovery-and-lgd.md#evidence), not executed | Approximate calibration; estate/collateral assumptions |
| Pool and per-name defaults | Copula thresholds; capped factor rates; static lognormal intensity | Spec variant; deterministic returns None; per-name default | [MODELS-AND-VALUATIONS-CREDIT-07](07-pool-defaults.md) — Draft | Observed; [Conditional rate and seeded latent-default cases](07-pool-defaults.md#evidence), not executed | Ignored mean reversion; nonlinear expectation; clipping |
| Prepayment and pool dependence | CPR/SMM integration; refinance/burnout; Markov regimes; scalar reductions | Spec variant; PSA default; explicit persistent state | [MODELS-AND-VALUATIONS-CREDIT-08](08-prepayment-and-pool-correlation.md) — Draft | Observed; [Expectation/transition and bump cases](08-prepayment-and-pool-correlation.md#evidence), not executed | Baseline expectation; scalar-matrix loss; PSD after bumps |
| Default copulas and latent factors | Gaussian/t/RFL/multi-factor; Hermite/Laguerre; Cholesky | Gaussian default; explicit alternatives and factor presets | [MODELS-AND-VALUATIONS-CREDIT-09](09-copula-kernels.md) — Draft | Observed; [Marginal, tail and integration cases](09-copula-kernels.md#evidence), not executed | Shape fallbacks; sector fractions; convergence |
| Portfolio loss and stochastic recovery | Indexed simulation; logistic-normal recovery; empirical VaR/fractional ES | Explicit loadings/copula/seed; optional recovery model | [MODELS-AND-VALUATIONS-CREDIT-10](10-portfolio-loss-and-correlated-recovery.md) — Draft | Observed; [Deterministic paths, moments and tail-mass cases](10-portfolio-loss-and-correlated-recovery.md#evidence), not executed | Recovery center versus mean; tail error; zero-loading factor |
| Liquidity and execution estimates | Roll/Amihud; Kyle; Almgren–Chriss; spread LVaR; liquidation tiers | Explicit calculator; helper synthetic 20bp spread | [MODELS-AND-VALUATIONS-CREDIT-11](11-liquidity.md) — Draft | Observed; [Formula, domain and trajectory cases](11-liquidity.md#evidence), not executed | Heuristic coefficients; units; cutoff adoption |
| Single-name CDS valuation and schedules | Conditional survival; piecewise default/AoD integrals; clean conventions | HazardRate; instrument conventions | [MODELS-AND-VALUATIONS-CREDIT-12](12-cds-legs.md) — Draft | Observed; [Independent AoD integral and rolled-coupon regressions](12-cds-legs.md#evidence), not executed | Calendar approximation; same-day cash; vendor benchmarks |
| CDS/index credit, rate and margin risk | Quote rebootstrap; recovery replay; JTD/EL; proxy margin | Metric registry and sensitivity controls | [MODELS-AND-VALUATIONS-CREDIT-13](13-cds-and-index-risk.md) — Draft | Observed; [Repricing, factor and convention cases](13-cds-and-index-risk.md#evidence), not executed | Expiry JTD/proxy; forward-start EL; replay policy |
| CDS index valuation and projected rows | Single curve/active constituents; analytic legs; midpoint informational flows | Explicit IndexPricing mode | [MODELS-AND-VALUATIONS-CREDIT-14](14-index-valuation.md) — Draft | Observed; [Weight/default-factor and aggregation cases](14-index-valuation.md#evidence), not executed | Maturity receivable loss; cashflow/PV mismatch |
| CDS option premium | Mean-calibrated lognormal spread and annuity-aware payoff quadrature | BloombergCdso; native-strike Black vol source | [MODELS-AND-VALUATIONS-CREDIT-15](15-cds-option-quadrature.md) — Draft | Observed; [Lifecycle, factor/strike and settlement cases](15-cds-option-quadrature.md#evidence), not executed | Distressed limits; stub/FEP parity; independent vendor grid |
| CDS option risk and implied vol | Screen Black ratios; quote hedge ratios; fixed-model Theta; log-vol Brent | Strike-dependent risk; explicit premium target | [MODELS-AND-VALUATIONS-CREDIT-16](16-cds-option-risk-and-inversion.md) — Draft | Observed; [Lifecycle and risk formula/contract evidence](16-cds-option-risk-and-inversion.md#evidence), not executed | Screen/PV distinction; expiry; bump/replay consistency |
| Tranche expected losses | Base-correlation differences; finite binomial; enumeration/grid; copula integration | Gaussian default or explicit copula; issuer data selects heterogeneity | [MODELS-AND-VALUATIONS-CREDIT-17](17-tranche-loss-distribution.md) — Draft | Observed; [Loss invariants and regression reference cases](17-tranche-loss-distribution.md#evidence), not executed | Narrow tranches; grid budget; monotonicity repairs |
| Tranche premium/protection projection | Loss/recovery-eroded premiums; expected default timing; upfront | Schedule conventions and index-prefix settlement lag | [MODELS-AND-VALUATIONS-CREDIT-18](18-tranche-projection.md) — Draft | Observed; [Schedule and seasoning cases](18-tranche-projection.md#evidence), not executed | Public timing loss; maturity cutoff; heterogeneous timing |
| Tranche quote and payoff risk | Unit-coupon spreads; quote/rate replay; correlation bump; recovery; JTD | Mixed dispatch and Gaussian-specialized metric constructors | [MODELS-AND-VALUATIONS-CREDIT-19](19-tranche-risk.md) — Draft | Observed; [Finite-difference and sign cases](19-tranche-risk.md#evidence), not executed | Copula propagation; issuer recovery; boundary denominator |
| Exchange and liability management | Supplied-PV comparison; 2% hurdle; repurchase/fee/dropdown arithmetic | Explicit LME type; fixed tender hurdle | [MODELS-AND-VALUATIONS-CREDIT-20](20-liability-management.md) — Draft | Observed; [Scenario formula and invalid-input cases](20-liability-management.md#evidence), not executed | Hurdle adoption; PV/par naming; omitted priority economics |

## Source and evidence boundary

The ledger includes financially material configuration, validation, dispatch, reporting fallbacks and trait defaults as algorithm-bearing where they affect results. Reexports/error formatting are support; five dedicated test files are test-only. Runtime implementations interleaved with test helpers are included, notably Student-t copula kernels and tranche schedule/risk calculations.

No existing method is adopted by this audit. Source/test inspection does not establish economic model suitability, empirical calibration or numerical accuracy.

Related family decisions: [CDS survival and integration](0030-cds-survival-and-integration.md) and [tranche loss-model selection](0031-tranche-loss-model-selection.md). Canonical reference entries are in [REFERENCES](../../../REFERENCES.md).

## Follow-up tracking

The [credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations) own adoption questions, implementation discrepancies and missing validation. The [central follow-ups](../../FOLLOW_UPS.md#financial-methodology-audit) coordinate cross-domain work.
