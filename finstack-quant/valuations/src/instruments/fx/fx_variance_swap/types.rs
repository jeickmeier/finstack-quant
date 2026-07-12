//! FX variance swap type definitions and pricing logic.

use super::pricer;
use crate::cashflow::traits::CashflowProvider;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::traits::CurveDependencies;
use crate::instruments::common_impl::traits::Instrument as InstrumentTrait;
use crate::instruments::common_impl::traits::InstrumentCurves;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::stats::RealizedVarMethod;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Result;

pub use crate::instruments::common_impl::parameters::PayReceive;

fn default_observation_bdc() -> BusinessDayConvention {
    BusinessDayConvention::Following
}

/// FX variance swap instrument.
///
/// Payoff: Notional * (Realized Variance - Strike Variance)
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct FxVarianceSwap {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Base currency (foreign)
    pub base_currency: Currency,
    /// Quote currency (domestic)
    pub quote_currency: Currency,
    /// Optional spot identifier used to look up historical series.
    #[builder(optional)]
    pub spot_id: Option<String>,
    /// Variance notional (in quote currency units)
    pub notional: Money,
    /// Strike variance (annualized)
    pub strike_variance: f64,
    /// Start date of observation period
    #[schemars(with = "String")]
    pub start_date: Date,
    /// Contractual end of the observation period.
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Optional cash-settlement date. Defaults to the adjusted final observation date.
    #[serde(default)]
    #[builder(optional)]
    #[schemars(with = "Option<String>")]
    pub settlement_date: Option<Date>,
    /// Observation frequency
    pub observation_freq: Tenor,
    /// Base-currency calendar used in the joint observation calendar.
    pub base_calendar_id: String,
    /// Quote-currency calendar used in the joint observation calendar.
    pub quote_calendar_id: String,
    /// Business-day convention applied to observation dates.
    #[serde(default = "default_observation_bdc")]
    #[builder(default = BusinessDayConvention::Following)]
    pub observation_bdc: BusinessDayConvention,
    /// Preserve month-end rolls for month/year observation frequencies.
    #[serde(default)]
    #[builder(default)]
    pub observation_end_of_month: bool,
    /// Method for calculating realized variance (defaults to CloseToClose)
    #[serde(default)]
    #[builder(default)]
    pub realized_var_method: RealizedVarMethod,
    /// Series ID for open prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
    /// Defaults to `spot_id` (or currency-pair string) when absent.
    #[serde(default)]
    #[builder(optional)]
    pub open_series_id: Option<String>,
    /// Series ID for high prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
    /// Defaults to `spot_id` (or currency-pair string) when absent.
    #[serde(default)]
    #[builder(optional)]
    pub high_series_id: Option<String>,
    /// Series ID for low prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
    /// Defaults to `spot_id` (or currency-pair string) when absent.
    #[serde(default)]
    #[builder(optional)]
    pub low_series_id: Option<String>,
    /// Series ID for close prices. Defaults to `spot_id` (or currency-pair string) when absent.
    #[serde(default)]
    #[builder(optional)]
    pub close_series_id: Option<String>,
    /// Pay/receive variance
    pub side: PayReceive,
    /// Domestic currency discount curve ID
    pub domestic_discount_curve_id: CurveId,
    /// Foreign currency discount curve ID
    pub foreign_discount_curve_id: CurveId,
    /// FX volatility surface ID
    pub vol_surface_id: CurveId,
    /// Day count convention for time calculations
    pub day_count: DayCount,
    /// Attributes for scenario selection
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl FxVarianceSwap {
    /// Validate static contract invariants shared by builders, JSON, and direct
    /// instrument pricing.
    pub fn validate(&self) -> Result<()> {
        if self.base_currency == self.quote_currency {
            return Err(finstack_quant_core::Error::Validation(
                "FxVarianceSwap base and quote currencies must differ".to_string(),
            ));
        }
        if self.notional.currency() != self.quote_currency
            || !self.notional.amount().is_finite()
            || self.notional.amount() <= 0.0
        {
            return Err(finstack_quant_core::Error::Validation(
                "FxVarianceSwap notional must be positive, finite, and in quote currency"
                    .to_string(),
            ));
        }
        if !self.strike_variance.is_finite() || self.strike_variance < 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "FxVarianceSwap strike_variance must be finite and non-negative".to_string(),
            ));
        }
        if self.start_date >= self.maturity {
            return Err(finstack_quant_core::Error::Validation(
                "FxVarianceSwap start_date must precede maturity".to_string(),
            ));
        }
        if self.observation_freq.count() == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "FxVarianceSwap observation frequency must be positive".to_string(),
            ));
        }
        Ok(())
    }

    /// Create a canonical example FX variance swap (EUR/USD, 1Y).
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        use time::Month;
        FxVarianceSwap::builder()
            .id(InstrumentId::new("FXVAR-EURUSD-1Y"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .spot_id("EURUSD".to_string())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .strike_variance(0.04)
            .start_date(
                Date::from_calendar_date(2024, Month::January, 2).expect("Valid example date"),
            )
            .maturity(
                Date::from_calendar_date(2025, Month::January, 2).expect("Valid example date"),
            )
            .observation_freq(Tenor::daily())
            .base_calendar_id("TARGET2".to_string())
            .quote_calendar_id("USNY".to_string())
            .observation_bdc(BusinessDayConvention::Following)
            .observation_end_of_month(false)
            .realized_var_method(RealizedVarMethod::CloseToClose)
            .side(PayReceive::Receive)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("Example FxVarianceSwap construction should not fail")
    }

    pub(crate) fn validate_as_of(&self, context: &MarketContext, as_of: Date) -> Result<()> {
        let dom = context.get_discount(self.domestic_discount_curve_id.as_str())?;
        let for_curve = context.get_discount(self.foreign_discount_curve_id.as_str())?;
        let dom_base = dom.base_date();
        let for_base = for_curve.base_date();
        if as_of < dom_base || as_of < for_base {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxVarianceSwap valuation as_of date ({}) precedes curve base date (dom {}, for {}).",
                as_of, dom_base, for_base
            )));
        }
        let final_observation = self.final_observation_date()?;
        let settlement = self.effective_settlement_date()?;
        if settlement < final_observation {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxVarianceSwap '{}' settlement date ({settlement}) precedes final observation ({final_observation})",
                self.id
            )));
        }
        Ok(())
    }

    /// Adjusted final observation/fixing date.
    pub fn final_observation_date(&self) -> Result<Date> {
        Ok(self
            .observation_dates()?
            .last()
            .copied()
            .unwrap_or(self.maturity))
    }

    /// Cash-settlement date, defaulting to the final adjusted observation.
    pub fn effective_settlement_date(&self) -> Result<Date> {
        Ok(self
            .settlement_date
            .unwrap_or(self.final_observation_date()?))
    }

    pub(crate) fn series_id(&self) -> String {
        if let Some(id) = &self.spot_id {
            id.clone()
        } else {
            format!("{}{}", self.base_currency, self.quote_currency)
        }
    }

    pub(crate) fn spot_rate(&self, context: &MarketContext, as_of: Date) -> Result<f64> {
        if let Some(fx) = context.fx() {
            let rate = fx
                .rate(FxQuery::new(self.base_currency, self.quote_currency, as_of))?
                .rate;
            return Ok(rate);
        }
        let spot_id = self.series_id();
        let scalar = context.get_price(&spot_id).map_err(|_| {
            finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                id: spot_id,
            })
        })?;
        Ok(crate::metrics::scalar_numeric_value(scalar))
    }

    /// Calculate payoff given realized variance.
    pub fn payoff(&self, realized_variance: f64) -> Money {
        let variance_diff = realized_variance - self.strike_variance;
        Money::new(
            self.notional.amount() * variance_diff * self.side.sign(),
            self.notional.currency(),
        )
    }

    /// Get observation dates based on frequency.
    ///
    /// # Weekday-Aware Daily Observations
    ///
    /// For day-tenor observations, dates advance in business-day steps. The
    /// calendar is read from `attributes.meta["observation_calendar_id"]`
    /// (or `calendar_id`) and defaults to weekends-only. Non-business start
    /// and maturity dates adjust following and preceding, respectively.
    /// This is consistent with:
    /// - Market data availability (FX spot rates published on weekdays)
    /// - Annualization factor of 252 (trading days per year)
    ///
    /// For other frequencies (weekly, monthly), contractual calendar dates are included and
    /// the caller should ensure alignment with market data.
    pub fn observation_dates(&self) -> Result<Vec<Date>> {
        pricer::observation_dates(self)
    }

    /// Calculate annualization factor based on observation frequency.
    ///
    /// # Daily Observations
    ///
    /// For daily observations, returns 252 (standard trading days per year).
    /// This is consistent with `observation_dates()` which skips weekends.
    ///
    /// # Other Frequencies
    ///
    /// | Frequency | Factor |
    /// |-----------|--------|
    /// | Monthly   | 12     |
    /// | Quarterly | 4      |
    /// | Semi-annual | 2    |
    /// | Annual    | 1      |
    /// | Weekly    | 52     |
    /// | Bi-weekly | 26     |
    pub fn annualization_factor(&self) -> f64 {
        pricer::annualization_factor(self)
    }

    /// Calculate realized fraction based on observation counts.
    pub fn realized_fraction_by_observations(&self, as_of: Date) -> Result<f64> {
        pricer::realized_fraction_by_observations(self, as_of)
    }

    /// Fraction of the observation period elapsed at `as_of`, measured by the
    /// instrument's day-count convention.
    ///
    /// This is the seasoning weight the pricer (`pricer::compute_pv`) uses to
    /// blend already-annualized realized and forward variance. Risk metrics
    /// (vega, variance vega, expected variance) must use the *same* weight as
    /// the booked PV; an observation-count fraction only coincides for
    /// perfectly uniform schedules and drifts for weekend-skipping daily
    /// schedules near maturity.
    pub fn time_elapsed_fraction(&self, as_of: Date) -> Result<f64> {
        pricer::time_elapsed_fraction(self, as_of)
    }

    /// Get historical prices aligned to observation dates when available.
    pub fn get_historical_prices(&self, context: &MarketContext, as_of: Date) -> Result<Vec<f64>> {
        pricer::get_historical_prices(self, context, as_of)
    }

    /// Calculate partial realized variance for the elapsed period.
    pub fn partial_realized_variance(&self, context: &MarketContext, as_of: Date) -> Result<f64> {
        pricer::partial_realized_variance(self, context, as_of)
    }

    /// Calculate implied forward variance for the remaining period.
    pub fn remaining_forward_variance(&self, context: &MarketContext, as_of: Date) -> Result<f64> {
        pricer::remaining_forward_variance(self, context, as_of)
    }
}

