//! Constants for structured credit instruments.
//!
//! This module contains all industry-standard constants, default values,
//! and fee structures used across structured credit modeling.

use super::{DealFees, DefaultAssumptions};
use crate::instruments::fixed_income::structured_credit::assumptions::{
    embedded_registry_or_panic, StructuredCreditAssumptionRegistry,
};
use finstack_core::currency::Currency;
use finstack_core::Result;

// ============================================================================
// TIME CONSTANTS
// ============================================================================

/// Average days per year for structured credit day count calculations (ACT/365.25).
///
/// Re-exported from `finstack_core::dates::AVERAGE_DAYS_PER_YEAR`.
/// Uses 365.25 to account for leap years over multi-year amortisation horizons.
pub use finstack_core::dates::AVERAGE_DAYS_PER_YEAR;

/// Default months in a year
pub const MONTHS_PER_YEAR: i32 = 12;

/// Default periods per year for quarterly calculations
pub const QUARTERLY_PERIODS_PER_YEAR: f64 = 4.0;

// ============================================================================
// VALIDATION CONSTANTS
// ============================================================================

/// Default basis points divisor
pub const BASIS_POINTS_DIVISOR: f64 = 10_000.0;

/// Percentage conversion factor
pub const PERCENTAGE_MULTIPLIER: f64 = 100.0;

// ============================================================================
// MARKET-STANDARD NUMERICAL TOLERANCES
// ============================================================================
// These tolerances are calibrated to market conventions for structured credit.
// Reference: Bloomberg BVAL, Intex, and industry-standard pricing systems.

/// Solver tolerance for Z-spread calculations (decimal spread).
///
/// Market standard: 0.1 bps = 0.0001% = 1e-6 in decimal terms.
/// Z-spread is quoted in bps to 1 decimal place (e.g., 125.3 bps).
pub const Z_SPREAD_SOLVER_TOLERANCE: f64 = 1e-6;

/// Solver tolerance for YTM calculations (decimal yield).
///
/// Market standard: 0.1 bps = 1e-6 in decimal terms.
/// YTM is quoted in bps to 1 decimal place.
pub const YTM_SOLVER_TOLERANCE: f64 = 1e-6;

/// Initial bracket size for Z-spread solver (decimal spread).
///
/// ±500 bps is sufficient for most structured credit instruments.
/// Extreme distressed securities may require wider brackets.
pub const Z_SPREAD_INITIAL_BRACKET: f64 = 0.05; // ±500 bps

/// Minimum prepayment rate (floor)
pub const MIN_PREPAYMENT_RATE: f64 = 0.0;

// ============================================================================
// REGISTRY-BACKED MARKET ASSUMPTIONS
// ============================================================================

/// Mortgage prepayment seasonality adjustments by month (Jan=index 0).
pub fn mortgage_seasonality() -> [f64; 12] {
    assumptions_registry().mortgage_seasonality()
}

/// Credit card payment seasonality adjustments by month (Jan=index 0).
pub fn credit_card_seasonality() -> [f64; 12] {
    assumptions_registry().credit_card_seasonality()
}

/// Baseline unemployment rate for default models.
pub fn baseline_unemployment_rate() -> f64 {
    assumptions_registry()
        .simulation_defaults()
        .baseline_unemployment_rate
}

/// Standard PSA speeds for scenario analysis.
pub fn standard_psa_speeds() -> &'static [f64] {
    assumptions_registry().standard_psa_speeds()
}

/// Standard CDR rates for scenario analysis.
pub fn standard_cdr_rates() -> &'static [f64] {
    assumptions_registry().standard_cdr_rates()
}

/// Standard severity rates for scenario analysis.
pub fn standard_severity_rates() -> &'static [f64] {
    assumptions_registry().standard_severity_rates()
}

/// Standard CLO senior management fee (bps).
pub fn clo_senior_mgmt_fee_bps() -> f64 {
    clo_fees().senior_mgmt_fee_bps
}

/// Standard CLO subordinated management fee (bps).
pub fn clo_subordinated_mgmt_fee_bps() -> f64 {
    clo_fees().subordinated_mgmt_fee_bps
}

/// Standard ABS servicing fee (bps).
pub fn abs_servicing_fee_bps() -> f64 {
    abs_fees().servicing_fee_bps
}

/// Standard CMBS master servicer fee (bps).
pub fn cmbs_master_servicer_fee_bps() -> f64 {
    required_optional(
        cmbs_fees().master_servicer_fee_bps,
        "standard CMBS master servicer fee",
    )
}

/// Standard CMBS special servicer fee (bps).
pub fn cmbs_special_servicer_fee_bps() -> f64 {
    required_optional(
        cmbs_fees().special_servicer_fee_bps,
        "standard CMBS special servicer fee",
    )
}

/// Standard RMBS servicing fee (bps).
pub fn rmbs_servicing_fee_bps() -> f64 {
    rmbs_fees().servicing_fee_bps
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
    assumptions_registry()
        .simulation_defaults()
        .pool_balance_cleanup_threshold
}

/// Default resolution lag in months for cashflow generation.
pub fn default_resolution_lag_months() -> u32 {
    assumptions_registry()
        .simulation_defaults()
        .resolution_lag_months
}

/// Standard PSA ramp-up period (months).
pub fn psa_ramp_months() -> u32 {
    assumptions_registry().psa_curve().ramp_months
}

