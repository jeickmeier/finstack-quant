//! Cashflow construction for bonds.
//!
//! Implements [`CashflowProvider`] for [`Bond`], producing a signed canonical
//! schedule that preserves fees, signed notionals, and all valid cash events.
//! Pure PIK accretion is omitted; the notional evolution it drives is captured
//! in the balance path.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

use crate::cashflow::builder::CashflowRepresentation;

use super::types::Bond;

impl finstack_quant_cashflows::CashflowScheduleSource for Bond {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &MarketContext,
        _as_of: Date,
    ) -> Result<crate::cashflow::builder::CashFlowSchedule> {
        let schedule = if let Some(ref custom) = self.custom_cashflows {
            custom.clone()
        } else {
            self.full_cashflow_schedule(curves)?
        };

        let representation = if self.has_floating_coupons() {
            CashflowRepresentation::Projected
        } else {
            CashflowRepresentation::Contractual
        };

        Ok(schedule.with_representation(representation))
    }
}
