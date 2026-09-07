# models::closed_form

Closed-form and semi-analytical pricing formulas for European-style options:
Black-Scholes/Garman-Kohlhagen vanillas and Greeks, Asian, barrier, lookback,
quanto, and Heston Fourier pricing. Every formula carries an academic citation
in its module doc comment.

The module serves two roles:

1. Production pricing where an analytical solution exists and is appropriate.
2. Reference values against which the Monte Carlo, tree, and PDE engines are
   validated.

## Position in the stack

Within the crate, `closed_form` reaches for
[`models::volatility::black`](../volatility/) (the `d1`/`d2` helpers) and for
`crate::instruments::common_impl` — `parameters::OptionType` on every
call/put-selecting entry point, and `helpers::get_unitless_scalar_strict` in
the Heston parameter validator. From core it uses `math::special_functions`
(`norm_cdf`, `norm_pdf`), `math::NeumaierAccumulator`,
`math` special functions and neutral types such as `BarrierType`. Black and
Bachelier pricing formulas are owned directly by this module.

It reads no market data, no curves, and no `MarketContext`: apart from the
payoff/barrier enums and the `BarrierParams`/`HestonPricingParams` grouping structs,
every argument is a flat `f64`.

Consumed by `valuations::instruments` — `equity/equity_option`,
`equity/variance_swap`, `exotics/{asian,barrier,lookback}_option`,
`fx/fx_barrier_option`, `fx/quanto_option`, and the shared
`common_impl/pricing` helpers — by `models::pde` as a convergence anchor, and
by the Python/WASM bindings via [`dispatch.rs`](dispatch.rs).

## Layout

| File | Contents |
|------|----------|
| [`mod.rs`](mod.rs) | Re-exports and module-level references |
| [`vanilla.rs`](vanilla.rs) | Black-Scholes/Garman-Kohlhagen price, Greeks, Black-76, payoff and finiteness guards |
| [`asian.rs`](asian.rs) | Geometric (Kemna-Vorst) and arithmetic (Turnbull-Wakeman) average-price options |
| [`barrier.rs`](barrier.rs) | All eight continuously monitored barrier types, touch probabilities, rebates |
| [`lookback.rs`](lookback.rs) | Fixed- and floating-strike lookbacks |
| [`quanto.rs`](quanto.rs) | Cross-currency drift-adjusted vanillas |
| [`heston/`](heston/) | Heston stochastic volatility via Gil-Pelaez Fourier inversion |
| [`implied_vol.rs`](implied_vol.rs) | Newton-Raphson + bisection implied-vol solvers |
| [`dispatch.rs`](dispatch.rs) | String-keyed routing shared by the Python and WASM bindings |

All analytical Greeks live in `vanilla.rs`, on `bs_greeks`,
`bs_greeks_unchecked`, and the crate-internal `bs_vega_unchecked`.

## Black-Scholes / Garman-Kohlhagen (`vanilla.rs`)

```text
C = S·e^(-qT)·N(d₁) - K·e^(-rT)·N(d₂)
P = K·e^(-rT)·N(-d₂) - S·e^(-qT)·N(-d₁)

d₁ = [ln(S/K) + (r - q + σ²/2)T] / (σ√T)
d₂ = d₁ - σ√T
```

| Item | Signature sketch |
|------|------------------|
| `bs_price` | `(spot, strike, r, q, sigma, t, OptionType) -> Result<f64>` |
| `bs_price_unchecked` | same inputs, `-> f64`; raw formula for validated Rust call sites |
| `bs_greeks` | `(spot, strike, r, q, sigma, t, OptionType, theta_days_per_year) -> Result<BsGreeks>` |
| `bs_greeks_unchecked` | same inputs, `-> BsGreeks`; raw formula for validated Rust call sites |
| `bs_vega_unchecked` | crate-internal vega primitive |
| `black_call` / `black_put` | `(forward, strike, sigma, t) -> f64` — undiscounted |
| `vanilla_expiry_payoff` | `(spot, strike, OptionType) -> Result<f64>` |
| `checked_closed_form_value` | `(value, what) -> Result<f64>` — the shared finiteness guard |
| `ONE_PERCENT` | `100.0`, the divisor that puts vega and rho on a per-1% basis |

