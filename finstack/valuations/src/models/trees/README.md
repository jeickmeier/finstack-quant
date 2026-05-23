# Tree-Based Pricing Models

Lattice methods for pricing American, Bermudan, and path-dependent fixed income and equity derivatives with early exercise features.

## Overview

This module provides a unified framework for tree-based (lattice) pricing of financial instruments. All tree models share a common backward-induction engine and implement a consistent `TreeModel`/`TreeValuator` trait interface, allowing instrument payoffs to be decoupled from the lattice evolution logic.

### Key capabilities

- **Equity options**: Binomial and trinomial trees with CRR, Leisen-Reimer, Jarrow-Rudd, Tian, and Boyle variants
- **Fixed income**: Curve-calibrated short-rate trees (Ho-Lee, Black-Derman-Toy) for callable/putable bonds and OAS calculation
- **Interest rate derivatives**: Hull-White 1F trinomial tree for Bermudan swaption pricing
- **Credit-rates hybrid**: Two-factor correlated binomial tree jointly modeling short rate and hazard rate
- **Barrier options**: Discrete barrier monitoring with knock-in/knock-out support
- **Greeks**: Finite-difference Greeks (delta, gamma, vega, theta, rho) with Richardson extrapolation and adaptive bump sizing

## Architecture

```
tree_framework.rs          ← Generic backward-induction engine + traits
├── binomial_tree.rs       ← Equity binomial trees (CRR, LR, JR, Tian)
├── trinomial_tree.rs      ← Equity trinomial trees (Standard, Boyle)
├── short_rate_tree.rs     ← Calibrated short-rate trees (Ho-Lee, BDT)
├── hull_white_tree.rs     ← Hull-White 1F trinomial for swaptions
└── two_factor_rates_credit.rs ← Correlated rates + credit 2D lattice
```

### Trait hierarchy

```
TreeValuator              TreeModel
  ├─ value_at_maturity()    ├─ price()
  └─ value_at_node()        └─ calculate_greeks()  [default impl]
```

**`TreeValuator`** encapsulates instrument-specific logic: what the payoff is at maturity and what decision to make at each intermediate node (hold vs. exercise, cap/floor, etc.).

**`TreeModel`** encapsulates the lattice itself: how state variables evolve and how backward induction is orchestrated. The default `calculate_greeks()` implementation uses central finite differences.

### Relationship between models

| Model | Branching | Factors | Calibration Target | Primary Use |
|-------|-----------|---------|-------------------|-------------|
| `BinomialTree` | Binomial | 1 (equity) | None (parametric) | American/Bermudan equity options |
| `TrinomialTree` | Trinomial | 1 (equity) | None (parametric) | Equity options, better convergence |
| `ShortRateTree` | Binomial or Trinomial | 1 (short rate) | Discount curve | Callable/putable bonds, OAS |
| `HullWhiteTree` | Trinomial | 1 (short rate) | Discount curve | Bermudan swaptions |
| `RatesCreditTree` | Binomial | 2 (rate + hazard) | Discount + hazard curves | Credit-risky bonds with embedded options |

## Model Details

### Binomial Trees (`BinomialTree`)

Four binomial tree variants, all using the shared `price_recombining_tree` engine:

| Variant | Method | Convergence | Notes |
|---------|--------|-------------|-------|
| **CRR** | Cox-Ross-Rubinstein | O(1/N) | Standard; `u = exp(σ√dt)`, `d = 1/u` |
| **Leisen-Reimer** | Peizer-Pratt inversion | O(1/N²) | Best accuracy per step; use odd step counts |
| **Jarrow-Rudd** | Equal probability | O(1/N) | `p = 0.5`; u,d chosen for correct moments |
| **Tian** | Third-moment matching | O(1/N) | Matches first three moments of the lognormal |

**Constructors:**

```rust
let tree = BinomialTree::leisen_reimer_odd(201); // Recommended: odd steps for LR
let tree = BinomialTree::crr(200);
let tree = BinomialTree::new(200, TreeType::Tian);
let tree = BinomialTree::new(200, TreeType::JR);
```

**Richardson extrapolation** is available to improve convergence from O(1/N²) to O(1/N⁴):

```rust
let price_n = tree_n.price(vars.clone(), ttm, &ctx, &valuator)?;
let price_2n = tree_2n.price(vars, ttm, &ctx, &valuator)?;
let improved = TreeGreeks::richardson_price(price_n, price_2n);
```

### Trinomial Trees (`TrinomialTree`)

Three-way branching (up/middle/down) provides smoother convergence and is particularly useful for barrier options where intermediate nodes reduce discretization bias.

