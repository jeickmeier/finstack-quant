# CORE-NUMERICS-04: Fixed and adaptive numerical integration

- Decision status: **Accepted — complete quadrature package, 2026-11-07**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Evidence inspected; exact-rational examples, bounded Gaussian-table comparisons and semantic reviews passed; production qualification pending**. Simpson values/proxies, missed-feature and subnormal-budget examples passed; nine source tables/87 node-weight pairs passed symmetry/positivity/moment and NumPy 2.5.1 comparisons. Linked Rust/host tests were located or inspected, not run; these checks do not certify accepted callback/work-limit contracts or true error bounds.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

This record owns all math/integration.rs scalar APIs: fixed Simpson, adaptive
Simpson, shared-node adaptive Simpson pair, composite/adaptive Gauss-Legendre,
reusable Legendre grids and Gauss-Hermite standard-normal expectations. It owns
sampling, absolute estimator budgets, work limits and checked numeric/error
contracts, not product measures, payoff discontinuities, integration truncation,
pricing acceptance or a domain-wide error certificate.

## Accepted decision: checked quadrature, budgets and measures

Accepted on **2026-11-07** after whole-record review. Runtime, caller/host
reconciliation and independent numerical/table qualification remain pending;
observed source behavior is retained separately below.

1. **Retain explicit fixed families and supported Gaussian orders.** Keep
   composite Simpson; scalar and paired adaptive Simpson; Gauss-Legendre order
   2/4/8/16 with composite/adaptive evaluation and reusable grids; and Hermite
   order 5/7/10/15/20. Reject unsupported orders and invalid panel counts without
   nearest-order replacement, hidden defaults or automatic family fallback.
   Do not add Kronrod, relative-error modes, infinite-interval transforms, dynamic
   node solving or a new integration/configuration framework to this package.

2. **Make canonical callbacks and results fallible.** Use FnMut(f64)->Result<f64>
   for scalar callbacks and FnMut(f64)->Result<(f64,f64)> for paired Simpson,
   removing the Copy requirement through borrowed/internal mutable evaluation.
   All integrators, including Hermite::integrate, return Result. Preserve the
   first callback error and source category, with node/component context where
   available; stop calling after it. Reject sampled NaN/infinity and unusable
   arithmetic instead of Ok(nonfinite), temporary zero or sentinel substitution.
   No parallel try_* API, blanket adapter trait or host callback framework.
   Callbacks must describe one deterministic function during the run; mutable
   diagnostics are permitted, but external side effects are not rolled back.

3. **Unify finite integration orientation and zero-width identities.** Require
   finite a,b. After admitting required order/count/tolerance/work parameters,
   a==b returns exact +0 (or the zero pair) without callback evaluation. For a>b,
   evaluate the same canonical increasing-bound rule and negate the final value;
   error estimates remain nonnegative. Reversed orientation must not change
   sampling/refinement decisions. Counts/order remain explicit even for a zero
   interval. The reusable grid constructor is a deliberate exception: it requires
   a<b and produces positive scaled weights, not an empty/reversed grid policy.

4. **Validate controls and bound adaptive work explicitly.** Simpson n is a
   positive even number of equal subintervals; composite Legendre panels is a
   positive whole count. Check count/index/evaluation arithmetic and required
   integer-to-float conversions rather than wrap, truncate or lose count identity.
   Every adaptive API takes an explicit positive max_evals alongside max_depth;
   count actual callback invocations, with one paired call counting once. The
   root is depth 0; depth 0 still allows its coarse/refined comparison (5 Simpson
   calls, 3*order Legendre calls on a nonzero interval). Validate parameters before
   callbacks; a budget too small to obtain required comparison data fails rather
   than returning a partial answer. Do not exceed the budget or insert a hidden
   retry/work cap. Use a bounded iterative traversal/internal stack rather than
   exposing the process stack to an arbitrary caller recursion depth.

5. **Retain Simpson definitions, including the uncorrected adaptive value.**
   Fixed Simpson is h/3 times endpoint weights 1, alternating interior 4/2,
   h=(b-a)/n. Adaptive Simpson compares coarse S1 with the sum S2 of two half
   panels, uses e=abs(S2-S1)/15 and returns the refined S2 on a terminal panel.
   Do not add a Richardson correction S2+(S2-S1)/15 or call the reported value
   extrapolated. Cache endpoint/midpoint samples and reuse them during bisection;
   the specified paired method applies these same formulas component-wise.
   Ideal polynomial exactness is degree<=3, not a guarantee for arbitrary payoffs.

