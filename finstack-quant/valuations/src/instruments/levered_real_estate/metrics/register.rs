//! Metric registration and calculators.

use super::super::{LeveredRealEstateEquity, levered_pricer};
use crate::instruments::Instrument;
use crate::metrics::sensitivities::bump_config;
use crate::metrics::sensitivities::{RfComponentDv01Calculator, RfComponentPriced};
use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry, MetricRegistryError};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_equity::real_estate::metrics::{
    bumped_cap_rate, ensure_cap_rate01_applicable,
};
use finstack_quant_valuations_equity::real_estate::pricer as asset_pricer;

impl RfComponentPriced for LeveredRealEstateEquity {
    /// Asset PV at the bumped property discount rate less the financing PV,
    /// which does not depend on that rate.
    fn pv_with_rf_bump(
        &self,
        market: &MarketContext,
        as_of: Date,
        bump: f64,
    ) -> finstack_quant_core::Result<f64> {
        levered_pricer::validate_currency(self)?;
        let asset_pv = asset_pricer::pv_with_rf_bump(&self.asset, as_of, bump)?;
        Ok(asset_pv - levered_pricer::financing_pv(self, market, as_of)?)
    }
}

/// `real_estate::cap_rate01` for [`LeveredRealEstateEquity`]: PV change per 1bp
/// cap-rate move on the underlying asset.
struct LeveredCapRate01Calculator;

impl MetricCalculator for LeveredCapRate01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bump_bp = bump_config::resolve(context)?.rate_bump_bp;
        let bump = bump_bp / 10_000.0;
        let market = context.curves.as_ref();
        let as_of = context.as_of;

        let levered: &LeveredRealEstateEquity = context.instrument_as()?;
        ensure_cap_rate01_applicable(&levered.asset)?;
        let mut up = levered.clone();
        up.asset = bumped_cap_rate(&levered.asset, bump)?;
        let mut down = levered.clone();
        down.asset = bumped_cap_rate(&levered.asset, -bump)?;

        let pv_up = up.value(market, as_of)?.amount();
        let pv_down = down.value(market, as_of)?.amount();

        Ok((pv_up - pv_down) / (2.0 * bump_bp))
    }
}

/// Register levered real estate equity metrics with the registry.
pub(crate) fn register_levered_real_estate_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), MetricRegistryError> {
    use crate::pricer::InstrumentType;

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::LeveredRealEstateEquity,
        metrics: [
            (Dv01, crate::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(crate::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(crate::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ],
        custom: [
            ("real_estate::levered_irr", super::levered::LeveredIrr),
            ("real_estate::equity_multiple", super::levered::EquityMultiple),
            ("real_estate::ltv", super::levered::LoanToValue),
            ("real_estate::ltv_at_origination", super::levered::LoanToValueAtOrigination),
            ("real_estate::dscr_min", super::levered::DscrMin),
            ("real_estate::dscr_min_interest_only", super::levered::DscrMinInterestOnly),
            ("real_estate::debt_payoff_at_exit", super::levered::DebtPayoffAtExit),
            ("real_estate::discount_rate01", RfComponentDv01Calculator::<LeveredRealEstateEquity>::new()),
            ("real_estate::cap_rate01", LeveredCapRate01Calculator),
        ]
    };

    Ok(())
}
