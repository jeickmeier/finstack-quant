# Closed-Form Pricing Models

Analytical and semi-analytical pricing formulas for European-style options and derivatives. Every formula is implemented with academic citations, robust edge-case handling, and comprehensive test coverage.

## Purpose

This module serves two roles:

1. **Production pricing** -- fast, exact valuations where closed-form solutions exist.
2. **Monte Carlo validation** -- reference prices and Greeks used to verify numerical engines.

All functions use continuous compounding, continuous dividend yields, and return per-unit (not contract-scaled) values.

---

## Module Structure

```
closed_form/
├── mod.rs          # Public re-exports and module-level documentation
├── vanilla.rs      # Black-Scholes / Garman-Kohlhagen pricing & Greeks
├── greeks.rs       # Standalone analytical Greek functions (delta, gamma, vega, theta, rho)
├── asian.rs        # Asian options (geometric exact, arithmetic Turnbull-Wakeman)
├── barrier.rs      # Barrier options (continuous monitoring, all 8 types)
├── lookback.rs     # Lookback options (fixed and floating strike)
├── quanto.rs       # Quanto options (cross-currency drift adjustment)
├── heston.rs       # Heston stochastic volatility via Fourier inversion
├── implied_vol.rs  # Implied volatility solvers (BS and Black-76)
└── README.md
```

### Dependency Graph

```
vanilla ◄──── greeks        (BsGreeks struct, d1/d2 helpers)
  │              │
  └──────────────┴──── implied_vol   (bs_price for objective, bs_vega for Newton)
                       asian          (internal vanilla_call_bs / vanilla_put_bs)
                       barrier        (self-contained BS-like expressions)
                       lookback       (self-contained, reflection principle)
                       quanto         (BS with adjusted drift)
                       heston         (Fourier; falls back to BS when σ_v ≈ 0)
```

---

## Models

### Black-Scholes / Garman-Kohlhagen (`vanilla.rs`, `greeks.rs`)

European call and put pricing with analytical Greeks under constant volatility.

| Function | Description |
|----------|-------------|
| `bs_price` | European call/put price |
| `bs_greeks` | All first-order Greeks in one call (returns `BsGreeks`) |
| `bs_call_delta`, `bs_put_delta` | Delta |
| `bs_gamma` | Gamma (same for calls and puts) |
| `bs_vega` | Vega per 1% vol change |
| `bs_call_theta`, `bs_put_theta` | Theta (annualized; divide by `theta_days_per_year`) |
| `bs_call_rho`, `bs_put_rho` | Rho per 1% rate change |
| `bs_call_greeks`, `bs_put_greeks` | Bundled Greeks including `rho_q` |

**Formulas:**

```
C = S·e^(-qT)·N(d₁) - K·e^(-rT)·N(d₂)
P = K·e^(-rT)·N(-d₂) - S·e^(-qT)·N(-d₁)

d₁ = [ln(S/K) + (r - q + σ²/2)T] / (σ√T)
d₂ = d₁ - σ√T
```

**`BsGreeks` struct** includes `delta`, `gamma`, `vega`, `theta`, `rho_r`, and `rho_q`, with `is_valid()` bounds checking and `clamped()` for numerical noise.

**References:**
- Black & Scholes (1973), *Journal of Political Economy*
- Merton (1973), *Bell Journal of Economics*
- Garman & Kohlhagen (1983), *Journal of International Money and Finance*

---

### Asian Options (`asian.rs`)

Average-price options with closed-form (geometric) and approximate (arithmetic) solutions.

| Function | Description |
|----------|-------------|
| `geometric_asian_call` / `_put` | Exact geometric average pricing (Kemna & Vorst 1990) |
| `geometric_asian_call_df` / `_put_df` | DF-first variant |
| `arithmetic_asian_call_tw` / `_put_tw` | Turnbull-Wakeman moment-matching approximation |
| `arithmetic_asian_call_tw_df` / `_put_tw_df` | DF-first variant |

**Geometric average** (exact): the log of the geometric average is normally distributed, so pricing reduces to Black-Scholes with adjusted volatility:

