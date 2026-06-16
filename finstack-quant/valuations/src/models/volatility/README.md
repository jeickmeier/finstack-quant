# Volatility Models

Volatility models and Black-Scholes building blocks used throughout the pricing framework.
Provides stochastic volatility (SABR, Heston), local volatility (Dupire), and
fundamental Black-Scholes / Bachelier helpers for option pricing across equity,
rates, FX, and commodity asset classes.

## Module Structure

```
volatility/
├── mod.rs               # Public re-exports
├── black.rs             # Black-Scholes and Black76 d₁/d₂ helpers
├── normal.rs            # Bachelier (normal) model for negative rates
├── sabr.rs              # SABR smile model, calibrator, and smile utilities
├── sabr_derivatives.rs  # Analytical gradients for SABR calibration
├── heston.rs            # Heston stochastic volatility with Fourier pricing
└── local_vol.rs         # Dupire local volatility surface construction
```

## Features

### Black-Scholes Helpers (`black.rs`)

Fundamental d₁/d₂ calculations for Black-Scholes and Black76 pricing. All functions
are `#[inline]` for performance in hot paths (Greeks, calibration loops).

| Function       | Formula                                                    | Use Case                    |
|----------------|------------------------------------------------------------|-----------------------------|
| `d1_d2`        | d₁ = [ln(S/K) + (r - q + σ²/2)T] / (σ√T), d₂ = d₁ - σ√T | Equity/FX options           |
| `d1_d2_black76`| d₁ = [ln(F/K) + σ²T/2] / (σ√T), d₂ = d₁ - σ√T            | Swaptions, caps/floors      |

**Edge-case behavior (t ≤ 0 or σ ≤ 0)**

| Moneyness      | d₁, d₂  | N(d₁) | Interpretation |
|----------------|----------|--------|----------------|
| ITM (S > K)    | +∞       | 1.0    | Delta = 1      |
| OTM (S < K)    | −∞       | 0.0    | Delta = 0      |
| ATM (S = K)    | 0.0      | 0.5    | Mathematical limit as t→0 |

For hot paths requiring both d₁ and d₂, always prefer the combined `d1_d2` / `d1_d2_black76`
functions over separate `d1` + `d2` calls to avoid redundant `ln` and `sqrt` computation.

### Bachelier / Normal Model (`normal.rs`)

Bachelier (normal) pricing for interest rate options and negative-rate environments.
The underlying is modeled as arithmetic Brownian motion, allowing negative values.

```text
Call = A · [(F − K) N(d) + σ√T n(d)]
Put  = A · [(K − F) N(−d) + σ√T n(d)]

where d = (F − K) / (σ√T)
      A = annuity (PV01)
```

| Function         | Description                                       |
|------------------|---------------------------------------------------|
| `d_bachelier`    | d = (F − K) / (σ√T) with expiry edge cases        |
| `bachelier_price`| Call/put price given normal vol and annuity factor |

**Typical use cases**: swaptions with normal vol quoting, caps/floors in EUR/JPY/CHF
markets, inflation rate options.

### SABR Model (`sabr.rs`)

The SABR (Stochastic Alpha Beta Rho) model captures volatility smile dynamics and
is the industry standard for rates and FX smile interpolation.

**Dynamics**

```text
dF = σ F^β dW₁
dσ = ν σ dW₂
⟨dW₁, dW₂⟩ = ρ dt
```

| Parameter | Role                     | Typical Range         |
|-----------|--------------------------|-----------------------|
| α (alpha) | Initial volatility level | > 0                   |
| β (beta)  | CEV backbone exponent    | [0, 1]; 0=normal, 1=lognormal |
| ν (nu)    | Vol-of-vol (wing curvature) | ≥ 0; typically 0.1–0.5 |
| ρ (rho)   | Skew (asset-vol correlation) | [-1, 1]; equities ≈ −0.2, rates ≈ 0 |
| shift     | Negative rate support    | > 0 when present       |

**SABRParameters** — Construction helpers

