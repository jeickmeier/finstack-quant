//! Trait implementations for CommodityOption.

use crate::instruments::commodity::commodity_option::CommodityOption;
use crate::metrics::{HasDayCount, HasExpiry, HasPricingOverrides};

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
