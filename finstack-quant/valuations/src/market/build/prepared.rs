//! Prepared quote envelopes for calibration pipelines.

use crate::instruments::Instrument;
use crate::market::build::cds::{build_cds_instrument, resolve_cds_quote_dates};
use crate::market::build::rates::{build_rate_instrument, resolve_rate_quote_dates};
use crate::market::build::xccy::build_xccy_instrument;
use crate::market::quotes::cds::CdsQuote;
use crate::market::quotes::rates::RateQuote;
use crate::market::quotes::xccy::XccyQuote;
use crate::market::BuildCtx;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::Result;
use std::fmt;
use std::sync::Arc;

/// A quote accompanied by its constructed instrument and precomputed time pillar.
///
/// This structure is the primary input for calibration solvers. It decouples the solver
/// from the details of quote parsing, convention resolution, and instrument construction.
/// The precomputed `pillar_time` allows solvers to efficiently sort and group quotes by
/// maturity without recalculating time-to-maturity for each iteration.
///
/// # Invariants
///
/// - `pillar_time` is calculated using the day-count convention chosen by the calibration target
/// - `pillar_date` corresponds to the maturity date of the instrument's pillar
/// - `instrument` is fully configured and ready for pricing
///
/// # Note
///
/// This is ephemeral and valid only for the `as_of` date used during construction.
/// If the valuation date changes, a new `PreparedQuote` must be created.
///
/// # Examples
///
/// ```text
/// # use finstack_quant_valuations::market::build::prepared::PreparedQuote;
/// # use finstack_quant_valuations::market::quotes::rates::RateQuote;
/// # use finstack_quant_valuations::market::quotes::ids::QuoteId;
/// # use finstack_quant_core::dates::Date;
/// # use std::sync::Arc;
/// # use finstack_quant_valuations::instruments::Instrument;
/// #
/// # fn example() -> finstack_quant_core::Result<()> {
/// // In practice, this would be created by a builder function
/// // let prepared = prepare_quote(quote, ctx)?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub(crate) struct PreparedQuote<Q> {
    /// The original market quote.
    ///
    /// Stored as `Arc` to allow sharing across multiple solver iterations without cloning.
    pub(crate) quote: Arc<Q>,
    /// The constructed instrument, fully configured for pricing.
    ///
    /// The instrument is ready to be priced and includes all necessary curve references,
    /// dates, and market conventions resolved from the quote.
    pub(crate) instrument: Arc<dyn Instrument>,
    /// The maturity date of the pillar (used for sorting / time axis).
    ///
    /// This is the resolved maturity date from the quote's pillar (either from a tenor
    /// calculation or a direct date specification).
    pub(crate) pillar_date: Date,
    /// The time-to-maturity of the pillar (in years), precomputed for the solver.
    ///
    /// This value is calculated once during construction and reused by calibration solvers
    /// for sorting, grouping, and time-axis calculations.
    pub(crate) pillar_time: f64,
}

impl<Q: fmt::Debug> fmt::Debug for PreparedQuote<Q> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedQuote")
            .field("quote", &self.quote)
            .field("pillar_date", &self.pillar_date)
            .field("pillar_time", &self.pillar_time)
            .field("instrument", &"<Instrument>")
            .finish()
    }
}

impl<Q> PreparedQuote<Q> {
    /// Create a new prepared quote.
    ///
    /// # Arguments
    ///
    /// * `quote` - The original market quote (wrapped in `Arc` for sharing)
    /// * `instrument` - The constructed instrument ready for pricing
    /// * `pillar_date` - The resolved maturity date of the pillar
    /// * `pillar_time` - The time-to-maturity in years, calculated from `as_of` to `pillar_date`
    ///
    /// # Returns
    ///
    /// A new `PreparedQuote` instance.
    ///
    /// # Examples
    ///
    /// ```text
    /// # use finstack_quant_valuations::market::build::prepared::PreparedQuote;
    /// # use finstack_quant_core::dates::Date;
    /// # use std::sync::Arc;
    /// # use finstack_quant_valuations::instruments::Instrument;
    /// #
    /// # fn example(quote: Arc<String>, instrument: Arc<dyn Instrument>) -> finstack_quant_core::Result<()> {
    /// let pillar_date = Date::from_calendar_date(2025, time::Month::January, 2).unwrap();
    /// let as_of = Date::from_calendar_date(2024, time::Month::January, 2).unwrap();
    /// let pillar_time = (pillar_date - as_of).whole_days() as f64 / 365.25;
    ///
    /// let prepared = PreparedQuote::new(quote, instrument, pillar_date, pillar_time);
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) fn new(
        quote: Arc<Q>,
        instrument: Arc<dyn Instrument>,
        pillar_date: Date,
        pillar_time: f64,
    ) -> Self {
        Self {
            quote,
            instrument,
            pillar_date,
            pillar_time,
        }
    }
}

