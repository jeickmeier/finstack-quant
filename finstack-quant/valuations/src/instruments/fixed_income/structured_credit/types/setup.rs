//! Configuration structures for structured credit instruments.
//!
//! This module contains deal-level configuration types including dates,
//! fees, coverage tests, and default assumptions.

use super::enums::DealType;
use crate::instruments::fixed_income::structured_credit::assumptions::{
    embedded_registry_or_panic, StructuredCreditAssumptionRegistry,
};
use crate::instruments::rates::irs::InterestRateSwap;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CreditRating;
use finstack_quant_core::{HashMap, Result};

use serde::{Deserialize, Serialize};

// ============================================================================
// DEAL CONFIGURATION STRUCTURES
// ============================================================================

/// Complete deal configuration for structured credit instruments
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DealConfig {
    /// Key deal dates
    pub dates: DealDates,
    /// Fee structure
    pub fees: DealFees,
    /// Coverage test parameters
    pub coverage_tests: CoverageTestConfig,
    /// Default prepayment and default assumptions
    pub default_assumptions: DefaultAssumptions,
    /// Hedge swaps (leveraging existing IRS infrastructure)
    #[serde(default)]
    pub hedge_swaps: Vec<InterestRateSwap>,
}

/// Key dates for a structured credit deal
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DealDates {
    /// Deal closing date
    #[schemars(with = "String")]
    pub closing_date: Date,
    /// First payment date
    #[schemars(with = "String")]
    pub first_payment_date: Date,
    /// End of reinvestment period (if applicable)
    #[schemars(with = "Option<String>")]
    pub reinvestment_end_date: Option<Date>,
    /// Legal final maturity date
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Payment frequency
    pub frequency: Tenor,
}

impl DealDates {
    /// Create new deal dates with required fields
    pub fn new(
        closing_date: Date,
        first_payment_date: Date,
        maturity: Date,
        frequency: Tenor,
    ) -> Self {
        Self {
            closing_date,
            first_payment_date,
            reinvestment_end_date: None,
            maturity,
            frequency,
        }
    }

    /// Add reinvestment period
    pub fn with_reinvestment_end(mut self, end_date: Date) -> Self {
        self.reinvestment_end_date = Some(end_date);
        self
    }
}

/// Fee structure for structured credit deals
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DealFees {
    /// Annual trustee fee (fixed amount)
    pub trustee_fee_annual: Money,
    /// Senior management fee (basis points per annum on collateral)
    pub senior_mgmt_fee_bps: f64,
    /// Subordinated management fee (basis points per annum)
    pub subordinated_mgmt_fee_bps: f64,
    /// Servicing fee (basis points per annum)
    pub servicing_fee_bps: f64,
    /// Master servicer fee (for CMBS/RMBS, basis points)
    pub master_servicer_fee_bps: Option<f64>,
    /// Special servicer fee (for CMBS, basis points)
    pub special_servicer_fee_bps: Option<f64>,
}

impl DealFees {
    /// Create CLO-style fee structure
    pub fn clo_standard(base_currency: finstack_quant_core::currency::Currency) -> Self {
        required_assumption(
            assumptions_registry().deal_fees("clo_standard", base_currency),
            "standard CLO fees",
        )
    }

    /// Create ABS-style fee structure
    pub fn abs_standard(base_currency: finstack_quant_core::currency::Currency) -> Self {
        required_assumption(
            assumptions_registry().deal_fees("abs_auto_standard", base_currency),
            "standard ABS fees",
        )
    }

    /// Create CMBS-style fee structure
    pub fn cmbs_standard(base_currency: finstack_quant_core::currency::Currency) -> Self {
        required_assumption(
            assumptions_registry().deal_fees("cmbs_standard", base_currency),
            "standard CMBS fees",
        )
    }

    /// Create RMBS-style fee structure
    pub fn rmbs_standard(base_currency: finstack_quant_core::currency::Currency) -> Self {
        required_assumption(
            assumptions_registry().deal_fees("rmbs_standard", base_currency),
            "standard RMBS fees",
        )
    }
}

