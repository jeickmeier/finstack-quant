//! Constants for structured credit instruments.
//!
//! This module contains all industry-standard constants, default values,
//! and fee structures used across structured credit modeling.

use super::DealFees;
use crate::instruments::fixed_income::structured_credit::assumptions::{
    embedded_registry_or_panic, StandardRates,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Result;

/// Average days per year for structured credit day count calculations (ACT/365.25).
///
/// Re-exported from `finstack_quant_core::dates::AVERAGE_DAYS_PER_YEAR`.
/// Uses 365.25 to account for leap years over multi-year amortisation horizons.
pub use finstack_quant_core::dates::AVERAGE_DAYS_PER_YEAR;

/// Number of calendar months in one year (12).
pub const MONTHS_PER_YEAR: i32 = 12;

/// Number of quarterly payment periods in one year (4.0).
pub const QUARTERLY_PERIODS_PER_YEAR: f64 = 4.0;

/// Divisor converting basis points to a decimal rate (10,000.0).
pub const BASIS_POINTS_DIVISOR: f64 = 10_000.0;

/// Multiplier converting a decimal rate to a percentage (100.0).
pub const PERCENTAGE_MULTIPLIER: f64 = 100.0;

// These tolerances are calibrated to market conventions for structured credit.
// Reference: Bloomberg BVAL, Intex, and industry-standard pricing systems.

/// Solver tolerance for Z-spread calculations (decimal spread).
///
/// The price objective is relative to the dirty settlement target. A tight
/// decimal-spread tolerance preserves clean/dirty round trips across notionals.
pub const Z_SPREAD_SOLVER_TOLERANCE: f64 = 1e-12;

/// Solver tolerance for YTM calculations (decimal yield).
///
/// Market standard: 0.1 bp = 1e-6 in decimal terms.
/// YTM is quoted in bp to 1 decimal place.
pub const YTM_SOLVER_TOLERANCE: f64 = 1e-6;

/// Initial bracket size for Z-spread solver (decimal spread).
///
/// ±500 bp is sufficient for most structured credit instruments.
/// Extreme distressed securities may require wider brackets.
pub const Z_SPREAD_INITIAL_BRACKET: f64 = 0.05; // ±500 bp

/// Lower bound for prepayment rates expressed as a decimal fraction.
pub const MIN_PREPAYMENT_RATE: f64 = 0.0;

/// Baseline unemployment rate for default models.
pub fn baseline_unemployment_rate() -> f64 {
    embedded_registry_or_panic()
        .simulation_defaults()
        .baseline_unemployment_rate
}

/// Standard PSA speeds for scenario analysis.
pub fn standard_psa_speeds() -> &'static [f64] {
    embedded_registry_or_panic().standard_psa_speeds()
}

/// Standard CDR rates for scenario analysis.
pub fn standard_cdr_rates() -> &'static [f64] {
    embedded_registry_or_panic().standard_cdr_rates()
}

/// Standard severity rates for scenario analysis.
pub fn standard_severity_rates() -> &'static [f64] {
    embedded_registry_or_panic().standard_severity_rates()
}

/// Standard CLO senior management fee (bp).
pub fn clo_senior_mgmt_fee_bp() -> f64 {
    clo_fees().senior_mgmt_fee_bp
}

/// Standard CLO subordinated management fee (bp).
pub fn clo_subordinated_mgmt_fee_bp() -> f64 {
    clo_fees().subordinated_mgmt_fee_bp
}

/// Standard ABS servicing fee (bp).
pub fn abs_servicing_fee_bp() -> f64 {
    abs_fees().servicing_fee_bp
}

/// Standard CMBS master servicer fee (bp).
pub fn cmbs_master_servicer_fee_bp() -> f64 {
    required_optional(
        cmbs_fees().master_servicer_fee_bp,
        "standard CMBS master servicer fee",
    )
}

/// Standard CMBS special servicer fee (bp).
pub fn cmbs_special_servicer_fee_bp() -> f64 {
    required_optional(
        cmbs_fees().special_servicer_fee_bp,
        "standard CMBS special servicer fee",
    )
}

/// Standard RMBS servicing fee (bp).
pub fn rmbs_servicing_fee_bp() -> f64 {
    rmbs_fees().servicing_fee_bp
}

/// Standard CLO trustee annual fee (USD).
pub fn clo_trustee_fee_annual() -> f64 {
    clo_fees().trustee_fee_annual.amount()
}

/// Standard ABS trustee annual fee (USD).
pub fn abs_trustee_fee_annual() -> f64 {
    abs_fees().trustee_fee_annual.amount()
}

/// Standard CMBS trustee annual fee (USD).
pub fn cmbs_trustee_fee_annual() -> f64 {
    cmbs_fees().trustee_fee_annual.amount()
}

/// Standard RMBS trustee annual fee (USD).
pub fn rmbs_trustee_fee_annual() -> f64 {
    rmbs_fees().trustee_fee_annual.amount()
}

/// AssetPool balance threshold (in base currency units) below which cashflow generation stops.
///
/// For example, for a USD-denominated pool, this means stop when balance < $100.
/// This prevents unnecessary computation for immaterial remaining balances.
pub fn pool_balance_cleanup_threshold() -> f64 {
    embedded_registry_or_panic()
        .simulation_defaults()
        .pool_balance_cleanup_threshold
}