```
σ_G = σ √[(2n + 1) / (6(n + 1))]     (discrete, n fixings)
σ_G = σ / √3                           (continuous limit)
```

**Arithmetic average** (Turnbull-Wakeman): matches the first two moments of the arithmetic average to a lognormal distribution:

```
M₁ = E[A],  M₂ = E[A²]
σ*² = ln(M₂ / M₁²),  μ* = ln(M₁) - σ*²/2
Price = df · (M₁·N(d₁) - K·N(d₂))
```

Accuracy is typically within 1% of Monte Carlo for reasonable parameters.

**References:**
- Kemna & Vorst (1990), *Journal of Banking & Finance*
- Turnbull & Wakeman (1991), *Journal of Financial and Quantitative Analysis*
- Levy (1992), *Journal of International Money and Finance*
- Curran (1994), *Management Science*
- Rogers & Shi (1995), *Journal of Applied Probability*

---

### Barrier Options (`barrier.rs`)

All 8 European barrier option types with continuous monitoring, plus touch probabilities and rebates.

| Function | Description |
|----------|-------------|
| `up_in_call`, `up_out_call` | Up barrier calls |
| `down_in_call`, `down_out_call` | Down barrier calls |
| `up_in_put`, `up_out_put` | Up barrier puts |
| `down_in_put`, `down_out_put` | Down barrier puts |
| `barrier_call_continuous` | Dispatcher by `BarrierType` enum |
| `barrier_put_continuous` | Dispatcher by `BarrierType` enum |
| `barrier_touch_probability` | P(barrier is hit before T) |
| `barrier_rebate_continuous` | Rebate paid at expiry (hit or no-hit) |
| `*_df` variants | DF-first API for all of the above |

**Key identity:** In + Out = Vanilla (verified in tests).

**Formulas** follow Reiner & Rubinstein (1991) using the reflection principle:

```
λ = (r - q + σ²/2) / σ²
C_do = Vanilla - (H/S)^(2λ) · [reflected vanilla terms]
```

**Discrete monitoring correction:** Real-world barriers are typically monitored daily. Apply the Broadie-Glasserman-Kou (1997) shift: `H_adj = H · exp(±0.5826 · σ · √Δt)`.

**Edge-case contract:** Public barrier wrappers remain finite for `t <= 0` and `σ <= 0`. Zero-vol and near-zero-vol touch probabilities fall back to the deterministic drift-path limit when the analytical expression becomes ill-conditioned. Low-level helper expiry behavior uses terminal spot as a convenience convention only; true realized expired barrier settlement belongs at the instrument layer via `observed_barrier_breached`.

**References:**
- Reiner & Rubinstein (1991), *Risk Magazine*
- Merton (1973), *Bell Journal of Economics*
- Broadie, Glasserman & Kou (1997), *Mathematical Finance*

---

### Lookback Options (`lookback.rs`)

Fixed-strike and floating-strike lookback options with continuous monitoring.

| Function | Description |
|----------|-------------|
| `fixed_strike_lookback_call` | Payoff: max(S_max - K, 0) |
| `fixed_strike_lookback_put` | Payoff: max(K - S_min, 0) |
| `floating_strike_lookback_call` | Payoff: S_T - S_min |
| `floating_strike_lookback_put` | Payoff: S_max - S_T |

**Floating-strike call formula** (Goldman, Sosin & Gatto 1979):

```
C = S·e^(-qT)·N(a₁) - S_min·e^(-rT)·N(a₂)
  + S·e^(-rT)·(σ²/2b)·[(S/S_min)^(-2b/σ²)·N(-a₂) - e^(bT)·N(-a₁)]

where b = r - q
```

The `r = q` degenerate case is handled via L'Hopital's rule (tolerance: 1e-4). Fixed-strike variants decompose into intrinsic value plus a floating-strike premium.

**References:**
- Goldman, Sosin & Gatto (1979), *Journal of Finance*
- Conze & Viswanathan (1991), *Journal of Finance*
- Cheuk & Vorst (1997)
- Haug (2007), *The Complete Guide to Option Pricing Formulas*, Ch. 6

---

### Quanto Options (`quanto.rs`)

Cross-currency options priced in a different currency than the underlying's denomination.