6. **Keep Legendre's mapping and adaptive estimator explicit.** For each panel,
   map the stored weight-one rule on [-1,1] by a stable midpoint and half-width.
   Composite evaluation uses order nodes per panel and the scaled interval
   weights once. Adaptive Legendre uses the same order for coarse Q1 and two-half
   Q2, e=abs(Q2-Q1), returning Q2 with no order-dependent divisor or correction.
   Reuse known child coarse estimates instead of evaluating them again. This is
   an operational difference estimator, not a Kronrod rule or guaranteed
   conservative bound. More panels do not increase the rule's polynomial degree.

7. **Select absolute global estimator budgets without an accuracy claim.**
   Scalar tol must be finite and nonnegative in integral units. Refinement uses
   local shares; split represented budgets deterministically as left=tol/2,
   right=tol-left, with no epsilon floor. Zero shares and tol=0 are allowed, but
   zero requests a zero computed estimator, not exact true integration. Terminal
   leaf values and absolute errors are summed with checked compensated totals;
   success requires a finite final value and finite summed estimator<=original
   tol. A depth-limited leaf may exceed its local share if the complete global
   sum still passes. Depth exhaustion alone never establishes success; inability
   to finish required sampling within the hard callback budget errors, with no
   partial integral or family/tolerance replacement.

8. **Give paired Simpson two explicit component budgets.** Replace its single
   tol with an explicit [tol0,tol1], each finite nonnegative and expressed in its
   own component's integral units. Evaluate both components at each shared node;
   refine the shared mesh when either component needs refinement. Split and sum
   each budget/error independently and require both final gates. A large value
   or spare tolerance in one component cannot consume the other's budget; no
   norm, relative scaling or variance/covariance interpretation is implied.
   Preserve tuple order; if both components are invalid at one sample, identify
   the first component in tuple order. Pair results need not match independent
   scalar runs bit-for-bit because their refined meshes can differ.

9. **Use stable geometry and reject genuine sampling stagnation.** Compute
   midpoints/panel maps without avoidable a+b or b-a overflow. Required new
   abscissas must be finite and representably located in their proper panel;
   distinct required nodes cannot silently collapse to repeated endpoints and
   manufacture zero estimated error. A nonzero interval whose needed refinement
   cannot be represented is a numerical/geometric failure, not zero width.
   Avoid unrequested full-span/per-node representations where a stable direct
   integral calculation is available. No arbitrary minimum interval or displaced
   endpoint samples. Simpson samples its endpoints; singular/undefined endpoint
   treatment must be selected by the caller, not silently skipped.

10. **Use checked scaled weighted accumulation throughout.** Apply shared
    Neumaier-style compensated accumulation to fixed rules, adaptive value/error
    aggregation and each pair component; validate samples, required products,
    partial totals and final outputs. Reject detectable unusable arithmetic; this
    is not a universal roundoff/underflow-detection certificate. Scale/reorder bounded weights and panel
    factors to avoid 4*f or an unnormalized Hermite sum overflowing when the
    requested result is representable. Nonzero required terms/estimators lost
    through underflow must be reformulated or error, not interpreted as zero
    contribution or convergence; compensation cannot recover a lost product.
    Exact cancellation and ordinary valid output rounding remain distinct.
    Finite signed subnormal results are allowed. Some valid mathematical inputs can
    still fail the checked evaluation range; no arbitrary-precision layer or
    portable ULP/relative-error guarantee is selected.

11. **Keep reusable Legendre grids as checked positive-interval data.**
    gauss_legendre_grid requires finite a<b, supported order and panels>=1.
    Return deterministically ordered abscissa/scaled-weight pairs so dotting
    weight*f(x) applies the panel scale exactly once. Validate representable
    nodes, finite strictly positive weights, checked order*panels capacity and
    fallible allocation; reject zero-underflow weights or invalid geometry rather
    than publish a partially valid grid. Direct composite integration may stream
    the same rule instead of allocating a grid, and need not fail solely because
    an unrequested separately stored weight cannot represent its scaled factor.
    Batch consumers still own checked callback/output aggregation of exposed grids.

