# MODELS-ANALYTIC-11: Black, Merton jump and variance-gamma characteristic laws

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Risk-neutral log-return characteristic laws feeding COS. BS stores finite-domain responsibility in its wrapper; Merton validates nonnegative diffusion/jump intensity/jump std; VG requires positive σ,ν and finite rates/θ.

## Choice and rationale

BS Gaussian diffusion, compound-Poisson lognormal jumps, and gamma-time-changed Brownian returns are explicit choices. Their drift compensators preserve the intended forward moment; no calibration is embedded here.

## Algorithm

BS exponent is iu(r−q−σ²/2)T−σ²u²T/2. Merton adds λT[exp(iuμJ−σJ²u²/2)−1] and subtracts iuλ[exp(μJ+σJ²/2)−1]T; c1 includes λμJ, c2=σ²T+λT(σJ²+μJ²), c3/c4 are λT times third/fourth raw jump moments. VG φ=exp{iu(r−q+ω)T−(T/ν)ln[1−iuθν+σ²νu²/2]}, ω=ln(1−θν−σ²ν/2)/ν. VG cumulants include c1=(r−q+ω+θ)T, c2=(σ²+νθ²)T and analytic higher moments. These cumulants determine the COS window, not just report statistics.

## Selection and controls

Merton wrapper calls validate, including finite compensator. VG constructor/validate requires martingale argument>0. Public struct literals and derived serde can bypass validation; CF methods themselves mostly propagate invalid arithmetic. VG ω returns NaN for a nonpositive argument.

## Conventions

λ is jumps/year; μJ and σJ describe log jumps, not arithmetic mean/std. ν is gamma-time variance parameter and θ Brownian drift under time change. Cumulants are of log returns and carry annual-time scaling.

## Limitations, failure and fallback

Merton doc formula omits −σ²/2 in its printed drift while implementation includes it. Degenerate zero-jump/diffusion inputs may validate in Merton but later fail COS window construction. No jumps in rates/carry are modeled.

## Alternatives and tradeoffs

The three laws are supported alternatives through distinct wrapper functions. Other Lévy laws require an explicit characteristic-function implementation; no model is inferred from data.

## Implementation and validation evidence

[black_scholes.rs](../../../../finstack-quant/models/src/fourier/characteristic_function/black_scholes.rs), [merton.rs](../../../../finstack-quant/models/src/fourier/characteristic_function/merton.rs), [variance_gamma.rs](../../../../finstack-quant/models/src/fourier/characteristic_function/variance_gamma.rs) and [mod.rs](../../../../finstack-quant/models/src/fourier/characteristic_function/mod.rs). Module tests compare analytic cumulants to finite differences and λ→0 Merton to BS; these are identity/limit checks. References: [Merton jumps](../../../REFERENCES.md#merton-1976-jump), [Variance gamma](../../../REFERENCES.md#madan-carr-chang-1998).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
