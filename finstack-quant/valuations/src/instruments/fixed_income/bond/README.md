# Bond

Fixed, floating, callable/utable, amortizing, and PIK bonds with discount, hazard, tree (OAS), and structural-credit pricing paths.

## Overview

- **Types**: Fixed, FRN, zero, amortizing, callable/putable, PIK
- **Engines**: Discount (`BondEngine`), hazard, binomial tree + OAS, Merton MC for structural credit
- **Metrics**: Price, yield, duration, convexity, spreads (Z, OAS, I, ASW, DM), DV01
- **Cashflows**: Holder-view positive amounts for long positions

## Module Structure

```
bond/
├── mod.rs                   # Main module entry point and re-exports
├── types.rs                 # Bond struct, CallPut, CallPutSchedule
├── cashflow_spec.rs         # CashflowSpec enum (Fixed/Floating/Amortizing)
├── cashflows.rs             # Cashflow generation utilities
├── pricing/
│   ├── mod.rs               # Module declarations and convenience re-exports
│   ├── engine/              # Core pricing math (one per model)
│   │   ├── mod.rs
│   │   ├── discount.rs      # BondEngine: PV = Σ CF_i × DF_i
│   │   ├── hazard.rs        # HazardBondEngine: survival-weighted PV + FRP recovery
│   │   ├── tree.rs          # TreePricer: binomial tree for callable/putable + OAS
│   │   └── merton_mc.rs     # MertonMcEngine: structural credit MC for PIK bonds
│   ├── pricer/              # Registry adapters (thin glue: downcast → engine → ValuationResult)
│   │   ├── mod.rs
│   │   ├── discount.rs      # SimpleBondDiscountingPricer
│   │   ├── hazard.rs        # SimpleBondHazardPricer
│   │   ├── oas.rs           # SimpleBondOasPricer
│   │   └── merton_mc.rs     # SimpleBondMertonMcPricer (+ cash-equiv Z-spread/YTM)
│   ├── quote_conversions.rs # Price ↔ yield ↔ spread conversion functions
│   ├── ytm_solver.rs        # Newton-Brent yield-to-maturity solver
│   └── settlement.rs        # Settlement date and accrued interest utilities
└── metrics/                 # Bond-specific metric calculators
    ├── mod.rs
    ├── accrued.rs           # Accrued interest
    ├── duration_macaulay.rs
    ├── duration_modified.rs
    ├── convexity.rs
    └── price_yield_spread/  # Price, yield, and spread metrics
        ├── mod.rs
        ├── prices.rs        # Clean/dirty price
        ├── ytm.rs           # Yield to maturity
        ├── ytw.rs           # Yield to worst
        ├── z_spread.rs      # Zero-volatility spread
        ├── oas.rs           # Option-adjusted spread
        ├── i_spread.rs      # Interpolated spread
        ├── dm.rs            # Discount margin (FRNs)
        ├── asw.rs           # Asset swap spreads
        └── embedded_option_value.rs
```

### Design: Engines vs Pricers

- **Engines** (`engine/*.rs`) contain the core pricing math. They take a `Bond` + `MarketContext` and return a PV. They know nothing about the registry.
- **Pricers** (`pricer/*.rs`) are thin registry adapters. They downcast the instrument, call the appropriate engine, and wrap the result in a `ValuationResult`. Adding a new pricing model means one engine file + one pricer file.
- **Utilities** (`quote_conversions.rs`, `ytm_solver.rs`, `settlement.rs`) are shared helpers used by both engines and metrics.

## Feature Set

### Bond Types

#### Fixed-Rate Bonds

Standard bonds with fixed coupon payments at regular intervals.

```rust
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::types::Bps;
use time::macros::date;

let bond = Bond::fixed(
    "BOND-001",
    Money::new(1_000_000.0, Currency::USD),
    0.05,  // 5% coupon
    date!(2025 - 01 - 01),
    date!(2030 - 01 - 01),
    "USD-OIS",
);
```

#### Floating-Rate Notes (FRNs)

Bonds with floating coupon rates tied to an index (e.g., SOFR, LIBOR).

```rust
let frn = Bond::floating(
    "FRN-001",
    Money::new(1_000_000.0, Currency::USD),
    "USD-SOFR-3M".into(),
    Bps::new(200),
    date!(2025 - 01 - 01),
    date!(2030 - 01 - 01),
    Tenor::quarterly(),
    DayCount::Act360,
    "USD-OIS",
);
```

#### PIK (Payment-in-Kind) Bonds

Bonds where coupons accrete to notional instead of being paid in cash. Supported via `CouponType::PIK` on the cashflow spec. The Merton MC engine handles PIK accrual dynamically with endogenous hazard feedback and dynamic recovery.

```rust
// Build via the Python API:
// Bond.builder("PIK-001").coupon_rate(0.085).coupon_type("pik").build()

// Or via PikSchedule for scheduled PIK windows:
// MertonMcConfig(merton=m, pik_schedule=[(0.0, "pik"), (2.0, "cash")])
```

#### Callable/Putable Bonds

Bonds with embedded options allowing early redemption.

