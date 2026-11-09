# CORE-NUMERICS-02: Linear, log-linear and monotone Hermite interpolation

- Decision status: **Accepted — complete value-interpolation package, 2026-11-07**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Evidence inspected; independent exact-rational/100-digit references and semantic reviews passed; production qualification pending**. PCHIP formulas/endpoint/C1 identities, sampled shapes, log-linear values and expression-level extreme/spacing counterexamples were checked independently. Linked Rust/host tests were located or inspected, not run; these references do not certify current implementation or a domain-wide bound.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

This record owns common scalar interpolation state, validation, value/derivative
queries, Linear, LogLinear and the specific shape-preserving CubicHermite recipe,
plus shared boundary/helper contracts in math/interp/{generic,types,traits,utils,
strategies}.rs. Forward-specific strategies and their economic restrictions remain
with [record03](forward-interpolation.md); financial curve construction and tails
remain with the core-market owners. The historical minimum-spacing/policy choices
are described separately below, not silently adopted by this scope statement.

## Accepted decision: validated scalar interpolation and boundary semantics

Accepted on **2026-11-07** after whole-record review. Runtime, caller/host
reconciliation and independent numerical qualification remain pending;
source-observed behavior is retained separately below.

1. **Keep numeric interpolation separate from curve economics.** Retain Linear,
   LogLinear and CubicHermite, with CubicHermite precisely the PCHIP recipe below,
   not an arbitrary user-supplied derivative spline. Require explicit style and
   boundary selection; remove shared enum defaults and inherited financial
   constructor/host/state defaults requiring those selections. No automatic style
   fallback, branded curve preset, date clock, probability/rate/DF interpretation
   or market-standard tail follows from a numeric interpolator. Existing
   forward-specific styles remain separately owned by record03.

2. **Expose one validated owned interpolation route.** Reuse the existing
   strategy implementations and enum dispatch behind one public immutable
   Interpolator constructed with explicit style, knots, values and extrapolation.
   Make generic strategy construction/evaluation and the raw public trait
   operations internal; callers cannot pair cached slopes/logs with another
   supplied array. Keep backing enum payloads private so public variants cannot
   bypass construction/restore validation. Remove the parallel unchecked route
   rather than retain compatibility wrappers or create a strategy-plugin registry.
   The implementation may retain internal compile-time/static dispatch.

3. **Admit finite ordered coordinates without an arbitrary spacing floor.**
   Require equal-length finite x/y arrays with at least 2 points and strictly
   increasing distinct x. Negative, shifted and non-time coordinates are valid;
   +/-0 are the same coordinate for ordering. Remove the 1e-10*max(|x_left|,1)
   minimum-gap rule and do not sort, merge, deduplicate, snap or perturb knots.
   Needed interval widths must be finite and positive in the supported checked
   calculation; width overflow is a calculation-range failure, not a financial
   spacing rule. Distinct close knots are not inherently invalid; genuinely
   unrepresentable required slope/cache calculations may still fail explicitly.

4. **Select exact Linear and LogLinear definitions and value domains.** For
   h=x1-x0 and u=(x-x0)/h, Linear is (1-u)*y0+u*y1 with derivative (y1-y0)/h.
   LogLinear is exp((1-u)*ln(y0)+u*ln(y1)), with derivative y(x)*ln(y1/y0)/h.
   Use stable equivalent evaluation rather than requiring these literal products
   or quotient. Linear and CubicHermite accept any finite signed ordinates,
   including zero. LogLinear intrinsically requires strictly positive ordinates.
   Remove ValidationPolicy, its Strict/AllowNegative variants and silently forced
   overrides: financial positivity/monotonicity checks belong to the owning curve,
   while strategy-intrinsic mathematical domains remain enforced.

