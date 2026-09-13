# models::volatility

Product-independent volatility evaluation, fitting, convention conversion,
arbitrage checks, and model parameterizations. The module owns SABR, SVI,
Heston, rough Heston, Dupire local volatility, implied-volatility inversion,
and concrete evaluation of the data-only volatility artifacts stored by core.
Black and Bachelier pricing formulas live in [`models::closed_form`](../closed_form/).

## Position in the stack

Depends on neutral core math and market-data artifacts. It does not resolve
artifacts from `MarketContext`; valuations owns lookup and override precedence,
then supplies a concrete `VolSource` to pricing code.

Consumed by [`models::closed_form`](../closed_form/) and
[`models::trees`](../trees/) (both use `black::d1_d2`), by
`calibration::hull_white` (`normal::bachelier_price`), by
`calibration::targets::vol` (`SabrCalibrator::calibrate`, the
SABR slice fitter behind `VolSurfaceModel::Sabr`), and by the rates/FX/vol
instrument pricers — `rates/{swaption, cap_floor, cms_option, cms_swap}`,
the asset-owned futures-option instruments, `fx/fx_digital_option`,
`exotics/range_accrual` — which reach for `normal::{bachelier_price,
d_bachelier}`, `black::{d1_d2, d1_black76, d2_black76, d1_d2_black76}`, and
`SABRParameters` / `sabr::SabrVolType`.

## Layout

| File | Contents |
|------|----------|
| [`mod.rs`](mod.rs) | Re-exports |
| [`black.rs`](black.rs) | `d1`, `d2`, `d1_d2`, `d1_black76`, `d2_black76`, `d1_d2_black76` |
| [`normal.rs`](normal.rs) | `d_bachelier`, `bachelier_price` |
| [`sabr/`](sabr/) | `SabrParameters`, `SabrModel`, `SabrVolType`, `SabrCalibrator`, `SabrSmile` |
| [`sabr_derivatives.rs`](sabr_derivatives.rs) | `SabrMarketData`, `SabrCalibrationDerivatives` — finite-difference gradients for the LM solver |
| [`source.rs`](source.rs) | `VolSource` plus surface, cube, and FX delta-volatility evaluation/materialization |
| [`arbitrage/`](arbitrage/) | Model-dependent volatility arbitrage checks |
| [`heston.rs`](heston.rs), [`rough_heston.rs`](rough_heston.rs), [`local_vol.rs`](local_vol.rs), [`svi.rs`](svi.rs) | Stochastic/local volatility engines and fitting |

`norm_cdf` and `norm_pdf` are re-exported from `finstack_quant_core::math` for
caller convenience; they are not defined here.

## Black-Scholes helpers (`black.rs`)

All six functions are `#[inline]` — they sit inside Greeks and calibration
loops.

| Function | Formula | Argument order |
|----------|---------|----------------|
| `d1_d2` | `d₁ = [ln(S/K) + (r - q + σ²/2)T] / (σ√T)`, `d₂ = d₁ - σ√T` | `(spot, strike, r, sigma, t, q)` |
| `d1_d2_black76` | `d₁ = [ln(F/K) + σ²T/2] / (σ√T)`, `d₂ = d₁ - σ√T` | `(forward, strike, sigma, t)` |

Note the `d1_d2` argument order: rate, then **volatility**, then time, then
dividend yield. Prefer the combined `d1_d2` / `d1_d2_black76` over separate
`d1` + `d2` calls in hot paths — they share one `ln` and one `sqrt`.

Degenerate inputs (`t <= 0` or `σ <= 0`) return the limiting values:

| Moneyness | `d₁`, `d₂` | `N(d₁)` | Reading |
|-----------|-----------|---------|---------|
| ITM (S > K) | `+∞` | 1.0 | Delta = 1 |
| OTM (S < K) | `−∞` | 0.0 | Delta = 0 |
| ATM (S = K) | 0.0 | 0.5 | Mathematical limit as `t → 0` |

References: Black & Scholes (1973); Black (1976).

## Bachelier / normal model (`normal.rs`)