```rust
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CallPutSchedule, CallPut};

let call_schedule = CallPutSchedule {
    calls: vec![
        CallPut { date: date!(2027 - 01 - 01), price_pct_of_par: 102.0 },
        CallPut { date: date!(2028 - 01 - 01), price_pct_of_par: 101.0 },
    ],
    puts: vec![],
};

let callable = Bond::builder()
    .id("CALLABLE-001".into())
    .notional(Money::new(1_000_000.0, Currency::USD))
    .issue(date!(2025 - 01 - 01))
    .maturity(date!(2030 - 01 - 01))
    .cashflow_spec(CashflowSpec::fixed(0.06, Tenor::semi_annual(), DayCount::Thirty360))
    .discount_curve_id("USD-OIS".into())
    .call_put(Some(call_schedule))
    .build()?;
```

### Pricing Engines

| Engine | Model Key | Description |
|--------|-----------|-------------|
| `BondEngine` | `discounting` | Standard PV using discount curves |
| `HazardBondEngine` | `hazard_rate` | Survival-weighted PV + FRP recovery leg |
| `TreePricer` | `tree` | Binomial tree for callable/putable bonds, OAS |
| `MertonMcEngine` | `merton_mc` | Structural credit MC for PIK bonds with endogenous hazard, dynamic recovery, and toggle exercise |

#### Merton MC Engine (PIK Bonds)

The Merton MC engine prices bonds with PIK features using a structural credit framework:

- **Merton model**: Asset value follows GBM; default = asset breaches barrier
- **Endogenous hazard**: Hazard rate increases with leverage (power law / exponential)
- **Dynamic recovery**: Recovery rate declines as PIK accrual grows notional
- **PIK schedule**: Per-coupon Cash/PIK/Split/Toggle modes, including time-stepped schedules
- **Toggle exercise**: Threshold, stochastic (sigmoid), or optimal (nested MC) PIK/cash decisions
- **Cash-equivalent metrics**: Z-spread and YTM computed on a cash-pay bond structure for cross-structure comparability
- **Barrier calibration**: `MertonModel::from_target_pd` calibrates the barrier to match historical annual PDs

### Metrics

#### Price Metrics

- **Clean Price**: Quoted price excluding accrued interest
- **Dirty Price**: Clean price plus accrued interest
- **Accrued Interest**: Interest accrued since last coupon

#### Yield Metrics

- **Yield to Maturity (YTM)**: Internal rate of return
- **Yield to Worst (YTW)**: Minimum yield across call/put/maturity paths

#### Risk Metrics

- **Macaulay Duration**: Weighted average time to cashflows
- **Modified Duration**: Interest rate sensitivity measure
- **Convexity**: Curvature of price/yield relationship
- **DV01**: Dollar value of 1bp rate change
- **CS01**: Credit spread sensitivity

#### Spread Metrics

- **Z-Spread**: Zero-volatility spread over discount curve
- **OAS**: Option-adjusted spread (for callable/putable bonds)
- **I-Spread**: Interpolated spread (YTM minus par swap rate)
- **Discount Margin**: Spread measure for floating-rate notes
- **Asset Swap Spreads**: Par and market asset swap spreads

## How to Add New Features

### Adding a New Pricing Engine

1. **Create the engine** in `pricing/engine/`:

```rust
// pricing/engine/my_model.rs
pub struct MyEngine;

impl MyEngine {
    pub fn price(bond: &Bond, market: &MarketContext, as_of: Date) -> Result<Money> {
        // Core pricing math
    }
}
```

2. **Create the pricer** in `pricing/pricer/`:

```rust
// pricing/pricer/my_model.rs
pub struct SimpleBondMyModelPricer;

impl Pricer for SimpleBondMyModelPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Bond, ModelKey::MyModel)
    }

    fn price_dyn(&self, instrument, market, as_of) -> Result<ValuationResult> {
        let bond = downcast to Bond;
        let pv = MyEngine::price(bond, market, as_of)?;
        Ok(ValuationResult::stamped(bond.id(), as_of, pv))
    }
}
```

3. **Register** in `pricer.rs`:

```rust
register_pricer!(registry, Bond, MyModel, SimpleBondMyModelPricer);
```

### Adding a New Metric

1. Create a `MetricCalculator` impl in `metrics/`
2. Register it in `register_bond_metrics()`
3. Add a `MetricId` variant if needed

## Cashflow Convention

All bond cashflows follow a **holder-view** convention:

- **Positive amounts** represent contractual inflows to a long holder (coupons, amortization, redemption)
- **PIK accrual** increases the outstanding notional; PIK coupons have zero cash flow but grow the redemption amount
- **Initial draw / funding legs** are handled outside the schedule (e.g., via trade price)

## Regional Market Conventions

| Market | Day Count | Frequency | Settlement |
|--------|-----------|-----------|------------|
| US Treasuries | 30/360 | Semi-annual | T+1 |
| UK Gilts | ACT/ACT | Semi-annual | T+1 |
| Eurozone | 30E/360 or ACT/ACT | Annual | T+2 |
| Japan | ACT/365F | Semi-annual | T+2 |

Use `Bond::with_convention()` for standard regional conventions.

## Limitations / Known Issues

- Deterministic curve inputs only; no stochastic rate/credit paths beyond the Merton MC engine.
- Does not model tax/withholding, accrued settlement pricing, or fail penalties.
- DV01/CS01 for Merton MC require re-running the simulation with bumped curves (expensive); currently only cash-equivalent Z-spread and YTM are computed inline.
- Inflation linkage and convertibility live in dedicated modules.
