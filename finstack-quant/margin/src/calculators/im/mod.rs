//! Initial margin calculators.
//!
//! This module provides different IM calculation methodologies:
//!
//! - [`HaircutImCalculator`](crate::calculators::im::HaircutImCalculator): For repos and securities financing
//! - [`SimmCalculator`](crate::calculators::im::SimmCalculator): ISDA SIMM for OTC derivatives
//! - [`ScheduleImCalculator`](crate::calculators::im::schedule::ScheduleImCalculator): BCBS-IOSCO regulatory schedule fallback
//! - [`ClearingHouseImCalculator`](crate::calculators::im::ClearingHouseImCalculator): CCP-specific methodologies

mod clearing;
mod haircut;
/// BCBS-IOSCO schedule-based IM fallback calculator.
pub mod schedule;
/// ISDA SIMM calculator and supporting types.
pub(crate) mod simm;

pub use clearing::{CcpMethodology, ClearingHouseImCalculator};
pub use haircut::{HaircutImCalculator, HAIRCUT_MPOR_DAYS};
pub use simm::SimmCalculator;

use crate::Marginable;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Conservative fallback IM: `|exposure_base| × conservative_rate`.
pub(crate) fn conservative_im(
    exposure_base: Money,
    conservative_rate: f64,
) -> finstack_quant_core::Result<Money> {
    exposure_base.abs().checked_mul_f64(conservative_rate)
}

pub(crate) fn require_im_exposure_base(
    methodology: &str,
    instrument: &dyn Marginable,
    context: &MarketContext,
    as_of: Date,
    missing_source: &str,
) -> finstack_quant_core::Result<Money> {
    instrument.im_exposure_base(context, as_of)?.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "{methodology} IM for instrument '{}' requires {missing_source} or an explicit IM exposure base; refusing to use current MtM as a notional proxy",
            instrument.id()
        ))
    })
}

/// External IM input (CCP feed or internal VaR/ES model).
pub trait ExternalImSource: Send + Sync {
    /// Return the externally sourced IM amount for `instrument`, if
    /// available. Returning `None` causes the calculator to fall back to
    /// its conservative proxy.
    fn external_initial_margin(
        &self,
        instrument: &dyn Marginable,
        context: &MarketContext,
        as_of: Date,
    ) -> Option<Money>;

    /// Optional MPOR override in calendar days.
    fn external_mpor_days(&self) -> Option<u32> {
        None
    }
}