Arithmetic Brownian motion on the underlying, so negative forwards and strikes
are admissible. This is the quoting convention for EUR/JPY/CHF swaptions and
caps/floors and for inflation rate options.

```text
Call = A · [(F − K)·N(d) + σ√T·n(d)]
Put  = A · [(K − F)·N(−d) + σ√T·n(d)]
d = (F − K) / (σ√T)
```

| Function | Signature |
|----------|-----------|
| `d_bachelier` | `(forward, strike, sigma, t) -> f64` |
| `bachelier_price` | `(OptionType, forward, strike, sigma, t, annuity) -> f64` |

`sigma` is **normal** volatility in absolute rate/price units, not a percentage
of the forward. `annuity` is the PV01 (sum of discount factors × accrual
fractions), so the returned premium is in the annuity's currency units. At
`t <= 0` the function returns `intrinsic × annuity`.

Reference: Bachelier (1900).

## SABR (`sabr/`)

```text
dF = σ F^β dW₁
dσ = ν σ dW₂
⟨dW₁, dW₂⟩ = ρ dt
```

| Parameter | Role | Range |
|-----------|------|-------|
| α (`alpha`) | Initial volatility level | > 0 |
| β (`beta`) | CEV backbone exponent | `[0, 1]`; 0 = normal, 1 = lognormal |
| ν (`nu`) | Vol-of-vol / wing curvature | ≥ 0, typically 0.1-0.5 |
| ρ (`rho`) | Skew | `(-1, 1)`; equities ≈ −0.2, rates ≈ 0 |
| `shift` | `Option<f64>` displacement for negative rates | > 0 when present |

### Vol quoting convention — read this first

`SABRModel::implied_volatility` returns a **normal (Bachelier)** volatility in
absolute rate units when β is within `BETA_SNAP_TOL = 1e-4` of 0, and a
**lognormal (Black)** volatility otherwise. Storing one in a surface that
expects the other is a silent unit error. `SABRModel::vol_type()` returns the
`SabrVolType::{Normal, Black}` tag, and
`implied_volatility_with_type(forward, strike, t)` returns the pair so callers
cannot drop the tag. The same rule binds calibration: pass normal quotes when
calibrating with β ≈ 0 and lognormal quotes otherwise.

### `SABRParameters`

| Constructor | β | Notes |
|-------------|---|-------|
| `new(α, β, ν, ρ)` | free | Validated, `-> Result<Self>` |
| `new_with_shift(α, β, ν, ρ, shift)` | free | Shifted variant |
| `equity_standard(α, ν, ρ)` | 1.0 | Lognormal backbone |
| `rates_standard(α, ν, ρ)` | 0.5 | Mixed |
| `normal(α, ν, ρ)` / `lognormal(α, ν, ρ)` | 0 / 1 | Endpoint backbones |
| `shifted_normal(...)` / `shifted_lognormal(...)` | 0 / 1 | With displacement |
| `equity_default()` / `rates_default()` | 1 / 0.5 | `const fn` presets, infallible |

Accessors: `shift()`, `is_shifted()`, `validate()`.

### `SABRModel`

Hagan et al. (2002) expansion **with the Obloj (2008) correction applied** to
the z/χ(z) ratio: geometric-mean moneyness replaces the difference-of-powers
form, cutting the error from O(ε²) to O(ε³) for intermediate β.

Residual accuracy limits after the correction:

- `T > 10Y`: still ~5-10 bp of error.
- `ν > 1.0`: extreme vol-of-vol.
- Strikes 3+ standard deviations from ATM.

β is snapped to exactly 0 or 1 within `BETA_SNAP_TOL` so the dedicated normal
and lognormal branches engage and `powf` with a near-zero exponent is avoided.
Near ATM the χ(z) ratio blends between a Taylor series and the exact form via a
Hermite smoothstep, which keeps Greeks continuous. `implied_volatility`
validates inputs up front (positive expiry, positive effective forward and
strike after any shift) and returns `Result<f64>`.

### `SABRCalibrator`

