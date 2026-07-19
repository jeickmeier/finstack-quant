//! Immutable market and portfolio states owned by one evaluation request.

use super::plan::{MarketStateId, PortfolioStateId};
use crate::Portfolio;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::PricingOptions;
use std::borrow::Cow;

pub(super) struct PreparedMarketState<'a> {
    pub(super) id: MarketStateId,
    pub(super) market: Cow<'a, MarketContext>,
    pub(super) as_of: Date,
    pub(super) pricing_options: PricingOptions,
}

impl<'a> PreparedMarketState<'a> {
    pub(super) fn borrowed(
        id: MarketStateId,
        market: &'a MarketContext,
        as_of: Date,
        config: &FinstackConfig,
    ) -> Self {
        Self {
            id,
            market: Cow::Borrowed(market),
            as_of,
            pricing_options: state_pricing_options(config),
        }
    }

    pub(super) fn owned(
        id: MarketStateId,
        market: MarketContext,
        as_of: Date,
        config: &FinstackConfig,
    ) -> Self {
        Self {
            id,
            market: Cow::Owned(market),
            as_of,
            pricing_options: state_pricing_options(config),
        }
    }
}

pub(super) struct PreparedPortfolioState<'a> {
    pub(super) id: PortfolioStateId,
    pub(super) portfolio: Cow<'a, Portfolio>,
}

impl<'a> PreparedPortfolioState<'a> {
    pub(super) fn borrowed(id: PortfolioStateId, portfolio: &'a Portfolio) -> Self {
        Self {
            id,
            portfolio: Cow::Borrowed(portfolio),
        }
    }

    pub(super) fn owned(id: PortfolioStateId, portfolio: Portfolio) -> Self {
        Self {
            id,
            portfolio: Cow::Owned(portfolio),
        }
    }
}

fn state_pricing_options(config: &FinstackConfig) -> PricingOptions {
    PricingOptions::default()
        .with_config(config)
        .with_new_hazard_recalibration_cache()
        .with_new_rate_recalibration_cache()
}
