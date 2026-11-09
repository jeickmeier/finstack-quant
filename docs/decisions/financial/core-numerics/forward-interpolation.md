# CORE-NUMERICS-03: Natural log-discount spline and Hagan–West forward interpolation

- Decision status: **Accepted — complete forward-interpolation package, 2026-11-07**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Evidence inspected; independent exact-rational/80-digit references and semantic reviews passed; production qualification pending**. Natural-system/polynomial identities and 426 Hagan-West regional value/primitive checks passed, including sign/scale mirrors, joins, zero area and exact-axis examples. Linked Rust/host tests were located or inspected, not run; these references do not certify current implementation, market fitness or domain-wide accuracy.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

This record owns the natural log-discount spline and Hagan-West regions in
math/interp/strategies.rs. It inherits the [accepted record 02 common contract](value-interpolation.md#accepted-decision-validated-scalar-interpolation-and-boundary-semantics):
one validated immutable interpolation route, explicit style/Flat/Extend/Error,
finite ordered distinct axes without a relative gap floor, exact stored knots,
fallible evaluation and strict rebuilt state. It owns the additional positive
ordinate domain, method construction, forward shape and one-sided derivatives;
financial curve normalization/clock/calibration remain with their actual owners.

## Accepted decision: natural log-discount and Hagan-West interpolation

Accepted on **2026-11-07** after whole-record review. Runtime, caller/host
reconciliation and independent numerical qualification remain pending;
source-observed behavior is retained separately below.

1. **Keep two explicit fixed methods, without financial presets.** Retain
   PiecewiseQuadraticForward as the natural cubic spline in Y=-ln(D), and
   MonotoneConvex as the unsmoothed Hagan-West regional method with its optional
   positivity collar omitted. No smoothing/tension, alternative spline boundary,
   automatic positive/negative-rate regime or new positivity flag is selected.
   These names identify numerical constructions, not vendor equivalence, a
   universal market default or fitness for every curve/calibration product.

2. **Require positive finite data on the supplied coordinate system.** Both
   methods require at least 2 finite strictly ordered distinct coordinates and
   matching strictly positive finite DFs; D>1, positive subnormals, increasing
   D and signed/mixed interval forwards are allowed. Coordinates may be shifted
   or negative. Do not insert (0,1), normalize D, infer a base date/day count or
   require a decreasing curve here. Y is dimensionless; f=dY/dx has inverse
   coordinate units, and is an annual decimal continuous forward only when the
   owner explicitly supplies annual time coordinates. Datum/clock, source dated
   factor and financial admissibility remain owner contracts.

3. **Adopt the natural log-discount system exactly.** Write h_i=x[i+1]-x[i],
   Y_i=-ln(D_i), q_i=(Y[i+1]-Y_i)/h_i and M_i=Y''(x_i). Require M0=M_last=0
   and solve h_left*M[i-1]+2*(h_left+h_right)*M_i+h_right*M[i+1]
   =6*(q_right-q_left). For s=x-x_i, define Y=a+b*s+c*s^2+d*s^3 with
   a=Y_i, b=q_i-h_i*(2*M_i+M[i+1])/6, c=M_i/2 and
   d=(M[i+1]-M_i)/(6*h_i). Return D=exp(-Y), and analytic
   D'=-(b+2*c*s+3*d*s^2)*D. Two points give M=0 and exactly the mathematical
   LogLinear method. No not-a-knot, clamped, smoothing or other implicit condition.

4. **State the natural spline's smoothness and global limits.** In real
   arithmetic Y and D are C2 in-range; instantaneous f=Y' is piecewise quadratic
   and C1, generally not C2. Natural endpoints mean Y''=f'=0, not f=0 or D''=0.
   The spline is a global fit and can change distant segments when a knot changes.
   It can overshoot forward levels and generate negative f even when all knot
   DFs decrease. Keep that unfiltered method behavior; no positivity clamp,
   monotone repair or automatic method substitution. Its endpoint construction
   does not dictate extrapolation: apply the selected common policy separately.

5. **Solve the natural system in stable scaled arithmetic.** Reuse a checked
   linear-time tridiagonal solve, with normalized widths/rows and stable adjacent
   log differences where needed. Positive finite widths define a nonsingular
   natural system in exact arithmetic. Remove the absolute |pivot|<EPS rejection;
   a valid small-unit grid is not singular solely because a pivot is small.
   Needed pivots/caches/results must be finite and usable after scaling, otherwise
   return a computation/range failure. Do not silently set coefficients to 0,
   perturb the grid, change boundary conditions or add a new spline framework.

6. **Adopt Hagan-West's provisional node estimates and interval conservation.**
   Set interval averages q_i=(Y[i+1]-Y_i)/h_i. Interior provisional forwards are
   F_i=(h_left*q_right+h_right*q_left)/(h_left+h_right); endpoints are
   F0=1.5*q0-0.5*F1 and F_last=1.5*q_last-0.5*F_previous. Two points use
   F0=F1=q0. On interval i let u=(x-x_i)/h_i, g0=F_i-q_i and
   g1=F[i+1]-q_i. Use the regional g and its matching primitive G below:
   Y(x)=Y_i+q_i*(x-x_i)+h_i*G(u), D=exp(-Y), and within the open interval
   f=q_i+g(u), D'=-f*D. G(0)=G(1)=0 preserves the average q_i and original
   discount ratio in real arithmetic. A provisional F_i is a construction target,
   not a guaranteed actual derivative at a degenerate interval endpoint.

7. **Use exact regional classification and remove the absolute epsilon override.**
   For nonzero opposite-signed g0/g1, choose Region I when
   0.5*abs(g0)<=abs(g1)<=2*abs(g0), Region II when abs(g1)>2*abs(g0), and
   Region III otherwise. Same nonzero signs select Region IV. Equality at the
   nondegenerate boundaries selects Region I; formulas agree there. Handle exact
   zero axes separately under point 9. Remove the fixed 1e-14 near-zero Region I
   override; small nonzero deviations keep their prescribed region. Use exact
   sign tests and stable normalized comparisons, not products or overflowing
   doubled magnitudes. No tolerance-driven flatness, smoothing or alternate
   polynomial repair is introduced.

8. **Remove automatic positivity projection rather than infer a rate regime.**
   Delete apply_monotonicity_constraints and the current local clipping into
   [0,2*q] bounds. Keep signed provisional and interpolated forwards, without
   clipping or a hidden whole-curve/local positive-rate switch. Even q_i>=0 on
   every interval no longer implies everywhere nonnegative f; that guarantee is
   not part of this selected uncollared method. A caller requiring a nonnegative
   forward regime must explicitly select/validate an eligible method and its
   actual interpolated domain, not treat positive D or positive average forwards
   as sufficient. Do not add a positivity-mode wrapper or automatically fall back
   to LogLinear. Positive DFs and monetary/financial eligibility remain distinct.

9. **Retain the exact-axis limiting shape and truthful one-sided derivatives.**
   If g0=g1=0, g=G=0. If exactly one is zero, retain the degenerate limit
   g=G=0 throughout 0<u<1, with G(0)=G(1)=0. There can be an endpoint forward
   jump; do not fill the axis with Region I, snap small values onto it or claim
   universal continuous forwards/C1 DFs. Exact stored DFs remain continuous in
   the real method. At an interior knot, interp_prime uses the actual left
   interval derivative limit; first/last endpoints use their actual inward limit.
   On a degenerate interval that limit is -q_i*D_i at the left and
   -q_i*D[i+1] at the right, not -F_endpoint*D. Provisional endpoint deviations
   can still describe regional construction limits, but must not override the
   derivative of the chosen DF function. A caller requiring universal smoothness
   must choose an explicitly eligible method or reject the curve, not silently
   change this method.

10. **Apply literal Flat/Extend/Error tails anchored to source DFs.** Exact
    knots and Flat tails return the stored original DF directly. Flat has
    derivative 0 strictly outside. Extend fixes the actual inward endpoint forward
    f_b and uses log(D(x))=log(D_b)-f_b*(x-x_b), D'=-f_b*D(x), on both sides;
    for Hagan-West use the actual limit from point 9 rather than a conflicting
    cached target. It extends log discount linearly, not the natural cubic
    polynomial or a flat zero rate. Error rejects outside queries. Endpoints
    themselves use in-range derivative conventions; a deliberate Flat boundary
    may have a derivative jump. No tail clipping or implicit policy choice.

11. **Check the requested quantity and all genuinely required numerical state.**
    Use the same g/G and spline construction for values and derivatives; do not
    maintain inconsistent independent formulas or stale cache/grid pairs.
    Preserve small adjacent log-ratio information and use normalized sums/branch
    widths to avoid obvious weighted-sum overflow, cancellation and false axes.
    A nonzero required width/slope/deviation lost to zero must be reformulated or
    error, not interpreted as a flat/zero-deviation case. Needed coefficients,
    interval forwards and evaluations must be finite. DF results must be strictly
    positive and finite, admitting positive subnormals and rejecting zero-underflow.
    Derivative requests are separate quantities; avoid requiring an unrequested
    DF or derivative representation when stable direct evaluation is available.
    Unusable checked intermediate/output range remains an explicit numerical
    failure, not a promise that every finite mathematical input succeeds.

12. **Keep precision and financial claims bounded.** Exact formulas preserve
    knot data and interval areas mathematically; off-knot and derivative results
    remain rounded binary64 computations. Scaling coordinates, discount data or
    tiny deviations can change floating evaluation even when real formulas have
    corresponding invariances. Do not publish bitwise/reference equivalence,
    domain-wide ULP/relative error or zero/forward-positivity/no-arbitrage claims
    from formula/round-trip/property tests. Hagan-West branch changes, exact axes
    and optional collar removal affect calibration/risk shape; natural cubic
    global coupling affects sensitivities. Their acceptance, conditioning and
    residual/bump budgets stay with financial solver/model owners.

13. **Retain the accepted common state/API and strict method identity.**
    Construction/restoration must validate full owned style/knots/DFs/explicit
    boundary and rebuild private spline/Hagan-West state. No supplied coefficient,
    provisional-forward or region cache is trusted; no public raw strategy bypass,
    generic default, ValidationPolicy, hidden positivity flag or legacy boundary
    alias remains. Wire/style names stay piecewise_quadratic_forward and
    monotone_convex, with the fixed method definitions selected here. Unknown or
    retired state errors rather than migrating/repairing. Publish atomically.

14. **Reconcile the real curve/host/calibration owners without adopting them.**
    Propagate shared Result/constructor/wire changes, exact-knot identities,
    actual one-sided derivative/tail forwards, omitted collar/epsilon and stable
    construction through existing callers and hosts in one working slice.
    DiscountCurve's current sorting, synthetic origin insertion, monotonic/forward
    filters and clock/base-date handling remain separately reviewed owner policy;
    they are not generic method admission. Update source docs claiming universal
    continuous forwards, global negative-rate regime detection or Bloomberg/vendor
    parity. No new host interpolator class, calibration model, rate-regime engine,
    automatic repair or backward-compatibility layer is selected. Acceptance
    does not execute or independently qualify downstream runtime/market behavior.

### Accepted Hagan-West regional definitions

For 0<u<1, h is the interval width and g0/g1 are its deviations. Define
f=q+g and G(u)=integral from 0 to u of g. The expressions are mathematical targets;
stable equivalent algebra must respect the same branches and primitive.

| Region | Forward adjustment g(u) and primitive G(u) |
| --- | --- |
| I | g=g0*(1-4u+3u^2)+g1*(-2u+3u^2); G=u*(1-u)*(g0*(1-u)-g1*u). |
| II | w=-3*g0/(g1-g0), eta=1-w, d=max(u-eta,0), v=d/w; g=g0+(g1-g0)*v^2; G=g0*u+(g1-g0)*d*v^2/3. |
| III | eta=3*g1/(g1-g0), d=max(eta-u,0), v=d/eta; g=g1+(g0-g1)*v^2; G=-g1*(1-u)-(g0-g1)*d*v^2/3. |
| IV | eta=g1/(g0+g1), a=-g0*eta. Left u<=eta: v=u/eta, g=a+(g0-a)*(1-v)^2, G=a*u+(g0-a)*u*(1-v+v^2/3). Right: d=1-u,v=d/(1-eta),g=a+(g1-a)*(1-v)^2,G=-a*d-(g1-a)*d*(1-v+v^2/3). |
| Exact zero axis | Open-interval g=G=0; both endpoint integrals are 0. Derivative limits use this actual interval function, not an incompatible provisional F. |

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Positive D>1, increasing D or negative/shifted finite coordinates | Admitted if common ordered/checked state requirements hold; no synthetic anchor or forward floor. |
| Zero/negative/NaN/infinite DF, duplicate/nonfinite axes or missing method/tail | Validation error, not repair or default. |
| Two-point natural spline / Hagan-West | LogLinear / constant interval forward q; exact original knot DFs. |
| Natural endpoint condition | Y''=f'=0; it does not mean f=0 or D''=0. |
| Natural t=[0,1,2],Y=[0,.01,.10] | M1=.12; first forward=-.01+.06*t^2, despite decreasing knot DFs. No positivity clamp. |
| Natural collinear log data on widths1e-18 with tiny raw pivots | Stable finite solution or explicit genuine range error; no \|pivot\|<EPS rejection merely due to coordinate units. |
| Hagan-West q=[.01,.10,.01] on equal widths | Uncollared endpoint F=-.0125, interiors .055; positive interval averages do not guarantee nonnegative forwards. |
| Any negative interval average among positives | No local/global regime switch; retain the same fixed uncollared regional method. |
| Nonzero deviations both below 1e-14 | Select the exact sign/ratio region; no absolute-threshold Region I override. |
| Region I/II or I/III nondegenerate equality boundary | Region I by convention; regional formulas/areas agree. |
| Exact one-zero deviation axis | Interior correction 0 and possible forward jumps; no new continuous axis polynomial. |
| Unit knots with Y=[0,1,2,4] | q=[1,1,2], provisional F=[1,1,1.5,2.25]; interval[1,2] has actual interior forward 1. |
| Derivative at t=2 on that example | Left derivative=-exp(-2), not cached-target -1.5*exp(-2); a different right limit is possible. |
| Exact knot/Flat tail with stored D=.1 | Return original D exactly, not exp(-reconstructed log polynomial). |
| Extend at boundary b | D=D_b*exp(-f_b*(x-x_b)) mathematically; use actual inward f_b and stable range checks. |
| Flat outside / Error outside / nonfinite x under any policy | Original endpoint DF and derivative 0 / query error / validation error. |
| Natural-spline comparison with SciPy defaults | Not the same boundary method; explicitly use natural on -lnD and compare tails separately. |
| Positive finite DF underflow result0 / positive subnormal result | Computation error / admitted if checks pass. |
| Close positive DFs with equal rounded standalone logs | Preserve local log-ratio information or error, not a false flat interval. |
| Invalid or supplied stale coefficient/forward/region cache | Atomic restore rejection; rebuild from canonical inputs only. |
| Existing curve origin/monotonic constraints or market/model preference | Separately owned; generic method acceptance does not adopt them. |

## Observed algorithm

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

## Observed selection and controls

Chosen by `InterpStyle::PiecewiseQuadraticForward` or `MonotoneConvex` (or corresponding generic strategies). Both force positive-value validation and share the knot-gap floor and NaN query/boundary policy. Hagan-West evaluates through cached cumulative logs at knots; the natural spline reconstructs DFs from its polynomial, including its boundary values. Exact stored-knot preservation is the accepted02 target, not verified current bitwise behavior. No user-selectable Hagan–West positivity flag or spline boundary condition is exposed.

## Observed conventions

For annual time knots, forwards are continuously compounded decimal annual rates; `-ln D` is cumulative dimensionless discount. Positivity of DF is distinct from positivity of forwards. `FlatZero` at this kernel is flat DF.

## Observed limitations, failure and fallback

The natural spline may overshoot forward rates; it has no positivity/monotonicity limiter. Hagan–West uses fixed `1e-14` deviation detection and local clipping, which changes interpolated forward shape. Rustdoc claims that any negative discrete forward disables all positivity projection; the inspected code instead uses local tests. No vendor equivalence is established by the code's descriptive claims.

## Observed alternatives and tradeoffs

[Log-linear and Hermite](value-interpolation.md) are implemented alternatives. A globally disabled positivity projection, explicit negative-rate policy flag, or different spline endpoint conditions would be unimplemented adoption choices. Natural spline gives smooth global fit; Hagan–West preserves interval averages while controlling local forward shape.

## Evidence

- [Owning strategy code](../../../../finstack-quant/core/src/math/interp/strategies.rs)
  contains the natural tridiagonal system, coefficient evaluation, Hagan-West
  node estimates, local positivity collar, fixed 1e-14 region override and exact-axis
  special case. Natural |pivot|<EPS rejection is coordinate-scale dependent.
  The Hagan-West endpoint branch returns a provisional nonzero deviation even
  where the open-axis interval function is zero; blindly using that value in
  -fD is not the actual one-sided derivative of the chosen DF function.
- [Tests](../../../../finstack-quant/core/tests/math/interp.rs)
  include a hand-derived natural three-knot polynomial, rational fixtures for
  all four nondegenerate regions/sign mirrors, interval-area/property checks
  and finite-difference comparisons. These identify evidence, not a passing
  production result or qualification of the exact-axis knot derivative.
- [DiscountCurve construction](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/builder.rs)
  sorts/prepends an origin and applies owner monotonic/forward checks;
  [curve evaluation](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/curve.rs)
  and existing hosts/calibration consume the interpolation. Those owner choices
  are distinct from admitting an already supplied generic coordinate/DF grid.

[Hagan-West 2008, Methods for Constructing a Yield Curve](https://dlu-umich.github.io/docs/HaganWest.pdf),
sections 6/6.1 and Figure 5, describes optional positivity collaring and exceptional
axis discontinuities. This accepted package selects the unsmoothed regional construction,
omits collaring and preserves the axis limit with corrected derivative semantics;
it does not adopt the paper's historical positive-rate/no-arbitrage assumptions
or claim universal continuity, vendor parity or market suitability.
[Official SciPy CubicSpline documentation](https://docs.scipy.org/doc/scipy/reference/generated/scipy.interpolate.CubicSpline.html)
identifies natural second-derivative conditions, C2 construction and independently
selected extrapolation. It defaults to a different boundary method; comparisons
must explicitly choose natural and must not assume matching tails. Primary
sources were consulted on 2026-11-07, not treated as current vendor qualification.
[Hagan-West background](../../../REFERENCES.md#hagan-west-monotone-convex) and
[de Boor](../../../REFERENCES.md#de-boor-splines) retain their narrower reference roles.

Independent exact-rational calculations passed 426 regional forward/primitive
comparisons against separately integrated piecewise polynomials, including all
four regions, sign/scale mirrors, equality joins, zero interval area and exact
zero axes. Natural tridiagonal/coefficient/endpoint/first-and-second-derivative
identities passed on two/three/multiple-knot examples, including a tiny-width
collinear grid. Reference examples confirmed negative forwards without a collar,
the small-deviation epsilon-override distinction and the actual one-sided axis
forward; an 80-digit DF derivative comparison distinguishes that limit from the
cached nodal target. These are formula/reference calculations, not execution of
production Rust or an implementation of the accepted state/query contract. Independent
natural/common, caller/host and regional-math reviews found no material issue.

Production Rust/host/model tests were located or inspected, not executed.
Numerical extremes and actual consumer behavior require separate qualification;
these references do not establish a global floating-point or market guarantee.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
