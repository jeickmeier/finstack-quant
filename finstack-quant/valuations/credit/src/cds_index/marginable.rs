//! `Marginable` implementation for [`CdsIndex`].

use super::CdsIndex;
use crate::cds::marginable::{credit_simm_sensitivities, CreditSimmTerms};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_margin::types::NettingSetId;
use finstack_quant_margin::types::OtcMarginSpec;
use finstack_quant_margin::types::SimmSensitivities;
use finstack_quant_margin::Marginable;
use finstack_quant_valuations_core::instruments::common_impl::margin::netting_set_id_from_spec;
use finstack_quant_valuations_core::instruments::Instrument;

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
        credit_simm_sensitivities(
            self,
            market,
            as_of,
            CreditSimmTerms {
                qualifier: &self.index_name,
                maturity: self.premium_leg.end,
                recovery_rate: self.protection_leg.recovery_rate,
                notional: Money::new(
                    self.notional.amount() * self.index_factor,
                    self.notional.currency(),
                )?,
                side: self.side,
            },
        )
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        self.value(market, as_of)
    }
}