| Function | Description |
|----------|-------------|
| `quanto_drift_adjustment` | Returns `-ρ·σ_S·σ_X` |
| `quanto_call` / `quanto_put` | Full quanto pricing with FX correlation |

**Formula:**

```
μ_quanto = μ_asset - ρ·σ_asset·σ_fx
F_adj = S · exp((r_for - q - ρ·σ_S·σ_X) · T)
C_quanto = e^(-r_dom·T) · [F_adj·N(d₁) - K·N(d₂)]
```

**References:**
- Garman & Kohlhagen (1983), *Journal of International Money and Finance*
- Brigo & Mercurio (2006), *Interest Rate Models*, Section 13.16

---

### Heston Stochastic Volatility (`heston.rs`)

European option pricing under the Heston (1993) model via Fourier inversion.

| Function | Description |
|----------|-------------|
| `heston_call_price_fourier` | Call price (default integration settings) |
| `heston_put_price_fourier` | Put price via put-call parity |
| `heston_call_price_fourier_with_settings` | Call with custom `HestonFourierSettings` |
| `heston_put_price_fourier_with_settings` | Put with custom settings |

**`HestonParams`**: `r`, `q`, `kappa` (mean reversion), `theta` (long-run variance), `sigma_v` (vol-of-vol), `rho` (correlation), `v0` (initial variance).

**`HestonFourierSettings`**: `u_max` (integration upper limit, default 100), `panels` (composite GL panels, default 100), `gl_order` (GL quadrature order, default 16), `phi_eps` (singularity guard, default 1e-8).

**Algorithm:**

```
C = S·e^(-qT)·P₁ - K·e^(-rT)·P₂

P_j = 0.5 + (1/π) ∫₀^∞ Re[e^(-iφ·ln K) · ψ_j(φ) / (iφ)] dφ
```

Uses the "Little Heston Trap" formulation (Albrecher et al. 2007) to avoid branch-cut discontinuities. Falls back to Black-Scholes when `sigma_v < 1e-10`.

**References:**
- Heston (1993), *Review of Financial Studies*
- Carr & Madan (1999), *Journal of Computational Finance*
- Albrecher, Mayer, Schoutens & Tistaert (2007), *Wilmott Magazine*
- Lord & Kahl (2010), *Mathematical Finance*

---

### Implied Volatility Solvers (`implied_vol.rs`)

Newton-Raphson with bisection fallback for robust implied vol extraction.

| Function | Description |
|----------|-------------|
| `bs_implied_vol` | Black-Scholes/Garman-Kohlhagen implied vol |
| `black76_implied_vol` | Black-76 (forward-based) implied vol |

**Algorithm:**
1. Bracket the root in `[1e-8, 10.0]` (expanding `hi` by 1.5x up to 50 tries).
2. Phase 1: Newton-Raphson using analytical vega (up to 15 iterations).
3. Phase 2: Bisection fallback (up to 200 iterations, tolerance 1e-10).

Returns `Err` if the target price cannot be bracketed (violates arbitrage bounds).

---

## Conventions

| Convention | Value | Notes |
|------------|-------|-------|
| Compounding | Continuous | All rates and yields |
| Vega | Per 1% vol | Multiply raw ∂V/∂σ by 0.01 |
| Rho | Per 1% rate | Multiply raw ∂V/∂r by 0.01 |
| Theta | Per day | Divide annualized theta by `theta_days_per_year` |
| Day-count basis | Caller's choice | 365 (calendar), 252 (business), 360 (money market) |
| DF-first API | `*_df` variants | Preferred when discount factor comes from a curve |

### Edge Case Handling

- **t <= 0**: Returns intrinsic value
- **σ <= 0**: Deterministic forward pricing
- **Barrier options**: Use barrier-state-dependent expiry conventions and deterministic touch logic; see the barrier section above for the full contract
- **Deep ITM/OTM**: Prices clamped to non-negative; `BsGreeks::clamped()` for numerical noise
- **r = q** (lookback): L'Hopital limit form (tolerance 1e-4)
- **sigma_v ≈ 0** (Heston): Falls back to Black-Scholes

---