/// Coverage test configuration
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoverageTestConfig {
    /// OC trigger levels by tranche ID
    pub oc_triggers: HashMap<String, f64>,
    /// IC trigger levels by tranche ID
    pub ic_triggers: HashMap<String, f64>,
    /// Haircuts by rating for OC calculations
    pub haircuts: HashMap<CreditRating, f64>,
    /// Par value test threshold (if applicable)
    pub par_value_threshold: Option<f64>,
}

impl CoverageTestConfig {
    /// Create new empty configuration
    pub fn new() -> Self {
        Self {
            oc_triggers: HashMap::default(),
            ic_triggers: HashMap::default(),
            haircuts: Self::default_haircuts(),
            par_value_threshold: None,
        }
    }

    /// Standard CLO haircuts (conservative)
    pub fn default_haircuts() -> HashMap<CreditRating, f64> {
        assumptions_registry().coverage_haircuts()
    }

    /// Add OC test for a tranche
    pub fn add_oc_test(
        &mut self,
        tranche_id: impl Into<String>,
        trigger_level: f64,
    ) -> finstack_quant_core::Result<&mut Self> {
        if !trigger_level.is_finite() || trigger_level <= 1.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "OC trigger level must be finite and greater than 1.0, got {trigger_level}"
            )));
        }
        self.oc_triggers.insert(tranche_id.into(), trigger_level);
        Ok(self)
    }

    /// Add IC test for a tranche
    pub fn add_ic_test(
        &mut self,
        tranche_id: impl Into<String>,
        trigger_level: f64,
    ) -> finstack_quant_core::Result<&mut Self> {
        if !trigger_level.is_finite() || trigger_level <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "IC trigger level must be finite and positive, got {trigger_level}"
            )));
        }
        self.ic_triggers.insert(tranche_id.into(), trigger_level);
        Ok(self)
    }

    /// Set custom haircuts
    pub fn with_haircuts(mut self, haircuts: HashMap<CreditRating, f64>) -> Self {
        self.haircuts = haircuts;
        self
    }
}

impl Default for CoverageTestConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Default assumptions for structured credit modeling
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DefaultAssumptions {
    /// Base annual CDR (Constant Default Rate)
    pub base_cdr_annual: f64,
    /// Base recovery rate on defaults
    pub base_recovery_rate: f64,
    /// Base annual CPR (Constant Prepayment Rate)
    pub base_cpr_annual: f64,
    /// PSA speed multiplier (for RMBS)
    pub psa_speed: Option<f64>,
    /// SDA speed multiplier (for RMBS)
    pub sda_speed: Option<f64>,
    /// ABS monthly prepayment speed (for auto ABS)
    pub abs_speed_monthly: Option<f64>,
    /// Asset-type specific annual CPRs
    #[serde(default)]
    pub cpr_by_asset_type: HashMap<String, f64>,
    /// Asset-type specific annual CDRs
    #[serde(default)]
    pub cdr_by_asset_type: HashMap<String, f64>,
    /// Asset-type specific recovery rates
    #[serde(default)]
    pub recovery_by_asset_type: HashMap<String, f64>,
}

impl DefaultAssumptions {
    /// CLO default assumptions
    pub fn clo_standard() -> Self {
        required_assumption(
            assumptions_registry().default_assumptions("clo_standard"),
            "standard CLO default assumptions",
        )
    }

    /// RMBS default assumptions
    pub fn rmbs_standard() -> Self {
        required_assumption(
            assumptions_registry().default_assumptions("rmbs_standard"),
            "standard RMBS default assumptions",
        )
    }

    /// Auto ABS default assumptions
    pub fn abs_auto_standard() -> Self {
        required_assumption(
            assumptions_registry().default_assumptions("abs_auto_standard"),
            "standard Auto ABS default assumptions",
        )
    }

    /// CMBS default assumptions
    pub fn cmbs_standard() -> Self {
        required_assumption(
            assumptions_registry().default_assumptions("cmbs_standard"),
            "standard CMBS default assumptions",
        )
    }
}

impl Default for DefaultAssumptions {
    fn default() -> Self {
        assumptions_registry().generic_default_assumptions()
    }
}

