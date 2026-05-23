# Range Accrual

## Features

- Coupon accrues when underlying stays within `[lower_bound, upper_bound]` across an observation schedule.
- **Bounds Type**: Supports both absolute bounds (e.g., rate levels) and relative bounds (e.g., 95%-105% of initial spot).
- **Quanto Support**: Equity/FX correlation, FX vol surface, and FX spot for accurate drift adjustment.
- **Mid-Life Valuations**: Historical fixing mechanism for valuations where some observations have already occurred.
- **Two Pricing Methods**: Static replication (default) and Monte Carlo.

## Pricing Methodology

### Static Replication (Default)

The default pricing method uses static replication via digital call spreads:

- Replicates range accrual as a portfolio of digital options at each observation date
- Naturally captures volatility skew/smile from the vol surface
- More accurate than GBM Monte Carlo for instruments with significant smile exposure
- Efficient computation without simulation variance

### Monte Carlo

Monte Carlo pricing is used when `mc_seed_scenario` is set in pricing overrides:

- GBM simulation with discrete observations
- Accrual fraction = proportion of fixes within [lower, upper]
- Supports complex path-dependent features
- Optional quanto drift adjustment

Both methods apply:
- Discount factor from observation dates to payment date
- Historical fixing data for mid-life valuations
- Effective bounds based on `BoundsType`

## Bounds Interpretation

The `bounds_type` field controls how bounds are interpreted:

| BoundsType | Description | Example |
|------------|-------------|---------|
| `Absolute` (default) | Bounds are absolute price/rate levels | `lower=0.04, upper=0.06` for SOFR range |
| `RelativeToInitialSpot` | Bounds are multipliers of initial spot | `lower=0.95, upper=1.05` for ±5% equity range |

## Usage Example

```rust
use finstack_valuations::instruments::rates::range_accrual::{RangeAccrual, BoundsType};

// Equity-linked with relative bounds
let equity_range = RangeAccrual::example(); // Uses RelativeToInitialSpot

// Rate-linked with absolute bounds
let rate_range = RangeAccrual::example_absolute_bounds(); // Uses Absolute

let pv = equity_range.value(&market_context, as_of_date)?;
```

### Mid-Life Valuation

For instruments where some observations have already occurred:

```rust
let inst = RangeAccrualBuilder::new()
    // ... other fields ...
    .past_fixings_in_range_opt(Some(3))  // 3 past observations were in range
    .total_past_observations_opt(Some(6)) // 6 total past observations
    .build()?;
```

## Quanto Adjustment

For quanto range accruals (paying in a different currency):

1. Set `quanto_correlation` (asset vs FX correlation)
2. Set `fx_vol_surface_id` (FX volatility surface)
3. Optionally set `fx_spot_id` for accurate FX vol lookup (defaults to ATM approximation)

The drift adjustment follows: `drift = r - q - ρ × σ_asset × σ_FX`

## Validation

The `validate()` method checks:

- At least one observation date exists
- Observation dates are sorted ascending
- `lower_bound < upper_bound`
- `coupon_rate >= 0`
- Quanto consistency: `quanto_correlation` requires `fx_vol_surface_id`
- Past fixing consistency: both or neither of `past_fixings_in_range` / `total_past_observations`

## Metrics

- **PV**: Present value via static replication (default) or MC
- **Delta/Gamma**: Finite difference spot sensitivity
- **Vega/Vanna/Volga**: Vol sensitivities via generic FD calculators
- **Rho**: Interest rate sensitivity
- **DV01/BucketedDV01**: Parallel and key-rate curve risk

## Limitations / Known Issues

- Requires a compatible registered pricer for the selected pricing method
- Assumes GBM dynamics; no stochastic volatility or jumps
- Discrete observation only; no continuous monitoring adjustment
- Quanto handling uses correlation/vol inputs; no full multi-currency simulation
