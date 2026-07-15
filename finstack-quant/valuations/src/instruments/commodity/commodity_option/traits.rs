//! Trait implementations for CommodityOption.

use crate::instruments::commodity::commodity_option::CommodityOption;
use crate::instruments::common_impl::traits::{EquityDependencies, EquityInstrumentDeps};
use crate::metrics::{HasDayCount, HasExpiry, HasPricingOverrides};

impl EquityDependencies for CommodityOption {
    fn equity_dependencies(&self) -> finstack_quant_core::Result<EquityInstrumentDeps> {
        let mut builder = EquityInstrumentDeps::builder()
            .vol_surface(self.vol_surface_id.as_str())
            .reference_strike(self.strike);
        if let Some(ref spot_id) = self.spot_id {
            builder = builder.spot(spot_id.as_str());
        }
        builder.build()
    }
}

impl HasPricingOverrides for CommodityOption {
    fn pricing_overrides_mut(&mut self) -> &mut crate::instruments::PricingOverrides {
        &mut self.pricing_overrides
    }
}

impl HasExpiry for CommodityOption {
    fn expiry(&self) -> finstack_quant_core::dates::Date {
        self.expiry
    }
}

impl HasDayCount for CommodityOption {
    fn day_count(&self) -> finstack_quant_core::dates::DayCount {
        self.day_count
    }
}
