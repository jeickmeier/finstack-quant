//! Product-independent liquidity risk metrics and market-impact models.
//!
//! This module provides market microstructure liquidity modeling for traded
//! positions. It is orthogonal to the balance-sheet liquidity ratios in
//! `finstack-quant-statements-analytics` and focuses on:
//!
//! - **Spread estimation**: Roll (1984) effective spread and Amihud (2002)
//!   illiquidity ratio from return/volume data.
//! - **Liquidity-adjusted VaR (LVaR)**: Bangia et al. (1999) framework
//!   combining exogenous spread costs, endogenous position-size effects,
//!   and time-to-liquidation horizon adjustments.
//! - **Market impact models**: Almgren-Chriss (2001) optimal execution with
//!   permanent/temporary impact decomposition, and Kyle (1985) linear lambda.
//! - **Portfolio liquidity scoring**: Position-level days-to-liquidate, tier
//!   classification, and aggregate portfolio liquidity reports.
//!
//! # Architecture
//!
//! The module is structured in layers:
//!
//! 1. **Types** (`types`): `LiquidityProfile`, `LiquidityTier`, `LiquidityConfig`
//! 2. **Estimators** (`estimators`): Pure functions on `&[f64]` slices
//! 3. **LVaR** (`lvar`): Composes with existing VaR numbers
//! 4. **Impact** (`impact`, `almgren_chriss`, `kyle`): Trade execution cost models
//!
//! # Usage
//!
//! Estimate the effective spread from a return series, then charge that spread
//! against an existing VaR number:
//!
//! ```
//! use finstack_quant_models::liquidity::{lvar_bangia_scalar, roll_effective_spread};
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let returns = [0.004, -0.005, 0.006, -0.004, 0.005, -0.006];
//! let spread = roll_effective_spread(&returns).expect("at least two returns");
//! assert!(spread > 0.0);
//!
//! // VaR uses the loss-sign convention, so it is non-positive.
//! let result = lvar_bangia_scalar(-100_000.0, spread, 0.25 * spread, 0.99, 1_000_000.0)?;
//! assert!(result.spread_cost >= 0.0);
//! assert!(result.lvar <= result.var);
//! # Ok(())
//! # }
//! ```

mod almgren_chriss;
mod estimators;
mod impact;
mod kyle;
mod lvar;
mod registry;
mod types;

use finstack_quant_core::{Error, Result};

fn invalid_input(message: impl Into<String>) -> Error {
    Error::Validation(message.into())
}

pub use types::{
    days_to_liquidate, liquidity_tier, LiquidityConfig, LiquidityProfile, LiquidityTier,
    SpreadVolatilityKind,
};

pub use estimators::{amihud_illiquidity, roll_effective_spread};

pub use lvar::{lvar_bangia_scalar, LvarBangiaScalar};

pub use almgren_chriss::AlmgrenChrissModel;
pub use impact::{ExecutionTrajectory, ImpactEstimate, TradeParams};
pub use kyle::KyleLambdaModel;

