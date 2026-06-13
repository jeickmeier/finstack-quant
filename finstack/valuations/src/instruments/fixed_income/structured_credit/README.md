# Structured Credit Module

Unified implementation for ABS, RMBS, CMBS, and CLO instruments with waterfall modeling, behavioral assumptions, and comprehensive risk metrics.

## Quick Reference

```rust
use finstack_valuations::instruments::fixed_income::structured_credit::prelude::*;

// Create a CLO
let clo = StructuredCredit::new_clo(
    "MY_CLO", pool, tranches,
    closing_date, legal_maturity, "USD-OIS",
);

// Value the deal
let pv = clo.value(&market_context, as_of)?;

// Get metrics
let result = clo.price_with_metrics(&context, as_of, &[
    MetricId::WAL, MetricId::DurationMod, MetricId::Cs01,
])?;
```

## Module Structure

```
structured_credit/
├── mod.rs              # Main module with exports and documentation
├── pricer.rs           # StructuredCreditDiscountingPricer
├── README.md           # This file
│
├── types/              # All data structures
│   ├── mod.rs          # StructuredCredit struct, trait implementations
│   ├── constructors.rs # new_abs(), new_clo(), new_cmbs(), new_rmbs()
│   ├── constants.rs    # Industry constants (PSA, SDA, fees, etc.)
│   ├── enums.rs        # DealType, AssetType, TrancheSeniority, etc.
│   ├── pool.rs         # AssetPool, PoolAsset, ReinvestmentPeriod
│   ├── tranches.rs     # Tranche, TrancheStructure, TrancheCoupon
│   ├── waterfall.rs    # Waterfall, WaterfallTier, Recipient
│   ├── results.rs      # TrancheCashflows, TrancheValuation
│   ├── setup.rs        # DealConfig, DealDates, DealFees
│   ├── reinvestment.rs # ReinvestmentManager for CLO reinvestment periods
│   └── stochastic.rs   # Stochastic configuration helpers
│
├── pricing/            # Pricing and cashflow projection (pure functions)
│   ├── mod.rs          # Re-exports
│   ├── deterministic.rs # Core simulation loop
│   ├── waterfall.rs    # Waterfall execution logic
│   ├── coverage_tests.rs # OC/IC test calculations
│   ├── diversion.rs    # Diversion rules with cycle detection
│   └── stochastic/     # Stochastic pricing
│       ├── prepayment/ # Factor-correlated, Richard-Roll models
│       ├── default/    # Copula-based, intensity process models
│       ├── correlation/ # Correlation structures
│       ├── tree/       # Scenario tree infrastructure
│       ├── pricer/     # Stochastic pricing engine
│       └── metrics/    # Risk metrics and sensitivities
│
├── metrics/            # Risk metrics by category
│   ├── mod.rs          # Registration function, re-exports
│   ├── pricing/        # WAL, accrued, clean/dirty prices
│   ├── risk/           # Duration, spreads, DV01, CS01, sensitivities
│   ├── pool/           # WARF, WAS, CPR, CDR pool characteristics
│   └── deal_specific/  # ABS, CLO, CMBS, RMBS-specific metrics
│
├── utils/              # Helper functions
│   ├── mod.rs          # Re-exports
│   ├── rates.rs        # cpr_to_smm, cdr_to_mdr, psa_to_cpr
│   ├── rate_helpers.rs # Floating rate projection helpers
│   ├── simulation.rs   # RecoveryQueue, PeriodFlows
│   └── validation.rs   # Waterfall validation
│
└── templates/          # Pre-built deal templates
```

## Deal Types

| Type | Collateral | Prepayment Model | Key Metrics |
|------|------------|------------------|-------------|
| **ABS** | Auto loans, credit cards | CPR/ABS speed | Charge-off, delinquency |
| **CLO** | Leveraged loans | 15% CPR default | WARF, WAS, covenant |
| **CMBS** | Commercial mortgages | 10% CPR (lockout) | DSCR, LTV |
| **RMBS** | Residential mortgages | PSA/SDA curves | FICO, LTV |

## Behavioral Models

### Deterministic Models

Single-path models for standard valuation (from `types::DefaultModelSpec`, `types::PrepaymentModelSpec`):

| Model | Type | Use Case |
|-------|------|----------|
| **PSA** | Prepayment | RMBS standard ramp curve |
| **Constant CPR** | Prepayment | Flat annual rate |
| **SDA** | Default | RMBS standard ramp curve |
| **Constant CDR** | Default | Flat annual rate |
| **Constant Recovery** | Recovery | Fixed rate with lag |

### Stochastic Models

Multi-path simulation for advanced analytics (from `pricing::stochastic`):

| Model | Type | Use Case |
|-------|------|----------|
| **Copula-based** | Default | Correlated defaults, tail risk |
| **Intensity process** | Default | Time-varying hazard rates |
| **Factor-correlated** | Prepayment | Rate-sensitive prepayment |
| **Richard-Roll** | Prepayment | Mortgage refinancing model |

### When to Use Each

| Scenario | Use |
|----------|-----|
| Day-to-day valuation | Deterministic |
| Regulatory reporting | Deterministic |
| VaR / Expected Shortfall | Stochastic |
| Correlation risk analysis | Stochastic |
| Stress testing | Either (depends on complexity) |

## Waterfall Mechanics

```
Pool Cashflows → Fees → Senior Interest → Subordinate Interest → Principal → Equity
                        ↓ (if OC/IC fail)
                        Turbo to Senior
```

## Key APIs

### Creating Instruments