## Usage Examples

### Vanilla Pricing and Greeks

```rust
use finstack_quant_valuations::models::closed_form::{
    bs_price, bs_greeks, BsGreeks,
};
use finstack_quant_valuations::instruments::OptionType;

let spot = 100.0;
let strike = 100.0;
let r = 0.05;
let q = 0.02;
let vol = 0.20;
let t = 1.0;

let call_price = bs_price(spot, strike, r, q, vol, t, OptionType::Call);
let greeks = bs_greeks(spot, strike, r, q, vol, t, OptionType::Call, 365.0);

assert!(greeks.is_valid());
println!("Price: {:.4}, {}", call_price, greeks);
```

### Asian Option

```rust
use finstack_quant_valuations::models::closed_form::asian::{
    geometric_asian_call, arithmetic_asian_call_tw,
};

let price_geo = geometric_asian_call(100.0, 100.0, 1.0, 0.05, 0.02, 0.20, 252);
let price_arith = arithmetic_asian_call_tw(100.0, 100.0, 1.0, 0.05, 0.02, 0.20, 252);

// Arithmetic >= Geometric (AM-GM inequality)
assert!(price_arith >= price_geo - 0.01);
```

### Barrier Option

```rust
use finstack_quant_valuations::models::closed_form::barrier::{
    down_out_call, up_in_call, BarrierType, barrier_call_continuous,
};

let doc_price = down_out_call(100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.20);
let uic_price = barrier_call_continuous(
    100.0, 100.0, 120.0, 1.0, 0.05, 0.02, 0.20, BarrierType::UpIn,
);
```

### Heston Stochastic Volatility

```rust
use finstack_quant_valuations::models::closed_form::heston::{
    heston_call_price_fourier, HestonParams, HestonFourierSettings,
};

let params = HestonParams::new(
    0.05,   // r
    0.02,   // q
    2.0,    // kappa (mean reversion)
    0.04,   // theta (long-run variance)
    0.3,    // sigma_v (vol-of-vol)
    -0.7,   // rho (asset-variance correlation)
    0.04,   // v0 (initial variance = 20% vol)
)
.unwrap();

let price = heston_call_price_fourier(100.0, 100.0, 1.0, &params);
```

### Implied Volatility

```rust
use finstack_quant_valuations::models::closed_form::implied_vol::{
    bs_implied_vol, black76_implied_vol,
};
use finstack_quant_valuations::instruments::OptionType;

let iv = bs_implied_vol(100.0, 100.0, 0.05, 0.02, 1.0, OptionType::Call, 9.5)
    .expect("should converge");
```

---

## Test Coverage

Every module includes inline `#[cfg(test)]` tests verifying:

| Category | What Is Tested |
|----------|----------------|
| **Positivity** | Option prices are non-negative |
| **Intrinsic** | Expired options return intrinsic value |
| **Parity** | Put-call parity, barrier in+out=vanilla |
| **Monotonicity** | Price increases with vol, decreases with strike (calls) |
| **Bounds** | Greeks satisfy theoretical constraints |
| **Edge cases** | Zero vol, zero time, r=q, deep ITM/OTM |
| **Cross-validation** | Heston vs. volatility module, DF vs. rate-based APIs |
| **Regression** | Turnbull-Wakeman formula fix, clamping behavior |

| File | Tests |
|------|-------|
| `vanilla.rs` | 9 |
| `greeks.rs` | 5 |
| `asian.rs` | 15 |
| `barrier.rs` | 22 |
| `lookback.rs` | 15 |
| `quanto.rs` | 5 |
| `heston.rs` | 12 |
| **Total** | **83** |

---

## Adding a New Closed-Form Model

1. **Create `new_model.rs`** in this directory.

2. **Add the module** to `mod.rs`:

   ```rust
   pub mod new_model;
   pub use new_model::{new_model_call, new_model_put, NewModelParams};
   ```

