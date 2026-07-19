//! Result types for structured credit tranche valuation.
//!
//! This module provides result types for individual tranche valuation within
//! structured credit instruments (CLO, ABS, RMBS, CMBS).

use crate::cashflow::traits::DatedFlows;
use crate::metrics::MetricId;
use finstack_quant_core::cashflow::CashFlow;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;

use serde::{Deserialize, Serialize};

/// Result containing tranche-specific cashflows and metadata.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrancheCashflows {
    /// Tranche identifier.
    pub tranche_id: String,
    /// Cashflow schedule for this tranche (simple dated flows).
    #[schemars(with = "String")]
    pub cashflows: DatedFlows,
    /// Detailed cashflows with proper classification using CFKind.
    pub detailed_flows: Vec<CashFlow>,
    /// Interest cashflows (component of total).
    #[schemars(with = "String")]
    pub interest_flows: DatedFlows,
    /// Principal cashflows (component of total).
    #[schemars(with = "String")]
    pub principal_flows: DatedFlows,
    /// PIK capitalization flows.
    #[schemars(with = "String")]
    pub pik_flows: DatedFlows,
    /// Write-down flows (loss allocation reducing tranche balance).
    #[schemars(with = "String")]
    pub writedown_flows: DatedFlows,
    /// Final tranche balance after all payments.
    pub final_balance: Money,
    /// Total interest received.
    pub total_interest: Money,
    /// Total principal received.
    pub total_principal: Money,
    /// Total PIK capitalized.
    pub total_pik: Money,
    /// Total write-down (loss allocation).
    pub total_writedown: Money,
}

/// Tranche-specific valuation result.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrancheValuation {
    /// Tranche identifier.
    pub tranche_id: String,
    /// Present value of all cashflows.
    pub pv: Money,
    /// Clean price (as percentage of par).
    pub clean_price: f64,
    /// Dirty price (as percentage of par).
    pub dirty_price: f64,
    /// Accrued interest.
    pub accrued: Money,
    /// Weighted average life.
    pub wal: f64,
    /// Modified duration.
    pub modified_duration: f64,
    /// Z-spread (basis points).
    pub z_spread_bps: f64,
    /// CS01 (credit DV01).
    pub cs01: f64,
    /// Yield to maturity.
    pub ytm: f64,
    /// Additional metrics.
    pub metrics: HashMap<MetricId, f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;

    #[test]
    fn test_tranche_cashflows_creation() {
        let cashflows = TrancheCashflows {
            tranche_id: "AAA".to_string(),
            cashflows: vec![],
            detailed_flows: vec![],
            interest_flows: vec![],
            principal_flows: vec![
                (
                    Date::from_calendar_date(2024, time::Month::June, 30).expect("valid date"),
                    Money::new(100_000.0, Currency::USD),
                ),
                (
                    Date::from_calendar_date(2025, time::Month::June, 30).expect("valid date"),
                    Money::new(100_000.0, Currency::USD),
                ),
            ],
            pik_flows: vec![],
            writedown_flows: vec![],
            final_balance: Money::new(0.0, Currency::USD),
            total_interest: Money::new(10_000.0, Currency::USD),
            total_principal: Money::new(200_000.0, Currency::USD),
            total_pik: Money::new(0.0, Currency::USD),
            total_writedown: Money::new(0.0, Currency::USD),
        };

        assert_eq!(cashflows.tranche_id, "AAA");
        assert_eq!(cashflows.principal_flows.len(), 2);
        assert_eq!(cashflows.total_principal.amount(), 200_000.0);
    }
}
