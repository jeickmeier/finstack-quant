//! Shared serde default helpers for instrument structs.

use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind};

use crate::instruments::SettlementType;

/// Default stub convention for optional schedule stub fields.
pub(crate) fn stub_short_front() -> StubKind {
    StubKind::ShortFront
}

/// Default business day convention for optional BDC fields.
pub(crate) fn bdc_modified_following() -> BusinessDayConvention {
    BusinessDayConvention::ModifiedFollowing
}

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
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind};

    #[test]
    fn stub_short_front_matches_short_front_variant() {
        assert_eq!(stub_short_front(), StubKind::ShortFront);
    }

    #[test]
    fn bdc_modified_following_matches_enum() {
        assert_eq!(
            bdc_modified_following(),
            BusinessDayConvention::ModifiedFollowing
        );
    }

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
