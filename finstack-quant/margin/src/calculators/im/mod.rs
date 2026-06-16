//! Initial margin calculators.
//!
//! This module provides different IM calculation methodologies:
//!
//! - [`HaircutImCalculator`]: For repos and securities financing
//! - [`SimmCalculator`]: ISDA SIMM for OTC derivatives
//! - [`ScheduleImCalculator`]: BCBS-IOSCO regulatory schedule fallback
//! - [`ClearingHouseImCalculator`]: CCP-specific methodologies
//! - [`InternalModelImCalculator`]: Internal model (VaR/ES-based) stub

mod clearing;
mod haircut;
mod internal;
/// BCBS-IOSCO schedule-based IM fallback calculator.
pub mod schedule;
/// ISDA SIMM calculator and supporting types.
pub mod simm;

pub use clearing::{CcpMethodology, ClearingHouseImCalculator};
pub use haircut::HaircutImCalculator;
pub use internal::InternalModelImCalculator;
pub use schedule::ScheduleImCalculator;
pub use simm::SimmCalculator;

use crate::traits::Marginable;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Conservative fallback IM: `|exposure_base| × conservative_rate`.
///
/// Shared by [`ClearingHouseImCalculator`] and [`InternalModelImCalculator`]
/// — any IM calculator that falls back to a pure percentage-of-exposure
/// heuristic should route through this helper to keep the formula in one
/// place.
#[must_use]
pub(crate) fn conservative_im(exposure_base: Money, conservative_rate: f64) -> Money {
    Money::new(exposure_base.amount().abs(), exposure_base.currency()) * conservative_rate
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

/// Unified external-IM input source used by calculators that can be driven
/// by an externally provided IM number (CCP feed, internal VaR/ES model).
///
/// Implementers typically already know which methodology / model they
/// represent — the previous trait variants that threaded a methodology tag
/// through every call were redundant and have been collapsed into this
/// single interface.
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

    /// Optional label (model/methodology) surfaced in the IM result
    /// breakdown map.
    fn external_model_name(&self) -> Option<String> {
        None
    }
}