`BsGreeks` carries `delta`, `gamma`, `vega`, `theta`, `rho_r` (domestic /
risk-free) and `rho_q` (dividend yield or foreign rate), so the same struct
serves equity and FX. `is_valid()` and `clamped()` deliberately check and clamp
only gamma and vega: the true delta bound is `|Δ| ≤ e^{−qT}`, which exceeds 1
under negative carry, and this type does not know `q` or `T`.

`bs_greeks` asserts `theta_days_per_year > 0.0` in release builds; a
non-positive basis would silently yield infinite theta.

References: Black & Scholes (1973); Merton (1973); Garman & Kohlhagen (1983).

## Asian options (`asian.rs`)

| Item | Notes |
|------|-------|
| `geometric_asian_call` / `_put` | Exact under geometric averaging (Kemna-Vorst) |
| `geometric_asian_call_df` / `_put_df` | Discount-factor-first variant; derives `r = -ln(df)/t` |
| `arithmetic_asian_call_tw` / `_put_tw` | Turnbull-Wakeman two-moment lognormal match |
| `arithmetic_asian_call_tw_df` / `_put_tw_df` | Discount-factor-first variant |
| `geometric_asian_price_times` | Arbitrary (unequally spaced) fixing schedule, `-> Result<f64>` |
| `arithmetic_asian_tw_price_times` | Same, Turnbull-Wakeman |

Equal-spacing entry points take `(spot, strike, time, rate, div_yield, vol,
num_fixings)`. `num_fixings == 0` selects the continuous-monitoring limit for
the geometric forms; for the arithmetic forms it returns `0.0` (no average can
be formed).

**Geometric average.** The log of the geometric average is normally
distributed, so pricing reduces to Black-Scholes with an adjusted volatility and
dividend yield. For `n` fixings at `t_i = iT/n` (no `t = 0` fixing):

```text
Var[ln G] = σ²·T·(n+1)(2n+1) / (6n²)      ⇒  σ_G = σ·√[(n+1)(2n+1) / (6n²)]
E[ln G]   = ln S + (r - q - σ²/2)·T·(n+1)/(2n)
```

The continuous limit is `σ_G = σ/√3`, `q_adj = (r+q)/2 + σ²/12`.

**Arithmetic average (Turnbull-Wakeman).** Match the first two moments of the
arithmetic average to a lognormal:

```text
σ*² = ln(M₂ / M₁²),   μ* = ln(M₁) - σ*²/2
d₁  = (μ* - ln K + σ*²) / σ*,   d₂ = d₁ - σ*
Price = df · (M₁·N(d₁) - K·N(d₂))
```

The `+σ*²` (not `+σ*²/2`) in `d₁` is correct here: the parameterisation is on
the log-average directly, not the standard Black-Scholes form. Every return
path is capped at the no-arbitrage bound `df·M₁`, because the moment-matching
approximation can overshoot for deep-ITM/high-vol inputs.

References: Kemna & Vorst (1990); Turnbull & Wakeman (1991); Levy (1992);
Curran (1994); Rogers & Shi (1995); Haug (2007) ch. 3.

## Barrier options (`barrier.rs`)

Reiner-Rubinstein continuous-monitoring formulas via the reflection principle,
with `λ = (r - q + σ²/2) / σ²`.

| Item | Notes |
|------|-------|
| `up_out_call`, `up_in_call`, `down_out_call`, `down_in_call` | `(spot, strike, barrier, time, rate, div_yield, vol) -> f64` |
| `up_out_put`, `up_in_put`, `down_out_put`, `down_in_put` | same shape (module-level only; not re-exported at `closed_form` root) |
| `barrier_call_continuous`, `barrier_put_continuous` | `(&BarrierParams, BarrierType) -> f64` |
| `barrier_touch_probability` | `(spot, barrier, time, rate, div_yield, vol, is_up) -> f64` |
| `barrier_rebate` | explicit `RebateTiming::{AtHit, AtExpiry}` |

