//! `Marginable` implementation for [`FiIndexTotalReturnSwap`].

use super::FiIndexTotalReturnSwap;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::units::ONE_BASIS_POINT;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_margin::Marginable;
use finstack_quant_margin::NettingSetId;
use finstack_quant_margin::OtcMarginSpec;
use finstack_quant_margin::SimmSensitivities;
use finstack_quant_valuations_core::instruments::common_impl::margin::assign_ir_tenor_bucket;
use finstack_quant_valuations_core::instruments::common_impl::margin::netting_set_id_from_spec;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::PayReceive;

impl Marginable for FiIndexTotalReturnSwap {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn margin_spec(&self) -> Option<&OtcMarginSpec> {
        self.margin_spec.as_ref()
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        self.margin_spec.as_ref().map(netting_set_id_from_spec)
    }

    fn simm_sensitivities(
        &self,
        market: &MarketContext,
        _as_of: Date,
    ) -> Result<SimmSensitivities> {
        let currency = self.notional.currency();
        let mut sens = SimmSensitivities::new(currency);

        let duration = self.index_duration(market)?;

        let dv01 = self.notional.amount().abs() * duration * ONE_BASIS_POINT;

        let signed_dv01 = match self.side {
            PayReceive::Receive => -dv01, // Long bond = short rates
            PayReceive::Pay => dv01,      // Short bond = long rates
        };

        // Map duration to appropriate tenor bucket
        let tenor = assign_ir_tenor_bucket(duration);

        sens.add_ir_delta(currency, tenor, signed_dv01);

        Ok(sens)
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        self.value(market, as_of)
    }
}
