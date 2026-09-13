# models::trees

Lattice pricing for instruments with early exercise or path-dependent state:
equity binomial trees, curve-calibrated short-rate trees, a Hull-White
trinomial tree for Bermudan swaptions, and a two-factor correlated rate/hazard
lattice for credit-risky callables.

A shared backward-induction engine drives every recombining model, and
instrument payoff logic is decoupled from lattice evolution through the
`TreeValuator` / `TreeModel` trait pair.

## Position in the stack

Depends on `finstack_quant_core` for `MarketContext`, the `Discounting` trait,
`HazardCurve`, `PiecewiseConstantCurve`, and `math::time_grid`, and on
[`models::volatility::black`](../volatility/) — `binomial_tree.rs` uses
`d1_d2` to feed the Leisen-Reimer Peizer-Pratt inversion. Consumed by the
instrument pricers listed under
[Usage in the codebase](#usage-in-the-codebase).

Nothing in this module is bound in Python or WASM; the lattices are reached
only through the instrument pricers.

## Layout

| Path | Contents |
|------|----------|
| [`tree_framework/`](tree_framework/) | Traits, `NodeState`, evolution parameters, the generic recombining engine, `state_keys` |
| [`binomial_tree.rs`](binomial_tree.rs) | `BinomialTree` (CRR, Leisen-Reimer) plus American/European/Bermudan entry points |
| [`short_rate_tree/`](short_rate_tree/) | `ShortRateTree`: Ho-Lee, Black-Derman-Toy, Black-Karasinski |
| [`hull_white_tree.rs`](hull_white_tree.rs) | `HullWhiteTree`: 1-factor trinomial in auxiliary x-space |
| [`two_factor_rates_credit/`](two_factor_rates_credit/) | `RatesCreditTree`: correlated rate + hazard 2D lattice (`calibration.rs`, `sampling.rs`, `pricing.rs`) |

The shared `price_recombining_tree` engine is binomial. Trinomial lattices
(the Black-Karasinski short-rate lattice, `HullWhiteTree`, and the
convertible-bond Tsiveriotis-Zhang engine over `EvolutionParams::equity_trinomial`)
carry their own backward induction.

## Traits

```text
TreeValuator                       TreeModel
  ├─ value_at_maturity(&NodeState)   └─ price(initial_vars, ttm, &MarketContext, &valuator)
  └─ value_at_node(&NodeState,
       continuation_value, dt)
```

`TreeValuator` owns the instrument: terminal payoff, and the per-node decision
(hold vs. exercise, cap/floor, coupon accrual). `TreeModel` owns the lattice:
state evolution and backward-induction orchestration. Both require
`Send + Sync`.

`initial_vars` is a plain `HashMap<&'static str, f64>` keyed by `state_keys`
constants.

Implementors: `BinomialTree`, `ShortRateTree`, `RatesCreditTree`.
`HullWhiteTree` is not a `TreeModel` — it exposes its own
`backward_induction`, `bond_price`, `forward_swap_rate`, and `annuity`
accessors instead, because swaption pricing needs the calibrated tree's
internals directly.

## Model comparison

| Model | Branching | Factors | Calibration target | Primary use |
|-------|-----------|---------|--------------------|-------------|
| `BinomialTree` | Binomial | 1 (equity) | None (parametric) | American / Bermudan equity and commodity options |
| `ShortRateTree` | Binomial (Ho-Lee, BDT) or trinomial (BK, κ > 0) | 1 (short rate) | Discount curve | Callable/putable bonds, term loans, OAS |
| `HullWhiteTree` | Trinomial | 1 (short rate) | Discount curve | Bermudan swaptions, mean reversion beyond the binomial limit |
| `RatesCreditTree` | Binomial × binomial | 2 (rate + hazard) | Discount + hazard curves | Credit-risky bonds and loans with embedded options |

Single-factor trees are O(N²) in time and O(N) in memory; the two-factor
lattice is O(N³) in time and O(N²) in memory.

## Binomial trees

Two variants share the `price_recombining_tree` engine:

| Variant | `TreeType` | Convergence | Notes |
|---------|-----------|-------------|-------|
| Cox-Ross-Rubinstein | `CRR` | O(1/N) | `u = exp(σ√dt)`, `d = 1/u` via `EvolutionParams::equity_crr` |
| Leisen-Reimer | `LeisenReimer` | O(1/N²) | Peizer-Pratt inversion; use odd step counts |

```rust
use finstack_quant_valuations::instruments::{OptionMarketParams, OptionType};
use finstack_quant_models::trees::BinomialTree;

let params = OptionMarketParams::new(
    100.0, // spot
    100.0, // strike
    0.05,  // rate
    0.20,  // volatility
    1.0,   // time_to_expiry
    0.02,  // dividend_yield
    OptionType::Put,
);

// leisen_reimer rounds an even request up (200 -> 201).
let tree = BinomialTree::leisen_reimer(200);
assert_eq!(tree.steps, 201);

let american = tree.price_american(&params)?;
let european = tree.price_european(&params)?;
assert!(american >= european - 1e-9); // early exercise never destroys value
# Ok::<(), finstack_quant_core::Error>(())
```

`BinomialTree::leisen_reimer(steps)` rounds positive even requests up to the next odd step count. Additional entry points:
`price_bermudan(&params, &exercise_times)`,
`price_american_with_discrete_dividends`, `price_bermudan_with_discrete_dividends`,
and `price_generic::<V: TreeValuator>`. Greeks are finite differences owned by
the instrument pricers, not the lattice.

## Short-rate trees

| Model | Dynamics | Vol convention | Negative rates | Mean reversion |
|-------|----------|----------------|----------------|----------------|
| Ho-Lee | `dr = θ(t)dt + σdW` | Normal (rate units) | Yes | Not supported — breaks lattice recombination; use `HullWhiteTree` |
| BDT / Black-Karasinski | `d(ln r) = [θ(t) − κ·ln r]dt + σdW` | Lognormal (proportional) | No | κ = 0 → binomial BDT; κ > 0 → trinomial BK |

Calibration uses Arrow-Debreu forward induction to reproduce the input discount
curve exactly at every step. For κ > 0 the lattice is a genuine trinomial
Black-Karasinski tree in `x = ln r`, reusing the Hull-White trinomial geometry
(spacing `σ√(3dt)`, width cap with edge branch switching, per-node
mean-reverting probabilities) with a Brent solve on the per-step additive shift
in `x`.

```rust
use finstack_quant_models::trees::{ShortRateTree, ShortRateTreeConfig};

// Ho-Lee, 100 steps, 80 bp normal vol. Or: ShortRateTreeConfig::bdt(100, 0.20, 0.0)
let config = ShortRateTreeConfig::ho_lee(100, 0.008);
let mut tree = ShortRateTree::new(config);
tree.calibrate(discount_curve, time_to_maturity)?;

let rate = tree.rate_at_node(10, 3)?;
```

`ShortRateTreeConfig` fields: `steps`, `model` (`ShortRateModel::HoLee` or
`::BlackDermanToy`), `volatility`, `mean_reversion` (must be `0.0` for Ho-Lee),
`compounding` (`TreeCompounding::{Continuous, Simple, SemiAnnual, Quarterly,
Monthly}` — Bloomberg's lognormal OAS model uses `Simple`), and
`curve_fit_tolerance_bp`. Constructors `ho_lee` and `bdt` set consistent
defaults; `Default` is Ho-Lee with `DEFAULT_NORMAL_VOL = 0.01`.

`calibrate` takes `(&dyn Discounting, time_to_maturity)` and rejects
`steps == 0` or a non-positive horizon. Ho-Lee and binomial BDT use equal
up/down probabilities; the drift lives in the calibrated node rates.

**Volatility conventions differ by model** and are not interchangeable:
Ho-Lee σ is absolute (50-150 bp, i.e. 0.005-0.015); BDT σ is proportional
(15-30%, i.e. 0.15-0.30). Convert with
`finstack_quant_models::volatility::convert_atm_volatility`.

**Node ordering differs by model.** Ho-Lee: node 0 is the *lowest* rate.
BDT (κ = 0, binomial): node 0 is the *highest* rate (`α·u^(n-1)`).
BK (κ > 0, trinomial): node 0 is the lowest (`j = −j_max`).

`short_rate_keys` supplies `SHORT_RATE` (the same key as
`state_keys::INTEREST_RATE`) and `OAS` (basis points). Every OAS reader and
writer uses the constant; a missing key prices with OAS = 0.

## Hull-White tree

Two-phase construction: build the lattice in auxiliary x-space where
`x(t) = r(t) − α(t)`, then calibrate `α(t)` by forward induction against the
discount curve.

```text
dr(t) = [θ(t) − κr(t)]dt + σdW(t)
dx(t) = −κx(t)dt + σdW(t)
```

Level spacing is `dx_i = σ√(3·dt_{i-1})`, matching the variance of the step
arriving at level `i`. Width is set by the natural branching geometry (each
node's central child plus one node either side), optionally hard-capped by
`config.max_nodes`; if that cap is too tight the transition probabilities go
negative and calibration fails loudly. Boundary handling follows Hull & White
(1994).

```rust
use finstack_quant_models::trees::{HullWhiteTree, HullWhiteTreeConfig};

let config = HullWhiteTreeConfig {
    kappa: 0.03,
    sigma: 0.01,
    steps: 100,
    max_nodes: None,
    ..Default::default()
};

// Exercise dates land exactly on grid points.
let tree = HullWhiteTree::calibrate_with_times(
    config,
    discount_curve,
    time_to_maturity,
    &exercise_times,
)?;

let price = tree.backward_induction(&terminal_values, |step, node, continuation| {
    continuation.max(exercise_value(step, node))
})?;
```

`backward_induction` takes terminal values indexed by node at the final step
(`terminal_values.len()` must equal `tree.num_nodes(tree.num_steps())`) and a
closure `(step, node_idx, continuation_value) -> f64`. `calibrate` is the
uniform-grid shorthand for `calibrate_with_times(.., &[])`;
`calibrate_with_times_and_volatility` additionally accepts a left-continuous
piecewise-constant `σ` schedule whose knots are merged into the grid so no
transition straddles a change.

| Parameter | Typical range |
|-----------|---------------|
| `kappa` | 0.01-0.10 |
| `sigma` | 0.005-0.015 (50-150 bp normal) |
| `steps` | 50-200 (cost is O(n²)) |

## Two-factor rates + credit tree

`RatesCreditTree` models the risk-free short rate and the credit hazard rate
jointly. Each factor is independently calibrated by Arrow-Debreu forward
induction — the rate factor to the discount curve (Ho-Lee style θ adjustment),
the hazard factor to the hazard curve's survival probabilities. Correlated
Bernoulli coupling produces four joint probabilities per node with
`cov = ρ√(var_r · var_h)`.

Node hazards pass through `max(raw, 0.0)` in both the forward recursion and the
backward induction, which is what makes them exact duals and lets the lattice
reproduce the survival curve as the valuator sees it.

```rust
use finstack_quant_models::trees::{RatesCreditConfig, RatesCreditTree};

let config = RatesCreditConfig {
    steps: 100,
    rate_vol: 0.01,
    hazard_vol: 0.02,
    correlation: 0.3,
    rate_mean_reversion: 0.0,
    hazard_mean_reversion: 0.0,
};
let mut tree = RatesCreditTree::new(config);
tree.calibrate(discount_curve, &hazard_curve, time_to_maturity)?;
```

`RatesCreditConfig::default()` is deterministic in **both** factors
(`rate_vol = hazard_vol = 0.0`). The lattice still reprices the discount and
survival curves exactly; it just carries no diffusion. A stochastic default
would let `..Default::default()` silently price optionality the caller never
asked for. `resolve_rates_credit_config` is the single mapping from an
instrument's pricing overrides to this config.

**Mean-reversion limit.** `calibrate` rejects `rate_mean_reversion` or
`hazard_mean_reversion` above `KAPPA_MAX = 0.15`. Discount-curve repricing
stays exact for any κ, but the fixed-geometry binomial lattice collapses the
conditional variance of the factor as κ grows, degrading option values for
callable bonds and term loans. **The binding quantity is `κ·T`, not `κ`:** at
κ = 0.15 the lattice edge retains at most 44% of its intended variance over 5
years and none beyond `T = 1/κ ≈ 6.7` years, where the up-probability clamps to
0 or 1 and those nodes — degenerate Bernoulli marginals carrying no correlation
— drop the configured correlation entirely. `KAPPA_MAX` is therefore a coarse
guard, not a proof of accuracy: read `rate_variance_retention()`,
`hazard_variance_retention()`, and `hazard_floor_saturation()` after
calibration for what the configured `(κ, σ, T, steps)` actually produced. Use
`HullWhiteTree` when κ·T approaches 1.

Other accessors: `max_feasible_correlation(ttm)`, `rate_at_node`,
`hazard_at_node`, `recovery_rate`, `conditional_discount_factors`, and
`price_with_node_coupons::<V: TreeValuator>`.

## State variables

Nodes carry a `HashMap<&'static str, f64>` keyed by `state_keys`:

| Constant | Key | Meaning |
|----------|-----|---------|
| `SPOT` | `"spot"` | Underlying asset price |
| `INTEREST_RATE` | `"interest_rate"` | Risk-free short rate |
| `HAZARD_RATE` | `"hazard_rate"` | Default intensity |
| `DIVIDEND_YIELD` | `"dividend_yield"` | Continuous dividend yield |
| `VOLATILITY` | `"volatility"` | Volatility |
| `DF` | `"df"` | Pre-computed per-node discount factor |

`NodeState` pre-extracts `spot`, `interest_rate`, `hazard_rate`, and
`discount_factor` into cached fields so the hot path avoids hash lookups; the
accessors return `Option<f64>`. `get_var` / `get_var_or` reach anything else.
`single_factor_equity_state` assembles the common equity map.

## Usage in the codebase

| Instrument | Model | Location |
|------------|-------|----------|
| American / Bermudan equity options | `BinomialTree::leisen_reimer` | `instruments/equity/equity_option/pricing/black.rs` |
| Commodity options | `BinomialTree::leisen_reimer` | `instruments/commodity/commodity_option/types.rs` |
| Callable / putable bonds | `ShortRateTree`, `RatesCreditTree`, `HullWhiteTree` | `instruments/fixed_income/bond/pricing/engine/tree/` |
| Term loans | `ShortRateTree`, `RatesCreditTree` | `instruments/fixed_income/term_loan/pricing/tree_engine.rs` |
| Bermudan swaptions | `HullWhiteTree::calibrate_with_times` | `instruments/rates/swaption/` |
| Convertible bonds | Tsiveriotis-Zhang engine over `EvolutionParams` (binomial or trinomial) | `instruments/fixed_income/convertible/pricing/` |

## Serialization

Tree models and their configuration types are runtime-only and implement
neither `Serialize` nor `Deserialize`. They are constructed on demand during
pricing and are not part of any persistent JSON schema. If persistence is ever
needed (scenario storage, calibration caching), add serde to the configuration
structs (`EvolutionParams`) only and keep the runtime engine
types non-serializable.

## Performance

- Complexity as tabulated above; step-count guidance: 50 for fast estimates,
  100-200 for production, 200+ for high precision.
- `NodeState` caching removes hash lookups from the inner loop.
- Parallel Greeks, node-value caching, and SIMD are deliberately deferred to
  keep the engine simple and deterministic.

## Verification

```bash
# Unit tests for this module (never `cargo test` — it would run doc tests).
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/models::trees/)'

# One model at a time.
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/trees::short_rate_tree/)'
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/hull_white/)'

mise run rust-test
mise run rust-lint

# Criterion suite. No bench targets these lattices directly; they are exercised
# through the instrument benches (option_pricing, bond_pricing,
# convertible_pricing, swaption_pricing).
mise run rust-bench
```

## Adding a tree model

1. Add the file (or directory) and declare it in [`mod.rs`](mod.rs) with the
   public re-exports.
2. Implement `TreeModel`. The shortest path is to build a `RecombiningInputs`
   and call `price_recombining_tree(inputs)` — note it takes the struct **by
   value**. Fields: `steps`, `initial_vars`, `time_to_maturity`,
   `market_context`, `valuator`, `up_factor`, `down_factor`, `prob_up`,
   `prob_down`, `interest_rate`, `custom_state_generator`,
   `custom_rate_generator`.
3. Implement `TreeValuator` for the instrument: terminal payoff in
   `value_at_maturity`, the hold-vs-exercise decision in `value_at_node`.
4. For calibrated trees, add a `calibrate()` that stores per-node state
   privately and inject it through `custom_state_generator` /
   `custom_rate_generator`.
5. Add any new state keys to `state_keys`; add a cached `NodeState` field only
   if the key is read on the hot path.
6. Derive evolution parameters through `EvolutionParams::equity_crr` /
   `equity_trinomial` / `with_drift` where possible — they validate that the
   risk-neutral probabilities lie in `[0, 1]` and sum to one in release builds,
   which is the guard against silent lattice arbitrage.

## References

- Cox, J., Ross, S. & Rubinstein, M. (1979). "Option Pricing: A Simplified
  Approach." *Journal of Financial Economics*, 7(3), 229-263.
- Ho, T. & Lee, S. (1986). "Term Structure Movements and Pricing Interest Rate
  Contingent Claims." *Journal of Finance*, 41(5), 1011-1029.
- Boyle, P. (1986). "Option Valuation Using a Three-Jump Process."
  *International Options Journal*, 3, 7-12. (Trinomial construction used by
  `EvolutionParams::equity_trinomial`.)
- Black, F., Derman, E. & Toy, W. (1990). "A One-Factor Model of Interest Rates
  and Its Application to Treasury Bond Options." *Financial Analysts Journal*,
  46(1), 33-39.
- Black, F. & Karasinski, P. (1991). "Bond and Option Pricing when Short Rates
  are Lognormal." *Financial Analysts Journal*, 47(4), 52-59.
- Hull, J. & White, A. (1994). "Numerical Procedures for Implementing Term
  Structure Models I: Single-Factor Models." *Journal of Derivatives*, 2(1),
  7-16.
- Tsiveriotis, K. & Fernandes, C. (1998). "Valuing Convertible Bonds with
  Credit Risk." *Journal of Fixed Income*, 8(2), 95-102.
- Leisen, D. & Reimer, M. (1996). "Binomial Models for Option Valuation —
  Examining and Improving Convergence." *Applied Mathematical Finance*, 3(4),
  319-346.
- Hull, J. (2018). *Options, Futures, and Other Derivatives* (10th ed.),
  ch. 31.

Full bibliography with stable anchors: [docs/REFERENCES.md](../../../../../docs/REFERENCES.md).