12. **Keep Hermite's measure and raw-table interface unambiguous.** Retain
    private immutable physicists' nodes z_i/weights w_i for exp(-z^2) integration.
    integrate(f) means standard-normal expectation
    E[f(Z)]=sum_i w_i*f(sqrt(2)*z_i)/sqrt(pi), with the scaling applied once
    through stable checked arithmetic. Do not multiply another normal density.
    get_points/get_weights remain explicit immutable raw physicists' views for
    existing paired/nested consumers, not normalized standard-normal weights.
    Mean/variance/correlation transforms, other mixing distributions and financial
    measures remain with callers; no generic Gaussian transform or serialized
    custom-node quadrature framework is added.

13. **Qualify fixed tables and polynomial claims without product presets.**
    Retain only the supported fixed tables, requiring pinned source/version or
    reproducible derivation plus independent root/symmetry/positive-weight,
    normalization and polynomial-moment checks. Ideal m-point Gaussian rules
    integrate degree<=2m-1 under their stated measure; rounded tables/arithmetic
    need not do so exactly. The source's NumPy Hermite attribution is incomplete
    provenance, not independent qualification. Remove order-based recommendations
    claiming standard/high-precision Heston or other option-pricing fitness.
    No new runtime table generator/dependency or automatic table repair is chosen.

14. **Make sampling, limits and failures truthful.** Use deterministic rule-node
    order and left-before-right panel refinement after orientation normalization;
    this does not promise globally increasing callback coordinates during
    adaptation. Reuse mathematically shared samples/estimates, count every actual
    invocation and report actual reached depth/evaluation/work-limit cause.
    Callback failures retain their category; invalid controls use input/domain
    errors, arithmetic/geometric failures use computation errors and an unmet
    estimator budget uses convergence failure. Record the sampled coordinate or
    failing pair component when known; do not label configured max_depth/root
    midpoint as actual work or worst-leaf diagnostics. Keep scalar/tuple returns
    and existing error mechanisms rather than add a result/report framework.

15. **Keep method limitations and financial acceptance with actual owners.**
    A finite sample error estimate can miss narrow support, oscillation, kinks
    and even smooth functions vanishing at all sampled nodes. Returning success
    never certifies true error<=tol or establishes a price/risk error budget.
    Callers own breakpoints/transforms, truncation/tails, endpoint conventions,
    measure/density/discounting, component units and independent qualification;
    sums of separately integrated pieces need an explicit allocated overall
    budget and checked final aggregation. Nested/composed integrals also need
    owner work budgets; max_evals counts this integrator's callback invocations,
    not arbitrary work performed inside them. Reconcile scalar/pair callbacks, CDS/
    tranche/curve query adapters, copula/Gaussian/Student-t/multifactor and Fourier
    batch consumers together, removing temporary-zero/sentinel plumbing without
    claiming existing adapters swallowed errors where they actually rethrow them.
    Keep the deliberate Rust-only quadrature surface; host pricing/model errors
    propagate through existing mapping, with no new direct wrapper classes.
    Acceptance does not implement or qualify downstream financial algorithms.

### Accepted ideal polynomial degrees

| Rule | Supported nodes/panels | Ideal polynomial degree |
| --- | --- | --- |
| Composite Simpson | Positive even n subintervals | <=3 |
| Legendre | 2,4,8,16 nodes per panel | <=3,7,15,31 per panel |
| Physicists' Hermite / standard-normal transformation | 5,7,10,15,20 nodes | <=9,13,19,29,39 under the specified measure |