5. **Adopt the specified PCHIP slopes and Hermite polynomial.** Let
   h_i=x[i+1]-x[i] and d_i=(y[i+1]-y[i])/h_i. With 2 points, both nodal slopes
   equal d0 and the selected method reduces to Linear. At an interior node,
   m_i=0 if either adjacent secant is zero or their signs differ. Otherwise,
   m_i=(w1+w2)/(w1/d_left+w2/d_right), with w1=2*h_right+h_left and
   w2=h_right+2*h_left. The left endpoint candidate is
   ((2*h0+h1)*d0-h0*d1)/(h0+h1); mirror it at the right endpoint. Set it to 0
   if its sign disagrees with the endpoint secant; when the neighboring secants
   disagree in sign, cap its magnitude at 3 times that endpoint secant.
   Evaluate h00*y0+h10*h*m0+h01*y1+h11*h*m1, where
   h00=2u^3-3u^2+1, h10=u^3-2u^2+u, h01=-2u^3+3u^2 and h11=u^3-u^2;
   differentiate analytically. This is the Fritsch-Butland weighted-harmonic
   PCHIP/Moler endpoint recipe, correcting the source's incomplete
   Fritsch-Carlson-only attribution. No epsilon-flat secant or alternate slope
   formula is selected.

6. **Make shape/smoothness guarantees precise and limited.** In real arithmetic,
   these methods stay within each interval's endpoint value range; PCHIP follows
   the local increasing/decreasing/flat data shape, even for globally nonmonotone
   signed data. PCHIP is C1 across interior knots, generally not C2; Linear and
   LogLinear generally have derivative jumps. These properties apply in-range,
   not to arbitrary tails. Positive input values do not establish positive
   forwards, no arbitrage or positive value-space extensions. Do not clip values,
   slopes or negative valid tails to manufacture a financial guarantee. Floating
   point conditioning/rounding and required owner financial checks remain explicit.

7. **Return stored knot values exactly and state derivative sides.** At any
   exact knot query, return its stored ordinate directly, including stored signed
   zero; do not evaluate 0*an overflowing difference or exp(ln(y_i)). Between
   knots use the unique containing interval. At interior knots Linear/LogLinear
   derivatives use the left interval; first/last endpoints use their inward
   interval. These are one-sided conventions, not a claim of a two-sided
   derivative there. PCHIP returns its cached nodal slope. Exact endpoint queries
   use the in-range derivative regardless of the strictly-outside boundary rule.
   Comparisons are exact, with no epsilon knot snapping.

8. **Rename boundary policies by numeric behavior and make them explicit.**
   Replace FlatZero/FlatForward/None with Flat/Extend/Error and corresponding
   exact lowercase wire/text labels; retain no old aliases or generic default.
   Flat returns the original boundary ordinate exactly and derivative0 strictly
   outside either endpoint. Extend continues the endpoint secant for Linear,
   endpoint log secant for LogLinear and endpoint PCHIP derivative for
   CubicHermite; derivatives follow those same continued functions. Error rejects
   outside queries for both value and derivative. Endpoints themselves remain
   in-range. Flat is not a flat zero rate, and Extend is not universally a flat
   financial forward. Wrappers with different financial tail economics must state
   and implement that owner policy explicitly rather than overload these meanings.
   Record03 owns the method-specific extension of its forward strategies.

9. **Make value/derivative queries fallible and distinguish failures.**
   interp/interp_prime and dispatch return Result. Reject nonfinite query x before
   exact-knot/boundary identities, forbidden extrapolation and invalid state using
   the existing input/domain category; needed arithmetic/range failures use the
   computation category. Results must be finite; LogLinear values must remain
   strictly positive, so zero-underflow or overflow errors rather than returning
   a factor0/infinity. A nonzero required slope/derivative lost to zero is not an
   intentional flat segment; use stable computation or report range failure.
   Exact constant segments and mathematically zero derivatives remain valid.
   Value and derivative requests may have different representability: do not
   require an unused unrepresentable derivative merely to return an exact stored
   value or otherwise computable Linear value. PCHIP slopes are required state,
   so failure to construct them is a construction calculation error.

10. **Use stable bounded work without an unsupported global accuracy promise.**
    Preserve finite convex blends without forming avoidably overflowing y1-y0;
    compute log differences stably for nearby positives rather than declaring
    equal cached logs to be an exact-flat segment. Normalize/rescale PCHIP width
    weights and harmonic operations to avoid obvious sums/reciprocals overflowing
    or underflowing into a false zero slope. Detect sign differences directly,
    not through an overflowing/underflowing slope product. Evaluate analytic
    derivatives and required caches consistently, failing explicitly when the
    supported range is insufficient. No arbitrary-precision layer, automatic
    style replacement, output clamp or tolerance-driven shape repair is added.
    Mathematical shape preservation and stable formulas do not establish exact
    off-knot values, a portable ULP bound or domain-wide relative-error accuracy.

