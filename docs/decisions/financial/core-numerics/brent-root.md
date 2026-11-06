# CORE-NUMERICS-05: Bracket search and Brent scalar root finding

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/solver.rs`: scalar `f(x)=0` with infallible or fallible callbacks. Initial-guess search requires finite ordered bounds; explicit-bracket solve sorts endpoints. Financial units and monotonicity come from the callback.

## Choice and rationale

Brent combines bracket preservation with secant and inverse quadratic interpolation, falling back to bisection when an interpolated step is unsafe. It avoids derivatives and requires a sign-changing interval or exact endpoint root. Search does not establish root uniqueness.

## Algorithm

1. Clamp a finite initial guess to configured bounds. Initial half-width is explicit if supplied; otherwise `min(0.01|x0|,1)`, replaced by 0.01 if the unbounded adaptive value is below `1e-6`.
2. Evaluate endpoint residuals once. Up to 20 expansions move each endpoint outward by twice the current full width, clipped to bounds. Test both a new endpoint against its previous same-side endpoint and the full pair, so a one-sided crossing is retained. Nonfinite residuals or absence of sign change error.
3. Maintain Brent `(a,b,c)` state with `b` the best residual endpoint and `(b,c)` bracketing the root. Reset the bracket when signs match and swap roles when `|fc|<|fb|`.
4. Stop on exact zero or half-bracket width≤`2 epsilon |b|+tolerance/2`. If previous progress and residual ordering permit, use secant when two points coincide or inverse quadratic interpolation otherwise. Accept the interpolation only under the two bracket/progress inequalities; otherwise bisect.
5. Enforce a minimum signed step of the same x tolerance, evaluate, reject nonfinite residual, and repeat. Exhaustion returns `SolverConvergenceFailed` with iteration count, last x, and residual. Callback errors propagate directly.

## Selection and controls

Defaults: x tolerance `1e-12`, max 100 iterations, search bounds [-1e6,1e6], adaptive initial size. Builders overwrite fields directly. `solve_in_bracket` bypasses search and configured search bounds; `solve_fallible` provides callback error propagation. No Newton fallback is implemented.

## Conventions

Tolerance is in x units, not residual/currency units. Residual magnitude is diagnostic only; convergence through bracket width does not demand a small residual. A discontinuous sign change can therefore satisfy the algorithmic termination without a true zero.

## Limitations, failure and fallback

Search validates bounds/initial guess/size, but tolerance is not validated as finite positive. Explicit-bracket evaluation does not independently reject nonfinite endpoints before calling the callback. Endpoint exact-zero handling can succeed before checking iteration budget or x tolerance. Even-multiplicity roots and narrow sign changes can be missed by expansion.

## Alternatives and tradeoffs

Brent is the only scalar root solver in this file. Bisection is an internal branch, not a separate adopted method. Newton, safeguarded Newton, monotonic domain-specific brackets and residual postchecks are possible caller strategies; no universal replacement is proposed.

## Evidence

- [Solver](../../../../finstack-quant/core/src/math/solver.rs); [Brent reference](../../../REFERENCES.md#brent-1973).
- Inline `test_brent_solver` and bounded polynomial cases: known mathematical roots. `test_brent_solve_fallible_matches_solve_bit_for_bit`: shared-kernel consistency. `test_brent_find_bracket_catches_one_sided_sign_change`, nonfinite-callback and exhaustion cases: branch/error evidence.
- [Integration solver tests](../../../../finstack-quant/core/tests/math/solver.rs) provide additional located use cases; no tests were executed.

The generic solver does not determine financial residual acceptance, admissible root ranges or root-selection policy.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
