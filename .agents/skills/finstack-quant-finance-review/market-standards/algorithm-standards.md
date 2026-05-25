# Algorithm Standards for Quantitative Finance

## Curve Interpolation Methods

### Professional library defaults

| Library | Discount curve | Forward curve | Vol surface |
|---------|---------------|---------------|-------------|
| QuantLib | Log-linear on DF | Linear on zero | Bilinear |
| Bloomberg | Monotone convex | Monotone convex | SABR |
| Numerix | Cubic spline | Cubic spline | SABR/SVI |
| FinCad | Various | Various | Various |

### Interpolation method selection

| Method | Use case | Pros | Cons |
|--------|----------|------|------|
| Linear on zero | Quick approximation | Simple | Discontinuous forwards |
| Log-linear on DF | Standard choice | Stable DF interpolation | Forward shape may be too simple |
| Monotone convex | Production curves | Smooth, shape-preserving | More complex |
| Cubic spline | General | C² continuous | May oscillate |
| Tension spline | Avoiding overshoot | Controlled | Extra parameter |

### Monotone convex (Hagan-West rates-curve interpolation)

```
Key properties:
1. Produces a smooth, shape-preserving discount / forward curve
2. Targets smooth, shape-preserving forward curves
3. C¹ continuous (first derivative continuous)

Implementation (Hagan-West algorithm):
- Compute discrete forwards f_i from discount factors
- Apply monotonicity and shape filters to avoid oscillatory forwards
- Construct piecewise quadratic interpolant

Do not treat this as an "ISDA CDS standard". CDS standard modeling is a separate hazard/survival bootstrapping problem.
```

### Audit checklist - Interpolation

- [ ] Discount factors are consistent with the input curve and rate regime
- [ ] Do not enforce monotonically decreasing discount factors when valid negative-rate curves imply DF > 1
- [ ] No negative forward rates (unless intentional)
- [ ] Extrapolation policy explicit (flat, linear, none)
- [ ] Boundary conditions handled (at t=0, beyond max date)
- [ ] Round-trip: DF(0) = 1.0 always

---

## Root-Finding Algorithms

### Professional defaults

| Problem | Algorithm | Tolerance | Max iterations |
|---------|-----------|-----------|----------------|
| Yield from price | Newton-Raphson | 1e-10 | 100 |
| Implied vol | Safeguarded Newton / Jaeckel / Brent fallback | 1e-8 | 20-50 |
| Curve bootstrap | Newton-Raphson | 1e-12 | 100 |
| IRR calculation | Brent | 1e-10 | 100 |

### Newton-Raphson (standard for smooth functions)

```
x_{n+1} = x_n - f(x_n) / f'(x_n)

Convergence: Quadratic when close to root
Initial guess: Critical for convergence

For yield: Start with coupon rate or 5%
For implied vol: Use arbitrage bounds plus a robust initial guess; prefer Jaeckel-style logic or safeguarded Newton/Brent
```

### Brent's method (bracketing, guaranteed convergence)

```
Combines:
- Bisection (slow but guaranteed)
- Secant method (fast but may fail)
- Inverse quadratic interpolation

Use when:
- Function may not be smooth
- Derivative not available
- Need guaranteed convergence

QuantLib: Brent is default for generic solvers
```

### Implied volatility initial guess

```
Jaeckel (industry standard):
See "Let's Be Rational" - provides near-optimal guess

Practical desk-standard guidance:
- Use Jaeckel / rational approximation when available
- Otherwise run bounded Newton with a Brent fallback and arbitrage-checked brackets
- Treat simple closed-form approximations as near-ATM heuristics only, not universal production formulas
```

### Audit checklist - Root-finding

- [ ] Convergence tolerance appropriate (1e-8 to 1e-12)
- [ ] Max iterations reasonable (50-100)
- [ ] Initial guess uses domain knowledge
- [ ] Handles non-convergence gracefully
- [ ] Validated on edge cases (deep ITM/OTM, near expiry)

---

## Monte Carlo Methods

### Professional standards