/// Prepare a rate quote into an instrument + pillar time.
pub(crate) fn prepare_rate_quote(
    quote: RateQuote,
    build_ctx: &BuildCtx,
    curve_day_count: DayCount,
    base_date: Date,
    swap_use_payment_delay: bool,
) -> Result<PreparedQuote<RateQuote>> {
    let maturity_date =
        resolve_rate_quote_dates(&quote, build_ctx, swap_use_payment_delay)?.pillar_date();
    let instrument = build_rate_instrument(&quote, build_ctx)?;
    let instrument: Arc<dyn Instrument> = instrument.into();

    let pillar_time =
        curve_day_count.year_fraction(base_date, maturity_date, DayCountContext::default())?;

    Ok(PreparedQuote::new(
        Arc::new(quote),
        instrument,
        maturity_date,
        pillar_time,
    ))
}

/// Prepare an XCCY basis-swap quote into an instrument + pillar time.
///
/// Uses the pair convention (registered via `XccyConventionId`) to construct the
/// underlying [`crate::instruments::rates::xccy_swap::XccySwap`]. The pillar date is
/// the swap's maturity (far date of the basis-swap quote). The convention's
/// `notional_exchange` field determines whether the resulting swap is fixed-notional
/// or `MtmResetting` — G10 pairs default to MtM-resetting per dealer convention.
pub(crate) fn prepare_xccy_quote(
    quote: XccyQuote,
    build_ctx: &BuildCtx,
    curve_day_count: DayCount,
    base_date: Date,
) -> Result<PreparedQuote<XccyQuote>> {
    let instrument: Box<dyn Instrument> = build_xccy_instrument(&quote, build_ctx)?;
    let instrument_arc: Arc<dyn Instrument> = instrument.into();
    let maturity_date = xccy_quote_pillar_date(&quote, instrument_arc.as_ref())?;
    let pillar_time =
        curve_day_count.year_fraction(base_date, maturity_date, DayCountContext::default())?;

    Ok(PreparedQuote::new(
        Arc::new(quote),
        instrument_arc,
        maturity_date,
        pillar_time,
    ))
}

/// Resolve the maturity (far) date for an `XccyQuote::BasisSwap`. The cleanest path is
/// to call `build_xccy_instrument` (which already does all the date math + convention
/// resolution) and read `leg1.end` off the constructed swap — both legs share the same
/// end date by builder construction.
fn xccy_quote_pillar_date(quote: &XccyQuote, instrument: &dyn Instrument) -> Result<Date> {
    use crate::instruments::rates::xccy_swap::XccySwap;
    let swap = instrument
        .as_any()
        .downcast_ref::<XccySwap>()
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "Built instrument for XccyQuote '{}' is not an XccySwap",
                quote.id().as_str()
            ))
        })?;
    Ok(swap.leg1.end)
}