/// Standard PSA terminal CPR.
pub fn psa_terminal_cpr() -> f64 {
    assumptions_registry().psa_curve().terminal_cpr
}

/// Default auto loan ABS speed (monthly).
pub fn default_auto_abs_speed() -> f64 {
    assumptions_registry().auto_abs_prepayment().monthly_speed
}

/// Default auto loan ramp period (months).
pub fn default_auto_ramp_months() -> u32 {
    assumptions_registry().auto_abs_prepayment().ramp_months
}

/// Standard SDA peak month for mortgages.
pub fn sda_peak_month() -> u32 {
    assumptions_registry().sda_curve().peak_month
}

/// Standard SDA peak CDR.
pub fn sda_peak_cdr() -> f64 {
    assumptions_registry().sda_curve().peak_cdr
}

/// Standard SDA terminal CDR.
pub fn sda_terminal_cdr() -> f64 {
    assumptions_registry().sda_curve().terminal_cdr
}

/// Default burnout threshold (months).
pub fn default_burnout_threshold_months() -> u32 {
    assumptions_registry()
        .simulation_defaults()
        .burnout_threshold_months
}

/// Default maximum single obligor concentration.
pub fn default_max_obligor_concentration() -> f64 {
    assumptions_registry()
        .concentration_limits()
        .max_obligor_concentration
}

/// Default maximum top 5 obligor concentration.
pub fn default_max_top5_concentration() -> f64 {
    assumptions_registry()
        .concentration_limits()
        .max_top5_concentration
}

/// Default maximum top 10 obligor concentration.
pub fn default_max_top10_concentration() -> f64 {
    assumptions_registry()
        .concentration_limits()
        .max_top10_concentration
}

/// Default maximum second lien concentration.
pub fn default_max_second_lien() -> f64 {
    assumptions_registry()
        .concentration_limits()
        .max_second_lien
}

/// Default maximum covenant-lite concentration.
pub fn default_max_cov_lite() -> f64 {
    assumptions_registry().concentration_limits().max_cov_lite
}

/// Default maximum DIP concentration.
pub fn default_max_dip() -> f64 {
    assumptions_registry().concentration_limits().max_dip
}

/// Standard CLO CDR (annual).
pub fn clo_standard_cdr() -> f64 {
    clo_assumptions().base_cdr_annual
}

/// Standard CLO recovery rate.
pub fn clo_standard_recovery() -> f64 {
    clo_assumptions().base_recovery_rate
}

/// Standard CLO CPR (annual).
pub fn clo_standard_cpr() -> f64 {
    clo_assumptions().base_cpr_annual
}

/// Standard RMBS CDR (annual).
pub fn rmbs_standard_cdr() -> f64 {
    rmbs_assumptions().base_cdr_annual
}

/// Standard RMBS recovery rate.
pub fn rmbs_standard_recovery() -> f64 {
    rmbs_assumptions().base_recovery_rate
}

/// Standard RMBS CPR (annual).
pub fn rmbs_standard_cpr() -> f64 {
    rmbs_assumptions().base_cpr_annual
}

/// Standard RMBS PSA speed.
pub fn rmbs_standard_psa() -> f64 {
    required_optional(rmbs_assumptions().psa_speed, "standard RMBS PSA speed")
}

/// Standard RMBS SDA speed.
pub fn rmbs_standard_sda() -> f64 {
    required_optional(rmbs_assumptions().sda_speed, "standard RMBS SDA speed")
}

/// Standard Auto ABS CDR (annual).
pub fn abs_auto_standard_cdr() -> f64 {
    abs_assumptions().base_cdr_annual
}

/// Standard Auto ABS recovery rate.
pub fn abs_auto_standard_recovery() -> f64 {
    abs_assumptions().base_recovery_rate
}

/// Standard Auto ABS speed (monthly).
pub fn abs_auto_standard_speed() -> f64 {
    required_optional(
        abs_assumptions().abs_speed_monthly,
        "standard auto ABS monthly speed",
    )
}

/// Standard CMBS CDR (annual).
pub fn cmbs_standard_cdr() -> f64 {
    cmbs_assumptions().base_cdr_annual
}

/// Standard CMBS recovery rate.
pub fn cmbs_standard_recovery() -> f64 {
    cmbs_assumptions().base_recovery_rate
}

/// Standard CMBS CPR (annual).
pub fn cmbs_standard_cpr() -> f64 {
    cmbs_assumptions().base_cpr_annual
}

fn assumptions_registry() -> &'static StructuredCreditAssumptionRegistry {
    embedded_registry_or_panic()
}

#[allow(clippy::expect_used)]
fn required_assumption<T>(result: Result<T>) -> T {
    result.expect("embedded structured-credit assumptions registry value should exist")
}

#[allow(clippy::expect_used)]
fn required_optional<T>(value: Option<T>, _label: &str) -> T {
    value.expect("embedded structured-credit assumptions registry optional value should exist")
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

fn clo_assumptions() -> DefaultAssumptions {
    required_assumption(assumptions_registry().default_assumptions("clo_standard"))
}

fn rmbs_assumptions() -> DefaultAssumptions {
    required_assumption(assumptions_registry().default_assumptions("rmbs_standard"))
}

fn abs_assumptions() -> DefaultAssumptions {
    required_assumption(assumptions_registry().default_assumptions("abs_auto_standard"))
}

fn cmbs_assumptions() -> DefaultAssumptions {
    required_assumption(assumptions_registry().default_assumptions("cmbs_standard"))
}