Levenberg-Marquardt on normalized vega-weighted squared relative quote errors over
(α, ν, ρ), with β fixed by the caller.

| Method | Notes |
|--------|-------|
| `new()` | tolerance 1e-4, max 2000 iterations |
| `high_precision()` | tolerance 1e-8, max 200 iterations (Bloomberg VCUB territory) |
| `with_tolerance` / `with_max_iterations` | Fluent overrides |
| `with_shift(SabrShift)` | `None`, `Fixed(s)` for an explicit displacement, or `Auto` to pick a standardized shift when the forward or a strike is negative |
| `with_atm_pinning(bool)` | Solves α analytically for an exact ATM match, then fits ν and ρ only |
| `calibrate(F, &strikes, &vols, T, β)` | LM with finite-difference Jacobian under the configured shift/pinning settings |
| `calibrate_with_diagnostics(...)` | Same fit, returning `SabrCalibrationOutcome` with solver statistics |

The solve uses log(alpha), ensuring positive alpha without a quote-scale-dependent
upper or lower bound. Nu is bounded to [0.001, 2.0] and rho to [-0.99, 0.99].
The configured tolerance limits the maximum relative error of each final volatility
quote, independently of numerical solver convergence. The outcome reports signed
errors in the original quote units and the maximum relative quote error. A fit
outside this budget fails, including stalled and iteration-limited solves.

### `SABRSmile`

`SABRSmile::new(model, forward, time_to_expiry)`.

| Method | Notes |
|--------|-------|
| `atm_vol()` | `-> Result<f64>` |
| `vol_type()` | Delegates to the model's `SabrVolType` |
| `generate_smile(&strikes)` | `-> Result<Vec<f64>>` |
| `strike_from_delta(delta, is_call)` | Delta-to-strike inversion |
| `validate_no_arbitrage(&strikes, r, q)` | `-> Result<ArbitrageValidationResult>` |
| `check_no_arbitrage(&strikes, r, q)` | `-> Result<()>`; errors when arbitrage is present |
| `repair_arbitrage(&strikes, r, q, max_iter)` | Iterative smoothing |

`ArbitrageValidationResult` carries `ButterflyViolation` and
`MonotonicityViolation` records, plus `is_arbitrage_free()` and
`worst_butterfly_severity()`.

### `sabr_derivatives.rs`

Despite the name, these are **central finite-difference** gradients, not
hand-derived analytical ones. `SABRCalibrationDerivatives` implements
`finstack_quant_core::math::solver_multi::AnalyticalDerivatives` by evaluating
`SABRModel::implied_volatility` — the same function the calibration objective
uses — so gradient and objective are exactly consistent and the accuracy
pitfalls of hand-derived Hagan-expansion gradients are avoided. Finite
differences are the only path; the module offers no analytical alternative to
fall back from.

`SABRMarketData` (`forward`, `time_to_expiry`, `strikes`, `market_vols`,
`beta`, `shift`) is `Serialize`/`Deserialize`/`JsonSchema`; use
`SABRMarketData::new(...)` or `new_with_shift(...)` for the validated
constructors.

References: Hagan, Kumar, Lesniewski & Woodward (2002); Obloj (2008).

## Model ownership

| Model | Home |
|-------|------|
| Heston parameters, characteristic function, and calibration | `models::volatility::heston` |
| Heston Fourier *pricing* | [`models::closed_form::heston`](../closed_form/heston/) |
| Dupire local volatility (`LocalVolSurface`) | `models::volatility::local_vol` |
| SVI surface | `models::volatility::svi` |
| Rough Heston | `models::volatility::rough_heston` |
| SABR parameters, smile, and calibration | `models::volatility::sabr` |
| Black-76 / Bachelier pricing and vega | `models::closed_form::volatility` |
| Normal↔lognormal conversion and implied-volatility inversion | `models::volatility` |

## Model selection