/// Build and evaluate a uniform Almgren-Chriss market-impact estimate.
///
/// The model's impact coefficients are derived from `avg_daily_volume` using
/// the same empirical calibration as [`AlmgrenChrissModel::from_profile`],
/// evaluated on a synthetic profile with a 0.2% (20 bp) proportional
/// bid-ask spread around the reference price:
///
/// ```text
/// gamma = permanent_impact_coef × spread / (2 · ADV)      // spread = 0.002 · mid
/// eta   = temporary_impact_coef × volatility · mid / √ADV
/// ```
///
/// so `permanent_impact_coef` and `temporary_impact_coef` are dimensionless
/// multipliers on the ADV-derived base calibration (`1.0` keeps the base
/// calibration; `0.0` disables that component), and a deeper market (larger
/// ADV) produces a strictly smaller impact cost, all else equal. Callers who
/// have externally calibrated *absolute* `gamma`/`eta` coefficients should
/// construct [`AlmgrenChrissModel::new`] directly instead.
///
/// # Arguments
///
/// * `position_size` - Signed quantity to execute; its sign determines trade
///   direction while its magnitude determines participation.
/// * `avg_daily_volume` - Positive average daily tradable volume in the same
///   quantity units as `position_size`. Feeds the coefficient calibration
///   above.
/// * `volatility` - Positive daily volatility as a decimal fraction.
/// * `execution_horizon_days` - Intended execution horizon in trading days.
/// * `permanent_impact_coef` - Dimensionless multiplier on the ADV-derived
///   permanent-impact calibration.
/// * `temporary_impact_coef` - Dimensionless multiplier on the ADV-derived
///   temporary-impact calibration; must be positive.
/// * `reference_price` - Optional positive mid price; `None` uses unit price
///   and reports scale-free impact.
///
/// # Errors
///
/// Returns an error when the liquidity inputs are not finite and positive, or
/// when the underlying model/profile validation fails.
#[allow(clippy::too_many_arguments)]
pub fn almgren_chriss_uniform_impact(
    position_size: f64,
    avg_daily_volume: f64,
    volatility: f64,
    execution_horizon_days: f64,
    permanent_impact_coef: f64,
    temporary_impact_coef: f64,
    reference_price: Option<f64>,
) -> Result<ImpactEstimate> {
    if !avg_daily_volume.is_finite() || avg_daily_volume <= 0.0 {
        return Err(invalid_input(
            "avg_daily_volume must be finite and positive",
        ));
    }
    if !volatility.is_finite() || volatility <= 0.0 {
        return Err(invalid_input("volatility must be finite and positive"));
    }
    if let Some(price) = reference_price {
        if !price.is_finite() || price <= 0.0 {
            return Err(invalid_input("reference_price must be finite and positive"));
        }
    }

    let mid = reference_price.unwrap_or(1.0);
    let profile = LiquidityProfile::new(
        "AC_CALIBRATION",
        mid,
        mid * 0.999,
        mid * 1.001,
        avg_daily_volume,
        1.0,
        0.0,
    )?;
    // Route ADV into the coefficients via the `from_profile` calibration
    // (gamma = spread / (2·ADV), eta = σ·mid / √ADV); the caller's
    // coefficients scale that base multiplicatively. Previously the raw
    // coefficients were used directly, leaving `avg_daily_volume` validated
    // but inert — ADV 1e6 and 1e9 produced bit-identical costs.
    let base = AlmgrenChrissModel::from_profile(&profile, volatility)?;
    let model = AlmgrenChrissModel::new(
        permanent_impact_coef * base.gamma(),
        temporary_impact_coef * base.eta(),
        0.5,
    )?;
    let params = TradeParams {
        quantity: position_size,
        horizon_days: execution_horizon_days,
        daily_volatility: volatility,
        profile,
        risk_aversion: None,
        reference_price,
    };
    model.estimate_cost(&params)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `avg_daily_volume` was validated and then never used: the model was
    /// built directly from the caller's raw coefficients, so an execution in
    /// a name trading 1M shares/day cost bit-identically the same as one
    /// trading 1B shares/day. ADV must be live: deeper markets => strictly
    /// smaller impact cost, all else equal.
    #[test]
    fn uniform_impact_responds_to_avg_daily_volume() {
        let cost = |adv: f64| {
            almgren_chriss_uniform_impact(100_000.0, adv, 0.02, 5.0, 1.0, 1.0, Some(100.0))
                .expect("valid inputs")
                .total_cost
        };
        let thin = cost(1e6);
        let deep = cost(1e9);
        assert!(
            (thin - deep).abs() > 0.0,
            "ADV must change the cost (thin {thin}, deep {deep})"
        );
        assert!(
            deep < thin,
            "a deeper market must cost less (thin {thin}, deep {deep})"
        );
    }

    /// The caller coefficients scale the ADV-derived calibration
    /// multiplicatively, so doubling a coefficient doubles its component.
    #[test]
    fn uniform_impact_coefficients_scale_their_components() {
        let base = almgren_chriss_uniform_impact(100_000.0, 1e6, 0.02, 5.0, 1.0, 1.0, Some(100.0))
            .expect("valid inputs");
        let scaled =
            almgren_chriss_uniform_impact(100_000.0, 1e6, 0.02, 5.0, 2.0, 1.0, Some(100.0))
                .expect("valid inputs");
        assert!(
            (scaled.permanent_impact - 2.0 * base.permanent_impact).abs()
                <= 1e-9 * base.permanent_impact,
            "doubling the permanent coefficient must double the permanent cost"
        );
        assert!(
            (scaled.temporary_impact - base.temporary_impact).abs()
                <= 1e-9 * base.temporary_impact.abs().max(1.0),
            "the temporary component must be untouched"
        );
    }
}
