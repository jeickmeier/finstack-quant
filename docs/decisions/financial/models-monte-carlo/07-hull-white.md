# MODELS-MONTE-CARLO-07: Hull–White exact transition and curve-fitted theta levels

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in process/ou.rs and discretization/exact_hw1f.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and input domain

The short rate follows `dr=kappa(theta(t)-r)dt+sigma(t)dW`, with positive kappa, piecewise constant nonnegative volatility validated by shared Hull–White parameters, and finite theta knots. Here theta is a mean-reversion level, not the additive drift. Times are years, rates decimals. Negative rates are permitted. Exact transition is default; Euler is explicit.

## Algorithm and embedded curve fitting

Exact mean is `r exp(-kappa dt)+integral kappa theta(u) exp(-kappa(t+dt-u))du`; each theta segment is integrated with exponential weights. Variance integrates piecewise sigma² with the squared exponential kernel. Constant sigma uses `sigma sqrt((1-exp(-2kappa dt))/(2kappa))`, with a first-order small-kappa-dt expansion below 1e-8. Grid-first-step constants are cached; unequal dt recomputes them.

`calibrate_theta_from_curve` estimates instantaneous forwards via shared finite differences of positive DFs. It constructs the curve shift alpha(t): constant sigma uses `f(0,t)+sigma²(1-exp(-kappa t))²/(2kappa²)`; piecewise sigma uses the difference of single/double exponential variance integrals divided by kappa. Each theta level solves `alpha_end=alpha_start exp(-kappa width)+theta(1-exp(-kappa width))`. Positive-first knots are interval ends; zero-first knots are interval starts and an extra last interval width is extrapolated (one year if only one knot). Empty constant-sigma theta times return one constant initial forward level.

## Failure behavior and alternatives

Failed forward samples error, rather than installing zero drift. Low-level `sigma_variance_for_step` returns NaN on invalid times; the exact step takes `.max(0)` before its square root, which can suppress that NaN under Rust f64 semantics. The empty-theta constant-sigma shortcut does not construct the full time-dependent convexity shift. Curve-shift interpolation and finite-difference settings are delegated dependencies. This engine exposes a rate; it does not automatically integrate a bank account or price under pathwise discounting. Alternatives are finer theta grids or the separate rates-domain HW engine, not an assertion that scalar discounting is valid.

## Evidence

[Theta fitting and process](../../../../finstack-quant/models/src/monte_carlo/process/ou.rs), [exact stepping](../../../../finstack-quant/models/src/monte_carlo/discretization/exact_hw1f.rs). Tests cover flat-curve shift, forward jumps, multiple theta knots, integrated piecewise volatility and exact-step composition. [Simulation moments](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs) compare Gaussian mean/variance to explicit formulas. [Hull–White 1990](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives) and [Brigo–Mercurio](../../../REFERENCES.md#brigo-mercurio-2006-interest-rate-models) are model references; the located checks do not establish curve repricing accuracy for every theta grid.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
