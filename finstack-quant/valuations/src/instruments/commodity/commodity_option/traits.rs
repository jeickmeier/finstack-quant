//! Trait implementations for CommodityOption.

use crate::instruments::commodity::commodity_option::CommodityOption;
use crate::metrics::{HasDayCount, HasExpiry};

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