3. **Implement the formula** following these conventions:
   - Use continuous compounding (`exp(-r*t)`, not `1/(1+r)^t`).
   - Accept `spot`, `strike`, `time`, `rate`, `div_yield`, `vol` as individual `f64` args (match existing signatures).
   - Provide a `*_df` variant if the formula can benefit from a pre-computed discount factor.
   - Handle edge cases: `t <= 0` (return intrinsic), `vol <= 0` (deterministic forward), `spot <= 0`.
   - Clamp results to non-negative.
   - Scale vega per 1% and rho per 1% if providing Greeks.

4. **Document with academic references** in the module-level doc comment. Include:
   - The formula in ```` ```text ```` blocks.
   - Paper citation (author, year, journal, volume, pages).
   - Implementation notes for any numerical tricks.

5. **Add tests** covering at minimum:
   - Non-negativity across parameter sweeps.
   - Put-call parity (or the model's equivalent identity).
   - Convergence to Black-Scholes in the appropriate limit.
   - Intrinsic value at expiry.
   - Edge cases (zero vol, zero time, extreme moneyness).

6. **Run the test suite**: `cargo test -p finstack-quant-valuations -- closed_form`

---

## Academic References (Complete)

### Foundational

- Black, F. & Scholes, M. (1973). "The Pricing of Options and Corporate Liabilities." *Journal of Political Economy*, 81(3), 637-654.
- Merton, R. C. (1973). "Theory of Rational Option Pricing." *Bell Journal of Economics*, 4(1), 141-183.

### Asian Options

- Kemna, A. G. Z. & Vorst, A. C. F. (1990). "A Pricing Method for Options Based on Average Asset Values." *Journal of Banking & Finance*, 14(1), 113-129.
- Turnbull, S. M. & Wakeman, L. M. (1991). "A Quick Algorithm for Pricing European Average Options." *JFQA*, 26(3), 377-389.
- Levy, E. (1992). "Pricing European Average Rate Currency Options." *JIMF*, 11(5), 474-491.
- Curran, M. (1994). "Valuing Asian and Portfolio Options by Conditioning on the Geometric Mean Price." *Management Science*, 40(12), 1705-1711.
- Rogers, L. C. G. & Shi, Z. (1995). "The Value of an Asian Option." *Journal of Applied Probability*, 32(4), 1077-1088.

### Barrier Options

- Reiner, E. & Rubinstein, M. (1991). "Breaking Down the Barriers." *Risk Magazine*, 4(8), 28-35.
- Broadie, M., Glasserman, P. & Kou, S. G. (1997). "A Continuity Correction for Discrete Barrier Options." *Mathematical Finance*, 7(4), 325-349.
- Gobet, E. (2000). "Weak Approximation of Killed Diffusion Using Euler Schemes." *Stochastic Processes and their Applications*, 87(2), 167-197.

### Lookback Options

- Goldman, M. B., Sosin, H. B. & Gatto, M. A. (1979). "Path Dependent Options: Buy at the Low, Sell at the High." *Journal of Finance*, 34(5), 1111-1127.
- Conze, A. & Viswanathan, R. (1991). "Path Dependent Options: The Case of Lookback Options." *Journal of Finance*, 46(5), 1893-1907.

### Quanto Options

- Garman, M. B. & Kohlhagen, S. W. (1983). "Foreign Currency Option Values." *JIMF*, 2(3), 231-237.
- Brigo, D. & Mercurio, F. (2006). *Interest Rate Models -- Theory and Practice* (2nd ed.). Springer.

### Stochastic Volatility

- Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic Volatility." *Review of Financial Studies*, 6(2), 327-343.
- Carr, P. & Madan, D. (1999). "Option Valuation Using the Fast Fourier Transform." *Journal of Computational Finance*, 2(4), 61-73.
- Albrecher, H., Mayer, P., Schoutens, W. & Tistaert, J. (2007). "The Little Heston Trap." *Wilmott Magazine*, January, 83-92.
- Lord, R. & Kahl, C. (2010). "Complex Logarithms in Heston-Like Models." *Mathematical Finance*, 20(4), 671-694.

### Reference Texts

- Haug, E. G. (2007). *The Complete Guide to Option Pricing Formulas* (2nd ed.). McGraw-Hill.
- Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.). Pearson.
- Wilmott, P. (2006). *Paul Wilmott on Quantitative Finance* (2nd ed.). Wiley.
