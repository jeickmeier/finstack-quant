//! `Marginable` implementation for [`EquityTotalReturnSwap`].

use super::EquityTotalReturnSwap;
use crate::instruments::common_impl::margin::netting_set_id_from_spec;
use crate::instruments::PayReceive;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_margin::Marginable;
use finstack_quant_margin::NettingSetId;
use finstack_quant_margin::OtcMarginSpec;
use finstack_quant_margin::SimmSensitivities;

impl Marginable for EquityTotalReturnSwap {
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
        _market: &MarketContext,
        _as_of: Date,
    ) -> Result<SimmSensitivities> {
        let currency = self.notional.currency();
        let mut sens = SimmSensitivities::new(currency);

        // For Equity TRS, main sensitivity is equity delta
        // SIMM delta is P&L per 1% relative underlying move, not full notional.
        let delta = match self.side {
            PayReceive::Receive => self.notional.amount(),
            PayReceive::Pay => -self.notional.amount(),
        };

        // Use the underlier as the equity identifier
        let underlier = &self.underlying.ticker;
        sens.add_equity_delta(underlier, delta * 0.01);

        Ok(sens)
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        use crate::instruments::common_impl::traits::Instrument;
        self.value(market, as_of)
    }
}