| Constructor            | Description                                  |
|------------------------|----------------------------------------------|
| `new(α, β, ν, ρ)`     | Validated general construction               |
| `equity_standard(α, ν, ρ)` | β = 1.0 (lognormal backbone)            |
| `rates_standard(α, ν, ρ)`  | β = 0.5 (mixed normal/lognormal)        |
| `normal(α, ν, ρ)`     | β = 0 (normal backbone)                      |
| `lognormal(α, ν, ρ)`  | β = 1 (lognormal backbone)                   |
| `shifted_normal(α, ν, ρ, shift)` | β = 0 with negative rate support  |
| `shifted_lognormal(α, ν, ρ, shift)` | β = 1 with negative rate support |
| `equity_default()`     | α=0.20, β=1, ν=0.30, ρ=−0.20               |
| `rates_default()`      | α=0.02, β=0.5, ν=0.30, ρ=0                  |

**SABRModel** — Implied volatility via Hagan et al. (2002) expansion

- `implied_volatility(forward, strike, T)` — returns Black implied vol
- Smooth blending between Taylor series (|z| < 1e-5) and exact χ(z) formula (|z| > 1e-3) via Hermite smoothstep for continuous Greeks near ATM
- Special handling for β ∈ {0, 0.5, 1} and extreme ρ (±1)

**SABRCalibrator** — Levenberg-Marquardt calibration to market smiles

| Method                             | Description                                       |
|------------------------------------|---------------------------------------------------|
| `calibrate(F, K[], σ[], T, β)`     | Standard calibration (fixed β)                    |
| `calibrate_with_derivatives(...)`  | With analytical gradients (faster convergence)    |
| `calibrate_shifted(...)`           | With explicit shift for negative rates            |
| `calibrate_auto_shift(...)`        | Automatic shift selection                         |
| `calibrate_with_atm_pinning(...)` | Pins ATM vol exactly, fits wings                  |
| `high_precision()`                 | Tolerance 1e-8 (suitable for Bloomberg VCUB parity) |

Tolerance guidance:

| Tolerance | Use Case              | Accuracy         | Speed    |
|-----------|-----------------------|------------------|----------|
| 1e-4      | Quick screening       | ~0.5 vol bp      | Fast     |
| 1e-6      | Standard production   | ~0.01 vol bp     | Moderate |
| 1e-8      | High-precision (VCUB) | ~0.0001 vol bp   | Slow     |

**SABRSmile** — Smile generation and arbitrage checking

| Method                | Description                                       |
|-----------------------|---------------------------------------------------|
| `atm_vol()`           | ATM implied volatility                            |
| `generate_smile(K[])` | Implied vols across strike grid                   |
| `strike_from_delta(δ, is_call)` | Delta-to-strike conversion              |
| `validate_no_arbitrage(K[], r, q)` | Butterfly and monotonicity checks     |
| `check_no_arbitrage(K[], r, q)` | Returns `Err` if arbitrage present       |
| `repair_arbitrage(K[], r, q, max_iter)` | Iterative smile repair            |

**Accuracy limitations** (Hagan expansion):
- T > 5Y, ν > 0.5, or deep OTM: estimated error 10–50 bp
- Obloj (2008) correction is not applied

### SABR Calibration Derivatives (`sabr_derivatives.rs`)

Analytical gradients of the SABR least-squares objective with respect to
(α, ν, ρ) for use with the Levenberg-Marquardt solver. Falls back to finite
differences via `new_with_fd()` when analytical derivatives are unreliable
at parameter boundaries.

| Type                        | Description                                    |
|-----------------------------|------------------------------------------------|
| `SABRMarketData`            | Encapsulates forward, strikes, market vols, β, shift |
| `SABRCalibrationDerivatives`| Implements `AnalyticalDerivatives` trait        |

### Heston Stochastic Volatility (`heston.rs`)

Two-factor stochastic volatility model with semi-analytical pricing via Fourier
inversion of the characteristic function.

**Dynamics**

```text
dS_t = μ S_t dt + √v_t S_t dW_t^S
dv_t = κ(θ − v_t) dt + σ√v_t dW_t^v
⟨dW^S, dW^v⟩ = ρ dt
```

| Parameter | Role                   | Constraint               |
|-----------|------------------------|--------------------------|
| v₀        | Initial variance       | ≥ 0                      |
| κ (kappa) | Mean reversion speed   | ≥ 0                      |
| θ (theta) | Long-run variance      | ≥ 0                      |
| σ (sigma) | Vol-of-vol             | ≥ 0                      |
| ρ (rho)   | Spot-vol correlation   | [-1, 1]                  |

**Feller condition**: 2κθ > σ² ensures the variance process stays strictly positive.
A warning is logged when violated. Default parameters (v₀=0.04, κ=2, θ=0.04, σ=0.3, ρ=−0.5)
satisfy the condition: 2 × 2 × 0.04 = 0.16 > 0.09 = 0.3².

