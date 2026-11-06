# MODELS-ANALYTIC-08: Heston affine characteristic function and parameter domain

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Heston variance diffusion with positive v0,κ,θ,σv and −1<ρ<1. Fourier parameters accept finite r,q; constructor/serde validate, but public fields can be mutated.

## Choice and rationale

The two probability characteristic functions use a little-Heston-trap form, stable algebra for small vol-of-vol and series for small complex arguments. Feller violation warns but does not reject; callers may explicitly require Feller.

## Algorithm

For P1 choose u=1/2,b=κ−ρσv; for P2 u=−1/2,b=κ. Compute d=√[(iρσvφ−b)²−σv²(−φ²+2iuφ)], using the complex square-root branch, and denominator b−iρσvφ+d. Express the difference and g through division to avoid subtractive cancellation. Evolve with exp(−dT); use fifth-order expm1/log1p-related series when norms<1e−4. Exponent C+Dv0+iφ ln S gives the CF. φ=0 returns one. A relative denominator threshold 1e−8, nonfinite intermediates or real exponent>700 report Overflow with zero value; genuine numerical underflow reports Underflow. `deterministic_avg_variance` integrates mean-reverting expected variance and uses a second-order small-κT series below 1e−6.

## Selection and controls

`heston_pj_characteristic_function` selects j=1 or otherwise P2; it does not independently validate all scalar inputs. Pricing wrappers validate before calling it. Defaults v0=θ=.04,κ=2,σv=.3,ρ=−.5. `require_feller` is opt-in, with condition delegated to the shared MC process helper.

## Conventions

v0 and θ are annualized variances, κ inverse years, σv variance-process diffusion parameter, ρ Brownian correlation. P1 and P2 belong to different measures and are not the same generic log-return CF used by COS.

## Limitations, failure and fallback

Exact σv=0 is rejected by parameter validation although the stable formula supports a limiting sequence. Public CF invalid j silently selects P2. Overflow/underflow status must be honored; zero returned on overflow is not an economic value.

## Alternatives and tradeoffs

Original Heston complex-log forms and alternative branch tracking are not selected. Deterministic-average variance is a helper, not an automatic Black fallback in the Fourier pricer.

## Implementation and validation evidence

[heston.rs](../../../../finstack-quant/models/src/volatility/heston.rs) and [params.rs](../../../../finstack-quant/models/src/closed_form/heston/params.rs). [tests.rs](../../../../finstack-quant/models/src/closed_form/heston/tests.rs) checks zero frequency, limits and corruption; [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) has independent Heston price fixtures. References: [Heston](../../../REFERENCES.md#heston-1993), [Little Heston Trap](../../../REFERENCES.md#albrecher-2007-little-heston-trap).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