impl InstrumentTrait for FxVarianceSwap {
    impl_instrument_base!(crate::pricer::InstrumentType::FxVarianceSwap);

    fn validate_invariants(&self) -> Result<()> {
        self.validate()
    }

    fn expiry(&self) -> Option<Date> {
        self.effective_settlement_date().ok()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        if let Some(spot_id) = self.spot_id.as_deref() {
            deps.add_spot_id(spot_id);
        }
        if self.realized_var_method.requires_ohlc() {
            for series_id in [
                self.open_series_id.as_deref(),
                self.high_series_id.as_deref(),
                self.low_series_id.as_deref(),
                self.close_series_id.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                deps.add_series_id(series_id);
            }
        } else {
            deps.add_series_id(
                self.close_series_id
                    .clone()
                    .unwrap_or_else(|| self.series_id()),
            );
        }
        deps.add_vol_surface_id(self.vol_surface_id.as_str());
        deps.add_fx_pair(self.base_currency, self.quote_currency);
        Ok(deps)
    }

    fn base_value(&self, context: &MarketContext, as_of: Date) -> Result<Money> {
        pricer::compute_pv(self, context, as_of)
    }
}

// FxVarianceSwap uses both domestic and foreign curves for forward construction
impl CurveDependencies for FxVarianceSwap {
    fn curve_dependencies(&self) -> finstack_quant_core::Result<InstrumentCurves> {
        InstrumentCurves::builder()
            .discount(self.domestic_discount_curve_id.clone())
            .discount(self.foreign_discount_curve_id.clone())
            .build()
    }
}

