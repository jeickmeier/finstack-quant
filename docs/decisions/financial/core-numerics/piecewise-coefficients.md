# CORE-NUMERICS-21: Piecewise nonnegative coefficients and exponential variance integration

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/piecewise.rs::PiecewiseConstantCurve`: nonnegative finite piecewise coefficients, primarily volatility schedules; no curve calibration or discount-factor interpretation is inferred.

## Choice and rationale

Store right-continuous step values and integrate their square against an exponential kernel analytically. An expm1 expression reduces cancellation for small intervals; a fixed near-zero-kappa branch takes the continuous limit.

## Algorithm

1. Require equally sized nonempty times/values, first time exactly positive-zero bits, finite nonnegative knots/values and strictly increasing knots. Deserialization calls the same constructor.
2. At query t, find the partition point for knots≤t and select the preceding value, using index zero if none. This is right-continuous at knots and flat beyond the final knot.
3. `integrate_squared_exp_weight(kappa,A,start,end)` validates finite arguments, start≥0, end≥start and A≥end. If end−start≤1e-10 return 0.
4. Split the requested interval at every coefficient knot. For constant sigma on [l,r], use `sigma²(r−l)` if |kappa|<1e-12; otherwise `sigma² exp[−2 kappa(A−r)] * −expm1[−2 kappa(r−l)]/(2 kappa)`. Sum contributions with ordinary addition.

## Selection, controls and conventions

Caller supplies kappa, anchor and integration bounds. The helper returns the integral of `sigma(t)² exp[−2 kappa(A−t)] dt`; output units depend on the coefficient convention. Negative finite kappa is accepted even though particular mean-reverting models may require nonnegative kappa.

## Limitations, failure and fallback

`value_at` does not validate query finiteness: negative time and NaN select the first value; +infinity selects the last. −0 as the first knot is rejected. Very short intervals are zeroed. Exponential/square overflow can yield Ok(infinity), because final integral finiteness is not checked. No quadrature fallback or signed coefficient support exists.

## Alternatives and tradeoffs

Analytic step integration avoids quadrature error for the chosen step model. Smooth/interpolated volatility models and numerical integration are separate alternatives, not interchangeable without changing variance dynamics.

## Evidence

- [Implementation and inline tests](../../../../finstack-quant/core/src/math/piecewise.rs): `piecewise_constant_integrates_exp_weight_across_knots` reconstructs the two-segment integral (1e-14 absolute); `weighted_variance_retains_accuracy_near_zero_mean_reversion` compares with the zero-kappa limit (4e-15 absolute). These are formula/invariant cases. No market calibration or universal extreme-parameter bound is established.
- [Hull–White context](../../../REFERENCES.md#hull-white-1990-pricing-ird) applies when a model uses this exponential variance kernel; this helper itself does not select that model.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
