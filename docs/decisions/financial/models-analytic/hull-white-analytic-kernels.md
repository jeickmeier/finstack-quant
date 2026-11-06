# MODELS-ANALYTIC-23: Hull–White affine bonds, caplets and covariance corrections

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

One-factor Gaussian rate kernels with constant κ and scalar/piecewise-constant σ. Scalar calibration params require κ,σ>0; scheduled model requires κ>0 and delegates schedule validation to core PiecewiseConstantCurve. Some standalone kernels permit κ=0.

## Choice and rationale

Affine bond coefficients and integrated Gaussian variance price zero-coupon options and caplets; deterministic multi-curve ratios and payment-measure covariance adjust projection. Futures convexity helper is a leading low-rate/small-variance approximation, not the full quote-dependent bias.

## Algorithm

Bκ(t,T)=−expm1[−κ(T−t)]/κ with exact zero-product limit T−t. State variance is ∫σ²(u)exp[−2κ(t−u)]du; covariance=Var(earlier)exp[−κ|Δt|]. Bond vol=B(expiry,maturity)√integrated variance. ln A=ln P0(T)−ln P0(t)+B f0(t)−σ²(1−exp(−2κt))B²/(4κ), with Ho–Lee limit. ZCB option applies Black to P0(maturity),K P0(expiry),total bond vol,unit time; <1e−15 vol uses intrinsic. Simple caplet uses bond strike1/(1+τK), opposite bond option direction and multiplier (Pd(pay)/Pf(pay))(1+τK). If1+τK≤0, cap is linear and floor zero. Scheduled term caplet prices ratio Pf(start)/Pf(end) adjusted by exp[−Var(xfix)×ratio_loading×payment_loading]; ratio loading=B(fix,end)−B(fix,start), payment loading=B(fix,pay)−B(fix,end), total ratio vol=abs(bondvol(end)−bondvol(start)). Discount by Pd(pay). Cap/floor sums period prices.

## Selection and controls

Futures adjustment=.5σ²(Bsettle,end/τ) × (B0,2settle×Bsettle,end+B0,settle²). Delayed compounded coupon multiplier=exp[−Bend,pay×(Bstart,end exp(−κΔ)Var(xstart)+.5σ²Bstart,end²)]. Term multiplier uses fixing-state variance times differences of B. Approximate normal caplet vol=σ B0,τ/τ ×√[(1−exp(−2κtfix))/(2κtfix)], with small-κ limit. Market scalar-key helpers distinguish generic HW from cap/floor calibration, and expose a cap/floor σ-schedule key.

## Conventions

κ inverse years, σ absolute decimal short-rate units/√year, τ contractual accrual fraction distinct from model-year intervals; bond vol is total log standard deviation. Pf projects and Pd discounts. No stochastic basis factor is modeled.

## Limitations, failure and fallback

Low-level scalar helpers have unequal validation; bad arithmetic may return NaN or clamped intrinsic while Result variants validate schedules/times. Payment time is allowed between start/end; beyond-end is not mandatory. `state_covariance` returns zero when earlier time≤0 before complete time-domain checking. Convexity approximation must not be represented as exact futures bias.

## Alternatives and tradeoffs

Constant and piecewise σ variants are supported. Full quote-dependent futures convexity and stochastic multi-curve basis are outside these helper formulas.

## Implementation and validation evidence

[hull_white.rs](../../../../finstack-quant/models/src/rates/hull_white.rs). `delayed_term_caplet_matches_independent_payment_measure_integral` integrates Gaussian loading covariance independently; scalar/schedule equivalence and put/call parity are internal invariants. `convexity_matches_quantlib_low_rate_small_variance_expansion` distinguishes approximation from full bias. References: [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-ird), [Brigo–Mercurio](../../../REFERENCES.md#brigo-mercurio-2006-interest-rate-models).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