| Variant | Method | Notes |
|---------|--------|-------|
| **Standard** | Moment matching | `u = exp(σ√(2dt))`, three probabilities from first two moments |
| **Boyle** | Simplified | `λ = √(σ²dt + (drift·dt)²)`, `u = exp(λ)` |

```rust
let tree = TrinomialTree::standard(150);
let tree = TrinomialTree::boyle(150);
```

### Short-Rate Trees (`ShortRateTree`)

Curve-calibrated short-rate trees for pricing bonds with embedded options and calculating Option-Adjusted Spread (OAS).

| Model | Dynamics | Vol Convention | Negative Rates | Mean Reversion |
|-------|----------|----------------|----------------|----------------|
| **Ho-Lee** | `dr = θ(t)dt + σdW` | Normal (bps/yr) | Yes | No |
| **BDT** | `d(ln r) = [θ(t) − a·ln(r)]dt + σdW` | Lognormal (%) | No | Yes |

Calibration is performed via Arrow-Debreu forward induction to exactly reproduce the input discount curve at every tree step.

```rust
let config = ShortRateTreeConfig {
    steps: 100,
    model: ShortRateModel::HoLee,
    volatility: 0.01,           // 100 bps/yr normal vol
    mean_reversion: 0.0,        // Ho-Lee has no mean reversion
    branching: TreeBranching::Trinomial,
};
let mut tree = ShortRateTree::new(config);
tree.calibrate(&market_context, &curve_id, time_to_maturity)?;
let price = tree.price(initial_vars, time_to_maturity, &market_context, &valuator)?;
```

**Volatility conventions:**

| Model | Type | Parameter | Typical Range |
|-------|------|-----------|---------------|
| Ho-Lee | Normal/Absolute | σ (bps/yr) | 50–150 bps (0.005–0.015) |
| BDT | Lognormal/Relative | σ (%) | 15–30% (0.15–0.30) |

Use `finstack_core::math::volatility::convert_atm_volatility` to convert between conventions.

### Hull-White Tree (`HullWhiteTree`)

Industry-standard Hull-White 1-factor trinomial tree for Bermudan swaption pricing. Implements a two-phase construction:

1. **Build** the tree in auxiliary x-space where `x(t) = r(t) − α(t)`
2. **Calibrate** `α(t)` via forward induction to match the discount curve

**Dynamics:**

```
dr(t) = [θ(t) − κr(t)]dt + σdW(t)
dx(t) = −κx(t)dt + σdW(t)        (auxiliary variable)
```

**Tree geometry:**

- Spacing: `dx = σ√(3dt)`
- Width bound: `j_max = ⌈0.184 / (κ·dt)⌉`
- Transition probabilities with boundary handling per Hull & White (1994)

```rust
let config = HullWhiteTreeConfig {
    kappa: 0.03,    // 3% mean reversion
    sigma: 0.01,    // 100 bps vol
    steps: 100,
    max_nodes: None,
};
let tree = HullWhiteTree::calibrate(config, &disc_curve, time_to_maturity)?;
let price = tree.backward_induction(&exercise_dates, &swap_values)?;
```

| Parameter | Typical Range | Description |
|-----------|---------------|-------------|
| `kappa` | 0.01–0.10 | Mean reversion speed |
| `sigma` | 0.005–0.015 | Normal short-rate volatility (50–150 bps) |
| `steps` | 50–200 | Tree steps; cost is O(n²) |

### Two-Factor Rates + Credit Tree (`RatesCreditTree`)

Two-factor correlated binomial tree that jointly models the risk-free short rate and credit hazard rate. Both factors are independently calibrated to their respective market curves via Arrow-Debreu forward induction.

**Correlation:** Correlated Bernoulli coupling produces four joint probabilities for each (rate-up/down, hazard-up/down) combination, with `cov = ρ√(var_r · var_h)`.

```rust
let config = RatesCreditConfig {
    steps: 100,
    rate_vol: 0.01,
    hazard_vol: 0.20,
    correlation: 0.3,
    rate_mean_reversion: 0.0,  // must be <= KAPPA_MAX (0.15)
    hazard_mean_reversion: 0.0, // must be <= KAPPA_MAX (0.15)
};
let mut tree = RatesCreditTree::new(config);
tree.calibrate(&disc, &hazard_curve, time_to_maturity)?;
let price = tree.price(initial_vars, time_to_maturity, &market_context, &valuator)?;
```

