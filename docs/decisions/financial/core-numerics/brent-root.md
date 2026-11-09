# CORE-NUMERICS-05: Bracket search and Brent scalar root finding

- Decision status: **Accepted — complete Brent/root-search package, 2026-11-07**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Source inspected; independent mathematical/expression checks and semantic reviews passed**. Production solver and consumer qualification remain pending; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

This record owns math/solver.rs scalar bounded bracket search and one safeguarded
Brent kernel. It owns controls, sampling, finite arithmetic, termination and error
meaning, not economic domains, objective continuity, root uniqueness or financial
residual acceptance. [Accepted record 017](../core-conventions/017-irr.md#accepted-decision-irr-and-xirr-timing-eligibility-and-numerical-acceptance)
already selects IRR/XIRR's fixed full-domain log bracket, 100 updates, 1e-12
operational coordinate tolerance and separate finite returned-rate residual gate.
Those choices are preserved; the common solver contract below does not reopen them.

## Accepted decision: checked Brent solving and bounded search

Accepted on **2026-11-07** after whole-record review. Runtime,
caller/host reconciliation and independent numerical qualification remain pending;
source-observed behavior is retained separately below.

1. **Retain one safeguarded derivative-free Brent method.** Keep the existing
   bracket-preserving combination of secant, inverse quadratic interpolation
   (IQI) and internal bisection. Bisection is the algorithm's safeguard, not an
   automatic fallback to another financial solver. No Newton, broad root scan,
   minimizer, automatic retry or new solver/result framework is selected.
   Search and supplied-bracket entry points share this same checked kernel.

2. **Use one canonical fallible callback surface.** Both solve and
   solve_in_bracket take FnMut(f64)->Result<f64>, return Result<f64> and preserve
   callback failures. Delete solve_fallible and the parallel infallible wrappers;
   callers with a numerical function explicitly return Ok(value). Evaluate each
   requested point once and cache its matching residual. Immediately reject a
   sampled NaN/infinity before probing another point or accepting a bracket;
   stop after the first callback failure. No NaN, finite huge objective or zero
   substitute for a failed price/source evaluation. Callbacks must represent a
   deterministic objective during a run; diagnostic mutation is allowed, but
   already executed callback side effects are not transactional.

3. **Make numerical controls immutable, validated and explicit.** Use
   BrentSolver::new(tolerance,max_iterations)->Result with private controls and
   read-only accessors where needed. Require finite strictly positive tolerance
   in x units and max_iterations>=1. Remove Default, no-argument construction,
   mutating public fields and with_* configuration paths; no generic 1e-12/100
   or financial search-domain defaults. All controls are valid before callbacks
   or endpoint-zero success. Exact-zero input does not rescue an invalid control.
   No serialization/configuration scheme is added to this Rust-only object.

4. **Separate explicit bounded search inputs from a supplied bracket.**
   solve takes explicit initial_guess, min, max, initial_half_width and
   max_expansions in addition to f; the solver's own two controls apply afterward.
   Require finite min<max, finite initial_guess within [min,max] and positive finite
   initial_half_width. Reject an out-of-domain guess rather than clamp it. Zero
   max_expansions means the initial endpoint pair only; no hidden 20-round search,
   percentage-of-guess width, minimum 0.01 or +/-1e6 domain. solve_in_bracket takes
   finite a,b in either order and has no unrelated search settings to validate.
   Keep the bracket-search helper internal, not a second public configuration API.

5. **Define endpoint, singleton and sign rules before iteration.** Normalize
   signed-zero coordinates consistently, order supplied endpoints and sample the
   lower then upper point, checking each immediately; coincident endpoints are
   evaluated once. After required initial endpoint samples are valid, lower
   computed-zero wins, then upper zero, otherwise signs must differ. A singleton
   succeeds only if its one finite callback value is computed zero; a nonzero
   singleton or same-sign nonzero pair cannot enter Brent. Compare finite signs
   directly, never fa*fb, so residual product overflow/underflow cannot destroy
   bracketing. Computed callback zero is not proof of an exact mathematical zero
   or of an economically acceptable transformed result.

6. **Retain deterministic bounded expansion with an explicit heuristic.**
   The initial search pair is max(min,guess-half_width) and
   min(max,guess+half_width), formed with safe bounded arithmetic. Require a
   representably nondegenerate initial pair; do not invent a larger width if
   the caller's requested pair collapses. The guess is only a centre, not an
   additionally sampled candidate. Check the initial pair before any expansion.
   On each expansion freeze the old interval width w and generate left=a-2*w,
   right=b+2*w, clipped only as algorithmic proposals to the explicit search
   bounds. Without clipping the total width grows by 5, not 2. Try the new left
   endpoint/crossing against its old same-side endpoint first, then right, then
   the full pair; never skip a bad left sample in favor of a right crossing.
   Reuse unchanged/clipped endpoint samples. Stop when no representable new
   endpoint exists, bounds are exhausted or max_expansions is consumed. Failure
   means no bracket found by this path, not proof that no root exists.

7. **Keep the existing coordinate stopping criterion exactly scoped.**
   Let b be the current best bracket endpoint and c its opposite-sign endpoint.
   Stop only on computed f(b)==0 or
   abs(c-b)/2 <= 2*EPSILON*abs(b)+tolerance/2, evaluated stably. Do not add
   abs(f(b))<=tolerance: x units and residual/currency units are different.
   On width-based termination, the remaining mathematical bracket width is at
   most 4*EPSILON*abs(b)+tolerance; the returned endpoint is not
   certified within the half-width of the true root. No portable true-root,
   residual, correct-rounding or financial error guarantee follows from the test.
   A computed-zero exit does not imply that width bound. Accepted record 017's
   100/1e-12 operational log-space controls and its independent
   returned-rate/gross-PV residual gate apply unchanged at that owner.

8. **Make every proposed step finite, safe and representably useful.**
   Preserve (b,c) as a genuine finite bracket and cache residuals with their exact
   coordinates. Use stable midpoint/distance/tolerance arithmetic instead of
   requiring an overflowing full span; neither positive width nor the absolute
   tolerance contribution may silently disappear and fabricate convergence.
   Secant/IQI are optional proposals subject to the existing bracket/progress
   safeguards. If a denominator, ratio, intermediate or proposal is unusable,
   underflows into a misleading zero, leaves the bracket or repeats b, reject
   that proposal and use the internal bisection step. This is not a callback
   failure recovery. If no new representable interior query exists and the
   stopping criterion still fails, return explicit stagnation/range failure.
   Do not query infinity or accept the last iterate merely because progress stopped.

9. **Count actual work and enforce stage budgets.** max_iterations counts new
   Brent candidate evaluations/updates, excluding cached endpoint checks and
   bracket-search samples. Apply the same convergence gate before the first
   update and after every accepted new evaluation, including the last permitted
   one, after restoring consistent bracket state. Otherwise exhaustion errors.
   Count actual changed-endpoint expansion rounds separately; initial success is
   zero expansions. With checked count arithmetic, bounded search needs at most
   2+2*max_expansions samples and Brent at most max_iterations further samples;
   do not re-evaluate handed-off bracket endpoints. No unbounded loop, hidden
   extra update, private retry budget or new generic callback-budget framework.

10. **Report truthful failures with matching coordinates/residuals.**
    Invalid controls/endpoints/search admission use input/domain errors. Callback
    Err preserves its source category; nonfinite callback output and unusable
    arithmetic are computation failures. No bracket, unmet convergence or
    representable stagnation use appropriate existing solver diagnostics.
    Record actual stage/evaluation/expansion/update counts and failing x where
    known; a reported residual must correspond to that x. Never pair the smaller
    endpoint residual with an unsampled guess or the other endpoint. Failed
    sampling is not evidence of root absence. Keep the existing scalar/error
    mechanisms rather than adding a persisted report/state schema.

11. **State continuity and root-selection limitations explicitly.**
    The owner supplies an economically admissible domain and an objective that
    is finite and continuous throughout the relevant bracket for the usual
    mathematical guarantee. The solver can check samples, not prove continuity,
    precision, absence of poles or uniqueness. Finite discontinuous sign changes
    can satisfy the width test with residual 1 and no true zero. Tangential/even
    roots, a root exactly at an unsampled centre and narrow/multiple crossings
    can be missed by bounded expansion. Deterministic lower-side priority is not
    nearest-to-guess, lowest, highest, unique or economically preferred root
    selection. Choose an explicit owner bracket/filter where those meanings matter.

12. **Keep numerical performance/accuracy claims conditional.**
    Safeguarded interpolation can accelerate suitable problems; remove universal
    fixed 1.618 convergence-order and stricter-than-bisection accuracy claims.
    Conditioning, floating objective error and x transforms can make residual
    and root error behave differently; callback values rounded/underflowed to
    zero cannot be repaired by bracket logic. Stable arithmetic and reference
    tests do not certify a domain-wide ULP bound or bitwise external-library
    parity. Financial residual/domain checks apply after any returned-value
    transformation, including all endpoint/computed-zero/width exits.

13. **Reconcile actual consumers while preserving their separate contracts.**
    Propagate explicit controls/search choices, canonical fallible supplied
    brackets and removed fields/builders/aliases through rates/yields/spreads,
    volatility/calibration, goal-seek, chi-squared and model/host callers together.
    Remove price-error sentinels, NaN adapters and retries that hide failures,
    rather than retain unchecked private solver copies. Their financial bounds,
    residual gates, objective transformations and retry/root-selection choices
    stay separately reviewed owner policy; a generic finite error value cannot
    establish a usable price. Preserve accepted record 017's canonical shared-Brent path
    and complete final gate without adding a private IRR solver. Keep Brent
    Rust-only and propagate errors through existing enclosing host mappings; no
    new standalone wrappers, objective-selection framework or migration layer.
    Acceptance does not execute or qualify any financial inversion/model.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| tolerance 0/negative/NaN/infinity or max_iterations 0 | Constructor/input error before callbacks, even if an endpoint would return zero. |
| Nonfinite supplied endpoint with constant-zero callback | Input error and no callbacks; infinity cannot be returned as a root. |
| Bracket supplied in reverse order | Canonical lower-then-upper sampling; same solve domain. |
| Equal finite endpoints, f=0 / f!=0 | One sample and computed-zero success / no valid bracket. |
| Finite opposite residuals eta and -eta | Preserve sign bracket even though their product underflows to zero. |
| Exact lower endpoint zero with valid initial upper sample | Lower candidate wins; still subject to the owner's final domain/residual gate. |
| initial_guess outside explicit bounds or invalid half-width | Error, no clamping/default width. |
| max_expansions 0 | Initial pair can succeed; otherwise no further search, no hidden 20 expansions. |
| Initial pair endpoints both round to the centre | Geometry failure; no silently enlarged width or fake bracket. |
| One unconstrained expansion from [-.1,.1] | New endpoints[-.5,.5], total-width factor 5. |
| New left search sample NaN and later right crossing would succeed | Abort at the left sample, without probing/accepting the right crossing. |
| f=x^2, search centred at 0 with nonzero endpoint samples | May miss its even root; the centre is not automatically evaluated. |
| Explicit [-MAX,+MAX],f=x/MAX | Stable midpoint 0 is available; avoid infinite-span bisection failure. |
| Nonfinite/outside/repeating secant or IQI proposal | Use internal bisection; do not evaluate a bad proposal. |
| No representable new point and unmet stopping test | Stagnation error, not last-iterate success. |
| Continuous f=x^2-2 on [0,2] | Sign-bracketed mathematical target sqrt(2), subject to numerical/owner qualification. |
| Step objective -1 for x<.5,+1 otherwise on [0,1] | Width termination may return residual 1 despite no root; continuity is not proved. |
| Small residual in arbitrary currency units | Not a generic stop unless computed zero or coordinate-width criterion passes. |
| Last permitted update satisfies the normal gate | Success without an uncounted extra update. |
| No bracket found within finite domain/budget | Search-path failure, not proof of no root or permission to change economic bounds. |
| Financial callback Err mapped to finite 1e12/NaN | Remove adapter and propagate the source error; sentinel shape is not the economic objective. |
| IRR/XIRR accepted fixed log bracket100/1e-12 plus returned-rate residual gate | Preserve that complete owner contract; do not substitute generic residual/tolerance claims. |

## Observed algorithm

1. Clamp a finite initial guess to configured bounds. Initial half-width is explicit if supplied; otherwise `min(0.01|x0|,1)`, replaced by 0.01 if the unbounded adaptive value is below `1e-6`.
2. Evaluate endpoint residuals once. Up to 20 expansions move each endpoint outward by twice the current full width, clipped to bounds. Test both a new endpoint against its previous same-side endpoint and the full pair, so a one-sided crossing is retained. Nonfinite residuals or absence of sign change error.
3. Maintain Brent `(a,b,c)` state with `b` the best residual endpoint and `(b,c)` bracketing the root. Reset the bracket when signs match and swap roles when `|fc|<|fb|`.
4. Stop on exact zero or half-bracket width≤`2 epsilon |b|+tolerance/2`. If previous progress and residual ordering permit, use secant when two points coincide or inverse quadratic interpolation otherwise. Accept the interpolation only under the two bracket/progress inequalities; otherwise bisect.
5. Enforce a minimum signed step of the same x tolerance, evaluate, reject nonfinite residual, and repeat. Exhaustion returns `SolverConvergenceFailed` with iteration count, last x, and residual. Callback errors propagate directly.

## Observed selection and controls

Defaults: x tolerance `1e-12`, max 100 iterations, search bounds [-1e6,1e6], adaptive initial size. Builders overwrite fields directly. `solve_in_bracket` bypasses search and configured search bounds; `solve_fallible` provides callback error propagation. No Newton fallback is implemented.

## Observed conventions

Tolerance is in x units, not residual/currency units. Residual magnitude is diagnostic only; convergence through bracket width does not demand a small residual. A discontinuous sign change can therefore satisfy the algorithmic termination without a true zero.

## Observed limitations, failure and fallback

Search validates bounds/initial guess/size, but tolerance is not validated as finite positive. Explicit-bracket evaluation does not independently reject nonfinite endpoints before calling the callback. Endpoint exact-zero handling can succeed before checking iteration budget or x tolerance. Even-multiplicity roots and narrow sign changes can be missed by expansion.

## Observed alternatives and tradeoffs

Brent is the only scalar root solver in this file. Bisection is an internal branch, not a separate adopted method. Newton, safeguarded Newton, monotonic domain-specific brackets and residual postchecks are possible caller strategies; no universal replacement is proposed.

## Evidence

- [Current solver/kernel](../../../../finstack-quant/core/src/math/solver.rs)
  exposes mutable default controls and separate infallible/fallible entry points.
  It accepts an unchecked infinite explicit endpoint if the callback returns 0,
  can bypass a newly nonfinite left search sample via a right crossing, and uses
  overflow-prone full-span/minimum-step arithmetic. Source field/return docs claim
  residual acceptance even though the loop tests exact computed zero or width.
- Inline known roots, bounded polynomials, one-sided bracket discovery, callback
  errors/nonfinite samples and exhaustion locate evidence. The current
  solve_fallible/solve comparison checks shared-kernel consistency, not external
  correctness or target APIs. [Integration solver tests](../../../../finstack-quant/core/tests/math/solver.rs)
  add located cases, not a production pass in this documentation review.
- [IRR/XIRR owner](../core-conventions/017-irr.md#accepted-decision-irr-and-xirr-timing-eligibility-and-numerical-acceptance)
  requires shared fallible fixed-bracket solving and its own finite returned-rate
  residual acceptance. The generic package does not replace that gate.
- [Bond OAS](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/oas.rs)
  and [term-loan tree OAS](../../../../finstack-quant/valuations/fixed-income/src/term_loan/pricing/tree_engine.rs)
  replace pricing failures with finite 1e12 objectives and retrieve the captured
  source error only if solving fails. These are located reconciliation targets,
  not a reproduced claim that either consumer accepted an incorrect price.
- [Goal seek](../../../../finstack-quant/statements-analytics/src/analysis/goal_seek.rs)
  has a NaN adapter and retry path as well as a final target-residual gate.
  [Chi-squared inversion](../../../../finstack-quant/core/src/math/distributions.rs)
  adapts a checked gamma failure to NaN and independently checks the final tail
  residual. Preserve each owner's final acceptance and legitimate objective
  transformations while replacing error-hiding adapters with fallible callbacks;
  neither existing gate qualifies the new solver contract by itself.
- [Rust-only numerical parity scope](../../../../finstack-quant-py/parity_contract.toml)
  leaves the solver unbound. Existing financial/model host entry points consume
  the shared errors; new standalone Brent host classes are not required.

[Brent's author publication page](https://maths-people.anu.edu.au/~brent/pub/pub005.html)
identifies the bracket/interpolation/bisection method and conditional acceleration.
[Netlib zeroin](https://netlib.org/go/zeroin.f) exhibits the retained coordinate
termination formula; its historical raw arithmetic is not approval to preserve
finite-range bugs. [SciPy brentq documentation](https://docs.scipy.org/doc/scipy/reference/generated/scipy.optimize.brentq.html)
states continuity/sign assumptions and distinct controls, not bitwise parity or
this library's exact budget/endpoint policy. These primary sources were consulted
on 2026-11-07. [Brent 1973](../../../REFERENCES.md#brent-1973) remains the canonical
background, not a financial root/domain/residual selection rule.

Independent exact-rational and binary64 expression checks on 2026-11-07 confirmed
the factor-5 expansion geometry, opposite subnormal signs despite a zero residual
product, the nonfinite-left/later-right crossing example, a stable zero midpoint
despite overflowing full span, and a narrow discontinuous bracket that passes
the width criterion with residual magnitude 1. They also separated a computed-zero
exit from the width bound and demonstrated the underflow of a naively halved
smallest-positive tolerance. These checks validate examples and mathematical
implications, not the Rust Brent implementation or an all-domain accuracy claim.

Production Rust/host/model tests were located or inspected, not executed. Callback,
search, finite-range, work/diagnostic and actual financial consumer qualification
remain pending; agreement on this package is not runtime execution or a universal
root/price accuracy certificate.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
