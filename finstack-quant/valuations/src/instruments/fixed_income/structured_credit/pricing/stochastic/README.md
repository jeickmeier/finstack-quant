# Stochastic Models for Structured Credit

This module provides advanced stochastic modeling capabilities for structured credit instruments (CLO, RMBS, CMBS, ABS). It enables Monte Carlo simulation, scenario trees, and correlation-aware risk analytics.

## Module Organization

```
stochastic/
├── mod.rs              # Re-exports and module documentation
├── README.md           # This file
├── tests.rs            # Integration tests
│
├── prepayment/         # Stochastic prepayment models
│   ├── mod.rs          # Re-exports
│   ├── spec.rs         # StochasticPrepaySpec configuration
│   ├── traits.rs       # StochasticPrepayment trait
│   ├── factor_correlated.rs  # Factor-correlated CPR model
│   └── richard_roll.rs       # Richard-Roll mortgage prepayment model
│
├── default/            # Stochastic default models
│   ├── mod.rs          # Re-exports
│   ├── spec.rs         # StochasticDefaultSpec configuration
│   ├── traits.rs       # StochasticDefault trait
│   ├── copula_based.rs # Gaussian/t-copula default correlation
│   └── intensity_process.rs  # Intensity-based (reduced-form) defaults
│
├── correlation/        # Correlation structures
│   └── mod.rs          # CorrelationStructure (asset, prepay-default, sector)
│
├── tree/               # Scenario tree infrastructure
│   ├── mod.rs          # Re-exports
│   ├── config.rs       # ScenarioTreeConfig, BranchingSpec
│   ├── node.rs         # ScenarioNode, ScenarioNodeId
│   └── tree.rs         # ScenarioTree, ScenarioPath
│
├── pricer/             # Stochastic pricing engine
│   ├── mod.rs          # Re-exports
│   ├── config.rs       # StochasticPricerConfig, PricingMode
│   ├── engine.rs       # StochasticPricer main engine
│   └── result.rs       # StochasticPricingResult, TranchePricingResult
│
└── metrics/            # Risk metrics and sensitivities
    ├── mod.rs          # Re-exports
    ├── calculator.rs   # StochasticMetricsCalculator
    └── sensitivities.rs # CorrelationSensitivities, SensitivityConfig
```

## Quick Start

### Enable Stochastic Modeling

```rust
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    StructuredCredit, StochasticPrepaySpec, StochasticDefaultSpec,
    CorrelationStructure,
};

// Create a CLO and enable stochastic defaults
let mut clo = StructuredCredit::new_clo(...);
clo.enable_stochastic_defaults(); // Auto-calibrates for deal type

// Or configure manually:
clo.with_stochastic_prepay(StochasticPrepaySpec::clo_standard())
   .with_stochastic_default(StochasticDefaultSpec::clo_standard())
   .with_correlation(CorrelationStructure::clo_standard());
```

### Run Stochastic Pricing

```rust
use finstack_quant_valuations::instruments::fixed_income::structured_credit::stochastic::{
    StochasticPricer, StochasticPricerConfig, PricingMode,
};

let config = StochasticPricerConfig::new()
    .mode(PricingMode::MonteCarlo { paths: 10_000 })
    .enable_greeks(true);

let pricer = StochasticPricer::new(config);
let result = pricer.price(&clo, &market_context, as_of)?;

println!("Expected NPV: {:.2}", result.expected_npv.amount());
println!("VaR(95%): {:.2}", result.var_95.amount());
```

## Key Components

### Prepayment Models

| Model | Use Case | Key Parameters |
|-------|----------|----------------|
| `FactorCorrelatedPrepay` | General ABS/CLO | `rate_sensitivity`, `factor_vol` |
| `RichardRollPrepay` | Agency RMBS | `current_rate`, `burnout_factor` |

### Default Models

| Model | Use Case | Key Parameters |
|-------|----------|----------------|
| `CopulaBasedDefault` | Corporate CLO | `asset_correlation`, `copula_type` |
| `IntensityProcessDefault` | CDS-like modeling | `base_intensity`, `vol`, `mean_reversion` |

### Correlation Structures

Pre-configured structures for common deal types:

```rust
// Auto-configured for deal type
CorrelationStructure::clo_standard()    // ~30% asset correlation, sectored
CorrelationStructure::rmbs_standard()   // ~15% for prime, ~25% subprime
CorrelationStructure::cmbs_standard()   // Property-type driven
CorrelationStructure::abs_auto_standard() // ~10% for consumer auto
```

### Pricing Modes

```rust
// Scenario tree (fast, discrete)
PricingMode::ScenarioTree {
    periods: 40,    // Quarterly for 10 years
    branches: 3,    // Up/mid/down
}

// Monte Carlo (accurate, slower)
PricingMode::MonteCarlo {
    paths: 10_000,  // Number of simulations
}
```

## Output Metrics

The `StochasticPricingResult` provides:

- **Expected NPV**: Average across scenarios
- **NPV Distribution**: Percentiles (1%, 5%, 25%, 50%, 75%, 95%, 99%)
- **VaR/CVaR**: Value at Risk and Conditional VaR
- **Greeks**: Correlation delta, vega (if enabled)
- **Per-Tranche Results**: WAL, duration, spread by scenario

## References

- Li, D. X. (2000). "On Default Correlation: A Copula Function Approach."
- Richard, S. F., & Roll, R. (1989). "Prepayments on Fixed-Rate Mortgage-Backed Securities."
- S&P CDO Evaluator methodology documentation.
- Moody's WARF methodology for CLO analysis.

## See Also

- [`StructuredCredit`](../types/mod.rs) for the main instrument type
- [`instrument_trait`](../instrument_trait.rs) for deterministic cashflow generation
- [`metrics`](../metrics/mod.rs) for standard (non-stochastic) risk metrics
