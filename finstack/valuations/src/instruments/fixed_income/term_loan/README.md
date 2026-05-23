# Term loan

Institutional term loans: DDTL, PIK, covenant-driven events, amortization, and embedded calls.

## Contents

- [Overview](#overview)
- [Module Structure](#module-structure)
- [Feature Set](#feature-set)
- [Usage Examples](#usage-examples)
- [Metrics and Analytics](#metrics-and-analytics)
- [Extending the Module](#extending-the-module)
- [Technical Architecture](#technical-architecture)

---

## Overview

The `term_loan` module provides deterministic cashflow generation, pricing, and risk analytics for institutional term loans, including leveraged loans, middle-market lending, and delayed-draw facilities. It implements industry-standard conventions for amortization, PIK capitalization, commitment fees, and covenant-driven events.

### Key Capabilities

- **Fixed and floating rate** interest (SOFR, LIBOR, custom indices with floors/caps)
- **Delayed-draw term loans (DDTL)** with commitment periods, step-downs, and fees
- **Payment-in-kind (PIK)** with dynamic toggles and split cash/PIK structures
- **Amortization schedules**: bullet, linear, percent-per-period, or custom
- **Covenant-driven events**: margin step-ups, cash sweeps, PIK toggles, draw restrictions
- **Original issue discount (OID)** with withheld or separate tracking
- **Borrower call schedules** with premium-to-par step-downs
- **Risk metrics**: DV01, CS01, YTM, YTC, YTW, discount margin, all-in rate

---

## Module Structure

```
term_loan/
├── mod.rs                  # Public API and re-exports
├── spec.rs                 # Serde-stable specifications (TermLoanSpec, DdtlSpec, TermLoanCovenantEvents)
├── types.rs                # Core instrument type (TermLoan) and trait implementations
├── cashflows.rs            # Deterministic cashflow generation engine
├── pricing.rs              # Discounting pricer
└── metrics/
    ├── mod.rs              # Metric registration
    ├── ytm.rs              # Yield to maturity
    ├── ytc.rs              # Yield to call
    ├── ytw.rs              # Yield to worst
    ├── ytn.rs              # Yield to N-year horizons (2Y, 3Y, 4Y)
    ├── discount_margin.rs  # Spread over reference curve
    ├── all_in_rate.rs      # All-in cost of funds
    └── irr_helpers.rs      # Shared IRR solving utilities
```

### Key Types

- **`TermLoan`**: Main instrument type (constructed via builder pattern)
- **`TermLoanSpec`**: Serializable specification for persistent storage
- **`RateSpec`**: Fixed or floating rate specification
- **`DdtlSpec`**: Delayed-draw features (commitment, draws, fees)
- **`TermLoanCovenantEvents`**: Covenant-driven events (sweeps, toggles, margin step-ups)
- **`AmortizationSpec`**: Principal repayment schedule

---

## Feature Set

### 1. Interest Rate Structures

#### Fixed Rate

```rust
RateSpec::Fixed { rate_bp: 600 }  // 6.00% fixed
```

#### Floating Rate with Floor/Cap

```rust
RateSpec::Floating(FloatingRateSpec {
    index_id: CurveId::new("USD-SOFR-3M"),
    spread_bp: 300.0,        // +300 bps spread
    gearing: 1.0,
    index_floor_bp: Some(0.0),     // 0% floor
    all_in_cap_bp: Some(500.0),     // 5% cap
    reset_freq: Tenor::quarterly(),
    reset_lag_days: 2,
    dc: DayCount::Act360,
    bdc: BusinessDayConvention::ModifiedFollowing,
    calendar_id: None,
})
```

### 2. Amortization Schedules

#### Bullet Loan (No Amortization)

```rust
amortization: AmortizationSpec::None
```

#### Linear Amortization

```rust
amortization: AmortizationSpec::Linear {
    start: create_date(2026, Month::January, 1)?,
    end: create_date(2030, Month::January, 1)?,
}
```

#### Percent Per Period

```rust
amortization: AmortizationSpec::PercentPerPeriod {
    bp: 250,  // 2.5% of original notional per payment period
}
```

#### Custom Schedule

```rust
amortization: AmortizationSpec::Custom(vec![
    (create_date(2026, Month::June, 30)?, Money::new(1_000_000.0, Currency::USD)),
    (create_date(2027, Month::June, 30)?, Money::new(2_000_000.0, Currency::USD)),
])
```

### 3. Delayed-Draw Term Loans (DDTL)

Supports:

- **Commitment periods** with availability windows
- **Draw schedules** (scheduled or actual)
- **Commitment step-downs** (reducing availability over time)
- **Commitment fees** (on undrawn amounts)
- **Usage fees** (on drawn amounts)
- **Original issue discount (OID)** with withheld or separate tracking

```rust
ddtl: Some(DdtlSpec {
    commitment_limit: Money::new(50_000_000.0, Currency::USD),
    availability_start: create_date(2025, Month::January, 1)?,
    availability_end: create_date(2026, Month::January, 1)?,
    draws: vec![
        DrawEvent {
            date: create_date(2025, Month::March, 15)?,
            amount: Money::new(20_000_000.0, Currency::USD),
        },
        DrawEvent {
            date: create_date(2025, Month::September, 15)?,
            amount: Money::new(15_000_000.0, Currency::USD),
        },
    ],
    commitment_step_downs: vec![
        CommitmentStepDown {
            date: create_date(2025, Month::July, 1)?,
            new_limit: Money::new(40_000_000.0, Currency::USD),
        },
    ],
    usage_fee_bp: 50,       // 50 bps on drawn amounts
    commitment_fee_bp: 25,  // 25 bps on undrawn
    fee_base: CommitmentFeeBase::Undrawn,
    oid_policy: Some(OidPolicy::WithheldPct(200)),  // 2% withheld
})
```

### 4. Payment-in-Kind (PIK)

Supports:

- **Full PIK**: All interest capitalized
- **Split PIK**: Partial cash, partial capitalization
- **Dynamic toggles**: Covenant or date-driven PIK activation

```rust
// Full PIK
coupon_type: CouponType::PIK

// Split PIK (60% cash, 40% PIK)
coupon_type: CouponType::Split {
    cash_pct: 0.6,
    pik_pct: 0.4,
}

// Dynamic PIK toggles via covenants
covenants: Some(TermLoanCovenantEvents {
    pik_toggles: vec![
        PikToggle {
            date: create_date(2026, Month::June, 30)?,
            enable_pik: true,  // Switch to PIK after this date
        },
    ],
    ..Default::default()
})
```

### 5. Covenant-Driven Events

```rust
covenants: Some(TermLoanCovenantEvents {
    // Margin step-ups (covenant penalties or rating migrations)
    margin_stepups: vec![
        MarginStepUp {
            date: create_date(2026, Month::December, 31)?,
            delta_bp: 200,  // +200 bps penalty
        },
    ],

    // Cash sweeps (mandatory prepayments)
    cash_sweeps: vec![
        CashSweepEvent {
            date: create_date(2027, Month::June, 30)?,
            amount: Money::new(5_000_000.0, Currency::USD),
        },
    ],

    // PIK toggles
    pik_toggles: vec![
        PikToggle {
            date: create_date(2026, Month::June, 30)?,
            enable_pik: true,
        },
    ],

    // Draw stop dates (covenant breach)
    draw_stop_dates: vec![
        create_date(2026, Month::March, 31)?,
    ],
})
```

### 6. Borrower Call Options

```rust
call_schedule: Some(LoanCallSchedule {
    calls: vec![
        LoanCall {
            date: create_date(2027, Month::January, 15)?,
            price_pct_of_par: 103.0,  // 3% premium in year 2
        },
        LoanCall {
            date: create_date(2028, Month::January, 15)?,
            price_pct_of_par: 101.5,  // 1.5% premium in year 3
        },
        LoanCall {
            date: create_date(2029, Month::January, 15)?,
            price_pct_of_par: 100.0,  // At par thereafter
        },
    ],
})
```

---

## Usage Examples

### Example 1: Plain Vanilla Term Loan (Fixed Rate, Bullet)

```rust
use finstack_valuations::instruments::fixed_income::term_loan::*;
use finstack_core::money::Money;
use finstack_core::currency::Currency;
use finstack_core::dates::*;
use finstack_core::types::{InstrumentId, CurveId};
use time::Month;

let loan = TermLoan::builder()
    .id(InstrumentId::new("TL-001"))
    .currency(Currency::USD)
    .notional_limit(Money::new(100_000_000.0, Currency::USD))
    .issue(create_date(2025, Month::January, 15)?)
    .maturity(create_date(2030, Month::January, 15)?)
    .rate(RateSpec::Fixed { rate_bp: 550 })  // 5.50%
    .pay_freq(Tenor::quarterly())
    .day_count(DayCount::Act360)
    .bdc(BusinessDayConvention::ModifiedFollowing)
    .stub(StubKind::None)
    .discount_curve_id(CurveId::new("USD-CREDIT"))
    .amortization(AmortizationSpec::None)  // Bullet
    .coupon_type(CouponType::Cash)
    .build()?;

// Price the loan
let pv = loan.value(&market_context, as_of_date)?;

// Generate cashflows
let cashflows = loan.dated_cashflows(&market_context, as_of_date)?;
```

### Example 2: Floating Rate TL with Linear Amortization

```rust
let floating_spec = FloatingRateSpec {
    index_id: CurveId::new("USD-SOFR-3M"),
    spread_bp: 300.0,
    gearing: 1.0,
    index_floor_bp: Some(0.0),
    all_in_cap_bp: None,
    reset_freq: Tenor::quarterly(),
    reset_lag_days: 2,
    dc: DayCount::Act360,
    bdc: BusinessDayConvention::ModifiedFollowing,
    calendar_id: None,
};

let loan = TermLoan::builder()
    .id(InstrumentId::new("TL-FLOAT-001"))
    .currency(Currency::USD)
    .notional_limit(Money::new(75_000_000.0, Currency::USD))
    .issue(create_date(2025, Month::June, 1)?)
    .maturity(create_date(2032, Month::June, 1)?)
    .rate(RateSpec::Floating(floating_spec))
    .pay_freq(Tenor::quarterly())
    .day_count(DayCount::Act360)
    .bdc(BusinessDayConvention::ModifiedFollowing)
    .stub(StubKind::None)
    .discount_curve_id(CurveId::new("USD-CREDIT"))
    .amortization(AmortizationSpec::Linear {
        start: create_date(2028, Month::June, 1)?,
        end: create_date(2032, Month::June, 1)?,
    })
    .coupon_type(CouponType::Cash)
    .build()?;
```

### Example 3: DDTL with Commitment Fees

```rust
let ddtl_spec = DdtlSpec {
    commitment_limit: Money::new(200_000_000.0, Currency::USD),
    availability_start: create_date(2025, Month::January, 1)?,
    availability_end: create_date(2027, Month::January, 1)?,
    draws: vec![
        DrawEvent {
            date: create_date(2025, Month::April, 1)?,
            amount: Money::new(80_000_000.0, Currency::USD),
        },
        DrawEvent {
            date: create_date(2026, Month::January, 1)?,
            amount: Money::new(60_000_000.0, Currency::USD),
        },
    ],
    commitment_step_downs: vec![],
    usage_fee_bp: 50,
    commitment_fee_bp: 35,
    fee_base: CommitmentFeeBase::Undrawn,
    oid_policy: Some(OidPolicy::WithheldPct(150)),  // 1.5% OID
};

let loan = TermLoan::builder()
    .id(InstrumentId::new("DDTL-001"))
    .currency(Currency::USD)
    .notional_limit(Money::new(200_000_000.0, Currency::USD))
    .issue(create_date(2025, Month::January, 1)?)
    .maturity(create_date(2032, Month::January, 1)?)
    .rate(RateSpec::Floating(floating_spec))
    .pay_freq(Tenor::quarterly())
    .day_count(DayCount::Act360)
    .bdc(BusinessDayConvention::ModifiedFollowing)
    .stub(StubKind::None)
    .discount_curve_id(CurveId::new("USD-CREDIT"))
    .amortization(AmortizationSpec::None)
    .coupon_type(CouponType::Cash)
    .ddtl_opt(Some(ddtl_spec))
    .build()?;
```

### Example 4: PIK Loan with Toggle

```rust
let covenant_spec = TermLoanCovenantEvents {
    pik_toggles: vec![
        PikToggle {
            date: create_date(2027, Month::June, 30)?,
            enable_pik: true,  // Switch to PIK if covenant breached
        },
    ],
    margin_stepups: vec![
        MarginStepUp {
            date: create_date(2027, Month::June, 30)?,
            delta_bp: 250,  // +250 bps penalty
        },
    ],
    ..Default::default()
};

let loan = TermLoan::builder()
    .id(InstrumentId::new("TL-PIK-001"))
    .currency(Currency::USD)
    .notional_limit(Money::new(50_000_000.0, Currency::USD))
    .issue(create_date(2025, Month::January, 1)?)
    .maturity(create_date(2030, Month::January, 1)?)
    .rate(RateSpec::Fixed { rate_bp: 900 })  // 9% (high-yield)
    .pay_freq(Tenor::semi_annual())
    .day_count(DayCount::Thirty360)
    .bdc(BusinessDayConvention::ModifiedFollowing)
    .stub(StubKind::None)
    .discount_curve_id(CurveId::new("USD-CREDIT"))
    .amortization(AmortizationSpec::None)
    .coupon_type(CouponType::Split {
        cash_pct: 0.5,
        pik_pct: 0.5,
    })
    .covenants_opt(Some(covenant_spec))
    .build()?;
```

### Example 5: Computing Metrics

```rust
use finstack_valuations::metrics::MetricId;

// Compute PV
let pv = loan.value(&market_context, as_of_date)?;

// Compute full analytics
let metrics = vec![
    MetricId::Ytm,
    MetricId::custom("ytw"),
    MetricId::custom("ytc"),
    MetricId::DiscountMargin,
    MetricId::custom("all_in_rate"),
    MetricId::Dv01,
    MetricId::BucketedDv01,
    MetricId::Cs01,
];

let result = loan.price_with_metrics(&market_context, as_of_date, &metrics)?;

println!("PV: {}", result.pv);
println!("YTM: {:?}", result.metrics.get(&MetricId::Ytm));
println!("DV01: {:?}", result.metrics.get(&MetricId::Dv01));
```

---

## Metrics and Analytics

### Supported Metrics

| Metric ID | Description | Implementation |
|-----------|-------------|----------------|
| `ytm` | Yield to maturity (IRR to final maturity) | `YtmCalculator` |
| `ytc` | Yield to first call (IRR to earliest call date) | `YtcCalculator` |
| `ytw` | Yield to worst (minimum of YTM, YTC, YT2Y, YT3Y, YT4Y) | `YtwCalculator` |
| `yt2y` | Yield to 2-year horizon | `Yt2yCalculator` |
| `yt3y` | Yield to 3-year horizon | `Yt3yCalculator` |
| `yt4y` | Yield to 4-year horizon | `Yt4yCalculator` |
| `discount_margin` | Spread over reference curve (for floating-rate loans) | `DiscountMarginCalculator` |
| `all_in_rate` | All-in cost of funds (includes fees, OID, and margin) | `AllInRateCalculator` |
| `dv01` | Interest rate sensitivity (parallel shift) | `UnifiedDv01Calculator` |
| `bucketed_dv01` | Key rate duration (bucketed sensitivity) | `UnifiedDv01Calculator` |
| `cs01` | Credit spread sensitivity (parallel) | `GenericParallelCs01` |
| `bucketed_cs01` | Credit spread sensitivity (bucketed) | `GenericBucketedCs01` |
| `theta` | Time decay (1-day roll-down) | Universal metric (registered globally) |

### Metric Calculation Examples

```rust
// Yield to maturity
let ytm_calc = YtmCalculator;
let ytm = ytm_calc.compute(&loan, &market_context, as_of_date)?;

// DV01 (parallel shift sensitivity)
let dv01_calc = UnifiedDv01Calculator::<TermLoan>::new(
    Dv01CalculatorConfig::parallel_combined()
);
let dv01 = dv01_calc.compute(&loan, &market_context, as_of_date)?;

// Bucketed DV01 (key rate duration)
let bucketed_dv01_calc = UnifiedDv01Calculator::<TermLoan>::new(
    Dv01CalculatorConfig::triangular_key_rate()
);
let bucketed = bucketed_dv01_calc.compute(&loan, &market_context, as_of_date)?;
```

---

## Extending the Module

### Adding a New Metric

1. **Create metric calculator** in `metrics/your_metric.rs`:

```rust
use crate::instruments::fixed_income::term_loan::TermLoan;
use crate::metrics::{MetricCalculator, MetricValue};
use finstack_core::market_data::context::MarketContext;
use finstack_core::dates::Date;

pub struct YourMetricCalculator;

impl MetricCalculator<TermLoan> for YourMetricCalculator {
    fn compute(
        &self,
        loan: &TermLoan,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<MetricValue> {
        // Your implementation here

        // Return scalar or vector metric
        Ok(MetricValue::Scalar(your_result))
    }
}
```

1. **Register metric** in `metrics/mod.rs`:

```rust
pub use your_metric::YourMetricCalculator;

pub fn register_term_loan_metrics(registry: &mut MetricRegistry) {
    // ... existing registrations ...

    registry.register_metric(
        MetricId::custom("your_metric"),
        Arc::new(YourMetricCalculator),
        &["TermLoan"],
    );
}
```

1. **Add tests** in `tests/instruments/term_loan/metrics/your_metric.rs`

### Adding a New Amortization Type

1. **Extend `AmortizationSpec` enum** in `spec.rs`:

```rust
pub enum AmortizationSpec {
    None,
    Linear { start: Date, end: Date },
    PercentPerPeriod { bp: i32 },
    Custom(Vec<(Date, Money)>),

    // Your new type:
    YourNewType {
        param1: Date,
        param2: f64,
    },
}
```

1. **Implement amortization logic** in `cashflows.rs` (in the `generate_cashflows` function, within the amortization match block):

```rust
match &loan.amortization {
    // ... existing variants ...

    AmortizationSpec::YourNewType { param1, param2 } => {
        // Calculate amortization payment
        let pay = calculate_your_amortization_logic(
            current_outstanding,
            param1,
            param2,
            d,
        );

        if pay.amount() > 0.0 {
            flows.push(CashFlow {
                date: d,
                reset_date: None,
                amount: pay,
                kind: CFKind::Amortization,
                accrual_factor: 0.0,
                rate: None,
            });

            principal_events.push(PrincipalEvent {
                date: d,
                delta: Money::new(-pay.amount(), pay.currency()),
            });
        }
    }
}
```

1. **Add validation** (if needed) and **tests**

### Adding a New Covenant Event Type

1. **Add field to `TermLoanCovenantEvents`** in `spec.rs`:

```rust
pub struct TermLoanCovenantEvents {
    pub margin_stepups: Vec<MarginStepUp>,
    pub pik_toggles: Vec<PikToggle>,
    pub cash_sweeps: Vec<CashSweepEvent>,
    pub draw_stop_dates: Vec<Date>,

    // Your new covenant event:
    pub your_events: Vec<YourEvent>,
}
```

1. **Define event struct**:

```rust
#[derive(Debug, Clone)]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YourEvent {
    pub date: Date,
    pub parameter: YourParam,
}
```

1. **Implement event logic** in `cashflows.rs` (within the date iteration loop)

2. **Add tests** demonstrating the new feature

### Supporting a New Rate Index

Floating rate indices are handled via the core library's `FloatingRateSpec`. To add support for a new index:

1. **Ensure curve exists** in `MarketContext` with the appropriate curve ID
2. **Use standard `FloatingRateSpec`**:

```rust
RateSpec::Floating(FloatingRateSpec {
    index_id: CurveId::new("YOUR-NEW-INDEX"),
    spread_bp: 250.0,
    // ... other params
})
```

No code changes to term_loan module required—the cashflow engine uses `project_floating_rate_from_market()` which supports any curve ID.

---

## Technical Architecture

### Cashflow Sign Conventions

The module uses a **funding-leg model** for internal cashflow representation:

- **Draws (funding)**: Negative `CFKind::Notional` flows (cash out from lender)
- **Redemptions**: Positive `CFKind::Notional` flows (cash in to lender)
- **Amortization**: Positive `CFKind::Amortization` flows (reduce outstanding)
- **PIK capitalization**: Positive `CFKind::PIK` flows (increase outstanding)
- **Interest/fees**: Positive flows (cash in to lender)

The `Notional.initial` is set to **0**, and outstanding principal is computed dynamically via `compute_outstanding_at()` by folding all principal events.

### Internal View vs. Holder View

- **Internal engine** (crate-private `generate_cashflows()`): Full schedule including funding legs
- **Public holder view** (`CashflowProvider::cashflow_schedule()` and `CashflowProvider::dated_cashflows()`): Contractual lender-facing flows only

### PIK Treatment in Pricing

PIK interest is **capitalized** into outstanding principal and **excluded from PV calculation**. Only cash flows are discounted:

- Coupons (Fixed, FloatReset, Stub)
- Amortization
- Redemptions (Notional)
- Fees

PIK capitalization affects the outstanding principal path and is reflected in the final redemption amount.

### Determinism Guarantees

- All cashflows are deterministic (no Monte Carlo)
- Decimal precision throughout (via `rust_decimal` and `Money`)
- Stable ordering: flows sorted by date, then by kind rank
- Parallel/serial equivalence for metrics (when using Decimal mode)

### Serde Stability

All specification types (`TermLoanSpec`, `DdtlSpec`, `TermLoanCovenantEvents`, etc.) use:

- `#[serde(deny_unknown_fields)]` for strict deserialization
- Stable field names for long-lived pipelines and golden tests

### Integration Points

- **Market data**: Requires discount curve (via `discount_curve_id`) and floating rate indices
- **Pricer registry**: Registered as `InstrumentType::TermLoan` with `ModelKey::Discounting`
- **Metric registry**: All metrics registered in `register_term_loan_metrics()`
- **Scenarios**: Full support via `Instrument` trait and attributes
- **Portfolio**: Currency-safe aggregation via explicit FX policies

---

## Testing

### Test Coverage

- **Unit tests**: Individual functions (amortization logic, margin calculations, etc.)
- **Integration tests**: Full cashflow generation and pricing (`tests/instruments/term_loan/`)
- **Property tests**: Invariants (e.g., PV decreases with higher discount rate)
- **Market standards**: Validation against industry conventions (`validation/market_standards.rs`)
- **Golden tests**: Regression testing with stable snapshots

### Running Tests

```bash
# All term loan tests
cargo test --package finstack-valuations term_loan

# Specific test module
cargo test --package finstack-valuations term_loan::cashflows

# With coverage
cargo tarpaulin --packages finstack-valuations --exclude-files "**/tests/*"
```

---

## References and Standards

- **Day count conventions**: Act/360, Act/365, 30/360 (ISDA standard)
- **Business day adjustments**: Following, Modified Following, Preceding (ISDA)
- **Floating rate conventions**: SOFR, historical LIBOR, custom indices
- **OID accounting**: GAAP/IFRS effective interest rate method (via `OidEirSpec` reporting schedules)
- **PIK capitalization**: Capitalized interest is excluded from PV and repaid via principal.

---

## Limitations / Known Issues

- Deterministic cashflow engine only; no stochastic credit or rate simulation within this module.
- EIR amortization schedules are reporting-only (do not change PV/metrics).
- Covenant modeling is limited to the provided toggles/step-ups/sweeps; bespoke legal triggers require extensions.
- Pricing excludes funding-side adjustments (FVA/CVA/DVA) and assumes single-currency loans.

---

## Pricing Methodology

- Deterministic cashflow engine builds funding, coupon, amortization, fees, OID, PIK, and call events using schedule/covenant specs.
- Discounting via loan discount curve on holder-view cashflows; floating coupons projected off reference curves with floors/caps and reset lags.
- Metrics like YTM/YTC/YTW solved via IRR on filtered cashflows; discount margin/all-in rate solved iteratively against spreads.

## Metrics

- Core metrics: PV, YTM/YTC/YTW/Yn-year, discount margin, all-in rate, DV01/bucketed DV01, CS01/bucketed CS01, theta.
- Accounting outputs: OID EIR amortization (`oid_eir_amortization`) and carrying value (`oid_eir_carrying_value`) series.
- Loan-specific outputs: outstanding balance path, amortization/call cashflow breakdowns, covenant-triggered event reporting.
- Supports custom metrics via registry; IRR helpers for yield solving shared in `metrics/irr_helpers.rs`.
