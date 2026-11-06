# CORE-NUMERICS-03: Natural log-discount spline and Hagan–West forward interpolation

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/interp/strategies.rs` discount-factor strategies and shared validation in [value interpolation](value-interpolation.md). Inputs are positive finite DFs at finite, strictly increasing knots; DFs may increase for negative forwards.

## Choice and rationale

`PiecewiseQuadraticForward` is a natural cubic spline in cumulative log discount `y=-ln D`; its derivative is quadratic forward rate. `MonotoneConvex` constructs instantaneous forwards from discrete forwards and integrates four Hagan–West shape regions. These are explicit alternatives, not global defaults or evidence of approval for all curves.

## Algorithm

**Natural log-discount spline**

1. Set `h_i=t[i+1]-t[i]`, `y_i=-ln D_i`. Solve the tridiagonal natural-spline system with boundary second derivatives `M_0=M_last=0` and interior right side `6[(y[i+1]-y[i])/h_i-(y[i]-y[i-1])/h[i-1]]`.
2. For `s=t-t_i`, coefficients are `a=y_i`, `b=(y[i+1]-y[i])/h_i-h_i(2M_i+M[i+1])/6`, `c=M_i/2`, `d=(M[i+1]-M_i)/(6h_i)`. Return `D=exp[-(a+bs+cs²+ds³)]`; derivative `D'=-(b+2cs+3ds²)D`.
3. `FlatForward` extends boundary `y` using boundary spline slope; `FlatZero` holds boundary DF fixed. A tridiagonal pivot below machine epsilon errors.

**Hagan–West**

1. Set interval average forwards `fd_i=(y[i+1]-y[i])/h_i`. Interior knot forward is `(h_left fd_right+h_right fd_left)/(h_left+h_right)`; endpoints use `1.5fd_boundary-0.5f_adjacent`. Two knots use the sole average forward at both ends.
2. Locally project endpoints into `[0,2fd_boundary]` only if the adjacent average is nonnegative. Project an interior knot into `[0,2 min(fd_left,fd_right)]` only if both adjacent averages are nonnegative. Negative-rate regions are left unprojected; this is a **local**, not whole-curve, regime rule.
3. Within a segment set `u=(t-t_i)/h`, `g0=f_i-fd`, `g1=f[i+1]-fd`. Compute forward `f(t)=fd+g(u)` and `D(t)=exp[-y_i-fd(t-t_i)-hG(u)]`, where `G` is the integral of `g` from 0 to `u`:
   - Near-zero deviations (both ≤`1e-14`), or opposite signs with `0.5|g0|≤|g1|≤2|g0|`: `g=g0(1-4u+3u²)+g1(-2u+3u²)`; `G=u(1-u)[g0(1-u)-g1u]`.
   - Opposite signs with `|g1|>2|g0|`: width `w=-3g0/(g1-g0)`, join `η=1-w`, `d=max(u-η,0)`, `v=d/w`; `g=g0+(g1-g0)v²`, `G=g0u+(g1-g0)d v²/3`.
   - Remaining opposite signs: `η=3g1/(g1-g0)`, `d=max(η-u,0)`, `v=d/η`; `g=g1+(g0-g1)v²`, `G=-g1(1-u)-(g0-g1)d v²/3`.
   - Same nonzero sign: `η=g1/(g0+g1)`, right width `1-η`, `a=-g0η`. On the left set `v=u/η`: `g=a+(g0-a)(1-v)²`, `G=au+(g0-a)u(1-v+v²/3)`. On the right set `d=1-u`, `v=d/(1-η)`: use `g=a+(g1-a)(1-v)²`, `G=-ad-(g1-a)d(1-v+v²/3)`.
4. Exact segment endpoints return endpoint deviation and integral zero; the remaining zero-deviation boundary case returns `(0,0)`. Return `D'=-fD`. `FlatForward` extends at the endpoint instantaneous forward; `FlatZero` holds endpoint DF.

## Selection and controls

Chosen by `InterpStyle::PiecewiseQuadraticForward` or `MonotoneConvex` (or corresponding generic strategies). Both force positive-value validation, preserve exact input knot DFs, and share the knot-gap floor and NaN query/boundary policy. No user-selectable Hagan–West positivity flag or spline boundary condition is exposed.

## Conventions

For annual time knots, forwards are continuously compounded decimal annual rates; `-ln D` is cumulative dimensionless discount. Positivity of DF is distinct from positivity of forwards. `FlatZero` at this kernel is flat DF.

## Limitations, failure and fallback

The natural spline may overshoot forward rates; it has no positivity/monotonicity limiter. Hagan–West uses fixed `1e-14` deviation detection and local clipping, which changes interpolated forward shape. Rustdoc claims that any negative discrete forward disables all positivity projection; the inspected code instead uses local tests. No vendor equivalence is established by the code's descriptive claims.

## Alternatives and tradeoffs

[Log-linear and Hermite](value-interpolation.md) are implemented alternatives. A globally disabled positivity projection, explicit negative-rate policy flag, or different spline endpoint conditions would be unimplemented adoption choices. Natural spline gives smooth global fit; Hagan–West preserves interval averages while controlling local forward shape.

## Evidence

- [Owning strategy code](../../../../finstack-quant/core/src/math/interp/strategies.rs), `build_hagan_west`, `apply_monotonicity_constraints`, `forward_adjustment`, and `PiecewiseQuadraticForwardStrategy::from_raw`.
- [Tests](../../../../finstack-quant/core/tests/math/interp.rs): `piecewise_quadratic_forward_matches_natural_cubic_and_is_continuous` uses an independently hand-derived three-knot polynomial; `hagan_west_reference::all_four_regions_match_reference_forwards_and_integrals` uses rational region fixtures and sign mirrors. Its finite-difference derivative checks are internal consistency evidence. Negative-rate and stressed-forward cases are property evidence.
- [Hagan–West](../../../REFERENCES.md#hagan-west-monotone-convex), [de Boor](../../../REFERENCES.md#de-boor-splines).

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
