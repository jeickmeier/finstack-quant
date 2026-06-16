use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};

/// Calculates dirty price for bonds (clean price + accrued interest).
///
/// Dirty price is the full price paid by the buyer, including accrued interest
/// since the last coupon payment. It is computed as:
/// ```text
/// Dirty Price = Clean Price + Accrued Interest(quote_date)
/// ```
///
/// When a quoted clean price is set, accrued interest is computed at the
/// **quote date** (settlement date) to match market convention. When no
/// quoted price is available, `base_value` (PV at `as_of`) is returned.
///
/// # Dependencies
///
/// Requires `Accrued` metric to be computed first (used for the model-PV path).
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // Dirty price is computed automatically when requesting bond metrics
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub(crate) struct DirtyPriceCalculator;

impl MetricCalculator for DirtyPriceCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Accrued]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;

        if let Some(clean_px) = bond.pricing_overrides.market_quotes.quoted_clean_price {
            // Market-quote path: accrued at the quote/settlement date, consistent
            // with how YTM, Z-spread, and the quote engine interpret market prices.
            let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
            return Ok(quote_ctx.dirty_from_clean_pct(clean_px, bond.notional.amount()));
        }

        // Model-PV path: base_value is already the dirty price (PV at as_of)
        Ok(context.base_value.amount())
    }
}

/// Calculates clean price for bonds (dirty price - accrued interest).
///
/// Clean price is the quoted price excluding accrued interest. It can be:
/// - Retrieved directly from `bond.pricing_overrides.market_quotes.quoted_clean_price` if set
/// - Computed from the base value (model PV) minus accrued interest at `as_of`
///
/// # Dependencies
///
/// Requires `Accrued` metric to be computed first (used for the model-PV path).
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // Clean price is computed automatically when requesting bond metrics
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub(crate) struct CleanPriceCalculator;

impl MetricCalculator for CleanPriceCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Accrued]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;

        if let Some(clean_px) = bond.pricing_overrides.market_quotes.quoted_clean_price {
            return Ok(clean_px * bond.notional.amount() / 100.0);
        }

        // Model-PV path: base_value is dirty (PV at as_of), subtract as_of accrued
        let dirty_px = context.base_value.amount();
        let accrued = context
            .computed
            .get(&MetricId::Accrued)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:Accrued".to_string(),
                })
            })?;

        Ok(dirty_px - accrued)
    }
}
