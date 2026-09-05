//! Result types for structured credit tranche valuation.
//!
//! This module provides result types for individual tranche valuation within
//! structured credit instruments (CLO, ABS, RMBS, CMBS).

use crate::cashflow::traits::DatedFlows;
use crate::metrics::MetricId;
use finstack_quant_core::cashflow::CashFlow;
use finstack_quant_core::money::Money;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Result containing tranche-specific cashflows and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TrancheCashflows {
    /// Tranche identifier.
    pub tranche_id: String,
    /// Cashflow schedule for this tranche (simple dated flows).
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub cashflows: DatedFlows,
    /// Detailed cashflows with proper classification using CFKind.
    pub detailed_flows: Vec<CashFlow>,
    /// Interest cashflows (component of total).
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub interest_flows: DatedFlows,
    /// Principal cashflows (component of total).
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub principal_flows: DatedFlows,
    /// PIK capitalization flows.
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub pik_flows: DatedFlows,
    /// Interest DEFERRED to future periods on a non-PIK tranche.
    ///
    /// SC-m11: non-PIK shortfalls used to be recorded in `pik_flows`. PIK means
    /// the unpaid interest is CAPITALIZED into the tranche balance and accrues
    /// thereafter; a non-PIK deferral is a separate senior claim that does not
    /// touch notional. Conflating them misleads any consumer reading
    /// `total_pik` as capitalized balance — the two have different effects on
    /// notional, on later interest due, and on OC denominators.
    #[serde(default)]
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub deferred_flows: DatedFlows,
    /// Write-down flows (loss allocation reducing tranche balance).
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub writedown_flows: DatedFlows,
    /// Final tranche balance after all payments.
    pub final_balance: Money,
    /// Total interest received.
    pub total_interest: Money,
    /// Total principal received.
    pub total_principal: Money,
    /// Total PIK capitalized.
    pub total_pik: Money,
    /// Total interest deferred on a non-PIK tranche (SC-m11).
    pub total_deferred: Money,
    /// Total write-down (loss allocation).
    pub total_writedown: Money,
}

/// Tranche-specific valuation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    pub z_spread_bp: f64,
    /// CS01 (credit DV01).
    pub cs01: f64,
    /// Yield to maturity.
    pub ytm: f64,
    /// Additional metrics.
    pub metrics: BTreeMap<MetricId, f64>,
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
                    Money::from((100_000_i64, Currency::USD)),
                ),
                (
                    Date::from_calendar_date(2025, time::Month::June, 30).expect("valid date"),
                    Money::from((100_000_i64, Currency::USD)),
                ),
            ],
            pik_flows: vec![],
            deferred_flows: Vec::new(),
            writedown_flows: vec![],
            final_balance: Money::from((0_i64, Currency::USD)),
            total_interest: Money::from((10_000_i64, Currency::USD)),
            total_principal: Money::from((200_000_i64, Currency::USD)),
            total_pik: Money::from((0_i64, Currency::USD)),
            total_deferred: Money::from((0_i64, Currency::USD)),
            total_writedown: Money::from((0_i64, Currency::USD)),
        };

        assert_eq!(cashflows.tranche_id, "AAA");
        assert_eq!(cashflows.principal_flows.len(), 2);
        assert_eq!(cashflows.total_principal.amount(), 200_000.0);
    }
}