11. **Keep immutable canonical state and atomic strict restoration.** Publish
    only fully validated owned style/knots/values/extrapolation. Serialize that
    complete model state; reject missing/unknown/retired policy fields, malformed
    enum labels and invalid arrays rather than defaulting, sorting or repairing.
    Restore through the same constructor and rebuild private method caches;
    never trust supplied slope/log/cache data. Invalid proposed replacement leaves
    the previous object unchanged. Preserve method identity and exact admitted
    source values; no lossy table/state may masquerade as a complete policy.
    Native generic implementation details are not extra public wire variants.

12. **Remove the unchecked flat-knot helper and reconcile actual profiles.**
    Retire interp_knots_flat and alternate private copies. Existing2+ point
    consumers should construct/reuse the canonical Linear+Flat interpolator with
    full validation, rather than rebuild/validate it for every query. Empty or
    malformed required data must not turn into0, an endpoint or false no-error
    output. Preserve a legitimate single-point constant only where the owner
    explicitly supports it and validates its state/query; it is not a two-point
    core interpolator or permission to synthesize a second knot. Missing EAD,
    spread, PD/hazard or fixing data keeps its owner-specific meaning, without
    a new generic zero/missing/imputation convention.

13. **Keep monotonicity diagnostics private and economically scoped.**
    find_monotone_violation remains an internal check over validated finite
    series with explicit finite nonnegative base_tol: find the first adjacent
    increase exceeding base_tol*max(|previous|,1). It reports that limited
    tolerance-relative check, without clipping, repairing or certifying no
    arbitrage. Evaluate its comparison without invalid arithmetic silently
    hiding a violation. Actual tolerance/units, permission for negative forwards
    and forward-specific shape projection remain record03/curve-owner decisions;
    no numeric monotonicity requirement is added to generic signed interpolation.

14. **Propagate one working public/state/consumer slice without adopting others.**
    Update shared style/boundary constructors, private strategies, Result
    propagation, snapshots/schema and existing curve/host/calibration consumers
    together, including forward03 mechanical integration and MVA/ECL helper
    replacement. Keep existing host enum/product/curve surfaces rather than add
    a standalone interpolator wrapper solely for parity. Map input/domain errors
    to Python ValueError and computation errors to RuntimeError using the shared
    mapping, with matching WASM failures. Method-dependent nonsmooth calibration
    sensitivities, source curve value domains, financial extrapolation and
    empirical/currency/date/clock choices remain with their owners. Acceptance
    does not implement or independently qualify those pricing/calibration models.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Empty/singleton generic interpolator; unequal arrays; NaN/infinite state | Validation error, no empty0, invented point or partial publication. |