**Pricing**

| Method                | Description                                 |
|-----------------------|---------------------------------------------|
| `price_european_call` | Fourier inversion with adaptive GL quadrature |
| `price_european_put`  | Via put-call parity                         |

**Numerical details**:
- Uses the "Little Heston Trap" (Albrecher et al., 2007) — replaces `exp(dT)` with
  `exp(−dT)` to avoid branch-cut discontinuities in the complex logarithm
- Adaptive Gauss-Legendre quadrature with dynamic upper bound scaled by σ and T
- Integration tolerance 1e-8, max depth 15, 8-point GL panels

### Local Volatility / Dupire (`local_vol.rs`)

Constructs a Dupire local volatility surface from an implied volatility surface.

**Dupire formula**

```text
σ_loc²(K,T) = [∂C/∂T + (r−q)K ∂C/∂K + qC] / [½ K² ∂²C/∂K²]
```

| Type              | Description                                          |
|-------------------|------------------------------------------------------|
| `BilinearInterp`  | Bilinear interpolation on a rectangular grid         |
| `LocalVolSurface` | Stores the computed local vol surface with base date |
| `LocalVolBuilder` | `from_implied_vol(...)` builds the surface           |

**Numerical implementation**:
- Scale-aware finite differences (relative bumps)
- Central differences for ∂C/∂K and ∂²C/∂K²
- Falls back to implied vol when ∂²C/∂K² ≤ 0 (calendar spread arbitrage)

## Model Selection Guide

| Model        | Strengths                                  | Limitations                          | Best For                         |
|--------------|--------------------------------------------|--------------------------------------|----------------------------------|
| Black-Scholes| Simple, fast, closed-form Greeks           | No smile, constant vol assumption    | Vanilla options, quick estimates |
| SABR         | Market-standard smile fit, fast evaluation | Expansion accuracy degrades for long T | Rates (swaptions, caps), FX     |
| Heston       | Rich smile dynamics, semi-analytical       | 5 parameters to calibrate            | Equity exotics, vol surface fit  |
| Local Vol    | Exact fit to market surface                | Forward smile dynamics unrealistic   | Barrier pricing, local hedging   |
| Bachelier    | Handles negative rates natively            | No smile                             | Negative-rate environments       |

## Integration with the Pricing Framework

The volatility module is consumed by:

- **Closed-form pricers**: `models::closed_form` (vanilla, barrier, lookback, Asian, quanto)
  use `d1_d2`, `d1_d2_black76`, `norm_cdf`, `norm_pdf`
- **Rates instruments**: Cap/floor, swaption, inflation cap/floor, CMS options
  use `bachelier_price`, `d_bachelier`, and SABR smile
- **FX instruments**: FX options, FX digitals, FX variance swaps
- **Equity instruments**: Vol index options, autocallables
- **Monte Carlo engine**: `models::monte_carlo` uses Heston process parameters
- **SABR calibration**: Exposed through Python and WASM bindings

## Usage Examples

### Rust

