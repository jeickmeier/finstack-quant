//! Shared initial-margin exposure-base validation and conservative fallback helpers.

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
