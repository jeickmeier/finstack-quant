# MODELS-ANALYTIC-27: Nelson–Siegel factor extraction and Diebold–Li VAR forecast

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Historical decimal yield panels with positive ascending tenor years, at least2 rows and3 tenors; VAR needs≥5 observations. Optional dates are length-checked but ordering/equal spacing is not enforced.

## Choice and rationale

Cross-sectional OLS extracts three Nelson–Siegel factors at fixed λ, followed by unconstrained VAR(1) with intercept. It is a historical forecast, not a risk-neutral no-arbitrage pricing model.

## Algorithm

Loadings X=[1,(1−exp(−λτ))/(λτ),(1−exp(−λτ))/(λτ)−exp(−λτ)], with small λτ limit. Solve (X′X)β=X′y by Cholesky for each row. Report per-tenor R²=1−SSR/SST; SST<1e−30 reports1 regardless SSR. Fit βt+1=c+Φβt by Cholesky normal equations with[1,βt] design. Solve μ=(I−Φ)⁻¹c by LU; singular/nonfinite mean errors. Q=residual′residual/max(nobs−4,1). Forecast βh=μ+Φ^h(βlast−μ); covariance Σh=Σj=0..h−1 Φ^j Q(Φ^j)′. Yields=Xβh; marginal95% bands ±1.96√max(XiΣhXi′,0).

## Selection and controls

Default λ=.7308 inverse years; explicit positive finite override. Factor extraction resets prior fitted dynamics. Standalone Nelson–Siegel yields allows zero tenor; panel tenors strictly positive. Serde validates finite shapes and joint fit fields but not stationarity or covariance PSD. No λ optimization or shrinkage.

## Conventions

Horizon is count of adjacent observations, not calendar years; dates are metadata. Yield units determine factor units and output bands. Forecast bands omit cross-sectional residual noise and parameter-estimation uncertainty.

## Limitations, failure and fallback

Normal equations can amplify conditioning errors; singular matrices reject. Invertible I−Φ does not imply stable VAR, so explosive forecasts are possible. Minimum5 rows can leave zero residual degrees of freedom, with denominator floored1. Constant-series R²=1 can hide a nonzero cross-sectional fitting error. Public panel fields allow invalid mutation after construction.

## Alternatives and tradeoffs

Kalman state-space DNS, stationary/shrunk VAR, λ optimization or Svensson are unimplemented alternatives. Separate PCA describes changes rather than imposing NS economic factor loadings.

## Implementation and validation evidence

[diebold_li.rs](../../../../finstack-quant/models/src/rates/dtsm/diebold_li.rs) and [types.rs](../../../../finstack-quant/models/src/rates/dtsm/types.rs). Known-NS factor recovery is synthetic formula evidence; var_mean_rejects_unit_root checks singular mean. [dtsm_serde.rs](../../../../finstack-quant/models/tests/dtsm_serde.rs) checks shape contracts. References: [Diebold–Li](../../../REFERENCES.md#diebold-li-2006), [Nelson–Siegel](../../../REFERENCES.md#nelson-siegel-1987).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
