//! Repo metrics module.
//!
//! Provides metric calculators specific to `Repo`.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_repo_metrics`.
//!
//! Exposed metrics:
//! - Collateral value and required collateral
//! - Collateral coverage ratio
//! - Repo interest amount
//! - DV01 (discount curve parallel bp)
//! - Funding risk (sensitivity to repo rate)
//! - Effective rate (with special collateral adj.)
//! - Time to maturity (years)
//! - Implied collateral return
//! - Accrued interest (currency amount)

mod accrued_interest;
mod implied_collateral_return;
mod register;
#[cfg(test)]
mod tests;

pub(crate) use register::register_repo_metrics;
#[cfg(test)]
use register::{CollateralValueCalculator, EffectiveRateCalculator, RequiredCollateralCalculator};