| Component | Standard | Notes |
|-----------|----------|-------|
| RNG | Mersenne Twister or Sobol | Sobol for < 1000 dims |
| Variance reduction | Antithetic + control variate | Standard practice |
| Path count | 10,000 - 1,000,000 | Depends on accuracy needs |
| Time steps | Match payment dates + extra | 50-250 typical |

### Euler-Maruyama discretization

```
dS = μ S dt + σ S dW

Euler: S_{t+Δt} = S_t × (1 + μ Δt + σ √Δt × Z)

Better: S_{t+Δt} = S_t × exp((μ - σ²/2) Δt + σ √Δt × Z)

The exponential form is the log-Euler / exact GBM step and prevents negative prices
```

### Sobol sequences (QuantLib/Bloomberg standard)

```
Use Sobol for:
- Dimension < 1000
- Smooth payoffs
- Greeks calculation

Benefits:
- Better convergence (O(1/N) vs O(1/√N))
- More uniform coverage

Direction numbers: Joe-Kuo (QuantLib default)
```

### Variance reduction

```
Antithetic variates:
V = (1/2) × [V(Z) + V(-Z)]
Reduces variance by ~50% for monotonic payoffs

Control variates:
V_cv = V + c × (X - E[X])
c = -Cov(V, X) / Var(X)
X = some correlated variate with known mean

For options: Use forward as control
For options: Prefer a closely related closed-form instrument as control variate
```

### Audit checklist - Monte Carlo

- [ ] RNG is Mersenne Twister or better (not LCG)
- [ ] Sobol used when appropriate (low dimension)
- [ ] Variance reduction implemented (antithetic minimum)
- [ ] Path count justified for accuracy requirement
- [ ] Convergence verified (doubling paths halves std error)

---

## Finite Difference Methods

### Grid construction (professional standard)

| Parameter | Typical value | Notes |
|-----------|---------------|-------|
| Spot steps | 100-500 | More near strike |
| Time steps | 50-250 | Match vol term structure |
| Spot range | 0 to 5×S | Wide enough for gamma |
| Concentration | √x or sinh | More points near ATM |

### Schemes

| Scheme | Stability | Accuracy | Use case |
|--------|-----------|----------|----------|
| Explicit | Conditional | O(Δt, Δx²) | Simple, small Δt |
| Implicit | Unconditional | O(Δt, Δx²) | Production |
| Crank-Nicolson | Unconditional | O(Δt², Δx²) | Standard choice |
| ADI (Alternating Direction) | For multi-dim | O(Δt, Δx²) | 2D+ problems |

### Crank-Nicolson (industry standard)

```
θ-scheme: θ = 0.5 (Crank-Nicolson)

V^{n+1} - V^n = θ × L[V^{n+1}] + (1-θ) × L[V^n]

where L is the differential operator

For American options: Combine with PSOR (Projected SOR)
```

### Boundary conditions

```
Vanilla call:
- S → 0: V → 0
- S → ∞: V → S - K × e^{-r(T-t)}  (linear)
- t = T: V = max(S - K, 0)

Vanilla put:
- S → 0: V → K × e^{-r(T-t)}
- S → ∞: V → 0
- t = T: V = max(K - S, 0)
```

### Audit checklist - Finite difference

- [ ] Grid concentrated near strike/barrier
- [ ] Crank-Nicolson or implicit (not explicit)
- [ ] Boundary conditions correct for payoff type
- [ ] American exercise uses PSOR or similar
- [ ] Greeks from finite difference on grid (not bumped reprice)

---

## Calibration Algorithms

### Vol surface calibration (SABR)

```
SABR model:
dF = α F^β dW_1
dα = ν α dW_2
E[dW_1 dW_2] = ρ dt

Hagan approximation for implied vol:
σ_B(K, F) = α / (FK)^((1-β)/2) × {z / x(z)} × {...}

Calibration: Minimize Σ [σ_market - σ_model]²
```

### Heston calibration