```rust
use finstack_quant_valuations::models::volatility::{
    d1_d2, d1_d2_black76, norm_cdf, norm_pdf,
    bachelier_price, d_bachelier,
    SABRParameters, SABRModel, SABRCalibrator, SABRSmile,
    HestonParameters, HestonModel,
    LocalVolBuilder,
};

// --- Black-Scholes d₁/d₂ ---
let (d1, d2) = d1_d2(100.0, 105.0, 0.05, 0.20, 0.5, 0.02);
let call_delta = (-0.02 * 0.5_f64).exp() * norm_cdf(d1);
let call_price = 100.0 * (-0.02 * 0.5_f64).exp() * norm_cdf(d1)
               - 105.0 * (-0.05 * 0.5_f64).exp() * norm_cdf(d2);

// --- Black76 for swaptions ---
let (d1_76, d2_76) = d1_d2_black76(0.05, 0.045, 0.20, 2.0);
let payer_swaption = annuity * (0.05 * norm_cdf(d1_76) - 0.045 * norm_cdf(d2_76));

// --- Bachelier for negative rates ---
let normal_price = bachelier_price(OptionType::Call, -0.002, -0.003, 0.0050, 1.0, 9.5);

// --- SABR: smile generation ---
let params = SABRParameters::rates_standard(0.02, 0.30, -0.10)?;
let model = SABRModel::new(params);
let vol = model.implied_volatility(0.03, 0.035, 1.0)?;

// --- SABR: calibrate to market ---
let calibrator = SABRCalibrator::new();
let strikes = vec![0.01, 0.02, 0.03, 0.04, 0.05];
let market_vols = vec![0.22, 0.20, 0.19, 0.195, 0.21];
let params = calibrator.calibrate(0.03, &strikes, &market_vols, 1.0, 0.5)?;

// --- SABR: high-precision calibration with analytical gradients ---
let calibrator = SABRCalibrator::high_precision();
let params = calibrator.calibrate_with_derivatives(0.03, &strikes, &market_vols, 1.0, 0.5)?;

// --- SABR: shifted for negative rates ---
let params = calibrator.calibrate_auto_shift(forward, &strikes, &market_vols, 1.0)?;

// --- SABR: arbitrage validation ---
let smile = SABRSmile::new(model, 0.03, 1.0);
let vols = smile.generate_smile(&strikes)?;
let result = smile.validate_no_arbitrage(&strikes, 0.03, 0.0)?;
if !result.is_arbitrage_free() {
    let repaired = smile.repair_arbitrage(&strikes, 0.03, 0.0, 10)?;
}

// --- Heston: semi-analytical pricing ---
let params = HestonParameters::new(0.04, 2.0, 0.04, 0.3, -0.5)?;
assert!(params.satisfies_feller_condition());
let heston = HestonModel::new(params);
let call = heston.price_european_call(100.0, 100.0, 1.0, 0.05, 0.02)?;
let put = heston.price_european_put(100.0, 100.0, 1.0, 0.05, 0.02)?;

// --- Local Vol: surface construction ---
let local_vol = LocalVolBuilder::from_implied_vol(
    &implied_vol_surface, base_date, spot, r, q, &strikes, &times,
)?;
let sigma_loc = local_vol.get_vol(0.5, 105.0)?;
```

### Python

```python
from finstack_quant.valuations import (
    SABRParameters, SABRModel, SABRCalibrator, SABRSmile,
    HestonParameters, HestonModel,
)

# SABR: calibrate to market smile
calibrator = SABRCalibrator()
params = calibrator.calibrate(
    forward=0.03,
    strikes=[0.01, 0.02, 0.03, 0.04, 0.05],
    market_vols=[0.22, 0.20, 0.19, 0.195, 0.21],
    time_to_expiry=1.0,
    beta=0.5,
)

# Generate smile from calibrated parameters
model = SABRModel(params)
vol = model.implied_volatility(forward=0.03, strike=0.035, time_to_expiry=1.0)

# Heston pricing
params = HestonParameters(v0=0.04, kappa=2.0, theta=0.04, sigma=0.3, rho=-0.5)
heston = HestonModel(params)
call_price = heston.price_european_call(S=100.0, K=100.0, T=1.0, r=0.05, q=0.02)
```

## Academic References

| Model / Concept          | Reference                                                                                                                |
|--------------------------|--------------------------------------------------------------------------------------------------------------------------|
| Black-Scholes            | Black, F. & Scholes, M. (1973). "The Pricing of Options and Corporate Liabilities." *Journal of Political Economy*, 81(3), 637-654. |
| Black76                  | Black, F. (1976). "The Pricing of Commodity Contracts." *Journal of Financial Economics*, 3(1-2), 167-179.                |
| Bachelier                | Bachelier, L. (1900). "Théorie de la spéculation." *Annales Scientifiques de l'École Normale Supérieure*, 17, 21-86.     |
| SABR                     | Hagan, P. S., Kumar, D., Lesniewski, A. S. & Woodward, D. E. (2002). "Managing Smile Risk." *Wilmott Magazine*, Sep, 84-108. |
| SABR correction          | Obloj, J. (2008). "Fine-tune your smile: Correction to Hagan et al." arXiv:0708.0998. (Cited as limitation; not applied.) |
| Heston                   | Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic Volatility with Applications to Bond and Currency Options." *Review of Financial Studies*, 6(2), 327-343. |
| Little Heston Trap       | Albrecher, H., Mayer, P., Schoutens, W. & Tistaert, J. (2007). "The Little Heston Trap." *Wilmott Magazine*, Jan, 83-92. |
| Dupire local vol         | Dupire, B. (1994). "Pricing with a Smile." *Risk Magazine*, 7(1), 18-20.                                                 |
| Arbitrage-free smoothing | Fengler, M. R. (2009). "Arbitrage-free smoothing of the implied volatility surface." *Quantitative Finance*, 9(4), 417-428. |

