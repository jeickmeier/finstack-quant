# CORE-NUMERICS-21: Checked piecewise coefficients and exponential variance integration

- Decision status: **Accepted — complete piecewise-coefficient package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded rational/Decimal references and read-only partition/arithmetic/caller reviews were completed on 2026-11-08; these do not certify production range/accuracy or an OU/Hull–White financial model.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/piecewise.rs::PiecewiseConstantCurve`: nonnegative finite piecewise coefficients, primarily volatility schedules; no curve calibration or discount-factor interpretation is inferred.

## Accepted decision: checked piecewise coefficients and exponential integration

Accepted on **2026-11-08** after whole-record review. Runtime,
caller/host/schema reconciliation and independent numerical/financial qualification
remain pending; observed behavior is retained below.

1. **Keep the coefficient curve and variance kernel explicit.** The curve stores
   finite nonnegative coefficients on a model-year axis. The sole integral is
   `I=integral_start^end sigma(u)^2*exp(-2*kappa*(anchor-u)) du` with one
   constant scalar kappa, not piecewise mean reversion. Kappa has inverse-time
   units; I has coefficient-squared times time units. It is an OU state-noise
   kernel when a model supplies that interpretation, not integrated-short-rate
   or bond-option variance by itself. No discount curve, calibration or financial
   volatility normalization is inferred.

2. **Retain one strict immutable validated state.** Keep private `times`/`values`,
   nonempty equal lengths, finite nonnegative entries, numerical zero first time
   and strictly increasing knots. Normalize signed zeros to `+0`; do not sort,
   merge close/redundant knots, insert a spacing floor or support signed values.
   Keep `new` and explicit `constant(value)`, both checked, with no default,
   mutable setter or unchecked constructor. Preserve existing `{times,values}`
   serialization and constructor-validated restoration/unknown-field rejection.
   Schema-expressible bounds must be faithful; relational shape/order checks
   still belong to runtime validation. No new restoration/state framework.

3. **Adopt the implemented right-continuous partition and flat tail.** Value i
   applies on `[times[i],times[i+1])`; the final value applies at/after the last
   knot for finite future times. At a knot, use its new value. Correct every
   misleading left-continuity description without changing this law. Retain
   redundant valid knots as input data; they do not change the exact integral
   of an otherwise unchanged schedule. Knot endpoint measure does not create
   an extra integration contribution or payment/event meaning.

4. **Use short canonical checked queries and integration names.** Select
   `get_value(time)->Result<f64>`, with finite `time>=0`, plus immutable slice
   getters `get_times`/`get_values`. Remove negative/NaN/infinite query fallback
   and the old `value_at`/`times`/`values` names. Rename the sole integral to
   `integrate(kappa,anchor,start,end)->Result<f64>`, deleting the old long name
   without aliases. All four arguments are required; the squared exponential
   kernel is explicitly documented, not a general integral/algorithm selector.

5. **Validate the whole integration request before identities.** Require finite
   kappa, anchor, start and end with `0<=start<=end<=anchor`. Core accepts finite
   signed kappa, including zero; financial mean-reversion restrictions remain
   with owners. Exact `start==end` returns zero only after full validation.
   Otherwise integrate every positive representable interval: remove the
   `1e-10`/ZERO_TOLERANCE duration shortcut. No oriented negative interval,
   endpoint snapping, before-origin extrapolation or hidden parameter repair.

6. **Partition exactly at schedule knots.** Split `[start,end]` at each distinct
   interior knot, selecting the right-continuous value at each segment start.
   The final coefficient remains flat beyond the last knot. Endpoints and
   segment lengths come from the supplied/stored represented coordinates;
   preserve tiny positive intervals and verify traversal progress. Do not
   synthesize an averaged coefficient, smooth a jump or coalesce an interval.

7. **Use the continuous limit only for exact zero kappa.** At `kappa==0`,
   segment contribution is exactly the mathematical `sigma^2*(r-l)`, evaluated
   with the scaling below. Remove `abs(kappa)<1e-12` and every other financial
   near-zero substitution. Small kappa alone says nothing about the anchor or
   interval products. Evaluating a dimensionless kernel near its continuous
   limit is numerical arithmetic, not permission to drop a nonzero kappa's
   anchor exponential or change the selected model.

8. **Choose one sign-stable analytic segment identity.** For length `d=r-l>0`,
   let `x=2*abs(kappa)*d` and `F(x)=-expm1(-x)/x`, with `F(0)=1`.
   For positive kappa choose hot endpoint r; for negative kappa choose l.
   Then `I_segment=sigma^2*d*exp(-2*kappa*(anchor-hot))*F(x)`.
   The exponential-difference argument is nonpositive for both signs, avoiding
   a growing expm1 followed by a tiny multiplier. A zero coefficient contributes
   exact zero and is skipped after request validation, before logs/exponentials.
   Use established expm1/log1p/scaled arithmetic; no quadrature, adaptive series,
   numerical solver, alternative kernel or new dependency/fallback method.

9. **Defer component restoration and retain scale information.** Compute the
   complete segment magnitude using scaled binary/log arithmetic. Do not require
   `sigma^2`, `2*kappa`, x, the anchor exponential or an individual segment
   contribution to fit binary64 before the requested total is assembled.
   In particular, do not form overflowing `2*abs(kappa)` before multiplying a
   small length, or erase a variance by squaring a tiny coefficient first.
   Evaluate dimensionless limits/range transitions truthfully; if necessary
   information cannot be resolved by the chosen bounded profile, error rather
   than returning a fabricated zero or an infinity-times-zero NaN.

10. **Accumulate positive contributions before restoring the total.** Select a
    private two-pass scaled/log accumulation: determine the largest positive
    segment magnitude, then combine normalized positive magnitudes in source
    order with checked Neumaier accumulation and restore the completed total.
    Preserve enough exponent information for several individually underflowing
    contributions to sum to a representable result. No contribution-level
    underflow refusal solely because that auxiliary scalar cannot fit.
    Track whether any active segment has positive coefficient separately from
    rounded magnitudes. No full segment-output buffer or public accumulator/
    diagnostic type is needed; no ordinary unscaled sum or per-segment rounding
    to zero before accumulation.

11. **Make zero and range behavior financial-data safe.** Successful zero means
    an exact empty interval or zero coefficient almost everywhere on the active
    interval. Every other exact integral is strictly positive for finite inputs.
    Preserve representable positive subnormals. A known positive final result
    rounding to zero, overflowing or numerically unresolved is a computation
    failure; no successful NaN/infinity, arbitrary floor, saturation or max(0)
    repair. This zero contract must not turn a failed variance into a selected
    deterministic process. The scalar/immutable curve remains unchanged on error.

12. **Bound work through the actual partition and preserve error categories.**
    Each pass uses one initial partition search and a forward walk over touched
    segments, with checked indices/progress and no hidden retries. Work is
    `O(log K+S)` with constant scratch, for K knots and S touched segments.
    Validate constructor shape and required allocation/layout arithmetic; map
    bad inputs to admission failures and allocation/arithmetic/range failures to
    computation errors with segment/bounds context. Do not flatten computation
    failures to validation, panic, create a new options/budget hierarchy or
    promise enormous valid inputs are cheap.

13. **Qualify analytic exactness and floating accuracy separately.** The formula
    integrates the selected step model analytically without quadrature error;
    floating evaluation is not exact real arithmetic. No universal ULP/relative
    bound, guaranteed success at all finite extremes, platform bit identity or
    bitwise invariance under knot splitting is claimed. Splitting an unchanged
    schedule must not systematically erase representable variance; tiny-duration,
    long-anchor/small-kappa and mixed-scale qualification is required. Explicit
    errors are preferable to silently changing coefficients, kappa or bounds.

14. **Reconcile the complete OU/Hull–White/tree failure chain.** Propagate checked
    query/integration results through HullWhiteParams, OU variance, tree setup/
    integral helpers, exact HW steppers and pricing/Greek consumers. Remove
    Err-to-NaN adapters and downstream NaN/negative-to-zero variance clamps;
    propagate first computation failures through the actual fallible caller
    chain, rather than selecting zero volatility or scalar fallback. Update all
    short names and correct continuity docs together. This does not change
    their separately owned equations, calibration, schedule precedence or drift.

15. **Preserve host shape and intentional generic scope.** Generic core
    PiecewiseConstantCurve remains Rust-only. Preserve its embedded model wire
    `{times,values}` and faithful validation/schema. Existing HullWhiteParams
    `get_sigma` Python/WASM APIs must expose checked query behavior through the
    central error mappings; update stub/runtime docs, declarations, examples and
    relevant parity checks in the later implementation slice. No new generic
    curve host class, compatibility alias, wire migration or binding-side math.

16. **Keep financial ownership and qualification explicit.** Models retain
    positive-kappa or strictly-positive-sigma admission where required, model
    clocks/origins, annualized rate-volatility units, schedule/scalar precedence,
    integrated-state/bond-vol transformations, calibration and price/risk error
    budgets. Core accepts nonnegative coefficients and finite signed kappa only
    for this finite-horizon mathematical kernel. Primary/maintained references,
    exact rational/Decimal examples and located tests are distinct from executed
    production, statistical or financial qualification. No runtime, tests,
    fixtures, generators, dependencies, schemas or hosts change in this review.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Unequal/empty arrays, negative/nonfinite value or invalid knots | Constructor/restoration error; no sorting, padding or floor. |
| First knot `-0.0` | Accept numerical zero and normalize to `+0`. |
| Knots `[0,1]`, values `[1,2]`, query exactly 1 | Return 2: right-continuous law; finite times above 1 also return 2. |
| Query negative, NaN or either infinity | Input error; no first/last-value fallback. |
| Empty interval with nonfinite kappa or invalid anchor | Error before zero identity. |
| All active values zero, with otherwise valid extreme parameters | Exact zero without evaluating meaningless zero-times-infinity products. |
| Constant sigma=`2^50`, kappa=0, length=`2^-100` | Mathematical integral 1; no short-duration zeroing. |
| Constant sigma=1, kappa=`2^-50`, anchor=`2^50`, interval `[0,1]` | About .1353352832366128121, not 1 from a small-kappa shortcut. |
| Kappa=0, `(sigma,length)=(2^600,2^-700)` / `(2^-600,2^700)` | Mathematical totals `2^500` / `2^-500`; intermediate square range is irrelevant. |
| Kappa=0, sigma=`2^-537`, redundant quarter-interval knots on `[0,1]` | Total `2^-1074`, though each quarter contribution alone rounds to zero. |
| Knots `[0,1]`, values `[1,2]`, anchor=2, integrate `[0,2]` | Kappa=0: 5; kappa=.5: `4-3/e-1/e^2`; kappa=-.5: `e^2+3e-4`. |
| Known positive total below representable output range or above MAX | Computation error; no deterministic-volatility or infinity result. |
| Downstream integration failure | Propagate error through host/model/tree/stepper chain; no Err-to-NaN-to-zero path. |

## Observed choice and rationale

Store right-continuous step values and integrate their square against an exponential kernel analytically. An expm1 expression reduces cancellation for small intervals; a fixed near-zero-kappa branch takes the continuous limit.

## Observed algorithm

1. Require equally sized nonempty times/values, first time exactly positive-zero bits, finite nonnegative knots/values and strictly increasing knots. Deserialization calls the same constructor.
2. At query t, find the partition point for knots≤t and select the preceding value, using index zero if none. This is right-continuous at knots and flat beyond the final knot.
3. `integrate_squared_exp_weight(kappa,A,start,end)` validates finite arguments, start≥0, end≥start and A≥end. If end−start≤1e-10 return 0.
4. Split the requested interval at every coefficient knot. For constant sigma on [l,r], use `sigma²(r−l)` if |kappa|<1e-12; otherwise `sigma² exp[−2 kappa(A−r)] * −expm1[−2 kappa(r−l)]/(2 kappa)`. Sum contributions with ordinary addition.

## Observed selection, controls and conventions

Caller supplies kappa, anchor and integration bounds. The helper returns the integral of `sigma(t)² exp[−2 kappa(A−t)] dt`; output units depend on the coefficient convention. Negative finite kappa is accepted even though particular mean-reverting models may require nonnegative kappa.

## Observed limitations, failure and fallback

`value_at` does not validate query finiteness: negative time and NaN select the first value; +infinity selects the last. −0 as the first knot is rejected. Very short intervals are zeroed. Exponential/square overflow can yield Ok(infinity), because final integral finiteness is not checked. No quadrature fallback or signed coefficient support exists.

## Observed alternatives and tradeoffs

Analytic step integration avoids quadrature error for the chosen step model. Smooth/interpolated volatility models and numerical integration are separate alternatives, not interchangeable without changing variance dynamics.

## Evidence

- [Core implementation and inline tests](../../../../finstack-quant/core/src/math/piecewise.rs)
  demonstrate constructor-validated private state and the actual right-continuous
  partition, despite left-continuity comments. Located two-segment and near-zero
  limit checks were inspected/located, **not executed**; they do not qualify
  nonfinite queries, tiny intervals, long anchors or full requested-output range.
- [Hull–White context](../../../REFERENCES.md#hull-white-1990-pricing-ird)
  identifies model use, not automatic selection of model parameters or variance
  interpretation. [Maintained QuantLib OU variance source](../../../REFERENCES.md#ou-state-variance-kernel)
  corroborates the constant-coefficient mathematical kernel; its own small-rate
  cutoff is not selected here. The sign-stable endpoint formula follows by direct
  integration. [Rust expm1 documentation](https://doc.rust-lang.org/std/primitive.f64.html#method.exp_m1)
  distinguishes the near-zero exponential difference from subtracting exp-1;
  dependency reuse does not certify the complete scaled arithmetic.
- Independent **2026-11-08** exact-rational and 150-digit Decimal examples checked
  tiny-duration/large-and-small-coefficient zero-rate targets, the representable
  subnormal total from individually underflowing quarter contributions, nonzero
  small-kappa/long-anchor behavior, both-sign scale rescue, split/whole constant
  integrals and positive signed-kappa two-segment examples. Separate rational
  Taylor bounds checked the e-based two-segment and small-kappa anchor targets.
  These references did not call production kernels or qualify runtime output.
- Read-only partition/arithmetic/caller reviews traced
  [HullWhiteParams](../../../../finstack-quant/models/src/rates/hull_white.rs),
  [OU variance and integrated-state use](../../../../finstack-quant/models/src/monte_carlo/process/ou.rs),
  [tree query/integral adapters](../../../../finstack-quant/models/src/trees/hull_white_tree.rs)
  and [exact HW stepper](../../../../finstack-quant/models/src/monte_carlo/discretization/exact_hw1f.rs).
  Current Err-to-NaN and max(0) paths are static reconciliation findings, not an
  executed Rust pricing reproduction. Existing
  [Python](../../../../finstack-quant-py/src/bindings/models/rates/hull_white.rs)
  and [WASM](../../../../finstack-quant-wasm/src/api/models/rates/hull_white.rs)
  get_sigma surfaces and [generic parity scope](../../../../finstack-quant-py/parity_contract.toml)
  identify required host error/docs reconciliation, not new generic bindings.

Analytic step integration does not qualify a calibrated volatility model,
all floating-point extremes or a financial variance/price/Greek error budget.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