| x=[0,5e-11],y=[0,1] | Valid ordered grid if the requested checked calculation fits; no global 1e-10 floor. |
| Finite negative or translated coordinates | Admitted; coordinate origin is not a financial time rule. |
| Duplicate x including -0/+0, reversed x or overflowing required width | Validation or calculation-range error as appropriate; no sort/merge/repair. |
| Linear/PCHIP y=[-1,0,2] / LogLinear with those values | Signed data admitted / positive-domain error. |
| Linear x=[0,1],y=[-MAX,+MAX],query0.5 | Finite value0 via stable blend; do not manufacture NaN from an overflowing difference. |
| The same Linear grid, derivative query | Range error if the true derivative is unrepresentable; do not invalidate its exact knot values. |
| Exact knot y=0.1 or signed zero, including LogLinear's valid positive knot | Return the original admitted ordinate, not exp(log(y)) reconstruction. |
| Linear x=[0,1,2],y=[0,1,3],derivative at1 | Left derivative1; immediately right derivative2. |
| First/last exact endpoint with Flat policy | Stored endpoint value and inward derivative; strictly outside has derivative0. |
| Flat beyond either end / Extend beyond either end / Error beyond either end | Exact endpoint value / stated working-space extension / explicit query error. |
| NaN or infinite x even under Flat | Error, not first endpoint. |
| LogLinear x=[0,1],y=[1,4],query0.5 | Value2 and derivative2*ln4; extension works in log-value space. |
| Increasing positive discount-like ordinates | No generic positive-forward/no-arbitrage guarantee. |
| PCHIP x=[0,1,2],y=[0,1,1] | Slopes[1.5,0,0]; constant second segment, no overshoot; first-segment midpoint 0.6875. |
| Two-point PCHIP | Its defined Linear reduction, not a silent replacement style. |
| PCHIP x=[-1e308,0,1e308],y=[0,1,2] | Stable slopes/value or explicit range failure; no overflowed weight sum interpreted as a flat curve. |
| PCHIP x=[0,1,2],y=[0,eta,2eta],eta=minimum subnormal | Preserve representable secant eta or error; reciprocal overflow must not create a false flat. |
| LogLinear extension overflows or underflows to0 | Computation error, no clipping or endpoint fallback. |
| Positive Linear/PCHIP data with negative Extend tail | Generic finite tail can be negative; financial owner decides eligibility rather than a generic floor. |
| Invalid restore or supplied stale slope/cache data | Atomic rejection; rebuild required caches only from complete canonical input. |
| Existing MVA spread input | Retain its existing validation; helper replacement is not evidence it currently accepts empty grids. |
| Owner-supported one-point ECL EAD profile | Keep its explicit constant-profile behavior; do not force 2 points or strict-positive DF rules. |
| Missing style/boundary or old flat_zero/flat_forward/none/validation fields | Error under the new canonical contract, no compatibility aliases/defaults. |

## Observed algorithm

1. `Interpolator::new` checks lengths, finite strictly increasing knots and finite values; `Strict` additionally requires values>0. Strategy construction applies the relative knot-spacing floor. Deserialization reconstructs through this same constructor.
2. Locate the segment with `partition_point(k<x)`: an exact interior knot uses its left segment. Let `h=x1-x0`, `u=(x-x0)/h`. Linear gives `y0+u(y1-y0)` and derivative `(y1-y0)/h`.
3. Log-linear requires positive values even under `AllowNegative`; cache `l_i=ln y_i`, return `exp(l0+u(l1-l0))`, and derivative `y(x)(l1-l0)/h`.
4. Cubic Hermite computes secants `d_i=(y[i+1]-y[i])/h_i`. Interior slope is zero if either adjacent secant is zero or their signs differ. Otherwise use `(w1+w2)/(w1/d_left+w2/d_right)`, with `w1=2h_right+h_left`, `w2=h_right+2h_left`. One-sided endpoint estimates are zeroed on sign mismatch and capped at `3d_endpoint` on adjacent sign change. Two points use the common secant.
5. Evaluate Hermite basis `h00=2u³-3u²+1`, `h10=u³-2u²+u`, `h01=-2u³+3u²`, `h11=u³-u²`: `y=h00 y0+h10 h m0+h01 y1+h11 h m1`. The derivative is the analytic basis derivative divided by `h`.
6. Nonfinite query returns NaN; out-of-range `None` returns NaN. `FlatZero` returns the boundary **value** and derivative zero. `FlatForward` extends the secant for linear, log secant for log-linear, and endpoint derivative for Hermite.
7. Separate `interp_knots_flat` performs linear interpolation with flat endpoint values, returns 0 for an empty array, and returns the right value for a locally nonincreasing segment. It does not perform the generic constructor's validation. `find_monotone_violation` finds the first increase exceeding `base_tol max(|previous|,1)`.

## Observed selection and controls

`Interpolator<S>` is compile-time strategy selection; `InterpStyle::build_enum` dispatches runtime style. `LogLinear`, `MonotoneConvex`, and `PiecewiseQuadraticForward` force strict positive input regardless of requested policy. Linear and Hermite permit signed values under `AllowNegative`; no automatic style fallback occurs. `InterpolationStrategy` exposes raw construction/evaluation methods whose supplied arrays must satisfy the documented contract.

## Observed conventions

The core interpolator acts on supplied ordinates. `FlatZero` means a flat ordinate here, not necessarily a flat financial zero rate; a curve wrapper can interpret or implement a financial boundary policy separately. Derivative units are ordinate units per knot unit.

