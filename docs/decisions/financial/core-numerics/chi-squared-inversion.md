# CORE-NUMERICS-15: Chi-squared quantiles through logarithmic tail inversion

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/distributions.rs::chi_squared_quantile`: p∈[0,1), finite df>0, with numerical df ceiling 1e8 for nonzero probabilities. Inputs/outputs are dimensionless. A p=0 call returns 0 before the ceiling check.

## Choice and rationale

Solve for log(x), comparing log lower-tail mass for p≤0.5 and log survival for the upper tail. This avoids absolute probability tolerances that would erase tiny tails and avoids negative iterates. The numerical ceiling is an implementation domain restriction, not a financial distribution choice.

## Algorithm

1. Validate p and df; p=0 returns zero. Set a=df/2; if rounding makes a=0, return zero. Compute log Gamma(a+1), requiring finiteness.
2. With y=x/2, evaluate log lower CDF using the recurrence `P(a,y)=exp(a log(y)−y−logGamma(a+1))+P(a+1,y)`. Combine the two positive terms by log-sum-exp; use the first term alone when shifted mass underflows, and cap the log result at 0.
3. Lower-tail residual is `log P(a,x/2)−log(p)`. Upper-tail residual is `log(1−p)−log Q(a,x/2)`; for y≤1 compute survival by `−expm1(log P)`, otherwise use statrs' regularized upper Gamma. Clamp residual to [-1000,1000].
4. Lower bracket is `log(min_subnormal)−log(2)`. If residual there is nonnegative, return zero as rounded-underflow output. Upper bracket is `log(df+64 sqrt(df)+128)`.
5. Brent solves in log-space with x tolerance 1e-13 and max 256 iterations. Exponentiate; require finite result and finite final log-tail residual≤1e-8 in magnitude. Solver failures are wrapped as validation errors.

## Selection and controls

All controls are internal and fixed. No inverse-CDF approximation is selected by df. The shared [Brent record](brent-root.md) governs its bracket-width stopping, but this caller adds a residual postcheck.

## Limitations, failure and fallback

p=1 and NaN/out-of-range p error. Very small quantiles may return zero; this is explicit representability handling, not an atom at zero in the continuous distribution. A valid mathematical quantile outside the guarded numerical domain errors. Accuracy still depends on statrs incomplete-Gamma evaluation and finite log residual clamping.

## Alternatives and tradeoffs

Direct x-space inversion, Wilson–Hilferty approximations and dependency inverse CDFs are not used. Log inversion costs repeated special-function evaluations but preserves relative tail information. It cannot express probabilities closer to one than f64 can represent.

## Evidence

- [Implementation and inline tests](../../../../finstack-quant/core/src/math/distributions.rs), [Brent reference](../../../REFERENCES.md#brent-1973).
- `chi_squared_quantile_matches_exponential_closed_form_in_both_tails` uses df=2, x=−2 log1p(−p), 1e-11 relative tolerance over p down to 1e-300. df=4 lower-tail tests use a separately coded Taylor expansion (1e-10 relative mass). Fractional-df tests use transformed midpoint quadrature but share log-Gamma; upper-tail checks share statrs functions. Distinguish these independence levels.

Tests do not exhaust the continuous df/probability domain, even where a special-case reference is independent.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
