# MODELS-AND-VALUATIONS-CREDIT-09: Copula thresholds, factor conditioning and numerical integration

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Gaussian, Student-t, clipped random-factor-loading and two-factor Gaussian kernels expose both conditional probabilities and latent simulation. Their factor representations and quadrature methods differ materially.

## Scope and input domain

Correlation in [0,1]; Student-t df finite and >2; RFL loading volatility [0,.5]. Checked conditional entry requires exactly num_factors finite inputs and finite threshold; unchecked kernels retain fallback behavior.

## Algorithm

1. Gaussian conditional PD is `Φ((c−√ρZ)/√(1−ρ))`, with exact independent/comonotonic branches and argument clipping at ±10. Generic expectation uses 20-point Gauss–Hermite; threshold c=Φ⁻¹(PD).
2. Student-t threshold is tν⁻¹(PD). Latent variable is `(√ρZ+√(1−ρ)ε)/√W`, W=χ²ν/ν shared across names. Conditioning on [Z,W] is normal; its separate one-t-factor form uses a tν+1 conditional law. Mixing quantiles require uniform strictly inside (0,1). Expectation uses 20-point Hermite times generalized Laguerre gamma quadrature derived from symmetric-Jacobi eigenpairs (order constrained 10–64). Paired expectation shares evaluations. Tail dependence is `2 tν+1(−√((ν+1)(1−ρ)/(1+ρ)))`.
3. RFL uses β=clip(location+loadingVol×η,0,1). Brent calibrates location so E[β²]=ρ, analytic clipped-normal second moment, tolerance 1e−13 and bracket [−40vol,1+40vol]. Conditional PD uses βZ and residual √(1−β²); nested 20-point Hermite integrates Z and η. Actual tail dependence equals the point mass at β=1; the two-sigma stress proxy is separately named.
4. MultiFactor fixes 40% of total systematic variance to a sector factor, 60% global; conditional normal PD uses both independent factors and residual √(1−ρ), integrated with 10×10 Hermite. Its single-systematic conditional method integrates sector variance into the residual.
5. Latent factor configuration separately supports one factor (volatility/mean reversion metadata), prepay-credit two-factor presets, and N-factor Cholesky correlation. Full correlated-factor generation applies the factor matrix and per-factor volatility; diagonal contribution intentionally returns only its diagonal term.

## Selection and controls

CopulaSpec defaults Gaussian and chooses implementations; RFL helper clamps its argument but direct constructor validates. LatentSingle defaults vol 1 and mean reversion 0; two-factor presets are RMBS (.20,.25,−.30) and CLO (.15,.30,−.20). Tranche integration overrides generic Gaussian/RFL/multi-factor quadrature with adaptive integration.

## Conventions

Copula correlation is squared systematic loading; RFL location calibration preserves its second moment. Student-t W is a positive scale variable, not another normal factor. The checked Student-t API requires two factors although the unchecked implementation also accepts its documented one-factor t representation.

## Limitations, failure and fallback

Wrong factor lengths in unchecked Gaussian/multi-factor return unconditional PD (debug assertions can panic); RFL fills missing factors with zero; Student-t wrong length returns unconditional t PD. Invalid RFL calibration returns NaN. Generic try_integrate catches returned errors but does not universally reject nonfinite successful values. Gaussian/multi-factor tail_dependence returns zero even at exactly comonotonic correlation.

## Alternatives and tradeoffs

Higher-order/adaptive integration is a possible alternative for discontinuous conditional payoffs; the tranche implementation already selects it for some variants. General sector membership and fitted loading fractions are not exposed in CopulaSpec::MultiFactor.

## Evidence

Owners: [Copula implementations](../../../../finstack-quant/models/src/correlation/copula) and [latent_factor.rs](../../../../finstack-quant/models/src/correlation/latent_factor.rs). Inline tests locate marginal, integration and tail-dependence checks; these do not establish independent quadrature convergence. References: [Li](../../../REFERENCES.md#li-2000-gaussian-copula), [t copula](../../../REFERENCES.md#demarta-mcneil-2005-t-copula), [RFL](../../../REFERENCES.md#andersen-sidenius-2005-rfl).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
