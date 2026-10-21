//! Real estate asset metrics module.
//!
//! Provides standard rate risk metrics for real estate valuations.

mod cap_rates;
mod levered;
mod returns;
mod sensitivities;

use crate::metrics::MetricRegistry;

use crate::instruments::equity::real_estate::{
    levered_pricer, pricer, LeveredRealEstateEquity, RealEstateAsset,
};
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

impl RfComponentPriced for LeveredRealEstateEquity {
    /// Asset PV at the bumped property discount rate less the financing PV,
    /// which does not depend on that rate.
    fn pv_with_rf_bump(
        &self,
        market: &MarketContext,
        as_of: Date,
        bump_at: &dyn Fn(f64) -> f64,
    ) -> finstack_quant_core::Result<f64> {
        levered_pricer::validate_currency(self)?;
        let asset_pv = pricer::pv_with_rf_bump(&self.asset, as_of, bump_at)?;
        Ok(asset_pv - levered_pricer::financing_pv(self, market, as_of)?)
    }
}

/// Register real estate asset metrics with the registry.
pub(crate) fn register_real_estate_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::RealEstateAsset,
        metrics: [],
        custom: [
            ("real_estate::discount_rate01", crate::metrics::RfComponentDv01Calculator::<crate::instruments::equity::real_estate::RealEstateAsset>::new()),
            // Custom real estate deal-style metrics.
            ("real_estate::going_in_cap_rate", cap_rates::GoingInCapRate),
            ("real_estate::exit_cap_rate", cap_rates::ExitCapRate),
            ("real_estate::unlevered_irr", returns::UnleveredIrr),
            ("real_estate::unlevered_multiple", returns::UnleveredMultiple),
            ("real_estate::unlevered_cash_on_cash_first", returns::UnleveredCashOnCashFirst),
            ("real_estate::cap_rate01", sensitivities::CapRate01Calculator),
        ]
    }

    Ok(())
}

/// Register levered real estate equity metrics with the registry.
pub(crate) fn register_levered_real_estate_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::LeveredRealEstateEquity,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ],
        custom: [
            ("real_estate::levered_irr", levered::LeveredIrr),
            ("real_estate::equity_multiple", levered::EquityMultiple),
            ("real_estate::ltv", levered::LoanToValue),
            ("real_estate::ltv_at_origination", levered::LoanToValueAtOrigination),
            ("real_estate::dscr_min", levered::DscrMin),
            ("real_estate::dscr_min_interest_only", levered::DscrMinInterestOnly),
            ("real_estate::debt_payoff_at_exit", levered::DebtPayoffAtExit),
            ("real_estate::discount_rate01", crate::metrics::RfComponentDv01Calculator::<crate::instruments::equity::real_estate::LeveredRealEstateEquity>::new()),
            ("real_estate::cap_rate01", sensitivities::CapRate01Calculator),
        ]
    };

    Ok(())
}