| Model | Strength | Limitation | Use for |
|-------|----------|------------|---------|
| Black-Scholes / Black-76 | Closed form, exact Greeks | No smile | Vanillas, quick marks |
| Bachelier | Handles negative rates natively | No smile | Negative-rate swaptions and caps |
| SABR | Market-standard smile fit, fast to evaluate | Expansion degrades past ~10Y and for ν > 1 | Rates and FX smile interpolation |
| Heston (core + `closed_form::heston`) | Rich smile dynamics, semi-analytical | Five parameters to calibrate | Equity exotics, surface fitting |
| Local volatility (core) | Exact fit to the market surface | Unrealistic forward smile dynamics | Barrier pricing, local hedging |

## Example

```rust
use finstack_quant_models::OptionType;
use finstack_quant_models::volatility::{
    bachelier_price, d1_d2, d1_d2_black76, norm_cdf,
    sabr::SabrVolType,
    SABRCalibrator, SABRModel, SABRParameters, SABRSmile,
};

// Black-Scholes delta: note the (spot, strike, r, sigma, t, q) order.
let (d1, _d2) = d1_d2(100.0, 105.0, 0.05, 0.20, 0.5, 0.02);
let call_delta = (-0.02_f64 * 0.5).exp() * norm_cdf(d1);

// Black-76 for a forward-quoted swaption leg.
let (d1_76, d2_76) = d1_d2_black76(0.05, 0.045, 0.20, 2.0);

// Bachelier with a negative forward and strike.
let receiver = bachelier_price(OptionType::Put, -0.002, -0.003, 0.0050, 1.0, 9.5);
assert!(receiver >= 0.0);

// SABR: build a rates smile at beta = 0.5 and read one vol off it.
let params = SABRParameters::rates_standard(0.02, 0.30, -0.10)?;
let model = SABRModel::new(params);
assert_eq!(model.vol_type(), SabrVolType::Black); // beta = 0.5 -> lognormal quotes
let vol = model.implied_volatility(0.03, 0.035, 1.0)?;

// Calibrate to a market smile, then check the fit for butterfly arbitrage.
let strikes = [0.01, 0.02, 0.03, 0.04, 0.05];
let market_vols = [0.22, 0.20, 0.19, 0.195, 0.21];
let fitted = SABRCalibrator::high_precision()
    .calibrate(0.03, &strikes, &market_vols, 1.0, 0.5)?;

let smile = SABRSmile::new(SABRModel::new(fitted), 0.03, 1.0);
let vols = smile.generate_smile(&strikes)?;
let report = smile.validate_no_arbitrage(&strikes, 0.03, 0.0)?;
if !report.is_arbitrage_free() {
    let _repaired = smile.repair_arbitrage(&strikes, 0.03, 0.0, 10)?;
}
# Ok::<(), finstack_quant_core::Error>(())
```

## Conventions

- Volatilities, rates, and correlations are decimals (`0.20` = 20%), never
  basis points. The one exception is `bachelier_price`'s `sigma`, which is an
  absolute rate volatility.
- Time is a year fraction; the day-count basis is the caller's choice.
- Everything here is `f64` analytics, not `Money` — see
  [INVARIANTS.md](../../../../../INVARIANTS.md) §1.
- `SABRParameters`, `SABRMarketData`, `ArbitrageValidationResult`,
  `ButterflyViolation`, and `MonotonicityViolation` all derive
  `Serialize`/`Deserialize`/`JsonSchema`, but only `SABRParameters` sets
  `deny_unknown_fields` (serde and schemars both). The three violation/result
  types are output views, where leniency is acceptable; `SABRMarketData` is a
  calibration **input** struct and its openness is a gap against the object
  strictness rule in
  [docs/SERDE_STABILITY.md](../../../../../docs/SERDE_STABILITY.md).
  `SABRModel`, `SABRCalibrator`, and `SABRSmile` are runtime types with no
  serde.
- Fallible constructors return `finstack_quant_core::Result<Self>` with
  `Error::Validation` messages that quote the offending value.

## Binding exposure

SABR is the only part of this module reachable from the host languages, and the
class names use the lower-cased acronym on both sides:

