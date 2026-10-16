//! Shared serde default helpers for cashflow specs.
//!
//! These functions supply the serde field defaults for
//! [`crate::builder::ScheduleParams`] and [`crate::builder::FeeSpec::PeriodicBp`].

use finstack_quant_core::dates::{BusinessDayConvention, StubKind};

/// Default stub convention for optional schedule stub fields (`ShortFront`).
pub(crate) fn stub_short_front() -> StubKind {
    StubKind::ShortFront
}

/// Default business day convention for optional BDC fields (`ModifiedFollowing`).
pub(crate) fn bdc_modified_following() -> BusinessDayConvention {
    BusinessDayConvention::ModifiedFollowing
}