These are mathematical rule properties, not exact floating-point output or
financial accuracy promises.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Invalid order, Simpson n=0/odd, zero panels, nonfinite bounds/tolerance or negative tol | Input/domain error, even before a zero-interval identity. |
| Equal finite integration endpoints with valid controls | Exact zero without invoking the callback; grid construction still requires a<b. |
| Reversed bounds | Same canonical sampling as forward bounds, negated integral; nonnegative estimator. |
| Callback Err or sampled NaN/infinity | Stop/error with cause/node context, no partial result or zero replacement. |
| Simpson polynomial x^3 on [0,1] | Ideal value 1/4; no automatic degree guarantee beyond 3. |
| Adaptive Simpson f=x^4 on [0,1] at depth 0 | S1=5/24, S2=77/384, proxy1/1920; return S2 if the global estimate passes, not Richardson-corrected1/5. |
| Adaptive Legendre coarse/fine difference | Raw absolute difference; not divided by 15 or an order-based factor. |
| tol=0 and computed estimator 0 | May pass; does not establish exact true integral. |
| tol=3*eta split into children | Use represented shares2*eta and eta, not two rounded2*eta shares. |
| Pair components with different units or magnitudes | Explicit independent [tol0,tol1]; shared refinement, separate global gates. |
| Nonzero interval depth 0 / max_evals below required root comparison | Root comparison allowed / work-limit error without exceeding budget. |
| Needed midpoint equals an endpoint through rounding | Numerical/geometric failure, not false convergence. |
| f=[x(x-1/4)(x-1/2)(x-3/4)(x-1)]^2 on [0,1] | Initial Simpson samples/proxy/value 0 but true integral 5/1419264>0; finite sampling can miss smooth structure. |
| Supported Gaussian m-point polynomial rule | Ideal degree 2m-1; extra panels do not turn it into degree 2m*panels-1. |
| Hermite f=1,Z,Z^2,Z^4 | Standard-normal targets 1,0,1,3; no second PDF multiplier. |
| Hermite constant f=MAX or eta | Avoid avoidable raw-sum/product overflow/underflow or error; no silent infinity/zero answer. |
| Legendre grid weights consumed by a batch model | Apply them once; do not multiply another panel width. |
| Unrepresentable stored grid weight but a direct integral can fit | Grid may fail its representation contract; direct integration uses its own checked requested-output path. |
| Unsupported order in an otherwise zero interval | Error; identity does not bypass parameter admission. |
| Existing callback adapter uses a temporary zero then rethrows Err | Remove the adapter through canonical fallibility; not evidence of an accepted wrong price. |
| A fixed order described as high-precision Heston | Remove unsupported product-accuracy claim; owner convergence/reference qualification is required. |

## Observed algorithm

1. Composite Simpson requires positive even interval count `n`, uses spacing `(b-a)/n`, endpoint weight 1, odd weight 4, even interior weight 2, and multiplies by `h/3`.
2. Adaptive Simpson starts with one Simpson panel, compares it with two half panels, estimates error `|I_halves-I_whole|/15`, and recursively halves both interval and tolerance. It returns the two-panel estimate without Richardson correction. Leaf errors are summed; success requires finite value and total estimated error≤tol after recursion.
3. Gauss–Legendre uses stored orders 2,4,8,16 on [-1,1], mapped by midpoint `a+(b-a)/2` and half-width. Composite integration builds each panel's transformed `(x,w)` grid and sums `w f(x)`.
4. Adaptive Gauss–Legendre compares one panel against two half panels, estimates `|I_halves-I_whole|` (no order-based divisor), recursively halves tolerance, and enforces the summed error at return.
5. Gauss–Hermite supports orders 5,7,10,15,20. Its tables integrate the `exp(-z²)` weight; `integrate(f)` computes `Σ w_i f(sqrt(2)z_i)/sqrt(pi)` with Neumaier accumulation, hence returns `E[f(Z)]` for standard normal Z.

6. Paired adaptive Simpson evaluates both components at shared nodes, retains the same uncorrected two-half estimates and `/15` proxies, and refines when either component needs it. The current single scalar tol is applied independently to each component's summed estimate; failed final components are reported in tuple order. It has the same depth/geometry and infallible-callback limits as the scalar rule.

## Observed selection and controls

Order/panel count/tolerance/depth are explicit arguments. Fixed Simpson validates only even positive count. Adaptive bounds and tolerance must be finite with tol≥0; max depth is supplied. Composite Legendre requires `b>a` and at least one panel; the internal single-panel evaluator permits reversed bounds and returns 0 for equal bounds. Unsupported Gaussian order errors.

## Observed conventions

Finite-interval outputs have integrand units times integration-variable units. Hermite includes the standard-normal density normalization already; callers must not multiply it by a second normal PDF. Tolerances are absolute integral units.

## Observed limitations, failure and fallback