## Adding New Features

### Adding a new volatility model

1. Create a new file (e.g., `rough_vol.rs`) in the `volatility/` directory.
2. Define a parameters struct with `Serialize, Deserialize` derives and validation
   in a fallible `new()` constructor returning `Result<Self>`.
3. Define a model struct that takes parameters and provides `implied_volatility()`
   or `price_*()` methods.
4. Add `pub mod rough_vol;` to `mod.rs` and add re-exports for public types.
5. Add unit tests covering:
   - Parameter validation (invalid inputs return errors)
   - Known analytical limits (e.g., convergence to Black-Scholes)
   - Literature reference values
   - Edge cases (ATM, deep OTM, zero vol, zero time)
6. Expose in Python bindings (`finstack-quant-py/src/valuations/`) and WASM bindings
   (`finstack-quant-wasm/src/valuations/`) as appropriate.
7. Add Python type stubs (`.pyi` files) under `finstack-quant-py/finstack_quant/valuations/`.

### Adding a new calibration method to SABR

1. Add a new method to `SABRCalibrator` in `sabr.rs`.
2. If the method requires analytical gradients, extend `SABRCalibrationDerivatives`
   in `sabr_derivatives.rs` accordingly.
3. Add tests verifying round-trip consistency: calibrate → generate smile → compare
   with input market data.
4. Update Python/WASM bindings if the method should be user-facing.

### Adding a new interpolation scheme for local vol

1. Add the new interpolation type in `local_vol.rs` (or a new file).
2. Update `LocalVolSurface` to use the new type (currently uses `Arc<BilinearInterp>`).
3. Add tests comparing against bilinear interpolation for known smooth surfaces.

### General checklist

- All parameter types must derive `Serialize, Deserialize` for config persistence.
- Input validation must return `finstack_quant_core::Result<T>` with descriptive
  `Error::Validation` messages including the offending value.
- Public constructors use the `Result<Self>` pattern for fallible creation.
- Functions in hot paths should be `#[inline]` and `#[must_use]`.
- Python bindings live under `finstack-quant-py/src/valuations/`.
- Python stub files live under `finstack-quant-py/finstack_quant/valuations/`.

## Testing

Unit tests are co-located in each module. Run with:

```bash
cargo test -p finstack-quant-valuations -- models::volatility
```

Individual model tests:

```bash
cargo test -p finstack-quant-valuations -- models::volatility::sabr
cargo test -p finstack-quant-valuations -- models::volatility::heston
cargo test -p finstack-quant-valuations -- models::volatility::local_vol
cargo test -p finstack-quant-valuations -- models::volatility::black
cargo test -p finstack-quant-valuations -- models::volatility::normal
```

### Test coverage highlights

- **Black-Scholes**: d₁/d₂ values against textbook, combined vs separate function consistency,
  edge cases (t=0, σ=0, ATM/ITM/OTM limits), Black76 equivalence when r=q=0.
- **SABR**: ATM vol recovery, smile generation monotonicity, calibration round-trips,
  shifted SABR for negative rates, arbitrage detection and repair, χ(z) series/exact
  blending continuity, extreme ρ handling, β ∈ {0, 0.5, 1} special cases.
- **Heston**: Convergence to Black-Scholes as σ→0, put-call parity, literature
  reference values, Feller condition warning, parameter validation.
- **Local Vol**: Dupire formula verification against known surfaces, fallback behavior
  when ∂²C/∂K² ≤ 0, bilinear interpolation grid consistency.
- **Bachelier**: Consistency with Black-Scholes in the lognormal limit, put-call parity,
  edge cases at expiry.

## Sibling Modules

| Module        | Description                                         |
|---------------|-----------------------------------------------------|
| `closed_form` | Black-Scholes Greeks, Asian, barrier, lookback, Heston CF |
| `correlation` | Copulas, recovery, factor models                    |
| `credit`      | Merton structural model, PIK toggle, hazard rates   |
| `monte_carlo` | MC engine, payoffs, pricers, Greeks, variance reduction |
| `trees`       | Binomial, trinomial, short-rate trees               |
