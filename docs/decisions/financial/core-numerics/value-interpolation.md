# CORE-NUMERICS-02: Linear, log-linear and monotone Hermite interpolation

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/interp/{generic,types,traits,utils,strategies}.rs`, limited here to common validation, linear, log-linear, cubic Hermite, helper interpolation, and boundary semantics. At least two equally sized finite knot/value arrays; knots strictly increase with gap at least `1e-10 max(|x_left|,1)`.

## Choice and rationale

Generic strategy selection separates interpolation from curve economics. The enum default is linear, the validation default strictly positive values, and the extrapolation default `FlatZero`. PCHIP slopes suppress overshoot where adjacent secants change sign. Curve-specific selection is documented in the core-market slice.

## Algorithm

1. `Interpolator::new` checks lengths, finite strictly increasing knots and finite values; `Strict` additionally requires values>0. Strategy construction applies the relative knot-spacing floor. Deserialization reconstructs through this same constructor.
2. Locate the segment with `partition_point(k<x)`: an exact interior knot uses its left segment. Let `h=x1-x0`, `u=(x-x0)/h`. Linear gives `y0+u(y1-y0)` and derivative `(y1-y0)/h`.
3. Log-linear requires positive values even under `AllowNegative`; cache `l_i=ln y_i`, return `exp(l0+u(l1-l0))`, and derivative `y(x)(l1-l0)/h`.
4. Cubic Hermite computes secants `d_i=(y[i+1]-y[i])/h_i`. Interior slope is zero if either adjacent secant is zero or their signs differ. Otherwise use `(w1+w2)/(w1/d_left+w2/d_right)`, with `w1=2h_right+h_left`, `w2=h_right+2h_left`. One-sided endpoint estimates are zeroed on sign mismatch and capped at `3d_endpoint` on adjacent sign change. Two points use the common secant.
5. Evaluate Hermite basis `h00=2u³-3u²+1`, `h10=u³-2u²+u`, `h01=-2u³+3u²`, `h11=u³-u²`: `y=h00 y0+h10 h m0+h01 y1+h11 h m1`. The derivative is the analytic basis derivative divided by `h`.
6. Nonfinite query returns NaN; out-of-range `None` returns NaN. `FlatZero` returns the boundary **value** and derivative zero. `FlatForward` extends the secant for linear, log secant for log-linear, and endpoint derivative for Hermite.
7. Separate `interp_knots_flat` performs linear interpolation with flat endpoint values, returns 0 for an empty array, and returns the right value for a locally nonincreasing segment. It does not perform the generic constructor's validation. `find_monotone_violation` finds the first increase exceeding `base_tol max(|previous|,1)`.

## Selection and controls

`Interpolator<S>` is compile-time strategy selection; `InterpStyle::build_enum` dispatches runtime style. `LogLinear`, `MonotoneConvex`, and `PiecewiseQuadraticForward` force strict positive input regardless of requested policy. Linear and Hermite permit signed values under `AllowNegative`; no automatic style fallback occurs. `InterpolationStrategy` exposes raw construction/evaluation methods whose supplied arrays must satisfy the documented contract.

## Conventions

The core interpolator acts on supplied ordinates. `FlatZero` means a flat ordinate here, not necessarily a flat financial zero rate; a curve wrapper can interpret or implement a financial boundary policy separately. Derivative units are ordinate units per knot unit.

## Limitations, failure and fallback

Strict positive input does not guarantee positive linear/Hermite extrapolation. Helpers and public raw strategy methods rely on caller shape/precondition contracts; they do not all enforce the full `Interpolator` checks. The simple flat helper can return an endpoint for NaN query through partition behavior; it is not equivalent to the validated interpolator.

## Alternatives and tradeoffs

Linear and log-linear trade smoothness for simplicity; Hermite offers C1 shape preservation without natural-spline global coupling. [Forward-specific strategies](forward-interpolation.md) are implemented alternatives for discount factors. Natural cubic interpolation of generic signed ordinates is not implemented here.

## Evidence

- [Generic construction](../../../../finstack-quant/core/src/math/interp/generic.rs), [dispatch/policy](../../../../finstack-quant/core/src/math/interp/types.rs), [strategies](../../../../finstack-quant/core/src/math/interp/strategies.rs), [utilities](../../../../finstack-quant/core/src/math/interp/utils.rs), [trait contract](../../../../finstack-quant/core/src/math/interp/traits.rs).
- [Interpolation tests](../../../../finstack-quant/core/tests/math/interp.rs): macro-expanded exact knots, shape rejection, endpoints, derivatives and serialization. These are formula/invariant and roundtrip evidence, not vendor benchmarks.
- [Fritsch–Carlson](../../../REFERENCES.md#fritsch-carlson-1980) and [de Boor](../../../REFERENCES.md#de-boor-splines). These references do not establish the provenance of the exact weighted harmonic slope recipe in the implemented PCHIP variant.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