Adaptive rules stop subdividing on nonfinite error, depth exhaustion or sufficiently small local error, then return a convergence error unless the aggregate estimate passes. Sampled error estimates can miss narrow/discontinuous features. Fixed rules do not uniformly validate integrand values and may return `Ok(NaN)`/infinity; their accumulation is ordinary except Hermite. No automatic fallback between integration families exists.

## Observed alternatives and tradeoffs

The supported families are explicit alternatives. Higher-order/dynamic Gaussian rules, Gauss–Kronrod error estimates, and splitting at known payoff discontinuities are possible additions, not present automatic behavior. Product callers must decide error budgets and endpoint treatment.

## Evidence

- [Implementation and tables](../../../../finstack-quant/core/src/math/integration.rs)
  includes adaptive_simpson_pair as well as scalar rules; the original record
  omitted that paired API. Fixed Simpson/Legendre can currently return nonfinite
  successful values; Hermite normalizes after a raw weighted sum. Midpoint/count/
  allocation, sampled-error propagation and work diagnostics require reconciliation.
- [Integration tests](../../../../finstack-quant/core/tests/math/integration.rs)
  include analytic polynomial/sine cases, standard-normal moments, global-estimator
  and depth-limit fixtures. These are bounded formula/invariant evidence, not
  universal true-error or product-pricing qualification.
- [Tranche conditioning integration](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/integration.rs)
  and [copula callback adapters](../../../../finstack-quant/models/src/correlation/copula/mod.rs)
  use temporary-zero/error-capture plumbing but rethrow captured failures before
  returning; this is a fallibility reconciliation need, not a demonstrated wrong PV.
  [Forward-curve period averaging](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/curve.rs)
  consumes Simpson and the accepted fallible interpolation contract.
  [Heston strip batching](../../../../finstack-quant/models/src/closed_form/heston/strip_pricer.rs)
  uses reusable Legendre grids; retain that legitimate batch interface.
- Existing [parity contract](../../../../finstack-quant-py/parity_contract.toml)
  keeps numerical integration Rust-only. Actual model/credit/curve consumers need
  propagated fallibility, not new standalone host quadrature classes. Their
  existing once-only Gaussian transformation is not a demonstrated double-PDF bug.

[NIST DLMF quadrature](https://dlmf.nist.gov/3.5) provides primary Simpson/Gaussian
background. [NumPy Legendre](https://numpy.org/doc/stable/reference/generated/numpy.polynomial.legendre.leggauss.html)
and [Hermite](https://numpy.org/doc/stable/reference/generated/numpy.polynomial.hermite.hermgauss.html)
document weight functions and degree 2m-1 mathematical exactness. These establish
rule definitions, not provenance/precision of every embedded literal. The Rust
source's Hermite attribution lacks a pinned NumPy generation/version record.
[SciPy/QUADPACK documentation](https://docs.scipy.org/doc/scipy/reference/generated/scipy.integrate.quad.html)
describes estimated errors and missed-feature limitations; it does not make this
library's Simpson/Legendre estimator equivalent to QUADPACK. Sources were consulted
on 2026-11-07, not treated as product or vendor certification.

[Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes) and
[Neumaier/Kahan context](../../../REFERENCES.md#higham-accuracy-and-stability)
remain background. Compensation cannot repair an already lost weighted product,
invalid samples or missing payoff structure.

Independent exact-rational checks passed the uncorrected x^4 Simpson estimates,
its /15 proxy and the five-zero-node smooth polynomial's positive integral
5/1419264. IEEE expression checks confirmed conserving versus independently
rounded subnormal tolerance shares, weighted Hermite eta underflow and avoidable
Simpson MAX overflow. All nine embedded Gaussian tables (87 node/weight pairs)
passed symmetry, positive-weight and polynomial-moment checks through degree 2m-1
at a 1e-12 scaled test threshold; the largest measured scaled residual was about
1.96e-14. A local NumPy 2.5.1 comparison found a maximum relative weight difference
of about 3.01e-14. This is bounded source-table arithmetic/dependency comparison,
not original generation provenance, production Rust execution or a global error
certificate. Independent fixed/adaptive, caller/API and Gaussian/reference reviews
found no material issue.

Production Rust/host/model tests were located or inspected, not executed. Original
table provenance, numerical extremes, accepted callback/work-limit behavior and financial
caller qualification remain pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