impl CashflowProvider for FxVarianceSwap {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        _context: &MarketContext,
        _as_of: Date,
    ) -> Result<crate::cashflow::builder::CashFlowSchedule> {
        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            Vec::new(),
            self.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional(),
                representation: crate::cashflow::builder::CashflowRepresentation::Placeholder,
                ..Default::default()
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::TenorUnit;
    use time::Month;

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid test date")
    }

    #[test]
    fn test_fx_variance_swap_curve_dependencies_includes_both_curves() {
        let swap = FxVarianceSwap::example();
        let deps = swap.curve_dependencies().expect("curve_dependencies");

        // Should include both domestic and foreign discount curves
        assert_eq!(
            deps.discount_curves.len(),
            2,
            "FxVarianceSwap should depend on both domestic and foreign curves"
        );
        assert!(
            deps.discount_curves.iter().any(|c| c.as_str() == "USD-OIS"),
            "Should include domestic curve"
        );
        assert!(
            deps.discount_curves.iter().any(|c| c.as_str() == "EUR-OIS"),
            "Should include foreign curve"
        );
    }

    #[test]
    fn test_fx_variance_swap_daily_observations_skip_weekends() {
        // Create a swap with daily observations over 1 week
        // Monday 2025-01-06 to Friday 2025-01-10 = 5 weekdays
        let swap = FxVarianceSwap::builder()
            .id(InstrumentId::new("TEST-VARSWAP"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .notional(Money::new(100_000.0, Currency::USD))
            .strike_variance(0.01)
            .start_date(date(2025, Month::January, 6)) // Monday
            .maturity(date(2025, Month::January, 10)) // Friday
            .observation_freq(Tenor::new(1, TenorUnit::Days))
            .base_calendar_id("TARGET2".to_string())
            .quote_calendar_id("USNY".to_string())
            .realized_var_method(RealizedVarMethod::CloseToClose)
            .side(PayReceive::Receive)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        let dates = swap.observation_dates().expect("observation schedule");

        // Should be exactly 5 weekdays (Mon-Fri)
        assert_eq!(
            dates.len(),
            5,
            "Should have 5 weekday observations: {:?}",
            dates
        );

        // Verify no weekends
        for d in &dates {
            assert!(
                d.weekday() != time::Weekday::Saturday && d.weekday() != time::Weekday::Sunday,
                "Should not include weekend: {:?}",
                d
            );
        }
    }

    #[test]
    fn test_fx_variance_swap_annualization_consistency() {
        // Create a swap with daily observations
        let swap = FxVarianceSwap::builder()
            .id(InstrumentId::new("TEST-VARSWAP"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .notional(Money::new(100_000.0, Currency::USD))
            .strike_variance(0.01)
            .start_date(date(2025, Month::January, 2))
            .maturity(date(2025, Month::December, 31))
            .observation_freq(Tenor::new(1, TenorUnit::Days))
            .base_calendar_id("TARGET2".to_string())
            .quote_calendar_id("USNY".to_string())
            .realized_var_method(RealizedVarMethod::CloseToClose)
            .side(PayReceive::Receive)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        let dates = swap.observation_dates().expect("observation schedule");
        let annualization = swap.annualization_factor();

        // Daily observations should use 252 annualization
        assert_eq!(annualization, 252.0);

        // A joint TARGET2/USNY schedule is smaller than the 252-day
        // annualization convention because either market's holidays are excluded.
        assert!(
            dates.len() >= 240 && dates.len() <= 252,
            "Daily joint-calendar observations should be plausible: got {}",
            dates.len()
        );
    }

    #[test]
    fn test_fx_variance_swap_weekly_observations_include_all_dates() {
        // Weekly observations should NOT skip weekends (week boundaries may fall on any day)
        let swap = FxVarianceSwap::builder()
            .id(InstrumentId::new("TEST-VARSWAP"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .notional(Money::new(100_000.0, Currency::USD))
            .strike_variance(0.01)
            .start_date(date(2025, Month::January, 4)) // Saturday
            .maturity(date(2025, Month::January, 25)) // Saturday
            .observation_freq(Tenor::new(1, TenorUnit::Weeks))
            .base_calendar_id("TARGET2".to_string())
            .quote_calendar_id("USNY".to_string())
            .realized_var_method(RealizedVarMethod::CloseToClose)
            .side(PayReceive::Receive)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        let dates = swap.observation_dates().expect("observation schedule");
        let annualization = swap.annualization_factor();

        // Weekly should use 52 annualization
        assert_eq!(annualization, 52.0);

        // Weekly observations: Jan 4, 11, 18, 25 = 4 dates
        assert_eq!(dates.len(), 4, "Weekly over 3 weeks should have 4 dates");
    }

    #[test]
    fn test_fx_variance_swap_realized_fraction_monotonic() {
        let swap = FxVarianceSwap::example();

        let start_frac = swap
            .realized_fraction_by_observations(swap.start_date)
            .expect("start fraction");
        let mid_date = swap.start_date + time::Duration::days(90);
        let mid_frac = swap
            .realized_fraction_by_observations(mid_date)
            .expect("mid fraction");
        let end_frac = swap
            .realized_fraction_by_observations(swap.maturity)
            .expect("end fraction");

        assert_eq!(start_frac, 0.0, "Should be 0 at start");
        assert!(
            mid_frac > 0.0 && mid_frac < 1.0,
            "Should be between 0 and 1 mid-way"
        );
        assert_eq!(end_frac, 1.0, "Should be 1 at maturity");
        assert!(
            mid_frac > start_frac && end_frac > mid_frac,
            "Realized fraction should be monotonically increasing"
        );
    }
}