**Mean reversion limit:** `rate_mean_reversion` and `hazard_mean_reversion` are capped at `KAPPA_MAX = 0.15`. Above that threshold the fixed-geometry binomial lattice collapses the conditional variance of the factor, degrading option-value accuracy for callable bonds and term loans (discount-curve repricing remains exact for any κ). Use `HullWhiteTree` for κ > 0.15.

## Barrier Options

Discrete barrier monitoring is supported through `BarrierSpec`:

```rust
let barrier = BarrierSpec {
    up_level: Some(120.0),
    down_level: None,
    rebate: 0.0,
    style: BarrierStyle::KnockOut,
};
```

**Barrier touch convention:** Non-strict inequality (`>=` for up, `<=` for down). This is more conservative for knock-out options and matches Bloomberg's behavior (differs from QuantLib's default strict inequality).

The framework populates `BARRIER_TOUCHED_UP` and `BARRIER_TOUCHED_DOWN` state keys at each node, which `TreeValuator` implementations can inspect via `NodeState::barrier_touched_up()` / `barrier_touched_down()`.

## Greeks

All `TreeModel` implementations support finite-difference Greeks via the default `calculate_greeks()` method:

| Greek | Method | Bump Size |
|-------|--------|-----------|
| Delta | Central difference on spot | 1% of spot |
| Gamma | Second-order central difference on spot | 1% of spot |
| Vega | Central difference on volatility | 1% absolute |
| Theta | Forward difference on time | 1 day |
| Rho | Central difference on rate | 1 bp |

**Adaptive bump sizing** adjusts spot bumps based on moneyness (smaller near ATM, larger for deep ITM/OTM):

```rust
let config = GreeksBumpConfig::adaptive().with_spot_bump(0.005);
```

**Richardson extrapolation** on Greeks (N vs 2N steps, O(h⁴) accuracy):

```rust
let coarse = tree_n.calculate_greeks(vars.clone(), ttm, &ctx, &valuator, None)?;
let fine = tree_2n.calculate_greeks(vars, ttm, &ctx, &valuator, None)?;
let improved = TreeGreeks::richardson_extrapolate(&coarse, &fine);
```

## State Variables

All tree nodes carry a `StateVariables` map (`HashMap<&'static str, f64>`) with standardized keys:

| Key | Constant | Description |
|-----|----------|-------------|
| `"spot"` | `state_keys::SPOT` | Underlying asset price |
| `"interest_rate"` | `state_keys::INTEREST_RATE` | Risk-free short rate |
| `"credit_spread"` | `state_keys::CREDIT_SPREAD` | Credit spread |
| `"hazard_rate"` | `state_keys::HAZARD_RATE` | Default intensity |
| `"dividend_yield"` | `state_keys::DIVIDEND_YIELD` | Continuous dividend yield |
| `"volatility"` | `state_keys::VOLATILITY` | Volatility |
| `"rate_volatility"` | `state_keys::RATE_VOLATILITY` | Rate vol (two-factor models) |
| `"df"` | `state_keys::DF` | Pre-computed discount factor |
| `"barrier_touched_up"` | `state_keys::BARRIER_TOUCHED_UP` | Up barrier flag (1.0/0.0) |
| `"barrier_touched_down"` | `state_keys::BARRIER_TOUCHED_DOWN` | Down barrier flag (1.0/0.0) |

Frequently accessed variables (`spot`, `interest_rate`, `hazard_rate`, `df`) are cached directly on `NodeState` to avoid hash lookups on the hot path.

## How to Add a New Tree Model

### 1. Add a new tree type

Create a new file in `models/trees/` (e.g., `my_new_tree.rs`) and register it in `mod.rs`:

```rust
pub mod my_new_tree;
pub use my_new_tree::MyNewTree;
```

### 2. Implement `TreeModel`

The simplest approach is to delegate to the shared `price_recombining_tree` engine:

```rust
use super::tree_framework::{
    price_recombining_tree, RecombiningInputs, TreeBranching,
    TreeModel, TreeValuator, StateVariables,
};

pub struct MyNewTree {
    pub steps: usize,
}

impl TreeModel for MyNewTree {
    fn price<V: TreeValuator>(
        &self,
        initial_vars: StateVariables,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64> {
        let inputs = RecombiningInputs {
            branching: TreeBranching::Binomial,
            steps: self.steps,
            initial_vars,
            time_to_maturity,
            market_context,
            valuator,
            // ... configure evolution parameters, barriers, etc.
        };
        price_recombining_tree(&inputs)
    }
}
```

### 3. Implement `TreeValuator` for your instrument