/// Default resolution lag in months for cashflow generation.
pub fn default_resolution_lag_months() -> u32 {
    embedded_registry_or_panic()
        .simulation_defaults()
        .resolution_lag_months
}

/// Standard PSA ramp-up period (months).
pub fn psa_ramp_months() -> u32 {
    embedded_registry_or_panic().psa_curve().ramp_months
}

/// Standard PSA terminal CPR.
pub fn psa_terminal_cpr() -> f64 {
    embedded_registry_or_panic().psa_curve().terminal_cpr
}

/// Standard SDA peak month for mortgages.
pub fn sda_peak_month() -> u32 {
    embedded_registry_or_panic().sda_curve().peak_month
}

/// Standard SDA peak CDR.
pub fn sda_peak_cdr() -> f64 {
    embedded_registry_or_panic().sda_curve().peak_cdr
}

/// Standard SDA terminal CDR.
pub fn sda_terminal_cdr() -> f64 {
    embedded_registry_or_panic().sda_curve().terminal_cdr
}

/// Default burnout threshold (months).
pub fn default_burnout_threshold_months() -> u32 {
    embedded_registry_or_panic()
        .simulation_defaults()
        .burnout_threshold_months
}

/// Default maximum single obligor concentration.
pub fn default_max_obligor_concentration() -> f64 {
    embedded_registry_or_panic()
        .concentration_limits()
        .max_obligor_concentration
}

/// Default maximum top 5 obligor concentration.
pub fn default_max_top5_concentration() -> f64 {
    embedded_registry_or_panic()
        .concentration_limits()
        .max_top5_concentration
}

/// Default maximum top 10 obligor concentration.
pub fn default_max_top10_concentration() -> f64 {
    embedded_registry_or_panic()
        .concentration_limits()
        .max_top10_concentration
}

/// Default maximum second lien concentration.
pub fn default_max_second_lien() -> f64 {
    embedded_registry_or_panic()
        .concentration_limits()
        .max_second_lien
}

/// Default maximum covenant-lite concentration.
pub fn default_max_cov_lite() -> f64 {
    embedded_registry_or_panic()
        .concentration_limits()
        .max_cov_lite
}

/// Default maximum DIP concentration.
pub fn default_max_dip() -> f64 {
    embedded_registry_or_panic().concentration_limits().max_dip
}

/// Standard CLO CDR (annual).
pub fn clo_standard_cdr() -> f64 {
    standard_rates("clo_standard").cdr_annual
}

/// Standard CLO recovery rate.
pub fn clo_standard_recovery() -> f64 {
    standard_rates("clo_standard").recovery_rate
}

/// Standard CLO CPR (annual).
pub fn clo_standard_cpr() -> f64 {
    standard_rates("clo_standard").prepayment_rate
}

/// Standard RMBS CDR (annual).
pub fn rmbs_standard_cdr() -> f64 {
    standard_rates("rmbs_standard").cdr_annual
}

/// Standard RMBS recovery rate.
pub fn rmbs_standard_recovery() -> f64 {
    standard_rates("rmbs_standard").recovery_rate
}

/// Standard RMBS PSA speed (multiplier of the PSA ramp).
pub fn rmbs_standard_psa() -> f64 {
    standard_rates("rmbs_standard").prepayment_rate
}

/// Standard Auto ABS CDR (annual).
pub fn abs_auto_standard_cdr() -> f64 {
    standard_rates("abs_auto_standard").cdr_annual
}

/// Standard Auto ABS recovery rate.
pub fn abs_auto_standard_recovery() -> f64 {
    standard_rates("abs_auto_standard").recovery_rate
}

/// Standard Auto ABS speed (monthly).
pub fn abs_auto_standard_speed() -> f64 {
    standard_rates("abs_auto_standard").prepayment_rate
}

/// Standard CMBS CDR (annual).
pub fn cmbs_standard_cdr() -> f64 {
    standard_rates("cmbs_standard").cdr_annual
}

/// Standard CMBS recovery rate.
pub fn cmbs_standard_recovery() -> f64 {
    standard_rates("cmbs_standard").recovery_rate
}

/// Standard CMBS CPR (annual, after the lockout).
pub fn cmbs_standard_cpr() -> f64 {
    standard_rates("cmbs_standard").prepayment_rate
}

#[allow(clippy::expect_used)]
fn required_assumption<T>(result: Result<T>) -> T {
    result.expect("embedded structured-credit assumptions registry value should exist")
}

#[allow(clippy::expect_used)]
fn required_optional<T>(value: Option<T>, _label: &str) -> T {
    value.expect("embedded structured-credit assumptions registry optional value should exist")
}

fn standard_rates(profile_id: &str) -> StandardRates {
    required_assumption(embedded_registry_or_panic().standard_rates(profile_id))
}

fn clo_fees() -> DealFees {
    DealFees::clo_standard(Currency::USD)
}

fn abs_fees() -> DealFees {
    DealFees::abs_standard(Currency::USD)
}

fn cmbs_fees() -> DealFees {
    DealFees::cmbs_standard(Currency::USD)
}

fn rmbs_fees() -> DealFees {
    DealFees::rmbs_standard(Currency::USD)
}