## Observed limitations, failure and fallback

Strict positive input does not guarantee positive linear/Hermite extrapolation. Helpers and public raw strategy methods rely on caller shape/precondition contracts; they do not all enforce the full `Interpolator` checks. The simple flat helper can return an endpoint for NaN query through partition behavior; it is not equivalent to the validated interpolator.

## Observed alternatives and tradeoffs

Linear and log-linear trade smoothness for simplicity; Hermite offers C1 shape preservation without natural-spline global coupling. [Forward-specific strategies](forward-interpolation.md) are implemented alternatives for discount factors. Natural cubic interpolation of generic signed ordinates is not implemented here.

## Evidence

- [Generic state/construction/restoration](../../../../finstack-quant/core/src/math/interp/generic.rs),
  [dispatch/policy](../../../../finstack-quant/core/src/math/interp/types.rs),
  [strategies](../../../../finstack-quant/core/src/math/interp/strategies.rs),
  [utilities](../../../../finstack-quant/core/src/math/interp/utils.rs) and
  [public raw trait](../../../../finstack-quant/core/src/math/interp/traits.rs)
  locate defaults, coordinate-dependent spacing, raw-array/cache pairing and
  NaN/helper fallbacks. Current Linear exact-knot arithmetic still forms the
  ordinate difference; LogLinear reconstructs values from cached logs.
- [Interpolation tests](../../../../finstack-quant/core/tests/math/interp.rs)
  cover selected shapes, boundaries, derivatives and serde. The test named
  exact_knot_values_preserved uses a 1e-12 approximation check; it does not prove
  the accepted bit-preserving stored-knot contract or all numerical extremes.
- [MVA spread integration/validation](../../../../finstack-quant/margin/src/xva/mva.rs)
  uses the helper but already rejects empty/nonfinite/duplicate/decreasing spread
  grids. [ECL data/query paths](../../../../finstack-quant/statements-analytics/src/analysis/ecl/types.rs)
  and [ECL engine](../../../../finstack-quant/statements-analytics/src/analysis/ecl/engine.rs)
  have owner-specific domains; a one-point EAD schedule is explicitly valid.
  Helper risks are not evidence that all validated consumers accept malformed
  data, nor a reason to impose a DF domain on their profiles.
- [Centralized Python error mapping](../../../../finstack-quant-py/src/errors.rs)
  supplies the existing ValueError/RuntimeError split; host curve/state surfaces
  must propagate the common API changes without duplicate math implementations.

[Official SciPy PCHIP documentation](https://docs.scipy.org/doc/scipy/reference/generated/scipy.interpolate.PchipInterpolator.html)
identifies the weighted harmonic interior rule and its Fritsch-Butland 1984
reference; its [primary implementation](https://github.com/scipy/scipy/blob/main/scipy/interpolate/_cubic.py)
provides the one-sided Moler endpoint rule and two-point reduction. These support
the selected mathematical recipe and limited shape/smoothness description,
not bitwise parity, every float boundary or a new Python runtime dependency.
Sources were consulted on 2026-11-07. [Fritsch-Carlson](../../../REFERENCES.md#fritsch-carlson-1980)
and [de Boor](../../../REFERENCES.md#de-boor-splines) remain background; they are
not by themselves provenance for this exact implemented slope recipe.

Independent exact-rational checks passed for the PCHIP plateau/nonuniform-grid
examples, polynomial endpoint and derivative identities and five sampled shapes
at 101 points per interval. Exact rational arithmetic also preserved the specified
subnormal secants and large-width collinear example; 100-digit log references
checked the positive geometric midpoint and distinct-nearby-value log difference.
Expression-level checks confirmed avoidable Linear difference overflow, log/exp
knot reconstruction changes and the coordinate-origin spacing counterexample.
These are independent formula checks and counterexamples, not production Rust
execution, proposed serialization tests or a proof from sampling of a global
floating-point shape/error bound. Independent math and consumer/host semantic
reviews found no material inconsistency.

Production Rust/host/model tests were inspected or located, not executed. The
accepted package selects formulas, identities, error/state contracts and ownership only;
production numerical and consumer qualification remain pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
