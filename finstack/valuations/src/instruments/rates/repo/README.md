# Repurchase Agreement (Repo)

## Features

- Term, open, or overnight repos with configurable repo rate, haircut, collateral spec (general vs special), and cashflow schedule.
- Supports explicit collateral notional, haircut application, and optional special-rate adjustments for specials.
- Uses generic discounting pricer for PV of borrow vs. repayment cashflows with curve-driven discounting.
- **GMRA 2011 compliant margining** with mark-to-market, net exposure, and tri-party margin support.

## Methodology & References

- PV computed from deterministic initial cash outflow and discounted repurchase cash inflow; haircut adjusts collateral requirement.
- Collateral type flag allows special-rate adjustment; otherwise treats trade as general collateral.
- Aligns with standard money-market repo conventions and GMRA 2011 margin maintenance standards.

## Usage Example

```rust
use finstack_valuations::instruments::rates::repo::{CollateralSpec, CollateralType, Repo, RepoType};
use finstack_core::{currency::Currency, dates::Date, money::Money, types::CurveId};
use time::Month;

let repo = Repo::builder()
    .id("REPO-1")
    .repo_type(RepoType::Term)
    .get_collateral(CollateralSpec::general())
    .cash_amount(Money::new(10_000_000.0, Currency::USD))
    .start(Date::from_calendar_date(2024, Month::January, 3)?)
    .maturity(Date::from_calendar_date(2024, Month::February, 3)?)
    .rate(0.0525)
    .haircut(0.02)
    .discount_curve_id(CurveId::new("USD-OIS"))
    .attributes(Default::default())
    .build()?;

let pv = repo.value(&market_context, Date::from_calendar_date(2024, Month::January, 3)?)?;
```

---

## Margining

Repo margining is implemented following **GMRA 2011** (Global Master Repurchase Agreement) standards. The margin module ensures that the cash provider remains adequately collateralized throughout the life of the transaction.

### Margin Types

| Type | Description | Use Case |
|------|-------------|----------|
| `None` | No margining - haircut only | Simple bilateral repos |
| `MarkToMarket` | Daily/periodic margin calls | Standard repo margining |
| `NetExposure` | Net margin across netting set | Multiple repos with same counterparty |
| `Triparty` | Agent-managed collateral | BNY Mellon, J.P. Morgan tri-party |

### Adding Margin Specification

```rust
use finstack_margin::{EligibleCollateralSchedule, MarginTenor, RepoMarginSpec, RepoMarginType};
use finstack_valuations::instruments::rates::repo::Repo;

// Create a repo with mark-to-market margining
let mut repo = Repo::example();

// Add GMRA-compliant margin spec
repo.margin_spec = Some(RepoMarginSpec {
    margin_type: RepoMarginType::MarkToMarket,
    margin_ratio: 1.02,              // 102% collateralization required
    margin_call_threshold: 0.01,     // 1% deviation triggers call
    call_frequency: MarginTenor::Daily,
    settlement_lag: 1,               // T+1 settlement
    pays_margin_interest: true,
    margin_interest_rate: Some(0.05), // 5% on cash margin
    substitution_allowed: true,
    eligible_substitutes: Some(EligibleCollateralSchedule::us_treasuries()),
});
```

### Convenience Constructors

```rust
// Simple margin spec (haircut only)
let spec = RepoMarginSpec::none();

// Standard mark-to-market
let spec = RepoMarginSpec::mark_to_market(1.02, 0.01);  // 102% ratio, 1% threshold

// Tri-party repo
let spec = RepoMarginSpec::triparty(1.02);  // Same-day settlement, tighter threshold
```

### Margin Calculations

The `RepoMarginSpec` provides methods for margin management:

```rust
let spec = RepoMarginSpec::mark_to_market(1.02, 0.01);
let cash_amount = 100_000_000.0;

// Required collateral = Cash × Margin Ratio
let required = spec.required_collateral(cash_amount);  // 102,000,000

// Call trigger = Required × (1 - Threshold)
let trigger = spec.call_trigger_value(cash_amount);    // 100,980,000

// Check if margin call needed
let current_collateral = 100_500_000.0;
if spec.requires_margin_call(cash_amount, current_collateral) {
    let deficit = spec.margin_deficit(cash_amount, current_collateral);
    println!("Margin call required: ${:.2}", deficit);
}

// Check excess collateral
let excess = spec.excess_collateral(cash_amount, 105_000_000.0);  // 3,000,000
```

### Using Marginable Trait

Repos implement the `Marginable` trait for integration with the margin framework:

```rust
use finstack_margin::{Marginable, SimmSensitivities};

let repo = Repo::example();
let market = MarketContext::new();
let as_of = Date::from_calendar_date(2024, Month::January, 15)?;

// Check if margin is configured
if repo.has_margin() {
    // Get repo-specific margin spec
    if let Some(spec) = repo.repo_margin_spec() {
        println!("Margin type: {}", spec.margin_type);
        println!("Margin ratio: {:.0}%", spec.margin_ratio * 100.0);
    }

    // Get netting set for aggregation
    let netting_set = repo.netting_set_id();

    // Calculate SIMM sensitivities (mainly short-term IR delta)
    let sensitivities = repo.simm_sensitivities(&market, as_of)?;

    // Get MTM for variation margin
    let mtm = repo.mtm_for_vm(&market, as_of)?;
}
```

### GMRA 2011 References

- **Paragraph 4**: Margin Maintenance mechanics
- **Paragraph 5**: Income Payments on margin transfers
- **Paragraph 8**: Collateral Substitution rules
- **Annex I**: Margin Ratio and Haircut specifications

---

## Limitations / Known Issues

- Fails-to-deliver penalties not modeled.
- Rehypothecation and reinvestment income not modeled.
- Currency/funding bases handled only through supplied discount curve.

## Metrics

- PV, implied repo rate (solve for rate matching price), and haircut-adjusted collateral requirement.
- DV01 on discount curve via generic bump calculators.
- Carry/roll to settlement via day-count accrual of repo interest.
- Margin deficit/excess calculations via `RepoMarginSpec` methods.
