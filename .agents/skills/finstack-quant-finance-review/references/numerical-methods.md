# Numerical Methods Reference

Detailed review criteria for numerical computing in quant finance.

## Floating-Point Arithmetic

### Invalid numeric states

- Reject or quarantine `NaN`, infinite, negative-discount-factor, negative-variance, and invalid-probability states at module boundaries.
- Flag `return f64::NAN` in pricing/risk paths unless the API explicitly models missing numeric output and every caller handles it.
- Rust: flag `partial_cmp(...).unwrap()` on floats. Use explicit NaN handling or `total_cmp` only when the chosen NaN ordering is financially acceptable.
- Treat solver failure as structured diagnostics with method, iterations, bracket/bounds, residual, and inputs needed for replay.

### Catastrophic Cancellation

When subtracting nearly equal numbers, relative error explodes. Common in:

- **Forward vs. central differences for Greeks**: `(f(x+h) - f(x))/h` loses precision when h is too small. Prefer central differences `(f(x+h) - f(x-h))/(2h)` or complex-step differentiation.
- **Variance computation**: Never use `E[X²] - (E[X])²` for Monte Carlo. Use Welford's online algorithm or two-pass computation.
- **Near-ATM options**: `max(S-K, 0)` when S ≈ K — use intrinsic value plus time value decomposition.

### Accumulation Errors

- Summing millions of small P&L values: use Kahan compensated summation or pairwise summation.
- Running averages in streaming contexts: use numerically stable incremental formulas.
- Flag any tight loop accumulating floats without compensation.

### Comparison and Branching

- Never use `==` for float comparison. Define domain-appropriate epsilon.
- `NaN != NaN` — ensure NaN checks use `is_nan()` / `np.isnan()` / `f64::is_nan()`.
- Check for `-0.0` vs `+0.0` in sign-sensitive calculations (e.g., barrier options).
- Rust: Watch for `f64::partial_cmp` returning `None` on NaN — handle explicitly.

### Overflow and Underflow

- `exp(x)` overflows for x > 709.78 (f64). Use log-space computation for likelihoods and large exponents.
- Deep OTM option probabilities underflow to 0. Use log-probabilities or complementary error functions.
- Factorial and gamma function: use `lgamma` / `log_gamma` for large arguments.
- WASM: Verify that JS `Math.exp()` and Rust `f64::exp()` behave identically at boundaries.

## Root-Finding and Optimization

### Implied Volatility Solvers

- **Newton-Raphson**: Converges quadratically but can diverge for extreme strikes. Check that vega (derivative) is not near zero before dividing. Limit iterations (typically 20 max). Use Brenner-Subrahmanyam or Corrado-Miller for initial guess.
- **Brent's method**: More robust for bracketed problems. Ensure bracket validity: `f(a) * f(b) < 0`.
- **Rational approximation**: For speed-critical paths, use Li (2006) or Jäckel (2015) rational approximation. Verify accuracy bounds.

### Calibration Optimization

- Check that the objective function is well-scaled (normalize by vega or relative error, not absolute price error).
- Verify parameter bounds are enforced (vol > 0, correlation ∈ [-1,1], mean reversion > 0).
- For multi-dimensional calibration (e.g., Heston), check for local minima — run from multiple starting points.
- Regularization: is there a penalty for extreme parameters? Without it, calibration can produce unstable surfaces.
- Gradient computation: if using adjoint/automatic differentiation, verify chain rule correctness.

## Monte Carlo Methods

### Random Number Generation

- Verify RNG is seeded for reproducibility. Flag `thread_rng()` (Rust) or `np.random.random()` (Python) without explicit seed.
- Check that the RNG has sufficient period for the simulation size. Mersenne Twister is fine for most cases; for >10⁹ paths, use a 64-bit generator.
- Low-discrepancy sequences (Sobol, Halton): verify correct dimensionality, scrambling, and that the skip parameter matches the application.

### Variance Reduction

- **Antithetic variates**: Check that `Z` and `-Z` are correctly paired through the entire path, not just the terminal value.
- **Control variates**: Verify the control variate has a known analytical expectation. Check that the beta coefficient is estimated from a pilot run, not hardcoded.
- **Importance sampling**: Verify the Radon-Nikodym derivative is correctly applied. Check that the proposal distribution has heavier tails than the target.
- **Stratified sampling**: Ensure strata boundaries are correct and inverse CDF is numerically stable at extremes.

### Path Simulation

- **Euler-Maruyama**: Check drift and diffusion terms match the SDE. Verify that `√dt` (not `dt`) multiplies the Brownian increment.
- **Log-Euler for GBM**: Use `log(S)` dynamics to avoid negative stock prices. Verify the Itô correction term `-0.5σ²dt` is present.
- **Milstein scheme**: Check that the derivative of the diffusion coefficient is correct. For multidimensional SDEs, verify the Lévy area terms.
- **Correlation**: Cholesky decomposition of the correlation matrix — verify the matrix is positive semi-definite before decomposition. Check that factor ordering is consistent.

## PDE Methods

### Finite Differences

- **Stability**: For explicit schemes, verify CFL condition `dt ≤ dx²/(2D)`. Flag any explicit scheme without stability check.
- **Theta scheme**: θ=0 (explicit), θ=0.5 (Crank-Nicolson), θ=1 (fully implicit). Crank-Nicolson can oscillate near discontinuities — use Rannacher timestepping (implicit for first few steps).
- **Grid design**: Non-uniform grids should concentrate points near strike, barriers, and boundaries. Check that grid stretching doesn't introduce excessive interpolation error.
- **Boundary conditions**: Verify asymptotic boundary conditions are correct (e.g., call → S - Ke^{-rT} as S → ∞). Check that boundary handling doesn't create artificial reflections.
- **American exercise**: For PSOR (projected SOR), verify the projection step `max(V, payoff)` is applied at each iteration, not just at convergence.

## Interpolation and Extrapolation

- **Vol surface**: Check interpolation method (linear in variance, cubic spline, SABR). Verify no-arbitrage constraints (butterfly ≥ 0, calendar spread ≥ 0).
- **Yield curves**: Check day-count conventions match the instrument. Verify bootstrap is consistent (discount factors are monotonically decreasing).
- **Extrapolation**: Flag any linear extrapolation beyond the data range. For vol surfaces, use flat extrapolation or a parametric wing model.
- **Cubic spline**: Verify boundary conditions (natural, not-a-knot, clamped). Check for oscillation in sparse data regions.