```
Heston model:
dS = μ S dt + √v S dW_1
dv = κ(θ - v) dt + σ √v dW_2
E[dW_1 dW_2] = ρ dt

Parameters: v₀, κ, θ, σ, ρ
Constraints: 2κθ > σ² (Feller condition)

Calibration: Levenberg-Marquardt on option prices
```

### Optimization algorithms

| Algorithm | Use case | Notes |
|-----------|----------|-------|
| Levenberg-Marquardt | Least squares | Standard for vol calibration |
| BFGS | General | Quasi-Newton |
| Nelder-Mead | Non-smooth | Derivative-free |
| Differential Evolution | Global | Avoids local minima |

### Audit checklist - Calibration

- [ ] Objective function uses correct weights (vega or 1/vega)
- [ ] Parameter bounds enforced (vol > 0, etc.)
- [ ] Feller condition checked for Heston
- [ ] Multiple starting points tried (global optimization)
- [ ] Calibration error reported (RMSE in vol terms)

---

## Greeks Calculation

### Bump-and-reprice standards

| Greek | Bump type | Size | Method |
|-------|-----------|------|--------|
| Delta | Spot | Configurable numerical step; smaller than desk shock for local Greeks | Central |
| Gamma | Spot | Configurable numerical step matched to payoff smoothness | Central second deriv |
| Vega | Vol | Configurable by quoting convention and surface units | Central |
| Theta | Time | 1 business day or desk-standard carry horizon | Forward |
| Rho | Rate | 1 bp typical reporting shock; numerical step may differ | Central |
| DV01 | Curve | 1 bp reporting shock; numerical step configurable | Central |

### Central difference formulas

```
First derivative (delta, vega, rho):
∂V/∂x ≈ [V(x+h) - V(x-h)] / (2h)
Error: O(h²)

Second derivative (gamma):
∂²V/∂x² ≈ [V(x+h) - 2V(x) + V(x-h)] / h²
Error: O(h²)

Cross derivative (vanna, volga):
∂²V/∂x∂y ≈ [V(x+h,y+k) - V(x+h,y-k) - V(x-h,y+k) + V(x-h,y-k)] / (4hk)
```

### Adjoint algorithmic differentiation (AAD)

```
For portfolios with many Greeks:
- AAD computes all Greeks in O(1) × pricing cost
- Bump-and-reprice is O(n) × pricing cost

Professional libraries increasingly use AAD:
- QuantLib: XAD integration available
- Numerix: Built-in AAD
- Bloomberg: AAD for production risk
```

### Audit checklist - Greeks

- [ ] Central differences used (not forward)
- [ ] Numerical differentiation step is distinct from desk risk-reporting shock
- [ ] Bump sizes appropriate for product smoothness and market quoting units
- [ ] Gamma/Vega stable (not noisy)
- [ ] Greeks consistent with no-arbitrage
- [ ] For production: Consider AAD for speed

---

## Common algorithm errors

### 1. Forward difference instead of central

```rust
// WRONG: Forward difference (O(h) error)
let delta = (pv_up - pv_base) / bump;

// CORRECT: Central difference (O(h²) error)
let delta = (pv_up - pv_down) / (2.0 * bump);
```

### 2. Linear interpolation on discount factors

```rust
// WRONG: Linear interpolation on DF
let df = df1 + (df2 - df1) * t_frac;  // Can produce negative forwards

// CORRECT: Log-linear interpolation
let df = (df1.ln() + (df2.ln() - df1.ln()) * t_frac).exp();
```

### 3. Euler discretization for GBM

```rust
// WRONG: Euler (can go negative)
let s_next = s * (1.0 + mu * dt + sigma * sqrt_dt * z);

// CORRECT: Log-Euler (always positive)
let s_next = s * ((mu - 0.5 * sigma * sigma) * dt + sigma * sqrt_dt * z).exp();
```

### 4. Poor implied vol initial guess

```rust
// WRONG: Fixed initial guess
let vol_init = 0.2;

// CORRECT: Use a production-grade initial guess with bounded solver fallback
let vol_init = jaeckel_initial_guess(forward, strike, option_price, t);
```