`BarrierType` is `finstack_quant_core::types::BarrierType`, not a type defined
here. `BarrierParams` groups `spot`/`strike`/`barrier`/`time`/`rate`/
`div_yield`/`vol`; `BarrierParams::with_df` is the discount-factor-first
constructor and returns `Result` — it rejects a non-positive or non-finite `df`
rather than coercing it to `rate = 0.0`. There are no `*_df` function variants
for barriers.

`RebateTiming::AtHit` is the market standard for knock-out rebates and prices
`rebate · E[e^{-r·τ} 1{τ≤T}]` via the Rubinstein-Reiner discounted
first-passage value; it returns a `NaN` sentinel when that closed form is
undefined (`μ² + 2r/σ² < 0`). Knock-in no-hit rebates always settle at expiry,
so `timing` is ignored for them.

Identities verified in tests: in + out = vanilla; up-and-out is `0.0` once
`spot >= barrier`.

**Edge-case contract.** Public wrappers stay finite for `time <= 0` and
`vol <= 0`. Zero-vol and near-zero-vol touch probabilities fall back to the
deterministic drift-path limit. `time <= 0` uses terminal spot as a convenience
convention only — realized expired barrier settlement needs observed path
history and belongs at the instrument layer (see `observed_barrier_breached` on
the FX barrier instrument).

**Discrete monitoring.** These formulas assume continuous monitoring and
underestimate discretely monitored barriers. Apply the Broadie-Glasserman-Kou
(1997) shift `H_adj = H · exp(±0.5826·σ·√Δt)` at the caller; it is not applied
inside this module.

References: Reiner & Rubinstein (1991); Merton (1973); Broadie, Glasserman &
Kou (1997); Gobet (2000); Fusai & Recchioni (2007); Haug (2007) ch. 4.

## Lookback options (`lookback.rs`)

| Item | Payoff | Trailing argument |
|------|--------|-------------------|
| `fixed_strike_lookback_call` | `max(S_max - K, 0)` | `spot_max` |
| `fixed_strike_lookback_put` | `max(K - S_min, 0)` | `spot_min` |
| `floating_strike_lookback_call` | `S_T - S_min` | `spot_min` (no `strike` argument) |
| `floating_strike_lookback_put` | `S_max - S_T` | `spot_max` (no `strike` argument) |

Floating-strike call (Goldman, Sosin & Gatto 1979; Haug 2007 ch. 6), with
`b = r - q`:

```text
C = S·e^(-qT)·N(a₁) - S_min·e^(-rT)·N(a₁ - σ√T)
  + S·e^(-rT)·(σ²/(2b))·[(S/S_min)^(-2b/σ²)·N(-d₃) - e^(bT)·N(-a₁)]

a₁ = [ln(S/S_min) + (b + σ²/2)T] / (σ√T)
d₃ = a₁ - 2b√T/σ
```

The `b → 0` degeneracy uses an L'Hôpital limiting form below
`RATE_EQ_DIV_TOL = 1e-7`. That threshold is an **absolute** floor by design: at
`|b| ≈ 5e-7` the general form is still well-conditioned while the limiting form
mis-prices by order 20%, so scaling the tolerance relative to `max(|r|, |q|)`
would be wrong. Earlier values of 1e-2 and 1e-4 produced visible price
discontinuities at the switch.

Fixed-strike variants decompose into discounted intrinsic plus a
floating-strike premium evaluated at a synthetic extremum.

References: Goldman, Sosin & Gatto (1979); Conze & Viswanathan (1991); Cheuk &
Vorst (1997); Haug (2007) ch. 6.

