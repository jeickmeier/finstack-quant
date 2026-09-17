//! Deal fee structures for structured credit instruments.
//!
//! Every other deal-level assumption reaches the engine through a field on
//! [`super::StructuredCredit`], [`super::AssetPool`], [`super::Tranche`] or
//! [`super::WaterfallRules`]; this module only holds the fee schedule the
//! template waterfall turns into fee tiers.

use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry_or_panic;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

use serde::{Deserialize, Serialize};

/// Fee structure for structured credit deals
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DealFees {
    /// Annual trustee fee (fixed amount)
    pub trustee_fee_annual: Money,
    /// Senior management fee (basis points per annum on collateral)
    pub senior_mgmt_fee_bp: f64,
    /// Subordinated management fee (basis points per annum), paid after every
    /// note coupon.
    pub subordinated_mgmt_fee_bp: f64,
    /// Servicing fee (basis points per annum)
    pub servicing_fee_bp: f64,
    /// Master servicer fee (for CMBS/RMBS, basis points)
    pub master_servicer_fee_bp: Option<f64>,
    /// Special servicer fee (for CMBS, basis points)
    pub special_servicer_fee_bp: Option<f64>,
    /// Manager incentive fee paid from the residual once equity has earned
    /// its hurdle IRR; `None` for deals without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incentive_fee: Option<IncentiveFeeSpec>,
}

/// Manager incentive fee: once the equity IRR to date (invested capital at
/// closing against every distribution to date, including the residual on the
/// current payment date) reaches `hurdle_irr`, the manager takes `share_pct`
/// of the residual interest proceeds ahead of equity.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct IncentiveFeeSpec {
    /// Equity IRR hurdle as an annual decimal (`0.12` = 12%).
    pub hurdle_irr: f64,
    /// Share of the residual paid to the manager once the hurdle is met, as
    /// a decimal fraction in `[0, 1]`.
    pub share_pct: f64,
}

impl DealFees {
    /// Create CLO-style fee structure
    pub fn clo_standard(base_currency: finstack_quant_core::currency::Currency) -> Self {
        required_assumption(
            embedded_registry_or_panic().deal_fees("clo_standard", base_currency),
            "standard CLO fees",
        )
    }

    /// Create ABS-style fee structure
    pub fn abs_standard(base_currency: finstack_quant_core::currency::Currency) -> Self {
        required_assumption(
            embedded_registry_or_panic().deal_fees("abs_auto_standard", base_currency),
            "standard ABS fees",
        )
    }

    /// Create CMBS-style fee structure
    pub fn cmbs_standard(base_currency: finstack_quant_core::currency::Currency) -> Self {
        required_assumption(
            embedded_registry_or_panic().deal_fees("cmbs_standard", base_currency),
            "standard CMBS fees",
        )
    }

    /// Create RMBS-style fee structure
    pub fn rmbs_standard(base_currency: finstack_quant_core::currency::Currency) -> Self {
        required_assumption(
            embedded_registry_or_panic().deal_fees("rmbs_standard", base_currency),
            "standard RMBS fees",
        )
    }
}

#[allow(clippy::expect_used)]
fn required_assumption<T>(result: Result<T>, _label: &str) -> T {
    result.expect("embedded structured-credit assumptions registry value should exist")
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn test_clo_fee_structure() {
        let fees = DealFees::clo_standard(Currency::USD);

        assert_eq!(fees.trustee_fee_annual.amount(), 50_000.0);
        assert_eq!(fees.senior_mgmt_fee_bp, 40.0);
        assert_eq!(fees.subordinated_mgmt_fee_bp, 20.0);
        let incentive = fees.incentive_fee.expect("standard CLO incentive fee");
        assert_eq!(incentive.hurdle_irr, 0.12);
        assert_eq!(incentive.share_pct, 0.2);
    }
}
