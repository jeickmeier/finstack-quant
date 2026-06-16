//! Reinvestment management for structured credit deals.
//!
//! This module provides reinvestment logic for CLO and similar deals
//! that have active reinvestment periods.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

use super::pool::{AssetPool, PoolAsset};

use serde::{Deserialize, Serialize};

/// Manages reinvestment during the reinvestment period (price-only selection)
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReinvestmentManager {
    /// End date of reinvestment period
    #[schemars(with = "String")]
    pub end_date: Date,
    /// Whether reinvestment is currently allowed
    pub reinvestment_allowed: bool,
}

impl ReinvestmentManager {
    /// Create new reinvestment manager
    pub fn new(end_date: Date) -> Self {
        Self {
            end_date,
            reinvestment_allowed: true,
        }
    }

    /// Check if reinvestment is allowed at a given date
    pub fn can_reinvest(&self, as_of: Date) -> bool {
        as_of < self.end_date && self.reinvestment_allowed
    }

    /// Select assets for reinvestment from available market opportunities
    pub fn select_assets(
        &self,
        available_cash: Money,
        market_opportunities: Vec<PoolAsset>,
        _current_pool: &AssetPool,
        _market: &MarketContext,
        _as_of: Date,
    ) -> Vec<PoolAsset> {
        let mut remaining_cash = available_cash;

        // Helper to calculate price pct safely
        let get_price_pct = |asset: &PoolAsset| -> f64 {
            if let Some(price) = asset.purchase_price {
                if asset.balance.amount() > 0.0 {
                    (price.amount() / asset.balance.amount()).max(0.0)
                } else {
                    1.0
                }
            } else {
                1.0
            }
        };

        let mut indices: Vec<usize> = (0..market_opportunities.len()).collect();
        indices.sort_unstable_by(|&i, &j| {
            let pi = get_price_pct(&market_opportunities[i]);
            let pj = get_price_pct(&market_opportunities[j]);
            // Use total_cmp for safe float comparison (handles NaN consistently)
            pi.total_cmp(&pj)
        });

        let mut selected = Vec::new();
        for idx in indices {
            let asset = &market_opportunities[idx];

            let price_pct = get_price_pct(asset);

            let cost_amount = asset.balance.amount() * price_pct;
            if cost_amount <= remaining_cash.amount() {
                // Reduce cash and take the whole asset
                let cost = Money::new(cost_amount, remaining_cash.currency());
                remaining_cash = remaining_cash.checked_sub(cost).unwrap_or(remaining_cash);
                selected.push(asset.clone());
            }
        }

        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::types::{AssetType, DealType};
    use finstack_quant_core::currency::Currency;

    #[test]
    fn test_selects_cheapest_assets_first() {
        let mgr = ReinvestmentManager::new(
            Date::from_calendar_date(2026, time::Month::January, 1).expect("valid date"),
        );
        let pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

        let a = PoolAsset {
            id: "A".to_string().into(),
            asset_type: AssetType::HighYieldBond { industry: None },
            balance: Money::new(100.0, Currency::USD),
            rate: 0.0,
            spread_bps: None,
            index_id: None,
            maturity: Date::from_calendar_date(2030, time::Month::January, 1).expect("valid date"),
            credit_quality: None,
            industry: None,
            obligor_id: None,
            is_defaulted: false,
            recovery_amount: None,
            purchase_price: Some(Money::new(95.0, Currency::USD)),
            acquisition_date: None,
            day_count: finstack_quant_core::dates::DayCount::Act360,
            smm_override: None,
            mdr_override: None,
        };

        let b = PoolAsset {
            purchase_price: Some(Money::new(98.0, Currency::USD)),
            ..a.clone()
        };
        let c = PoolAsset {
            purchase_price: Some(Money::new(102.0, Currency::USD)),
            ..a.clone()
        };

        let cash = Money::new(195.0, Currency::USD);
        let selected = mgr.select_assets(
            cash,
            vec![b, a, c],
            &pool,
            &MarketContext::default(),
            Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date"),
        );

        // Expect picks at 95 and 98 first (total 193) and skips 102 due to budget
        assert_eq!(selected.len(), 2);
        assert!(selected
            .iter()
            .any(|x| x.purchase_price.expect("should succeed").amount() == 95.0));
        assert!(selected
            .iter()
            .any(|x| x.purchase_price.expect("should succeed").amount() == 98.0));
    }
}
