# Swaption

## Features

- Options on interest rate swaps with configurable payer/receiver, strike, tenor, exercise style, and settlement (cash/physical).
- Supports Black lognormal or normal (Bachelier) volatility models plus optional SABR parameters and vol surface lookup.
- Helper methods for forward swap rate, annuity, and example builder; integrates pricing overrides (implied vol, quotes).
- **Bermudan swaption support** with Hull-White tree and LSMC pricing methods.

## Methodology & References

- Default pricing uses Black–76; if SABR params are supplied, uses SABR-implied Black vol; normal model available via `ModelKey`.
- Discounting from chosen curve; forward rate derived from swap legs (fixed vs floating) using market curves.
- Metrics and PV computed through `SimpleSwaptionBlackPricer` with deterministic curves/vols.

## Usage Example

### European Swaption

```rust
use finstack_valuations::instruments::rates::swaption::Swaption;

let swaption = Swaption::example();
let pv = swaption.value(&market_context, as_of_date)?;
```

### Bermudan Swaption

```rust
use finstack_valuations::instruments::rates::swaption::{
    BermudanSwaption, BermudanSwaptionPricer, BermudanSwaptionPricerConfig, HullWhiteParams,
};

// Create a 10NC2 Bermudan swaption (10-year swap, callable after 2 years)
let swaption = BermudanSwaption::example();

// Create pricer with Hull-White tree
// Note: For production, calibrate HW parameters to co-terminal Europeans
let pricer = BermudanSwaptionPricer::tree_with_config(BermudanSwaptionPricerConfig {
    hw_params: HullWhiteParams::default(),
    tree_steps: 100,
    ..Default::default()
});

let result = pricer.price_dyn(&swaption, &market_context, as_of_date)?;
```

## Supported Exercise Styles

| Style | Implementation | Pricing Method |
|-------|---------------|----------------|
| European | `Swaption` | Black-76, Bachelier, SABR |
| Bermudan | `BermudanSwaption` | Hull-White tree, LSMC |
| American | Planned | - |

## Limitations / Known Issues

- No stochastic rates/vol beyond SABR-implied vol; quanto/FX effects are out of scope.
- Settlement type toggles payout only; actual underlying swap execution must be handled externally for physical settlement.
- Hull-White parameters should be calibrated to co-terminal European swaptions for accurate Bermudan pricing.

## Metrics

### European Swaption Metrics

- PV plus swaption Greeks (delta/vega/theta/rho) from Black/normal formulas; gamma via bump-and-revalue.
- DV01/CS01 inherit from underlying swap sensitivities through forward/annuity mapping.
- Implied vol solver and par/forward strike reporting for calibration.

### Bermudan Swaption Metrics

- Delta, gamma, vega via bump-and-revalue on the Hull-White tree.
- Exercise probability profile showing risk-neutral exercise distribution.
- Bermudan premium (Bermudan value minus first-exercise European value).