```rust
use finstack_valuations::instruments::fixed_income::structured_credit::prelude::*;

// With defaults
let clo = StructuredCredit::new_clo(id, pool, tranches, close, maturity, curve);
let abs = StructuredCredit::new_abs(...);
let cmbs = StructuredCredit::new_cmbs(...);
let rmbs = StructuredCredit::new_rmbs(...);

// Using builder
let deal = StructuredCredit::builder()
    .id("DEAL_ID")
    .deal_type(DealType::CLO)
    .pool(pool)
    .tranches(tranches)
    // ...
    .build()?;
```

### Valuation

```rust
// Simple NPV
let pv = deal.value(&context, as_of)?;

// With metrics
let result = deal.price_with_metrics(&context, as_of, &[MetricId::WAL])?;

// Per-tranche
let tranche_val = deal.value_tranche_with_metrics("CLASS_A", &context, as_of, &metrics)?;
```

### Waterfall Execution

```rust
use finstack_valuations::instruments::fixed_income::structured_credit::{
    WaterfallBuilder, WaterfallTier, Recipient, PaymentType, AllocationMode,
    execute_waterfall,
};

// Build waterfall
let waterfall = WaterfallBuilder::new(Currency::USD)
    .add_tier(
        WaterfallTier::new("fees", 1, PaymentType::Fee)
            .add_recipient(Recipient::fixed_fee("trustee", "Trustee", Money::new(25_000.0, Currency::USD)))
    )
    .add_tier(
        WaterfallTier::new("interest", 2, PaymentType::Interest)
            .allocation_mode(AllocationMode::Sequential)
            .add_recipient(Recipient::tranche_interest("A_INT", "CLASS_A"))
    )
    .build();

// Execute (free function API)
let period_start = last_payment_date;
let result = execute_waterfall(
    &waterfall, available_cash, interest_collections, payment_date, period_start,
    &tranches, pool_balance, &pool, &market,
)?;
```

### Stochastic Pricing

```rust
use finstack_valuations::instruments::fixed_income::structured_credit::pricing::stochastic::{
    StochasticPrepaySpec, StochasticDefaultSpec, CorrelationStructure,
};

// Enable stochastic models
deal.enable_stochastic_defaults();

// Or configure manually
deal.with_stochastic_prepay(StochasticPrepaySpec::rmbs_agency(0.045))
    .with_stochastic_default(StochasticDefaultSpec::copula(0.02, 0.25))
    .with_correlation(CorrelationStructure::rmbs_standard());
```

### Rate Conversions

```rust
use finstack_valuations::instruments::fixed_income::structured_credit::{
    cpr_to_smm, smm_to_cpr, cdr_to_mdr, mdr_to_cdr, psa_to_cpr,
};

let smm = cpr_to_smm(0.06);       // 6% annual CPR → monthly SMM
let cpr = psa_to_cpr(1.5, 30);    // 150% PSA at month 30
let mdr = cdr_to_mdr(0.02);       // 2% annual CDR → monthly MDR
```

## Configuration

### Registry-backed assumptions (`data/assumptions/structured_credit_assumptions.v1.json`)

- Fee defaults
- PSA/SDA model parameters
- Concentration limits

### Structures (`types/setup.rs`)

- `DealConfig` - Complete deal configuration
- `DealDates` - Key dates
- `DealFees` - Fee structure by deal type
- `CoverageTestConfig` - OC/IC triggers
- `DefaultAssumptions` - Behavioral assumptions

## Market Conventions

### Day-Count Conventions

Interest accrual uses proper day-count conventions throughout:

- **Tranche interest**: Uses each tranche's `day_count` field (typically ACT/360)
- **Pool interest collections**: Uses asset-level day-count when available, defaults to ACT/360 for loans
- **Coverage tests**: Uses tranche payment frequency for IC calculations

### Payment Frequencies

The module respects tranche-specific payment frequencies rather than assuming quarterly:

- **ABS**: Typically monthly (`Tenor::monthly()`)
- **CLO**: Typically quarterly (`Tenor::quarterly()`)
- **CMBS**: Typically monthly (`Tenor::monthly()`)
- **RMBS**: Typically monthly (`Tenor::monthly()`)

Use `utils::frequency_periods_per_year(tenor)` to convert tenors to periods per year.

## Coverage Triggers

Two types of coverage triggers are available:

### Tranche-Level Triggers (`CoverageTrigger`)

Used for tranche-specific OC/IC thresholds:

```rust
use finstack_valuations::instruments::fixed_income::structured_credit::{
    CoverageTrigger, TriggerConsequence,
};

let trigger = CoverageTrigger::new(1.20, TriggerConsequence::DivertCashFlow)
    .with_cure_level(1.25);
```

### Waterfall-Level Triggers (`CoverageTrigger`)

Used when building waterfalls with coverage test diversion:

```rust
use finstack_valuations::instruments::fixed_income::structured_credit::waterfall::CoverageTrigger;

let waterfall = WaterfallBuilder::new(Currency::USD)
    // ... add tiers ...
    .add_coverage_trigger(CoverageTrigger {
        tranche_id: "CLASS_A".into(),
        oc_trigger: Some(1.25),
        ic_trigger: Some(1.20),
    })
    .build();
```

See `mod.rs` for module-level rustdoc (waterfall mechanics, pricing, behavioral models, references) and `pricing/stochastic/README.md` for stochastic modeling.

## Related Crates

- `finstack_core::dates` - Schedule generation, day counts
- `finstack_core::money` - Currency-safe arithmetic
- `finstack_valuations::metrics` - Metric registry and calculators
