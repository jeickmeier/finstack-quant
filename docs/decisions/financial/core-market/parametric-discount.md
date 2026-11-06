# CORE-MARKET-07: Nelson–Siegel and Svensson curve evaluation

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

`ParametricCurve` evaluates a user-supplied Nelson–Siegel (NS) or Nelson–Siegel–Svensson (NSS) parameterization, with analytic zero/instantaneous-forward formulas and realized-forward rolling. It does not fit the coefficients here. A parsimonious smooth term structure is the model rationale supported by the canonical references, but adoption and fit suitability remain open.

## Algorithm and domain

Write `g(x)=(1-exp(-x))/x`. NS continuous zero rate is `beta0 + beta1*g(t/tau) + beta2*(g(t/tau)-exp(-t/tau))`. NSS adds `beta3*(g(t/tau2)-exp(-t/tau2))`, using tau1 for the first decay. At `t<=0`, model zero returns `beta0+beta1`. NS instantaneous forward is `beta0+beta1*exp(-x)+beta2*x*exp(-x)`; NSS adds `beta3*x2*exp(-x2)`. Its nonpositive-time model branch returns the short zero rate.

Coefficients must be finite, decay horizons positive, and NSS horizons differ by at least 1e-10 in absolute value. Direct enum construction and plain model deserialization can precede validation; `from_params_vec` and curve building call it. Parameter vectors require exactly four (NS) or six (NSS) values. Negative rates/forwards are permitted; no monotonic-DF or forward-floor constraint is imposed.

The source discount function is one for nonpositive source time, otherwise `exp(-z(t)*t)`. With stored roll offset `o`, the curve returns `D(o+t)/D(o)` for `t>0`, and one for `t<=0`. Curve `zero_rate(t>0)=-ln(Dcurve(t))/t`; at nonpositive time it returns curve `forward_rate(0)`. The curve's analytic forward evaluates the model at `o+t`, but returns zero when that source time is negative. Rolling adds the configured day-count year fraction to offset and checks positive finite normalization. Act/365F is default; the `Discounting` trait has the generic finite-difference instantaneous-forward method described in [discount evaluation](discount-evaluation.md).

## Limitations and alternatives

The `g` calculation uses direct subtraction `1-exp(-x)` with no `expm1` or small-x series. Tiny positive `t/tau` can therefore lose its limiting value even though exactly zero has a separate branch. Existing tests cover zero and long time but do not establish tiny-positive-time accuracy. Queries also have no general finite/domain guard; extreme coefficients/times can overflow or underflow. Negative-source-time truncation is a chosen boundary convention, not a calibrated economic history.

A stable `-expm1(-x)/x`/series branch, analytic trait dispatch and explicit query-domain validation are possible changes. Supported NS versus NSS trades fewer parameters for an additional curvature component; nonparametric curves are separately supported. Context generic bumping rejects parametric curves rather than manufacturing a knot approximation.

## Evidence

Owning implementation and unit tests: [parametric_curve.rs](../../../../finstack-quant/core/src/market_data/term_structures/parametric_curve.rs). Tests located include `ns_zero_rate_at_zero`, `ns_long_term_rate`, `forward_rate_ns`, parameter-domain rejection, and `ns_and_nss_rolls_realize_discount_factors_and_survive_serde`; those are formula/invariant and round-trip evidence, not independent fitted-market benchmarks. Canonical references: [Nelson–Siegel](../../../REFERENCES.md#nelson-siegel-1987), [Svensson](../../../REFERENCES.md#svensson-1994).

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
