//! Shared serde default helpers for instrument structs.

use finstack_quant_core::dates::DayCount;

use crate::instruments::SettlementType;

pub(crate) use finstack_quant_cashflows::serde_defaults::{
    bdc_modified_following, stub_short_front,
};

/// Default day count convention for option instruments (ACT/365F).
pub(crate) fn day_count_act365f() -> DayCount {
    DayCount::Act365F
}

/// Default settlement type for option instruments (cash).
pub(crate) fn settlement_cash() -> SettlementType {
    SettlementType::Cash
}

/// Default contract multiplier (1.0).
pub(crate) fn multiplier_one() -> f64 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::DayCount;

    #[test]
    fn day_count_act365f_matches_enum() {
        assert_eq!(day_count_act365f(), DayCount::Act365F);
    }

    #[test]
    fn settlement_cash_matches_enum() {
        assert_eq!(settlement_cash(), crate::instruments::SettlementType::Cash);
    }

    #[test]
    fn multiplier_one_is_unity() {
        assert_eq!(multiplier_one(), 1.0);
    }
}
