//! Shared serde default helpers for cashflow specs.

use finstack_quant_core::dates::{BusinessDayConvention, StubKind};

/// Default stub convention for optional schedule stub fields.
pub(crate) fn stub_short_front() -> StubKind {
    StubKind::ShortFront
}

/// Default business day convention for optional BDC fields.
pub(crate) fn bdc_modified_following() -> BusinessDayConvention {
    BusinessDayConvention::ModifiedFollowing
}