## Quanto options (`quanto.rs`)

```text
μ_quanto = (r_for - q) - ρ·σ_S·σ_X
F_adj    = S · exp(μ_quanto · T)
C        = e^(-r_dom·T) · [F_adj·N(d₁) - K·N(d₂)]
```

`quanto_drift_adjustment(correlation, vol_asset, vol_fx)` returns `-ρ·σ_S·σ_X`.
`quanto_call` / `quanto_put` take `(spot, strike, time, rate_domestic,
rate_foreign, div_yield, vol_asset, vol_fx, correlation)` and return the price
in domestic currency per unit of foreign notional.

References: Garman & Kohlhagen (1983); Derman, Karasinski & Wecker (1990);
Brigo & Mercurio (2006) §13.16.

## Heston (`heston/`)

Gil-Pelaez P1/P2 Fourier inversion:

```text
C = S·e^(-qT)·P₁ - K·e^(-rT)·P₂
P_j = 0.5 + (1/π) ∫₀^∞ Re[e^(-iφ·ln K) · ψ_j(φ) / (iφ)] dφ
```

| Item | Notes |
|------|-------|
| `heston_call_price_fourier` | `(spot, strike, time, &HestonPricingParams, Option<&HestonFourierSettings>) -> f64` |
| `heston_put_price_fourier` | same, via put-call parity |
| `heston_call_prices_fourier` / `heston_put_prices_fourier` | strike-strip variants |
| `HestonStripPricer` | caches the strike-independent characteristic function on the quadrature grid |
| `HestonPricingParams` | `r` and `q` plus canonical `volatility::heston::HestonParams` — `new()` validates and returns `Result` |
| `HestonFourierSettings` | `u_max`, `panels`, `gl_order`, `phi_eps`; `new()` / `validate()` |
| `heston_defaults` | module of `KAPPA`/`THETA`/`SIGMA_V`/`RHO`/`V0` constants — the single source of truth for Heston defaults across the Fourier, PDE, and Monte Carlo equity pricers |

The "Little Heston Trap" algebra (Albrecher et al. 2007) lives once in
`models::volatility::heston`;
[`characteristic_fn.rs`](heston/characteristic_fn.rs) is a thin adapter that
maps this module's market-aware parameter grouping onto the volatility engine's
five stochastic parameters.

Passing `settings: None` selects
`HestonFourierSettings::for_maturity_with_variance(time, v0)`, which widens the
grid for short maturities and for small `v0` — the integrand tail decays on a
`u`-scale proportional to `1/√(v0·T)`. Buckets: `u_max = 200/panels = 200`
under 0.05y, `150/150` under 0.25y, the default `100/100` under 1y, `80/80`
beyond. `gl_order` must be one of `{2, 4, 8, 16}`; anything else fails
validation because there is no node/weight table for it and the strip pricer
would silently degrade to the slower per-strike path.

`sigma_v < 1e-10` falls back to Black-Scholes evaluated at the *deterministic
average variance* `v̄(T)`, not at `v0`. A Feller violation (`2κθ ≤ σ_v²`) logs a
warning but is informational only: the Fourier pricer never simulates the
variance path.

References: Heston (1993); Carr & Madan (1999); Albrecher, Mayer, Schoutens &
Tistaert (2007); Lord & Kahl (2010).

## Implied volatility (`implied_vol.rs`)

`bs_implied_vol(spot, strike, r, q, t, OptionType, target_price)` and
`black76_implied_vol(forward, strike, df, t, OptionType, target_price)`.

1. Reject non-finite inputs, non-positive `spot`/`strike`/`target_price`, and
   any target at or below intrinsic (an arbitrage violation).
2. Bracket in `[MIN_VOL = 1e-8, MAX_VOL = 10.0]`, starting the upper bound at
   `0.3` and expanding by 1.5x for up to 50 tries.
3. Newton-Raphson using analytical vega, at most `MAX_NEWTON_ITER = 15` steps;
   falls through to bisection when vega drops below `1e-15` or a step leaves
   the bracket.
