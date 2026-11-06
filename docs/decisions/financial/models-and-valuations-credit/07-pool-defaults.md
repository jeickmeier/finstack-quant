# MODELS-AND-VALUATIONS-CREDIT-07: Monthly pool defaults and per-name copula sampling

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Default specifications dispatch deterministic, conditional copula, lognormal intensity-shock or factor-shocked monthly defaults. The intensity specification advertises mean reversion that construction discards.

## Scope and input domain

Monthly pool default rates and single-period per-name default indicators. Constructor helpers clamp annual rates, loadings, volatilities and correlations; per-name construction instead rejects nonfinite/out-of-range correlation and unsupported multi-factor copulas.

## Algorithm

1. Deterministic spec defaults to 2% CDR and build returns None for a deterministic caller path. Copula CDR converts to MDR `1−(1−CDR)^(1/12)` then clips threshold probability to [1e−10,1−1e−10] before normal/t inverse CDF. Conditional probability delegates the selected copula; reported expected MDR is the unclipped annual-to-monthly result.
2. Intensity is `λ0 exp(−βσZ−(βσ)²/2)`; MDR=`1−exp(−λ/12)`. This is a static factor transform per call; seasoning is ignored and no persistent intensity state or mean reversion is applied. expected_mdr returns `1−exp(−λ0/12)`.
3. Factor-correlated defaults obtain deterministic MDR, convert errors to zero, shock by exp(−loading×vol×Z), and cap monthly rate at .50. Its expected MDR uses the analytic capped-lognormal expectation, including the .50 tail mass.
4. Per-name simulation draws one common mixing variable per period, then one idiosyncratic normal per name, computes latent variable and compares to the clipped marginal threshold. Antithetic mode negates idiosyncratic normals; systematic factor is supplied by caller. Conditional-probability APIs integrate out idiosyncratic shocks while sharing the sampled mixing draw.

## Selection and controls

Gaussian/Student-t/RFL are supported per-name variants; MultiFactor is rejected because the caller supplies a single systematic factor. Default PoolGranularity is PerName; LargeHomogeneous is an explicit variant consumed elsewhere. Missing conditional factor defaults to zero in intensity/factor-shock models.

## Conventions

CDR annual probability, MDR monthly probability, λ annual intensity, β signed loading, volatility dimensionless. The spec correlation accessor returns factor loading directly for FactorCorrelated whereas the built model reports abs(loading) capped at .99.

## Limitations, failure and fallback

Intensity expected MDR substitutes mean intensity before a nonlinear transform, so it differs from the expectation of conditional MDR when volatility is nonzero. mean_reversion is unused. Threshold clipping makes exact 0/1 marginals approximate in copula-based paths. Invalid deterministic base rates can become zero through unwrap_or.

## Alternatives and tradeoffs

True stateful CIR/intensity evolution and integrated lognormal MDR expectation are unimplemented alternatives. Per-name simulation retains concentration; the large-pool conditional-rate path suppresses idiosyncratic sampling.

## Evidence

Owners: [Default implementations](../../../../finstack-quant/models/src/credit/pool/default). Inline tests locate conditional/expected-rate and seeded sampling evidence; tests do not justify silently unused parameters. References: [Gaussian copula](../../../REFERENCES.md#li-2000-gaussian-copula), [Intensity models](../../../REFERENCES.md#lando-1998).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
