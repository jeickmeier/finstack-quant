//! Metrics-based P&L attribution.
//!
//! Fast approximation using pre-computed risk metrics (Theta, DV01, CS01, Vega, etc.)
//! to estimate factor contributions without full repricing. Supports both first-order
//! (linear) and second-order (convexity) terms for improved accuracy.
//!
//! # Algorithm
//!
//! 1. **Carry**: Total-return Theta over the matching attribution horizon
//! 2. **RatesCurves**:
//!    - Per-curve (if BucketedDv01 available): Σ(DV01_i × Δr_i) for each curve i
//!    - Fallback (aggregate DV01): DV01 × avg(Δr_i)
//!    - Second-order: ½ × Convexity × (Δr)² (if available)
//!   3. **CreditCurves**:
//!    - First-order: CS01 × Δs
//!    - Second-order: ½ × CS-Gamma × (Δs)² (if available)
//!   4. **Fx**: FX01 × Δfx
//!   5. **Volatility**:
//!    - First-order: Vega × Δσ
//!    - Second-order: ½ × Volga × (Δσ)²
//!    - Cross-term: CrossGammaSpotVol × Δspot_pct × Δσ_vol_pt
//!      (NOT Vanna — see the unit-contract note at the cross-factor site)
//!   6. **Market Scalars** (for options):
//!    - First-order: Delta × Δspot
//!    - Second-order: ½ × Gamma × (Δspot)²
//!   7. **Inflation**:
//!    - First-order: Inflation01 × Δi
//!    - Second-order: ½ × InflationConvexity × (Δi)²
//!   8. **ModelParameters**: Param01 metrics × param_shift
//!   9. **Residual**: Total P&L - sum(approximations)
//!
//! Aggregate Delta/Gamma and spot cross-Gamma use the first declared market scalar,
//! matching the sensitivity producer. Vega uses the instrument expiry and declared
//! reference strike; absent a reference point, only uniform surface moves are
//! supported. Differing moves across multiple vol sources flag the result invalid.
//! Precomputed Theta and CarryTotal must match the attribution horizon, even when
//! CouponIncome is supplied; the spec executor requests that horizon from the producer.
//! Cashflow collection and payment-date FX conversion failures are returned to the caller.
//!
//! # Advantages
//!
//! - No additional repricing
//! - Per-curve bucketed DV01 when available
//! - Second-order terms when the producer supplies them
//! - Works with already-computed ValuationResults
//!
//! # Disadvantages
//!
//! - Still approximate (third-order+ effects ignored)
//! - Less accurate than parallel/waterfall methods for extreme moves
//! - Large market moves (>100bp rates, >5% vol) can exceed reliable approximation range
//!
//! # Metric Unit Contracts
//!
//! This module expects metrics to follow these unit conventions:
//!
//! | Metric              | Unit            | Definition                                                |
//! |---------------------|-----------------|-----------------------------------------------------------|
//! | DV01                | $ / bp          | Dollar change per 1bp parallel rate shift                 |
//! | Convexity           | per-100 (street)| Street convexity: (∂²P/∂y²) / P / 100 (Bloomberg YAS)     |
//! | IrConvexity         | $ / decimal²    | Raw dollar second derivative ∂²PV/∂r² (swaps)             |
//! | CS01                | $ / bp          | Dollar change per 1bp spread shift                        |
//! | CsGamma             | $ / decimal²    | Dollar second derivative ∂²V/∂s² (spread in decimal)      |
//! | Vega                | $ / vol point   | Dollar change per 1% absolute vol shift                   |
//! | Volga               | $ / vol point²  | Dollar second derivative per vol point²                   |
//! | Theta               | $ / period      | Total-return carry over `theta_period_days`               |
//! | Inflation01         | $ / bp          | Dollar change per 1bp inflation-curve shift               |
//! | InflationConvexity  | $ / decimal²    | Dollar second derivative ∂²V/∂i² (inflation in decimal)   |
//!
//! **Important**: `Convexity` and `IrConvexity` have DIFFERENT producer
//! conventions and are consumed with different formulas :
//! the bond producer emits *street convexity* (`d²P/dy² / P / 100`,
//! Bloomberg YAS), so `ΔP_convexity = ½ × P₀ × Convexity × 100 × (Δr_decimal)²`;
//! the IRS producer emits the raw dollar second derivative `d²PV/dr²`, so
//! `ΔP_convexity = ½ × IrConvexity × (Δr_decimal)²` with no P₀ factor (a
//! near-par swap has PV ≈ 0 but real gamma).
//!
//! `InflationConvexity` uses the `CsGamma`-style $/decimal² convention (no P₀
//! multiplier): `ΔP_inflation_convexity = ½ × InflationConvexity × (Δi_decimal)²`.
//! A pricer emitting `InflationConvexity` in the dimensionless-percentage
//! convention used by `Convexity` would mis-attribute by a factor of P₀
//! (e.g. 1,000,000× for a $1M bond).
//!
//! If your convexity metric uses different units, apply the appropriate scaling
//! factor before passing to attribution.

mod attribute;
mod carry;
mod context;
mod credit;
mod cross_factor;
mod equity;
mod fx;
mod rates;
mod shifts;
mod volatility;

pub use attribute::attribute_pnl_metrics_based;
pub(crate) use shifts::extract_keyrate_per_curve;

#[cfg(test)]
mod tests;