4. Bisection, at most `MAX_ITER = 200` steps; converges on a price residual
   below `PRICE_TOL = 1e-10` or a bracket collapsed below `1e-12`.

`t <= 0` returns `Ok(0.0)`. Exhausting the iteration budget returns an explicit
non-convergence error rather than the last unconverged midpoint.

## String dispatch (`dispatch.rs`)

One match table shared by both host bindings, so Python and WASM cannot drift
apart on selector spelling or error text. Every dispatcher finishes through
`checked_closed_form_value`, so a non-finite price from a degenerate input
surfaces as a validation error instead of crossing the host boundary.

| Function | Selectors |
|----------|-----------|
| `barrier_call_str` | `direction ∈ {"up","down"}`, `knock ∈ {"in","out"}` |
| `asian_option_price_str` | `averaging ∈ {"arithmetic","geometric"}` |
| `lookback_option_price_str` | `strike_type ∈ {"fixed","floating"}` |
| `quanto_option_price` | call/put only |

## Conventions

| Convention | Value |
|------------|-------|
| Compounding | Continuous throughout (`exp(-r·t)`) |
| Dividends | Continuous yield `q`; also used as the foreign rate for FX |
| Vega | Per 1% absolute vol move (raw `∂V/∂σ` divided by `ONE_PERCENT`) |
| Rho | Per 1% rate move |
| Theta | Per day; divide the annualized value by `theta_days_per_year` (365 calendar, 252 business) |
| Time | Year fractions; day-count basis is the caller's choice |
| Scaling | Per unit of underlying — never multiplied by contract size |
| Rates | Decimals (`0.05` = 5%), never basis points |

Edge cases: `t <= 0` returns intrinsic; `σ <= 0` prices the deterministic
forward; prices are clamped non-negative. These are `f64` analytics, not
`Money` — see [INVARIANTS.md](../../../../../INVARIANTS.md) §1 for the
Decimal/f64 split.

## Example

```rust
use finstack_quant_models::OptionType;
use finstack_quant_models::closed_form::{
    barrier::down_out_call,
    bs_greeks, bs_price,
    heston::{heston_call_price_fourier, HestonPricingParams},
    implied_vol::bs_implied_vol,
};

let (spot, strike, r, q, vol, t) = (100.0, 100.0, 0.05, 0.02, 0.20, 1.0);

// Vanilla price and Greeks on a 365-day theta basis.
let price = bs_price(spot, strike, r, q, vol, t, OptionType::Call);
let greeks = bs_greeks(spot, strike, r, q, vol, t, OptionType::Call, 365.0);
assert!(greeks.is_valid());

// Invert the same price back to the input vol.
let iv = bs_implied_vol(spot, strike, r, q, t, OptionType::Call, price)?;
assert!((iv - vol).abs() < 1e-8);

// Knock-out barrier below spot: worth strictly less than the vanilla.
let ko = down_out_call(spot, strike, 90.0, t, r, q, vol);
assert!(ko < price);

// Heston with adaptive quadrature settings.
let params = HestonPricingParams::new(r, q, 2.0, 0.04, 0.3, -0.7, 0.04)?;
let heston = heston_call_price_fourier(spot, strike, t, &params, None)?;
assert!(heston > 0.0 && heston < spot);
# Ok::<(), finstack_quant_core::Error>(())
```

## Binding exposure

**Python** (`finstack_quant.valuations`): `bs_price`, `vanilla_expiry_payoff`,
`bs_greeks`, `bs_implied_vol`, `black76_implied_vol`, plus the four dispatchers
as `barrier_call`, `asian_option_price`, `lookback_option_price`,
`quanto_option_price`.

**WASM** (`valuations` namespace): the same set as `bsPrice`,
`vanillaExpiryPayoff`, `bsGreeks`, `bsImpliedVol`, `black76ImpliedVol`,
`barrierCall`, `asianOptionPrice`, `lookbackOptionPrice`, `quantoOptionPrice`.