impl DealConfig {
    /// Create standard deal configuration for a given deal type
    pub fn standard(
        deal_type: DealType,
        dates: DealDates,
        base_currency: finstack_quant_core::currency::Currency,
    ) -> Self {
        let profile_id = assumptions_registry().profile_id_for_deal_type(deal_type);
        let fees = required_assumption(
            assumptions_registry().deal_fees(profile_id, base_currency),
            "standard deal fees",
        );
        let default_assumptions = required_assumption(
            assumptions_registry().default_assumptions(profile_id),
            "standard deal default assumptions",
        );

        Self {
            dates,
            fees,
            coverage_tests: CoverageTestConfig::new(),
            default_assumptions,
            hedge_swaps: Vec::new(),
        }
    }

    /// Create CLO deal configuration
    pub fn clo_standard(
        dates: DealDates,
        base_currency: finstack_quant_core::currency::Currency,
    ) -> Self {
        Self::standard(DealType::CLO, dates, base_currency)
    }

    /// Create RMBS deal configuration
    pub fn rmbs_standard(
        dates: DealDates,
        base_currency: finstack_quant_core::currency::Currency,
    ) -> Self {
        Self::standard(DealType::RMBS, dates, base_currency)
    }

    /// Create ABS deal configuration
    pub fn abs_standard(
        dates: DealDates,
        base_currency: finstack_quant_core::currency::Currency,
    ) -> Self {
        Self::standard(DealType::ABS, dates, base_currency)
    }

    /// Create CMBS deal configuration
    pub fn cmbs_standard(
        dates: DealDates,
        base_currency: finstack_quant_core::currency::Currency,
    ) -> Self {
        Self::standard(DealType::CMBS, dates, base_currency)
    }

    /// Add hedge swap for interest rate or basis risk management
    /// Leverages existing IRS infrastructure from finstack-quant.
    pub fn with_hedge_swap(mut self, swap: InterestRateSwap) -> Self {
        self.hedge_swaps.push(swap);
        self
    }

    /// Add multiple hedge swaps
    pub fn with_hedge_swaps(mut self, swaps: Vec<InterestRateSwap>) -> Self {
        self.hedge_swaps.extend(swaps);
        self
    }
}

fn assumptions_registry() -> &'static StructuredCreditAssumptionRegistry {
    embedded_registry_or_panic()
}

#[allow(clippy::expect_used)]
fn required_assumption<T>(result: Result<T>, _label: &str) -> T {
    result.expect("embedded structured-credit assumptions registry value should exist")
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
    }

    #[test]
    fn test_deal_dates_creation() {
        let dates = DealDates::new(test_date(), test_date(), test_date(), Tenor::quarterly());

        assert_eq!(dates.closing_date, test_date());
        assert!(dates.reinvestment_end_date.is_none());
    }

    #[test]
    fn test_clo_fee_structure() {
        let fees = DealFees::clo_standard(Currency::USD);

        assert_eq!(fees.trustee_fee_annual.amount(), 50_000.0);
        assert_eq!(fees.senior_mgmt_fee_bps, 40.0);
        assert_eq!(fees.subordinated_mgmt_fee_bps, 20.0);
    }

    #[test]
    fn test_coverage_test_config() {
        let mut config = CoverageTestConfig::new();
        config.add_oc_test("CLASS_A", 1.25).unwrap();
        config.add_ic_test("CLASS_A", 1.20).unwrap();

        assert_eq!(config.oc_triggers.get("CLASS_A"), Some(&1.25));
        assert_eq!(config.ic_triggers.get("CLASS_A"), Some(&1.20));
    }

    #[test]
    fn test_coverage_test_config_rejects_invalid_triggers() {
        let mut config = CoverageTestConfig::new();

        assert!(config.add_oc_test("CLASS_A", 1.0).is_err());
        assert!(config.add_oc_test("CLASS_A", f64::NAN).is_err());
        assert!(config.add_ic_test("CLASS_A", 0.0).is_err());
        assert!(config.add_ic_test("CLASS_A", f64::INFINITY).is_err());
    }

    #[test]
    fn test_auto_recovery_rate() {
        let assumptions = DefaultAssumptions::abs_auto_standard();

        assert_eq!(assumptions.base_recovery_rate, 0.45);
    }
}
