# MODELS-ANALYTIC-13: SABR normal, lognormal and shifted asymptotic quotes

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Positive α, β∈[0,1], ν≥0, ρ∈[−1,1], optional strictly positive shift; T>0. Non-normal branches need positive effective F,K; β≈0 permits crossing zero.

## Choice and rationale

Hagan asymptotic expansions with explicit normal/lognormal variants, stable near-ATM/χ formulas and beta snapping. These approximate quote laws are not an arbitrage guarantee or full SABR transition density.

## Algorithm

For lognormal, effective F,K include shift; snap β<1e−4 to0 and |1−β|<1e−4 to1. Let z=(ν/α)(FK)^((1−β)/2)ln(F/K), χ=ln[(√(1−2ρz+z²)+z−ρ)/(1−ρ)]. Quote=α/[(FK)^((1−β)/2)(1+(1−β)²L²/24+(1−β)^4L^4/1920)] ×z/χ×(1+T A), with A=(1−β)²α²/[24(FK)^(1−β)]+ρβνα/[4(FK)^((1−β)/2)]+(2−3ρ²)ν²/24. Relative |F−K|<1e−8 uses ATM limit. Normal β=0 uses z=ν(F−K)/α and α(z/χ) × (1+(2−3ρ²)ν²T/24). General normal uses α(FK)^(β/2), log-ratio numerator/denominator series and negative β(2−β) α² correction. χ uses a fourth-order series below1e−5, exact above1e−3 and smoothstep blend between; z/χ has its own cubic series below1e−5. Correlation endpoint logarithmic limits reject undefined domains.

## Selection and controls

`implied_vol` returns normal only for β<1e−4, otherwise lognormal/shifted quote. Explicit `implied_vol_normal` or `_lognormal` bypass that convention choice. `rates_standard` fixes β=.5; defaults are equity α=.2,β=1,ν=.3,ρ=−.2 or rates α=.02,β=.5,ν=.3,ρ=0. These are explicit convenience values, not calibrated market data.

## Conventions

Normal quote has rate/price units per √year; lognormal quote relative annual units. α dimensions depend on β. Shift changes effective rates, not output currency.

## Limitations, failure and fallback

Parameters have public fields; quote methods do not universally re-run parameter validation. Negative/nonfinite calculated quotes error; tiny-alpha ATM branch can return zero. Normal and lognormal endpoint/small-argument thresholds differ. Valid parameter ranges do not ensure approximation accuracy in long expiry/extreme wings.

## Alternatives and tradeoffs

Normal β=0 exact-backbone asymptotic form and explicit normal general-β expansion are supported alternatives; price-equivalent conversion is a separate ATM routine, not these normal expansions.

## Implementation and validation evidence

[expansion.rs](../../../../finstack-quant/models/src/volatility/sabr/expansion.rs) and [parameters.rs](../../../../finstack-quant/models/src/volatility/sabr/parameters.rs). [tests.rs](../../../../finstack-quant/models/src/volatility/sabr/tests.rs) includes Hagan ATM hand-formula, endpoint/χ crossover and fractional-beta independent-formula tests. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) distinguishes QuantLib lognormal and mpmath exact-β0 normal references. Reference: [Hagan SABR](../../../REFERENCES.md#hagan-2002-sabr).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