The Heston Fourier pricer is not bound directly in either host; it is reached
through equity-option pricing. Lookback and Asian put/`_df`/`_times` variants
are Rust-only.

## Verification

```bash
# Unit tests for this module (never `cargo test` — it would run doc tests).
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/models::closed_form/)'

# One file at a time.
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/closed_form::barrier/)'

# Full Rust suite and lint.
mise run rust-test
mise run rust-lint
```

Tests are colocated in each file (`heston/` keeps its in
[`heston/tests.rs`](heston/tests.rs)) and cover non-negativity sweeps, put-call
parity, barrier in+out=vanilla, monotonicity in vol and strike, Greek bounds,
zero-vol / zero-time / `r=q` / deep-moneyness edge cases, DF-first versus
rate-based agreement, Heston-to-Black-Scholes convergence as `σ_v → 0`, and
literature reference values.

## Adding a formula

1. Create the file, declare it in [`mod.rs`](mod.rs), and re-export the public
   names.
2. Take flat `f64` arguments in the established order (`spot, strike, time,
   rate, div_yield, vol, ...`) and use continuous compounding.
3. Handle `t <= 0` (intrinsic), `vol <= 0` (deterministic forward), and
   non-positive spot; clamp results non-negative.
4. Add a `*_df` variant if a curve-sourced discount factor is the natural
   input, and route non-finite results through `checked_closed_form_value` on
   any `Result`-returning entry point.
5. Scale vega and rho by `ONE_PERCENT` if you expose Greeks.
6. Document the formula in a ```` ```text ```` block with a full citation
   (author, year, journal, pages) and a `docs/REFERENCES.md#anchor` where one
   exists — see [`.agents/rules/rust/documentation.md`](../../../../../.agents/rules/rust/documentation.md).
7. Test non-negativity, the model's parity identity, convergence to
   Black-Scholes in the appropriate limit, expiry intrinsic, and edge cases.

## References

Foundational: Black & Scholes (1973), *JPE* 81(3), 637-654; Merton (1973),
*Bell J. Econ.* 4(1), 141-183.

Asian: Kemna & Vorst (1990), *JBF* 14(1), 113-129; Turnbull & Wakeman (1991),
*JFQA* 26(3), 377-389; Levy (1992), *JIMF* 11(5), 474-491; Curran (1994),
*Mgmt. Sci.* 40(12), 1705-1711; Rogers & Shi (1995), *J. Appl. Prob.* 32(4),
1077-1088.

Barrier: Reiner & Rubinstein (1991), *Risk* 4(8), 28-35; Broadie, Glasserman &
Kou (1997), *Math. Finance* 7(4), 325-349; Gobet (2000), *SPA* 87(2), 167-197;
Fusai & Recchioni (2007), *JEDC* 31(3), 826-860.

Lookback: Goldman, Sosin & Gatto (1979), *J. Finance* 34(5), 1111-1127; Conze &
Viswanathan (1991), *J. Finance* 46(5), 1893-1907.

Quanto: Garman & Kohlhagen (1983), *JIMF* 2(3), 231-237; Brigo & Mercurio
(2006), *Interest Rate Models* (2nd ed.), §13.16.

Stochastic volatility: Heston (1993), *RFS* 6(2), 327-343; Carr & Madan (1999),
*J. Comp. Finance* 2(4), 61-73; Albrecher, Mayer, Schoutens & Tistaert (2007),
*Wilmott*, Jan, 83-92; Lord & Kahl (2010), *Math. Finance* 20(4), 671-694.

Texts: Haug (2007), *The Complete Guide to Option Pricing Formulas* (2nd ed.);
Hull (2018), *Options, Futures, and Other Derivatives* (10th ed.).

Full bibliography with stable anchors: [docs/REFERENCES.md](../../../../../docs/REFERENCES.md).
