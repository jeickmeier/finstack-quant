# MODELS-ANALYTIC-18: Raw SVI evaluation, bounded transformations and fit acceptance

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Single-expiry raw SVI w(k)=a+b[ρ(k−m)+√((k−m)²+s²)], k=ln(K/F); ≥5 finite positive strike/vol quotes for calibration.

## Choice and rationale

Fit unweighted total-variance residuals by one-start LM, then validate basic parameter/moment bounds. Butterfly diagnosis exists separately and is not required by the fitter.

## Algorithm

Validation requires b≥0,s>0,|ρ|<1, finite parameters, minimum variance a+bs√(1−ρ²)≥−1e−14 and Lee slope b(1+|ρ|)≤2+1e−12. IV=√(w/T), requiring finite k,T>0,w≥0. Density g=(1−kw′/(2w))²−(w′²/4)(1/w+1/4)+w″/2; w≤0 returns −∞. Uniform requested-grid scan reports worst g<−tol; invalid scan with n<2 or nonincreasing limits returns None. Fit transforms b=exp(x),ρ=tanh(x),s=exp(x), leaves a,m free. Seed a=nearest-ATM variance,b=.1,ρ=m=0,s=.1. After LM, validate parameters and reject approximate quote error above.005, calculated as RMSE(w)/(2√mean(w)) (or RMSE(w) near zero).

## Selection and controls

Default LM tolerance1e−12 and300 iterations; alternate solver accepted explicitly. No multistart or hard butterfly/calendar constraints. Returned iteration count is solver count. Acceptance does not inspect termination reason separately from parameter/RMSE checks.

## Conventions

Raw w is total variance, not annual variance; s is width in log-moneyness. Observed approximate “vol RMSE” omits √T in denominator, so it estimates standard-deviation error rather than annualized-vol error.

## Limitations, failure and fallback

Minimum-variance and Lee bounds are not sufficient for butterfly freedom. Calibration can return an interior butterfly violation because g is not called. Fixed .005 acceptance depends on maturity in annual-vol terms. Grid scan can report no violation for an invalid/too-small scan.

## Alternatives and tradeoffs

Constrained SSVI or arbitrage-penalized multistart are unimplemented alternatives. Existing analytic density scan is diagnostic, not surface calibration.

## Implementation and validation evidence

[svi.rs](../../../../finstack-quant/models/src/volatility/svi.rs). Synthetic fit recovery/noisy-fit rejection tests establish objective behavior; `durrleman_g_matches_finite_difference_density` checks the derivative formula. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) uses50-digit mpmath formula values, not independently calibrated market parameters. References: [SVI](../../../REFERENCES.md#gatheral-2004-svi), [SVI arbitrage](../../../REFERENCES.md#gatheral-jacquier-2014-svi).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
