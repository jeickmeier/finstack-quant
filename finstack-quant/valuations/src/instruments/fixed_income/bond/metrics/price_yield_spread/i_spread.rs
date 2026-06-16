use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, StubKind, Tenor};
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, DiscountCurveRateQuoteType,
};

/// Configuration for I-Spread fixed-leg conventions.
///
/// Controls the proxy swap fixed leg used to derive the par rate that is
/// subtracted from the bond's YTM.
#[derive(Debug, Clone)]
pub(crate) struct ISpreadConfig {
    /// Day-count convention for the proxy fixed leg used in the par rate.
    pub fixed_leg_day_count: DayCount,
    /// Payment frequency for the proxy fixed leg.
    pub fixed_leg_frequency: Tenor,
}

impl Default for ISpreadConfig {
    fn default() -> Self {
        Self {
            // Preserve previous behaviour: annual Act/Act proxy fixed leg.
            fixed_leg_day_count: DayCount::ActAct,
            fixed_leg_frequency: Tenor::annual(),
        }
    }
}

/// I-Spread: bond YTM minus interpolated swap par rate at same maturity.
///
/// Uses the bond's discount curve to approximate a par swap fixed leg with
/// configurable day-count and frequency (defaults to annual Act/Act).
///
/// The I-spread is computed as:
/// ```text
/// I-Spread = YTM - par_swap_rate
/// ```
/// where `par_swap_rate` is derived from the discount curve using a proxy
/// fixed-leg schedule.
///
/// # Dependencies
///
/// Requires `Ytm` metric to be computed first.
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
/// // I-spread is computed automatically when requesting bond metrics
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default)]
pub(crate) struct ISpreadCalculator {
    config: ISpreadConfig,
}

#[allow(dead_code)] // public API for external bindings
impl ISpreadCalculator {
    /// Create an I-Spread calculator with default (annual Act/Act) fixed-leg
    /// conventions.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Create an I-Spread calculator with explicit fixed-leg conventions.
    pub(crate) fn with_config(config: ISpreadConfig) -> Self {
        Self { config }
    }
}

impl MetricCalculator for ISpreadCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Ytm]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;

        // Bond YTM from dependencies
        let ytm = context
            .computed
            .get(&MetricId::Ytm)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:Ytm".to_string(),
                })
            })?;

        // Use the bond's discount curve as proxy for swap discounting (OIS collateral)
        let disc = context.curves.get_discount(&bond.discount_curve_id)?;

        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let flows = bond.pricing_dated_cashflows(&context.curves, context.as_of)?;
        let (yield_rate, spread_maturity, uses_workout_path) =
            if let Some((workout_yield, workout_flows, _)) =
                crate::instruments::fixed_income::bond::metrics::quoted_workout_path(
                    bond,
                    context.curves.as_ref(),
                    context.as_of,
                    &flows,
                )?
            {
                let maturity = workout_flows
                    .last()
                    .map(|(date, _)| *date)
                    .unwrap_or(bond.maturity);
                (workout_yield, maturity, maturity < bond.maturity)
            } else {
                (ytm, bond.maturity, false)
            };

        if uses_workout_path {
            let t = disc.day_count().year_fraction(
                quote_ctx.quote_date,
                spread_maturity,
                DayCountContext::default(),
            )?;
            if t > 0.0 {
                let df = disc.df_between_dates(quote_ctx.quote_date, spread_maturity)?;
                if df > 0.0 && df.is_finite() {
                    return Ok(yield_rate - (-df.ln() / t));
                }
            }
        }

        // Bloomberg-style I-spread is measured against the interpolated market
        // swap quote. When the curve carries its original calibration quotes,
        // use those instead of re-deriving a par coupon from fitted DFs.
        let use_default_proxy = matches!(self.config.fixed_leg_day_count, DayCount::ActAct)
            && self.config.fixed_leg_frequency == Tenor::annual();
        if use_default_proxy {
            if let Some(par_swap_rate) =
                interpolated_swap_quote_rate(disc.as_ref(), quote_ctx.quote_date, spread_maturity)?
            {
                return Ok(yield_rate - par_swap_rate);
            }
        }

        // Build proxy fixed-leg schedule using configured frequency and standard
        // business-day / stub rules when market quote metadata is unavailable.
        let mut fixed_leg_day_count = self.config.fixed_leg_day_count;
        let mut fixed_leg_frequency = self.config.fixed_leg_frequency;
        if matches!(self.config.fixed_leg_day_count, DayCount::ActAct)
            && self.config.fixed_leg_frequency == Tenor::annual()
        {
            if let crate::instruments::fixed_income::bond::CashflowSpec::Fixed(spec) =
                &bond.cashflow_spec
            {
                fixed_leg_day_count = spec.dc;
                fixed_leg_frequency = spec.freq;
            }
        }
        let dates: Vec<Date> = finstack_quant_core::dates::ScheduleBuilder::new(
            quote_ctx.quote_date,
            spread_maturity,
        )?
        .frequency(fixed_leg_frequency)
        .stub_rule(StubKind::ShortFront)
        .build()?
        .into_iter()
        .collect();

        if dates.len() < 2 {
            return Err(finstack_quant_core::Error::Validation(
                "I-spread calculation requires at least two fixed-leg schedule dates".to_string(),
            ));
        }

        let (par_swap_rate, annuity) =
            crate::instruments::fixed_income::bond::pricing::quote_conversions::par_rate_and_annuity_from_discount(
                disc.as_ref(),
                fixed_leg_day_count,
                Some(fixed_leg_frequency),
                &dates,
            )?;
        if annuity.abs() < 1e-12 {
            return Err(finstack_quant_core::Error::Validation(
                "I-spread calculation is undefined for near-zero fixed-leg annuity".to_string(),
            ));
        }

        Ok(yield_rate - par_swap_rate)
    }
}

pub(crate) fn interpolated_swap_quote_rate(
    disc: &DiscountCurve,
    quote_date: Date,
    maturity: Date,
) -> finstack_quant_core::Result<Option<f64>> {
    let Some(calibration) = disc.rate_calibration() else {
        return Ok(None);
    };
    let mut swap_quotes = calibration
        .quotes
        .iter()
        .filter(|quote| matches!(quote.quote_type, DiscountCurveRateQuoteType::Swap))
        .filter_map(|quote| {
            Tenor::parse(&quote.tenor)
                .ok()
                .map(|tenor| (tenor.to_years_simple(), quote.rate))
        })
        .collect::<Vec<_>>();
    if swap_quotes.len() < 2 {
        return Ok(None);
    }
    swap_quotes.sort_by(|left, right| left.0.total_cmp(&right.0));

    let target = disc.day_count().year_fraction(
        quote_date,
        maturity,
        finstack_quant_core::dates::DayCountContext::default(),
    )?;
    if target <= swap_quotes[0].0 {
        return Ok(Some(swap_quotes[0].1));
    }
    for pair in swap_quotes.windows(2) {
        let (t0, r0) = pair[0];
        let (t1, r1) = pair[1];
        if target <= t1 {
            let weight = (target - t0) / (t1 - t0);
            return Ok(Some(r0 + weight * (r1 - r0)));
        }
    }
    Ok(swap_quotes.last().map(|(_, rate)| *rate))
}
