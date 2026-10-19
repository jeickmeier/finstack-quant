//! Price, yield, and spread metrics for bonds.

/// Asset swap spread calculators (par, market, forward)
pub(crate) mod asw;
/// Discount margin calculator
pub(crate) mod dm;
/// Embedded option value calculator
pub(crate) mod embedded_option_value;
/// I-spread (interpolated spread) calculator
pub(crate) mod i_spread;
/// Moosmüller yield-to-maturity calculator
pub(crate) mod moosmuller_ytm;
/// Option-adjusted spread (OAS) calculator
pub(crate) mod oas;
/// Price calculators (clean and dirty)
pub(crate) mod prices;
/// Embedded-option bond OAS model vega
pub(crate) mod vega;
/// Yield-to-maturity (YTM) calculator
pub(crate) mod ytm;
/// Yield-to-worst (YTW) calculator
pub(crate) mod ytw;
/// Z-spread (zero-volatility spread) calculator
pub(crate) mod z_spread;

use crate::bond::pricing::quote_conversions::japanese_simple_yield;
use crate::bond::pricing::settlement::QuoteDateContext;
use crate::Bond;
pub(crate) use asw::{AssetSwapMarketCalculator, AssetSwapParCalculator};
pub use dm::DiscountMarginCalculator;
pub(crate) use embedded_option_value::EmbeddedOptionValueCalculator;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
pub(crate) use i_spread::ISpreadCalculator;
pub(crate) use moosmuller_ytm::MoosmullerYtmCalculator;
pub(crate) use oas::OasCalculator;
pub(crate) use prices::{CleanPriceCalculator, DirtyPriceCalculator};
pub(crate) use vega::BondVegaCalculator;
pub(crate) use ytm::YtmCalculator;
pub(crate) use ytw::YtwCalculator;
pub use z_spread::ZSpreadCalculator;

/// Calculates the Japanese simple yield (単利), JSDA convention.
///
/// ```text
/// N = Act365F.year_fraction(quote_date, maturity)
/// y = [C + (100 − P) / N] / P
/// ```
///
/// with `C` the annual coupon and `P` the CLEAN price, both in % of par. The
/// clean price is the quoted clean price when set, otherwise the model dirty
/// value at the quote date less accrued. The inverse used by the quote engine
/// is the same closed form, so quotes round-trip at any settlement date.
/// Street `ytm` is unchanged.
///
/// # Dependencies
///
/// None (accrued is computed internally at quote_date).
pub(crate) struct JapaneseSimpleYieldCalculator;

impl MetricCalculator for JapaneseSimpleYieldCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let notional = bond.notional.amount();
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;

        let clean_pct = if let Some(clean_px) = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct
        {
            clean_px
        } else {
            let dirty_at_quote = crate::bond::pricing::settlement::model_dirty_at_quote_date(
                bond,
                &context.curves,
                context.as_of,
                quote_ctx.quote_date,
                context.base_value.amount(),
            )?;
            (dirty_at_quote - quote_ctx.accrued_at_quote_date) / notional * 100.0
        };
        japanese_simple_yield(bond, quote_ctx.quote_date, clean_pct)
    }
}
