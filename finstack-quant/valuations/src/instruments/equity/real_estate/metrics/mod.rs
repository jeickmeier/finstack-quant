//! Real estate asset metrics module.
//!
//! Provides standard rate risk metrics for real estate valuations.

mod cap_rates;
mod levered;
mod returns;
mod sensitivities;

use crate::metrics::MetricRegistry;

use crate::instruments::equity::real_estate::{pricer, RealEstateAsset};
use crate::metrics::RfComponentPriced;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;

impl RfComponentPriced for RealEstateAsset {
    fn pv_with_rf_bump(
        &self,
        _market: &MarketContext,
        as_of: Date,
        bump_at: &dyn Fn(f64) -> f64,
    ) -> finstack_quant_core::Result<f64> {
        pricer::pv_with_rf_bump(self, as_of, bump_at)
    }
}

/// Register real estate asset metrics with the registry.
pub(crate) fn register_real_estate_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::RealEstateAsset,
        metrics: [
            // Rate risk via rf-component bump inside the property discount
            // rate : the asset always discounts at its
            // own rate, so DV01 bumps the additive risk-free component of
            // that rate rather than a market curve.
            (Dv01, crate::metrics::RfComponentDv01Calculator::<
                crate::instruments::RealEstateAsset,
            >::new(crate::metrics::RfDv01Mode::Parallel)),
            (BucketedDv01, crate::metrics::RfComponentDv01Calculator::<
                crate::instruments::RealEstateAsset,
            >::new(crate::metrics::RfDv01Mode::Bucketed)),
        ]
    };

    // Custom real estate deal-style metrics (non-core MetricId set).
    use std::sync::Arc;
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::going_in_cap_rate"),
        Arc::new(cap_rates::GoingInCapRate),
        &[InstrumentType::RealEstateAsset],
    );
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::exit_cap_rate"),
        Arc::new(cap_rates::ExitCapRate),
        &[InstrumentType::RealEstateAsset],
    );
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::unlevered_irr"),
        Arc::new(returns::UnleveredIrr),
        &[InstrumentType::RealEstateAsset],
    );
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::unlevered_multiple"),
        Arc::new(returns::UnleveredMultiple),
        &[InstrumentType::RealEstateAsset],
    );
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::unlevered_cash_on_cash_first"),
        Arc::new(returns::UnleveredCashOnCashFirst),
        &[InstrumentType::RealEstateAsset],
    );

    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::cap_rate_sensitivity"),
        Arc::new(sensitivities::CapRateSensitivity::default()),
        &[InstrumentType::RealEstateAsset],
    );
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::discount_rate_sensitivity"),
        Arc::new(sensitivities::DiscountRateSensitivity::default()),
        &[InstrumentType::RealEstateAsset],
    );
}

/// Register levered real estate equity metrics with the registry.
pub(crate) fn register_levered_real_estate_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::LeveredRealEstateEquity,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::LeveredRealEstateEquity,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::LeveredRealEstateEquity,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    };

    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::levered_irr"),
        Arc::new(levered::LeveredIrr),
        &[InstrumentType::LeveredRealEstateEquity],
    );
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::equity_multiple"),
        Arc::new(levered::EquityMultiple),
        &[InstrumentType::LeveredRealEstateEquity],
    );
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::ltv"),
        Arc::new(levered::LoanToValue),
        &[InstrumentType::LeveredRealEstateEquity],
    );
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::ltv_at_origination"),
        Arc::new(levered::LoanToValueAtOrigination),
        &[InstrumentType::LeveredRealEstateEquity],
    );
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::dscr_min"),
        Arc::new(levered::DscrMin),
        &[InstrumentType::LeveredRealEstateEquity],
    );
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::dscr_min_interest_only"),
        Arc::new(levered::DscrMinInterestOnly),
        &[InstrumentType::LeveredRealEstateEquity],
    );
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::debt_payoff_at_exit"),
        Arc::new(levered::DebtPayoffAtExit),
        &[InstrumentType::LeveredRealEstateEquity],
    );

    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::cap_rate_sensitivity"),
        Arc::new(sensitivities::CapRateSensitivity::default()),
        &[InstrumentType::LeveredRealEstateEquity],
    );
    registry.register_metric(
        crate::metrics::MetricId::custom("real_estate::discount_rate_sensitivity"),
        Arc::new(sensitivities::DiscountRateSensitivity::default()),
        &[InstrumentType::LeveredRealEstateEquity],
    );
}
