# CORE-NUMERICS-04: Fixed and adaptive numerical integration

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/integration.rs`: scalar quadrature on finite intervals and standard-normal expectations. Integrands return `f64`; no product or financial measure is inferred.

## Choice and rationale

Explicit Simpson, Gauss–Legendre and Gauss–Hermite APIs support different domains. Adaptive routines split a global absolute error budget across children and reject unmet total estimated error. Fixed Gaussian tables avoid repeated node solving; they are not adaptive accuracy certificates.

## Algorithm

1. Composite Simpson requires positive even interval count `n`, uses spacing `(b-a)/n`, endpoint weight 1, odd weight 4, even interior weight 2, and multiplies by `h/3`.
2. Adaptive Simpson starts with one Simpson panel, compares it with two half panels, estimates error `|I_halves-I_whole|/15`, and recursively halves both interval and tolerance. It returns the two-panel estimate without Richardson correction. Leaf errors are summed; success requires finite value and total estimated error≤tol after recursion.
3. Gauss–Legendre uses stored orders 2,4,8,16 on [-1,1], mapped by midpoint `a+(b-a)/2` and half-width. Composite integration builds each panel's transformed `(x,w)` grid and sums `w f(x)`.
4. Adaptive Gauss–Legendre compares one panel against two half panels, estimates `|I_halves-I_whole|` (no order-based divisor), recursively halves tolerance, and enforces the summed error at return.
5. Gauss–Hermite supports orders 5,7,10,15,20. Its tables integrate the `exp(-z²)` weight; `integrate(f)` computes `Σ w_i f(sqrt(2)z_i)/sqrt(pi)` with Neumaier accumulation, hence returns `E[f(Z)]` for standard normal Z.

## Selection and controls

Order/panel count/tolerance/depth are explicit arguments. Fixed Simpson validates only even positive count. Adaptive bounds and tolerance must be finite with tol≥0; max depth is supplied. Composite Legendre requires `b>a` and at least one panel; the internal single-panel evaluator permits reversed bounds and returns 0 for equal bounds. Unsupported Gaussian order errors.

## Conventions

Finite-interval outputs have integrand units times integration-variable units. Hermite includes the standard-normal density normalization already; callers must not multiply it by a second normal PDF. Tolerances are absolute integral units.

## Limitations, failure and fallback

Adaptive rules stop subdividing on nonfinite error, depth exhaustion or sufficiently small local error, then return a convergence error unless the aggregate estimate passes. Sampled error estimates can miss narrow/discontinuous features. Fixed rules do not uniformly validate integrand values and may return `Ok(NaN)`/infinity; their accumulation is ordinary except Hermite. No automatic fallback between integration families exists.

## Alternatives and tradeoffs

The supported families are explicit alternatives. Higher-order/dynamic Gaussian rules, Gauss–Kronrod error estimates, and splitting at known payoff discontinuities are possible additions, not present automatic behavior. Product callers must decide error budgets and endpoint treatment.

## Evidence

- [Implementation and Gaussian tables](../../../../finstack-quant/core/src/math/integration.rs).
- [Integration tests](../../../../finstack-quant/core/tests/math/integration.rs): `gauss_hermite_supported_orders_match_standard_normal_moments` compares moments 1,0,1,3,15; polynomial/sine tests use analytic integrals. `production_credit_audit_simpson_uses_total_error_budget` and max-depth tests inspect failure semantics; method comparisons are not independent pricing benchmarks.
- [Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes); [Neumaier/Kahan context](../../../REFERENCES.md#higham-accuracy-and-stability). Exact node-table source provenance is not independently established here.

Fixed quadrature does not provide certified error bounds. Estimated tolerance satisfaction is distinct from true integration error.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
