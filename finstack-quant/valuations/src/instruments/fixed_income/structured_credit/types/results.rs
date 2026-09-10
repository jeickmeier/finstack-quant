//! Result types for structured credit tranche valuation.
//!
//! This module provides result types for individual tranche valuation within
//! structured credit instruments (CLO, ABS, RMBS, CMBS).

use crate::cashflow::traits::DatedFlows;
use crate::metrics::MetricId;
use finstack_quant_core::cashflow::CashFlow;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::money::Money;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Contractual coupon accrual retained independently of paid or deferred cash.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TrancheAccrualPeriod {
    /// Unadjusted inclusive contractual accrual boundary.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// Unadjusted exclusive contractual accrual boundary.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
    /// Adjusted date on which the coupon is payable.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub payment_date: Date,
    /// Outstanding note balance at the start of the period, in note currency.
    pub opening_balance: Money,
    /// Annual decimal coupon after any contractual available-funds cap.
    pub coupon_rate: f64,
    /// Contractual convention used to accrue this note's coupon.
    pub day_count: DayCount,
}

impl TrancheAccrualPeriod {
    /// Calculate unpaid current-period accrued interest in note currency units.
    ///
    /// # Arguments
    ///
    /// * `settlement` - Buyer settlement date. Accrual starts at the contractual
    ///   boundary, is capped at the contractual end, and resets on payment.
    pub fn accrued(&self, settlement: Date) -> finstack_quant_core::Result<f64> {
        if settlement <= self.start || settlement >= self.payment_date {
            return Ok(0.0);
        }
        let fraction = self.day_count.year_fraction(
            self.start,
            settlement.min(self.end),
            DayCountContext::default(),
        )?;
        Ok(self.opening_balance.amount() * self.coupon_rate * fraction)
    }
}

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
    /// Contractual coupon periods with balances before projected principal events.
    pub accrual_periods: Vec<TrancheAccrualPeriod>,
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
    /// Clean settlement price as a percentage of original note balance.
    pub clean_price: f64,
    /// Dirty settlement price as a percentage of original note balance.
    pub dirty_price: f64,
    /// Current-period accrued interest at buyer settlement in note currency.
    pub accrued: Money,
    /// Weighted average life.
    pub wal: f64,
    /// Modified duration.
    pub modified_duration: f64,
    /// Z-spread (basis points).
    pub z_spread_bp: f64,
    /// CS01 (credit DV01).
    pub cs01: f64,
    /// Decimal yield reproducing the dirty settlement target (annual by default).
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
            accrual_periods: Vec::new(),
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
