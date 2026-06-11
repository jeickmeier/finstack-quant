//! Commodity Asian option instrument definition.
//!
//! Asian options on commodity forward prices, the dominant option type in
//! commodity markets. The average is typically computed over commodity
//! forward/futures prices for specific delivery periods.
//!
//! # Key Differences from Equity Asian Options
//!
//! - Uses **forward prices** from a price curve for each fixing date, not spot
//! - No dividend yield parameter (cost of carry is embedded in the forward curve)
//! - Seasoned options combine realized fixings with projected forwards
//!
//! # References
//!
//! - Kemna, A. G. Z., & Vorst, A. C. F. (1990). "A Pricing Method for Options
//!   Based on Average Asset Values."
//! - Turnbull, S. M., & Wakeman, L. M. (1991). "A Quick Algorithm for Pricing
//!   European Average Options."

use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::CommodityUnderlyingParams;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::exotics::asian_option::AveragingMethod;
use crate::instruments::OptionType;
use crate::instruments::PricingOverrides;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};

/// Commodity Asian option: option on the arithmetic or geometric average of
/// commodity prices.
///
/// This is the dominant option type in commodity markets. The average is
/// typically computed over commodity forward/futures prices for specific
/// delivery periods.
///
/// # Pricing Models
///
/// | Averaging | Model | Accuracy |
/// |-----------|-------|----------|
/// | Geometric | Kemna-Vorst (1990) with forwards | Exact closed-form |
/// | Arithmetic | Turnbull-Wakeman (1991) with forwards | ~1% vs Monte Carlo |
///
/// # Forward-Based Averaging
///
/// For each future fixing date `t_i`, the forward price `F(t_i)` is read from
/// the price curve. The average forward is:
/// ```text
/// F_avg = (Σ_realized + Σ F(t_i)) / n
/// ```
/// where the sum includes both realized fixings and projected forwards.
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct CommodityAsianOption {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Commodity underlying parameters (type, ticker, unit, currency).
    #[serde(flatten)]
    pub underlying: CommodityUnderlyingParams,
    /// Strike price per unit.
    pub strike: f64,
    /// Option type (call or put).
    pub option_type: OptionType,
    /// Averaging method (arithmetic or geometric).
    pub averaging_method: AveragingMethod,
    /// Dates on which the commodity price is observed for averaging.
    ///
    /// **Note**: These dates should be pre-adjusted for business day conventions.
    #[schemars(with = "Vec<String>")]
    pub fixing_dates: Vec<Date>,
    /// Already observed fixings for seasoned options (ex-date, price pairs).
    #[builder(default)]
    #[serde(default)]
    #[schemars(with = "Vec<(String, f64)>")]
    pub realized_fixings: Vec<(Date, f64)>,
    /// Contract quantity in commodity units.
    pub quantity: f64,
    /// Option expiry/settlement date for the payoff.
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Forward/futures price curve ID.
    pub forward_curve_id: CurveId,
    /// Discount curve ID for present value calculations.
    pub discount_curve_id: CurveId,
    /// Volatility surface ID for implied vol.
    pub vol_surface_id: CurveId,
    /// Day count convention.
    #[serde(default = "crate::serde_defaults::day_count_act365f")]
    #[builder(default = DayCount::Act365F)]
    pub day_count: DayCount,
    /// Pricing overrides.
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection and grouping.
    #[builder(default)]
    #[serde(default)]
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl CommodityAsianOption {
    /// Create a canonical example commodity Asian option for testing.
    ///
    /// Returns a WTI arithmetic average call option with monthly fixings.
    #[allow(clippy::expect_used)]
    pub fn example() -> Self {
        use time::macros::date;
        let fixing_dates = vec![
            date!(2025 - 01 - 31),
            date!(2025 - 02 - 28),
            date!(2025 - 03 - 31),
            date!(2025 - 04 - 30),
            date!(2025 - 05 - 31),
            date!(2025 - 06 - 30),
        ];
        Self::builder()
            .id(InstrumentId::new("WTI-ASIAN-6M"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .strike(75.0)
            .option_type(OptionType::Call)
            .averaging_method(AveragingMethod::Arithmetic)
            .fixing_dates(fixing_dates)
            .quantity(1000.0)
            .expiry(date!(2025 - 07 - 02))
            .forward_curve_id(CurveId::new("CL-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("CL-VOL"))
            .day_count(DayCount::Act365F)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("Example CommodityAsianOption with valid constants should never fail")
    }

    /// Validate the realized-fixing history against the fixing schedule.
    ///
    /// Errors (review finding M17) when:
    /// - `realized_fixings` contains duplicate dates (each would be
    ///   double-counted by [`accumulated_state`](Self::accumulated_state)), or
    /// - any scheduled `fixing_date <= as_of` has no realized fixing — a
    ///   missing past fixing silently deflates `hist_sum` and inflates the
    ///   seasoned effective strike.
    pub fn validate_realized_fixings(&self, as_of: Date) -> finstack_core::Result<()> {
        let mut seen: std::collections::BTreeSet<Date> = std::collections::BTreeSet::new();
        for (d, _) in &self.realized_fixings {
            if !seen.insert(*d) {
                return Err(finstack_core::Error::Validation(format!(
                    "CommodityAsianOption '{}' has duplicate realized fixing for date {d}",
                    self.id
                )));
            }
            if !self.fixing_dates.contains(d) {
                return Err(finstack_core::Error::Validation(format!(
                    "CommodityAsianOption '{}' has a realized fixing for {d}, which is \
                     not a scheduled fixing date (date mismatch)",
                    self.id
                )));
            }
        }

        for fixing_date in &self.fixing_dates {
            if *fixing_date <= as_of && !seen.contains(fixing_date) {
                return Err(finstack_core::Error::Validation(format!(
                    "CommodityAsianOption '{}' is missing a realized fixing for past \
                     fixing date {fixing_date} (as_of {as_of}); every fixing date on or \
                     before the valuation date must have a realized value",
                    self.id
                )));
            }
        }

        Ok(())
    }

    /// Get the accumulated state (sum, log_sum, count) from realized fixings.
    ///
    /// Only considers fixings that match dates in `fixing_dates` and are on or
    /// before `as_of`.
    ///
    /// # Non-positive fixings
    ///
    /// Non-positive fixings are included in the arithmetic sum but signal
    /// an undefined geometric average by setting `product_log` to `NEG_INFINITY`.
    pub fn accumulated_state(&self, as_of: Date) -> (f64, f64, usize) {
        let mut sum = 0.0;
        let mut product_log = 0.0;
        let mut count = 0;
        let mut has_non_positive = false;

        for (d, v) in &self.realized_fixings {
            if *d <= as_of && self.fixing_dates.contains(d) {
                sum += v;
                if *v > 0.0 {
                    product_log += v.ln();
                } else {
                    has_non_positive = true;
                }
                count += 1;
            }
        }

        if has_non_positive {
            product_log = f64::NEG_INFINITY;
        }

        (sum, product_log, count)
    }

    /// Compute the average forward price for remaining (future) fixing dates.
    ///
    /// Returns `(sum_of_forwards, count_of_future_fixings)`.
    pub fn future_forwards(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<(f64, usize)> {
        let price_curve = market.get_price_curve(self.forward_curve_id.as_str())?;
        let mut sum = 0.0;
        let mut count = 0;

        for &fixing_date in &self.fixing_dates {
            if fixing_date > as_of {
                let fwd = price_curve.price_on_date(fixing_date)?;
                sum += fwd;
                count += 1;
            }
        }
        Ok((sum, count))
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for CommodityAsianOption {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .forward(self.forward_curve_id.clone())
            .build()
    }
}

impl crate::instruments::common_impl::traits::Instrument for CommodityAsianOption {
    impl_instrument_base!(crate::pricer::InstrumentType::CommodityAsianOption);

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::AsianTurnbullWakeman
    }

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        deps.add_vol_surface_id(self.vol_surface_id.as_str());
        Ok(deps)
    }

    fn base_value(&self, market: &MarketContext, as_of: Date) -> finstack_core::Result<Money> {
        use crate::instruments::commodity::commodity_asian_option::pricer;
        pricer::compute_pv(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        self.fixing_dates.first().copied()
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.expiry)
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

crate::impl_empty_cashflow_provider!(
    CommodityAsianOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::Date;
    use time::Month;

    #[test]
    fn test_accumulated_state() {
        let fixings = vec![
            Date::from_calendar_date(2025, Month::January, 31).expect("valid date"),
            Date::from_calendar_date(2025, Month::February, 28).expect("valid date"),
            Date::from_calendar_date(2025, Month::March, 31).expect("valid date"),
        ];

        let mut asian = CommodityAsianOption::example();
        asian.fixing_dates = fixings.clone();

        // No history
        let (sum, _log_prod, count) = asian.accumulated_state(
            Date::from_calendar_date(2025, Month::April, 1).expect("valid date"),
        );
        assert_eq!(sum, 0.0);
        assert_eq!(count, 0);

        // Add history
        asian.realized_fixings = vec![(fixings[0], 72.0), (fixings[1], 74.0)];

        // Check at date between Feb and Mar
        let as_of = Date::from_calendar_date(2025, Month::March, 15).expect("valid date");
        let (sum, log_prod, count) = asian.accumulated_state(as_of);

        assert_eq!(sum, 146.0);
        assert_eq!(count, 2);
        assert!((log_prod - (72.0f64.ln() + 74.0f64.ln())).abs() < 1e-10);
    }

    #[test]
    fn test_example_construction() {
        let asian = CommodityAsianOption::example();
        assert_eq!(asian.fixing_dates.len(), 6);
        assert_eq!(asian.strike, 75.0);
        assert_eq!(asian.quantity, 1000.0);
    }
}
