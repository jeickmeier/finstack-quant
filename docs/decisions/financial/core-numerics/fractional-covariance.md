# CORE-NUMERICS-19: Fractional Brownian increment covariance

- Decision status: **Accepted — complete fractional-covariance package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded Decimal/algebra references and read-only formula/state/caller reviews were completed on 2026-11-08; these do not certify production accuracy, rounded PSD or a financial sampler.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/fractional.rs`: validated Hurst exponent wrapper and covariance matrices for increments on caller-specified time grids. No sampler, factorization, financial drift or rough-volatility equation is implemented here.

## Accepted decision: checked fractional increment covariance

Accepted on **2026-11-08** after whole-record review. Runtime,
caller/schema reconciliation and independent numerical/statistical/financial
qualification remain pending; observed behavior is retained below.

1. **Select standard unit-normalized fBM increments.** Use centered Gaussian
   fBM with `B_H(0)=0`, `Var(B_H(1))=1` and increment variance `delta^(2H)`.
   Its level covariance is `0.5*(t^(2H)+s^(2H)-|t-s|^(2H))` for nonnegative
   times. This fixes normalization, not a sampler. The original Mandelbrot–Van
   Ness moving-average normalization includes its own H-dependent amplitude;
   do not silently import that amplitude or a Volterra/Riemann–Liouville kernel.
   Times use one caller-chosen unit; covariance has units `time^(2H)`.

2. **Require the validated Hurst type throughout.** Retain private immutable
   `HurstExponent::new(h)` with finite `0<h<1`, no endpoint extension, epsilon
   snapping, clipping or default. Rename `value()` to canonical `get_value()`
   without an alias; keep `is_rough()` as the exact `H<0.5` classification.
   Above one half means more regular/persistent, not differentiable paths or
   an empirically validated financial regime. Preserve constructor-validated
   `{h}` restoration and unknown-field rejection; schemas must express the same
   exclusive bounds. No new mutation, unchecked/state-restore path or state type.

3. **Make the two existing helpers canonical and fallible.** Keep
   `fbm_increment_covariance(ti,ti1,tj,tj1,h)` and
   `fbm_increment_covariance_matrix(times,h)`, replacing raw `h: f64` with
   `HurstExponent` and returning `Result<f64>` / `Result<DMatrix<f64>>`.
   Remove the unchecked/raw-H path; no compatibility wrapper, duplicate helper,
   derivative, factor/report carrier or public algorithm/options selector.

4. **Admit explicit forward scalar intervals before identities.** All four
   endpoints must be finite and nonnegative, with `ti<=ti1` and `tj<=tj1`.
   Negative-time exclusion is project elapsed-time policy, not a denial of
   two-sided fBM mathematics. Permit overlap, containment, touching and zero
   widths; reject reversed intervals rather than orienting or sorting them.
   Validate both intervals completely before a zero-width result of exact zero.
   No minimum gap, inferred day count, origin reset or requirement to start at 0.

5. **Validate the entire matrix grid, including empty identities.** Require
   every supplied time finite/nonnegative and all successive values strictly
   increasing. Do not sort, deduplicate or add spacing floors. Empty and valid
   singleton grids return `0x0` after all supplied-value validation; duplicate,
   decreasing, negative or nonfinite entries error even in otherwise small
   inputs. Starting above zero is valid because the increment law is stationary.
   Matrix zero-length increments are rejected by strict ordering.

6. **Evaluate the increment law directly, with explicit exact branches.** For
   intervals `[a,b]`, `[c,d]` and `p=2H`, use
   `C=0.5*(|b-c|^p+|a-d|^p-|b-d|^p-|a-c|^p)`, eliminating all level powers and the obsolete level-subtraction path.
   Canonically order the two whole interval pairs for symmetric evaluation.
   After validation, branch in this order: zero width; exact `H=0.5` with
   `max(0,min(b,d)-max(a,c))`; identical intervals with direct `(b-a)^p`;
   touching intervals; then the general profile below. The Brownian maximum is
   the overlap definition, not numerical repair. No approximate H/adjacency test.

7. **Use a stable touching-interval identity near the Brownian limit.** For
   positive touching lengths `x,y`, set `M=max(x,y)`, `r=min(x,y)/M`,
   `e=p-1`. Evaluate
   `C=0.5*M^p*((1+r)*expm1(e*log1p(r))-r*expm1(e*log(r)))`.
   The two bracket contributions share the sign of `e`; use that structure,
   preserved log-ratios and deferred scaled products. Do not form an overflowing
   exponential followed by multiplication by tiny `r`, or underflow `r` before
   its fractional power can contribute. Equal lengths reduce to
   `x^p*expm1(e*ln(2))`. This is an algebraic branch, not an epsilon-H correction.

8. **Select one bounded general arithmetic profile and cancellation refusal.**
   Let `L` be the largest of the four endpoint distances. Form their normalized
   powers `q_k=(distance_k/L)^p` with log/exponent-preserving arithmetic; a
   structural zero distance contributes exact zero. A positive normalized power
   that cannot be represented as positive finite errors rather than becoming zero. Use stable `log1p` near
   equal distances, preserve positive distances through scaling, and use checked
   source-order Neumaier sums `S=q1+q2-q3-q4` and `B=q1+q2+q3+q4`.
   Require **`|S| > 8*EPSILON*B`**, with `EPSILON=2^-52`. Failure is a
   computation error, not zero covariance. Restore only `0.5*L^p*S` using
   scaled/logarithmic arithmetic; an unrequested standalone `L^p` need not fit.
   If required scaled information cannot be retained, error rather than dropping
   a positive term into a structural zero. The fixed gate is an operational
   refusal rule, **not a portable pow-function error bound or accuracy proof**.
   No quadrature, adaptive series, higher-precision fallback or new dependency.

9. **Preserve signs and distinguish genuine zeros from numerical failure.**
   Every returned covariance is finite. Positive-width self-variance must be
   strictly positive; known nonzero final underflow/overflow errors instead of
   returning zero/infinity. Preserve resolved subnormal signed cross-covariances
   without an epsilon floor. For positive disjoint/touching intervals, the sign
   is `sign(H-0.5)`; zero/wrong-sign computed results error outside the Brownian
   branch. For `H>0.5`, every pair of positive-width forward intervals has
   positive covariance, including overlap/containment; reject zero/negative
   computed results there too. Overlapping rough increments can have either
   sign. An unresolved
   general cancellation to zero errors even when a genuine overlapping zero
   might exist; it does not assert independence. Only established structural
   zero branches return exact zero. No absolute-value/clamp/renormalization fix.

10. **Return an atomic, exactly mirrored dense matrix.** Keep the existing
    `DMatrix<f64>` shape for `n=times.len()-1` increments. Preflight checked
    `n*n`, byte/layout arithmetic and fallible reservation before evaluation.
    Compute variances directly and one triangle once, mirroring each stored
    scalar exactly. Publish a complete finite symmetric matrix with positive
    diagonal, or return the first contextual failure and discard private output.
    No partially valid matrix. Construction costs `O(n^2)` time/storage; structural
    allocation admission is not a promise that every large request is cheap.
    Do not add a new arbitrary step cap, caller scratch API or result framework.

11. **Keep factorization and repair separate.** The exact covariance law is
    positive semidefinite; finite mirrored entries and positive diagonals do not
    certify rounded PSD, good conditioning or factorization success. Add no
    `O(n^3)` PSD gate, jitter, eigenvalue clip, unit-diagonal normalization,
    nearest-correlation repair or alternate-H/grid fallback. Accepted 07's
    factor/admission rules and actual consumer numerical failures remain
    separate; this function supplies covariance, not a usable sampler certificate.

12. **State error categories and nonuniform precision honestly.** Bad H/time/
    order/shape/resource admission is validation failure; allocation, required
    arithmetic/range loss, cancellation refusal and computed sign/variance
    failures are computation failures with interval or matrix-index context.
    No panic, successful NaN/infinity, sentinel, blanket validation remap or
    hidden parameter repair. Stable identities and checked finite output provide
    no universal relative/ULP bound, bitwise translation/scaling identity or
    guaranteed success for every mathematically valid input. Small H, near-half H,
    extreme scales and far-separated short intervals can be refused explicitly.
    Supplied rounded endpoints cannot recover a lost intended time interval.

13. **Reconcile actual fBM consumers without approving their approximation.**
    The Cholesky and windowed-conditional generators in `models/.../rng/fbm.rs`
    already construct Hurst values: pass that validated type and propagate every
    new covariance failure at full/near/previous blocks, covariance vectors and
    self-variances. Propagate computation/factorization failures truthfully rather
    than replacing them with validation or zeros. The windowed Schur residual's
    current negative/NaN-to-zero suppression is not approved: reconcile its
    checks/errors and fallible generation chain under accepted 17, with accepted
    07 factor/solve admission. This does not select the window, omitted long-range
    dependence, sampler/default, empirical H or financial approximation budget.

14. **Retain intentional host scope and faithful model wire state.** No direct
    generic core Python/WASM covariance helper or Hurst class is added. Hurst is
    already embedded as `{h}` in model wire types: preserve that representation,
    constructor validation and truthful generated bounds/declarations. Reconcile
    `get_value` in actual process, discretization and Volterra consumers, examples
    and docs without changing their stochastic law. Signature/wire implementation
    requires normal generator/parity/host checks in its later implementation
    slice; no wire format, host alias or migration is introduced here.

15. **Leave financial process and time ownership explicit.** Callers own actual
    elapsed-time clocks, units, volatility/noise amplitude, risk-factor mapping,
    calendars, sampling/factorization budgets, correlation/rough-volatility
    equations and price/risk acceptance. Stationary fBM covariance is not the
    Volterra/Riemann–Liouville covariance simply because H is shared. No drift,
    derivative, estimator, annualization, FFT/Toeplitz sampler or rough-volatility
    default is selected. Accepted 17/18 randomness and record 20 grids keep their
    own contracts/statuses.

16. **Separate algebra/reference checks from runtime and model qualification.**
    Cover exact Brownian overlap, direct self-variance, translation/homogeneity,
    adjacent signed covariance and the near-half touching identity, overlapping
    rough signs, irregular matrices, whole-input/empty/state boundaries and
    extreme-scale/cancellation/range/resource failures. Check one-triangle mirror
    and actual caller error propagation. Existing constructor/serde/shape tests,
    bounded high-precision references, rounded PSD studies and downstream
    statistical/financial validation serve different roles. No runtime, tests,
    fixtures, dependencies, schemas or hosts are changed by this review.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| H equals 0/1, NaN or infinity | Hurst construction/restoration error; no clamp/default. |
| Zero-width interval paired with a nonfinite/reversed interval | Validate all endpoints; error before zero identity. |
| Two valid scalar intervals, either width zero | Exact covariance zero. |
| Matrix `[]` / `[1]` / `[NaN]` | `0x0` / `0x0` / error. |
| Matrix `[2,3,5]` / `[0,0,1]` | Valid nonzero origin / duplicate-time error. |
| H=.5, intervals `[2^53,2^53+2]` against themselves | Variance 2 from direct overlap, without large-level subtraction. |
| H=.5, `[0,2]` and `[1,3]` | Covariance 1; disjoint intervals instead give zero. |
| Adjacent unit intervals, H=.25 / .75 | `sqrt(2)/2-1` / `sqrt(2)-1`, about -.2928932188 / +.4142135624. |
| Overlap `[0,2]`, `[1,3]`, H=.25 | `(sqrt(3)-1)/2`, about +.3660254038; rough does not mean all cross-covariances are negative. |
| Adjacent unit intervals, H=nextUp(.5) | Positive covariance about `1.539095918623324e-16` via the touching identity; no H snapping. |
| General unit intervals separated by gap 1e8, H=.75 | Mathematically nonzero covariance about `3.74999998125e-5`; fixed cancellation gate can refuse it. |
| Finite but indefinite rounded matrix / failed consumer factorization | No automatic repair or successful sampler claim; preserve numerical failure. |

## Observed choice and rationale

Use the fractional Brownian covariance directly, then take a four-corner difference to obtain increment covariance. `HurstExponent` admits finite 0<H<1 and labels H<0.5 as rough; the scalar/matrix helpers separately accept a raw h and do not enforce that wrapper.

## Observed algorithm

Set `C(t,s)=0.5[|t|^(2h)+|s|^(2h)−|t−s|^(2h)]`. For increments [ti,ti1] and [tj,tj1], compute `C(ti1,tj1)−C(ti1,tj)−C(ti,tj1)+C(ti,tj)`. A grid of n+1 times produces a dense n×n nalgebra matrix by evaluating this formula at each pair. Fewer than two times produces a 0×0 matrix.

## Observed selection, controls and conventions

H and the grid are caller-supplied; no approximation, jitter or matrix repair occurs. Time units determine covariance units of time^(2H). The same raw h must be used consistently with any downstream process normalization.

## Observed limitations, failure and fallback

Raw helper inputs are unchecked: nonfinite/nonmonotone/negative times and invalid h can produce mathematically unintended values or NaN. Absolute times in the formula permit negative arguments mechanically; that is not a financial negative-time endorsement. Four-term subtraction can lose precision for small increments far from zero. The output is not guaranteed PSD after floating-point error and is not automatically repaired.

## Observed alternatives and tradeoffs

Direct covariance and dense factorization in consumers support irregular grids but cost O(n²) storage. Toeplitz/FFT methods for uniform grids are separate sampler choices, not selected here. Stable increment-specific expansions are not implemented.

## Evidence

- [Core implementation and inline tests](../../../../finstack-quant/core/src/math/fractional.rs)
  show constructor-validated private Hurst state and `{h}` restoration, but raw
  unchecked scalar/matrix helpers and four level-covariance subtraction. Located
  half-H level-covariance, shape/symmetry, singleton-empty and serde assertions
  were inspected/located, **not executed**; they do not certify arbitrary-scale
  increment accuracy, rounded PSD or a downstream rough-volatility model.
- [Mandelbrot–Van Ness](../../../REFERENCES.md#mandelbrot-van-ness-1968)
  supplies stationary/self-similar increment theory, separated-increment
  correlation and nondifferentiability. Its amplitude convention is distinct from
  the selected standardized law. [Gao–Liu–Oosterlee–Temme equation 1](../../../REFERENCES.md#gao-liu-oosterlee-temme-2023-fbm)
  states that exact normalized covariance and Brownian limit. The endpoint and
  touching identities are algebraic consequences; the fixed general cancellation
  gate is project numerical policy, not a theorem from either source.
- Independent **2026-11-08** 110-digit Decimal/algebra references checked unit
  adjacent H=.25/.75 closed forms, positive overlapping H=.25 covariance,
  Brownian overlap, touching formulas for five H values/three length pairs,
  nextUp(.5), translation/homogeneity and three small positive-definite reference
  matrices. A nonzero far-gap example explicitly illustrates the chosen gate's
  refusal. The `[2^53,2^53+2]` example exposes why absolute-level subtraction
  is unsuitable; its required Brownian variance is exactly 2. These calculations
  did not call production kernels or reproduce a Rust runtime defect.
- Read-only formula/state/caller reviews identified the only covariance-helper
  consumer, [model fBM generators](../../../../finstack-quant/models/src/monte_carlo/rng/fbm.rs).
  Their current Schur clamp, infallible/debug-shape generation and factorization
  error remap require reconciliation, not inferred sampler acceptance. Existing
  [fractional simulation selection](../../../../finstack-quant/models/src/monte_carlo/simulate/fractional.rs)
  and [Volterra generator](../../../../finstack-quant/models/src/monte_carlo/rng/volterra.rs)
  retain separate model ownership. No direct core generic binding exists;
  [model wire Hurst shape](../../../../finstack-quant-wasm/types/generated/models/models.ts)
  and [parity scope](../../../../finstack-quant-py/parity_contract.toml) remain
  relevant to a later signature/schema implementation slice.

Covariance construction alone does not validate a sampler, empirical regime,
financial model, rounded PSD property or production numerical error bound.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