/// Prepare a CDS quote into an instrument + pillar time.
pub(crate) fn prepare_cds_quote(
    quote: CdsQuote,
    build_ctx: &BuildCtx,
    day_count: DayCount,
    base_date: Date,
) -> Result<PreparedQuote<CdsQuote>> {
    let maturity_date = resolve_cds_quote_dates(&quote, build_ctx)?.maturity;
    let instrument = build_cds_instrument(&quote, build_ctx)?;
    let instrument: Arc<dyn Instrument> = instrument.into();

    let pillar_time =
        day_count.year_fraction(base_date, maturity_date, DayCountContext::default())?;

    Ok(PreparedQuote::new(
        Arc::new(quote),
        instrument,
        maturity_date,
        pillar_time,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::quotes::ids::{Pillar, QuoteId};
    use crate::market::quotes::rates::RateQuote;
    use finstack_quant_core::HashMap;
    use time::Month;

    #[test]
    fn prepare_rate_quote_uses_future_period_end_as_pillar() {
        let as_of = Date::from_calendar_date(2025, Month::January, 10).expect("valid date");
        let mut curve_ids = HashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("forward".to_string(), "USD-SOFR".to_string());
        let ctx = BuildCtx::new(as_of, 1_000_000.0, curve_ids);

        let quote = RateQuote::Futures {
            id: QuoteId::new("USD-FUT-SEP25"),
            contract: "SR3".into(),
            expiry: Date::from_calendar_date(2025, Month::September, 15).expect("valid expiry"),
            price: 96.50,
            convexity_adjustment: None,
            vol_surface_id: None,
        };

        let prepared = prepare_rate_quote(quote, &ctx, DayCount::Act365F, as_of, true)
            .expect("prepared futures quote");

        let future = prepared
            .instrument
            .as_any()
            .downcast_ref::<crate::instruments::rates::ir_future::InterestRateFuture>()
            .expect("expected interest rate future");

        assert_eq!(
            prepared.pillar_date,
            future.period_end.expect("future period_end")
        );
    }

    #[test]
    fn prepare_rate_quote_uses_swap_end_as_pillar() {
        let as_of = Date::from_calendar_date(2025, Month::January, 10).expect("valid date");
        let mut curve_ids = HashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("forward".to_string(), "USD-SOFR".to_string());
        let ctx = BuildCtx::new(as_of, 1_000_000.0, curve_ids);

        let quote = RateQuote::Swap {
            id: QuoteId::new("USD-SOFR-OIS-SWAP-5Y"),
            index: finstack_quant_core::types::IndexId::new("USD-SOFR-OIS"),
            pillar: crate::market::quotes::ids::Pillar::Tenor(
                finstack_quant_core::dates::Tenor::new(
                    5,
                    finstack_quant_core::dates::TenorUnit::Years,
                ),
            ),
            rate: 0.0450,
            spread_decimal: None,
        };

        let prepared = prepare_rate_quote(quote, &ctx, DayCount::Act365F, as_of, true)
            .expect("prepared swap quote");

        assert!(
            prepared.pillar_time > 4.0,
            "5Y swap pillar time should be > 4.0, got {}",
            prepared.pillar_time
        );
        assert!(
            prepared.pillar_date > as_of,
            "swap pillar date should be after as_of"
        );
    }

    #[test]
    fn prepare_cds_quote_uses_resolved_imm_maturity_as_pillar() {
        let as_of = Date::from_calendar_date(2024, Month::January, 2).expect("valid date");
        let mut curve_ids = HashMap::default();
        curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
        curve_ids.insert("credit".to_string(), "ABC-CORP".to_string());
        let ctx = BuildCtx::new(as_of, 10_000_000.0, curve_ids);

        let explicit_maturity =
            Date::from_calendar_date(2026, Month::June, 20).expect("valid maturity");
        let quote = CdsQuote::CdsParSpread {
            id: QuoteId::new("CDS-TEST-IMM"),
            entity: "Test Corp".to_string(),
            convention: crate::market::conventions::ids::CdsConventionKey {
                currency: finstack_quant_core::currency::Currency::USD,
                doc_clause: crate::market::conventions::ids::CdsDocClause::IsdaNa,
            },
            pillar: Pillar::Date(explicit_maturity),
            spread_bp: 100.0,
            recovery_rate: 0.40,
        };

        let prepared =
            prepare_cds_quote(quote, &ctx, DayCount::Act365F, as_of).expect("prepared CDS quote");

        assert_eq!(prepared.pillar_date, explicit_maturity);
        assert!(prepared.pillar_time > 2.0);
    }
}