```rust
struct MyInstrumentValuator { /* payoff parameters */ }

impl TreeValuator for MyInstrumentValuator {
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
        // Terminal payoff logic
        let spot = state.spot().unwrap_or(0.0);
        Ok((spot - self.strike).max(0.0))
    }

    fn value_at_node(
        &self,
        state: &NodeState,
        continuation_value: f64,
        dt: f64,
    ) -> Result<f64> {
        // Intermediate node logic (e.g., early exercise decision)
        let intrinsic = (state.spot().unwrap_or(0.0) - self.strike).max(0.0);
        Ok(continuation_value.max(intrinsic))
    }
}
```

### 4. For calibrated trees

If your tree requires calibration to market curves (like `ShortRateTree` or `HullWhiteTree`):

- Add a `calibrate()` method that populates per-node state values
- Use a `StateGenerator` closure to inject calibrated values into `RecombiningInputs`
- Store calibrated data (e.g., per-node rates) as private fields

### 5. Add new state variable keys

If your model introduces new state variables, add constants to `state_keys`:

```rust
pub mod state_keys {
    // ... existing keys ...
    pub const MY_NEW_FACTOR: &str = "my_new_factor";
}
```

For performance-critical variables, consider adding a cached field to `NodeState`.

## Usage in the Codebase

Tree models are used throughout the instrument pricing layer:

| Instrument | Tree Model | File |
|------------|-----------|------|
| American equity options | `BinomialTree::leisen_reimer(201)` | `equity_option/pricer.rs` |
| Commodity options | `BinomialTree` | `commodity_option/` |
| Callable/putable bonds | `ShortRateTree` (Ho-Lee/BDT) | `bond/pricing/` |
| Term loans | `ShortRateTree`, `RatesCreditTree` | `term_loan/pricing/` |
| Bermudan swaptions | `HullWhiteTree` | `rates/swaption/pricer.rs` |
| Convertible bonds | `BinomialTree`, `TrinomialTree` | `fixed_income/convertible/` |
| Barrier options | `BinomialTree` with `BarrierSpec` | `tests/support/tree_barrier.rs` |

## Serialization Policy

Tree models and their configuration types are **runtime-only** structures and do not implement `Serialize`/`Deserialize`. They are constructed on-demand during pricing and are not part of any persistent JSON schema.

If future requirements emerge (e.g., scenario storage, calibration caching), serde support should be added only to configuration structs (`TreeParameters`, `EvolutionParams`) while keeping runtime engine types non-serializable.

## Performance Notes

- **Complexity**: Single-factor trees are O(N²) in time and O(N) in memory. Two-factor trees are O(N³) in time and O(N²) in memory.
- **Caching**: `NodeState` pre-extracts `spot`, `interest_rate`, `hazard_rate`, and `df` to avoid hash lookups on the hot path.
- **Step count guidance**: 50 steps for fast estimates, 100–200 for production pricing, 200+ with Richardson extrapolation for high precision.
- **Intentionally deferred**: Parallel Greeks computation, node value caching, and SIMD optimizations are deferred to keep the implementation simple and deterministic.

## References

- Cox, J., Ross, S. & Rubinstein, M. (1979). "Option Pricing: A Simplified Approach." *Journal of Financial Economics*, 7(3), 229–263.
- Leisen, D. & Reimer, M. (1996). "Binomial Models for Option Valuation — Examining and Improving Convergence." *Applied Mathematical Finance*, 3(4), 319–346.
- Tian, Y. (1993). "A Modified Lattice Approach to Option Pricing." *Journal of Futures Markets*, 13(5), 563–577.
- Jarrow, R. & Rudd, A. (1983). *Option Pricing*. Irwin.
- Boyle, P. (1988). "A Lattice Framework for Option Pricing with Two State Variables." *Journal of Financial and Quantitative Analysis*, 23(1), 1–12.
- Ho, T. & Lee, S. (1986). "Term Structure Movements and Pricing Interest Rate Contingent Claims." *Journal of Finance*, 41(5), 1011–1029.
- Black, F., Derman, E. & Toy, W. (1990). "A One-Factor Model of Interest Rates and Its Application to Treasury Bond Options." *Financial Analysts Journal*, 46(1), 33–39.
- Hull, J. & White, A. (1994). "Numerical Procedures for Implementing Term Structure Models: Single-Factor Models." *Journal of Derivatives*, 2(1), 7–16.
- Hull, J. (2018). *Options, Futures, and Other Derivatives*, 10th ed. Chapter 31: Interest Rate Derivatives: Models of the Short Rate.
- Broadie, M. & Detemple, J. (1996). "American Option Valuation: New Bounds, Approximations, and a Comparison of Existing Methods." *Review of Financial Studies*, 9(4), 1211–1250.
