//! `Marginable` implementation for [`CdsIndex`].

use super::CdsIndex;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::units::ONE_BASIS_POINT;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_margin::constants::CALENDAR_DAYS_PER_YEAR;
use finstack_quant_margin::constants::DURATION_APPROXIMATION_FACTOR;
use finstack_quant_margin::constants::STANDARD_CDS_MATURITY_YEARS;
use finstack_quant_margin::Marginable;
use finstack_quant_margin::NettingSetId;
use finstack_quant_margin::OtcMarginSpec;
use finstack_quant_margin::SimmSensitivities;
use finstack_quant_valuations_core::instruments::common_impl::margin::add_classified_credit_delta;
use finstack_quant_valuations_core::instruments::common_impl::margin::assign_credit_tenor_bucket;
use finstack_quant_valuations_core::instruments::common_impl::margin::netting_set_id_from_spec;
use finstack_quant_valuations_core::instruments::common_impl::margin::repriced_metric;
use finstack_quant_valuations_core::instruments::common_impl::traits::Instrument;

impl Marginable for CdsIndex {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn margin_spec(&self) -> Option<&OtcMarginSpec> {
        self.margin_spec.as_ref()
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        self.margin_spec.as_ref().map(netting_set_id_from_spec)
    }

    fn simm_sensitivities(&self, market: &MarketContext, as_of: Date) -> Result<SimmSensitivities> {
        let currency = self.notional.currency();
        let mut sens = SimmSensitivities::new(currency);

        let days_to_maturity = (self.premium_leg.end - as_of).whole_days().max(0) as f64;
        let years_to_maturity = days_to_maturity / CALENDAR_DAYS_PER_YEAR;
        let years_to_maturity = if years_to_maturity <= 0.0 {
            STANDARD_CDS_MATURITY_YEARS
        } else {
            years_to_maturity
        };

        let tenor = assign_credit_tenor_bucket(years_to_maturity);

        // Preferred path: repriced CS01 (respects the index survival/discount
        // curves, recovery and schedule). Sign reflects the actual index side.
        if let Some(cs01) = repriced_metric(
            self,
            market,
            as_of,
            finstack_quant_valuations_core::metrics::MetricId::Cs01,
        ) {
            add_classified_credit_delta(
                &mut sens,
                self.margin_spec.as_ref(),
                &self.index_name,
                tenor,
                cs01,
            )?;
            return Ok(sens);
        }

        // Fallback: flat risky-duration proxy.
        tracing::warn!(
            instrument_id = self.id.as_str(),
            "SIMM credit delta falling back to risky-duration proxy: repriced \
             CS01 unavailable (missing survival curve?)"
        );
        let recovery_rate = self.protection_leg.recovery_rate;
        let risky_duration =
            years_to_maturity * (1.0 - recovery_rate) * DURATION_APPROXIMATION_FACTOR;
        let cs01 = self.notional.amount().abs() * risky_duration * ONE_BASIS_POINT;
        let signed_cs01 = match self.side {
            finstack_quant_valuations_core::instruments::common_impl::parameters::legs::PayReceive::Pay => cs01,
            finstack_quant_valuations_core::instruments::common_impl::parameters::legs::PayReceive::Receive => -cs01,
        };
        add_classified_credit_delta(
            &mut sens,
            self.margin_spec.as_ref(),
            &self.index_name,
            tenor,
            signed_cs01,
        )?;

        Ok(sens)
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        self.value(market, as_of)
    }
}