**Python** — `finstack_quant.valuations`: `SabrParameters`, `SabrModel`,
`SabrSmile`, `SabrCalibrator`. `SabrCalibrator` exposes `calibrate`, taking
`(forward, strikes, market_vols, t, beta)` with `beta` required, plus
`high_precision()`, `with_tolerance()`, `with_shift(None | float | "auto")`
and `with_atm_pinning(bool)`.

**WASM** — `valuations` namespace: `SabrParameters`, `SabrModel`, `SabrSmile`,
`SabrCalibrator`.

`black.rs` and `normal.rs` are not bound directly; they surface through the
instrument pricers and through `valuations.bs_price` / `bsPrice`.

## Verification

```bash
# Unit tests for this module (never `cargo test` — it would run doc tests).
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/models::volatility/)'

# One area at a time.
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/volatility::sabr/)'

mise run rust-test
mise run rust-lint

# Criterion: the `sabr_slice` bench drives `SABRCalibrator` through the
# calibration API (`sabr_slice_calibration`).
mise run rust-bench
```

Every test in this module is a SABR test: [`sabr/tests.rs`](sabr/tests.rs) plus
`#[cfg(test)]` blocks in [`sabr/parameters.rs`](sabr/parameters.rs) and
[`sabr_derivatives.rs`](sabr_derivatives.rs). They cover parameter validation,
SABR ATM vol recovery, smile monotonicity, calibration round-trips, shifted
SABR on negative rates, arbitrage detection and repair, χ(z) series/exact
blending continuity, extreme ρ, the β ∈ {0, 0.5, 1} branches, and
finite-difference gradient consistency.

`black.rs` and `normal.rs` carry **no tests of their own** — no `d₁`/`d₂`
textbook check, no `t = 0` / `σ = 0` / ATM limit, no Bachelier reference value.
They are exercised only indirectly, through the closed-form and instrument
pricers that call them. Filling that gap is the first thing to do when
touching either file.

## Adding a model

1. New file (or directory) under `volatility/`, declared in [`mod.rs`](mod.rs)
   with explicit re-exports.
2. Parameters struct deriving `Serialize, Deserialize, schemars::JsonSchema`,
   validated in a fallible `new()` returning
   `finstack_quant_core::Result<Self>`.
3. Model struct exposing `implied_volatility()` and/or `price_*()`. Mark hot
   entry points `#[inline]` and `#[must_use]`.
4. If the model's vol output convention is β- or regime-dependent, expose a tag
   type the way `SabrVolType` does — an untagged vol is a unit bug waiting to
   happen.
5. Keep observed surface/cube data and structural validation in core; keep all
   evaluation, fitting, extrapolation, and pricing behavior in models.
6. Tests: parameter validation, known analytical limits (convergence to
   Black-Scholes), literature reference values, and ATM / deep-OTM / zero-vol /
   zero-time edge cases.
7. Cite the source in the module doc with author, year, journal, and a
   `docs/REFERENCES.md#anchor` where one exists.

## References

- Bachelier, L. (1900). "Théorie de la spéculation." *Annales Scientifiques de
  l'École Normale Supérieure*, 17, 21-86.
- Black, F. & Scholes, M. (1973). "The Pricing of Options and Corporate
  Liabilities." *Journal of Political Economy*, 81(3), 637-654.
- Black, F. (1976). "The Pricing of Commodity Contracts." *Journal of Financial
  Economics*, 3(1-2), 167-179.
- Hagan, P. S., Kumar, D., Lesniewski, A. S. & Woodward, D. E. (2002).
  "Managing Smile Risk." *Wilmott Magazine*, Sep, 84-108.
- Obloj, J. (2008). "Fine-tune your smile: Correction to Hagan et al."
  arXiv:0708.0998v2. (Applied.)
- Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic
  Volatility with Applications to Bond and Currency Options." *Review of
  Financial Studies*, 6(2), 327-343.
- Dupire, B. (1994). "Pricing with a Smile." *Risk Magazine*, 7(1), 18-20.

Full bibliography with stable anchors: [docs/REFERENCES.md](../../../../../docs/REFERENCES.md).
