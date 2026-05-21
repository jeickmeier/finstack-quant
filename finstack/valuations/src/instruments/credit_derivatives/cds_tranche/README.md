# CDS Tranche

## Features

- Synthetic CDO tranche instrument with attachment/detachment points, IMM scheduling, and buy/sell protection support.
- Computes PV, par spread, upfront, spread DV01, expected loss, and jump-to-default via the tranche pricer.
- Supports accumulated loss input for seasoned tranches and optional standard IMM scheduling helpers.
- **Multiple copula models**: Gaussian (default), Student-t (tail dependence), Random Factor Loading (stochastic correlation), Multi-factor (sector structure).
- **Stochastic recovery**: Optional market-correlated recovery that decreases in stressed markets.
- **Arbitrage-free base correlation**: Validation and smoothing (isotonic regression, PAVA) for market-consistent curves.

## Methodology & References

- Flexible copula-based engine with Gauss–Hermite integration for tranche expected loss.
- Premium leg handles accrual-on-default and mid-period loss timing consistent with ISDA/CDX conventions.
- Correlation and hazard inputs sourced from `CreditIndexData` in `MarketContext`.

## Copula Models

### One-Factor Gaussian (Default)

Standard market model with single systematic factor. Zero tail dependence.

```text
Aᵢ = √ρ · Z + √(1-ρ) · εᵢ
```

### Student-t Copula

Captures tail dependence - joint extreme defaults more likely than Gaussian predicts.

- Lower degrees of freedom = higher tail dependence
- Typical calibration: df ∈ [4, 10] for CDX tranches

### Random Factor Loading (RFL)

Stochastic correlation - correlation itself is random.

- Correlation typically higher in stressed markets
- Important for senior tranche pricing

### Multi-Factor

Sector-specific correlation structure for bespoke portfolios.

- Global factor + sector-specific factors
- Higher intra-sector vs inter-sector correlation

## Stochastic Recovery

Optional recovery model where recovery negatively correlates with the systematic factor:

```text
R(Z) = μ_R + ρ_R · σ_R · Z
```

where ρ_R < 0 (typically -0.3 to -0.5).

Captures the "double hit" effect: defaults cluster AND recovery falls in stress.

## Usage Example

```rust
use finstack_valuations::instruments::credit_derivatives::cds_tranche::{CDSTranche, CopulaSpec, RecoverySpec};
use finstack_valuations::instruments::credit_derivatives::cds_tranche::pricer::{CDSTranchePricer, CDSTranchePricerConfig};
use finstack_core::dates::Date;
use time::Month;

let as_of = Date::from_calendar_date(2024, Month::January, 5)?;
let tranche = CDSTranche::example();

// Standard Gaussian copula pricing
let pv = tranche.value(&market_context, as_of)?;

// Student-t copula with stochastic recovery
let config = CDSTranchePricerConfig::default()
    .with_student_t_copula(5.0)
    .with_stochastic_recovery();
let pricer = CDSTranchePricer::with_params(config);
let pv_stress = pricer.price_tranche(&tranche, &market_context, as_of)?;
```

## Arbitrage-Free Base Correlation

Base correlation curves are validated for:

1. **Monotonicity**: β(K₁) ≤ β(K₂) for K₁ < K₂
2. **Valid bounds**: 0 ≤ β(K) ≤ 1

Smoothing methods available:

- **Isotonic Regression (PAVA)**: Optimal L2 fit with monotonicity constraint
- **Strict Monotonic**: Simple forward enforcement
- **Weighted Smoothing**: Preserves curve shape while enforcing constraints

## Pricing Methodology

- Base-correlation copula: computes equity tranche EL curve, then derives [A,D] tranche EL via detachment/attachment differences.
- Protection/premium legs discounted on quote curve; accrual-on-default handled mid-period with Gauss–Hermite integration for accuracy near extreme correlations.
- Par spread solved via Newton-Raphson using tranche RPV01; supports accumulated loss input and IMM scheduling.

## Metrics

- PV (buyer/seller), par spread, upfront, spread DV01, expected loss, jump-to-default, and correlation delta via finite differences.
- Premium vs protection leg PV breakdown; tranche notional outstanding profiles.
- **Tail dependence coefficient** (λ_L): Indicates copula's ability to capture joint extreme defaults.
- Correlation and recovery sensitivities (Correlation01, Recovery01).
- Arbitrage validation results with detailed violation reporting.
