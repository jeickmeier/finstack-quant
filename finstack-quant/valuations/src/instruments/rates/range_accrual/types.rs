//! Range accrual instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::QuantoSpec;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::instruments::PricingOverrides;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId, Rate};

/// Specifies how the range bounds are interpreted.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BoundsType {
    /// Bounds are absolute price levels (e.g., 4500.0 for SPX).
    /// This is the market standard for rate-linked range accruals.
    #[default]
    Absolute,
    /// Bounds are relative to initial spot (e.g., 0.95 = 95% of initial).
    /// Common for equity-linked range accruals.
    RelativeToInitialSpot,
}

impl std::fmt::Display for BoundsType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BoundsType::Absolute => write!(f, "absolute"),
            BoundsType::RelativeToInitialSpot => write!(f, "relative_to_initial_spot"),
        }
    }
}

impl std::str::FromStr for BoundsType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "absolute" | "abs" => Ok(Self::Absolute),
            "relative_to_initial_spot" | "relative" | "pct" => Ok(Self::RelativeToInitialSpot),
            other => Err(format!(
                "Unknown bounds type: '{}'. Valid: absolute, relative_to_initial_spot",
                other
            )),
        }
    }
}

/// Range accrual instrument.
///
/// Range accrual notes pay coupons that accrue only when a reference rate or asset
/// stays within a specified range. The accrual is proportional to the number of
/// observation dates where the underlying is within [lower_bound, upper_bound].
///
/// # Bounds Interpretation
///
/// The `bounds_type` field controls how `lower_bound` and `upper_bound` are interpreted:
/// - `Absolute`: Bounds are absolute price levels (e.g., 4500.0 for SPX at 4700)
/// - `RelativeToInitialSpot`: Bounds are multipliers of the initial spot (e.g., 0.95 = 95%)
///
/// # Historical Fixings
///
/// For mid-life valuations, use `past_fixings_in_range` to specify how many past
/// observations were in range. The pricer will add this to expected future fixings.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RangeAccrual {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Underlying asset ticker symbol
    pub underlying_ticker: String,
    /// Observation dates for range checking (must be sorted ascending)
    #[schemars(with = "Vec<String>")]
    pub observation_dates: Vec<Date>,
    /// Lower bound of accrual range (interpretation depends on bounds_type)
    pub lower_bound: f64,
    /// Upper bound of accrual range (must be > lower_bound)
    pub upper_bound: f64,
    /// How to interpret the range bounds (default: Absolute)
    #[builder(default)]
    #[serde(default)]
    pub bounds_type: BoundsType,
    /// Coupon rate earned when in range (must be >= 0)
    pub coupon_rate: f64,
    /// Notional amount
    pub notional: Money,
    /// Day count convention
    pub day_count: finstack_quant_core::dates::DayCount,
    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Spot price identifier
    pub spot_id: PriceId,
    /// Volatility surface ID
    pub vol_surface_id: CurveId,
    /// Optional dividend yield curve ID
    pub div_yield_id: Option<CurveId>,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
    /// Optional quanto adjustment parameters. When provided, applies a drift
    /// correction for instruments whose payoff currency differs from the
    /// underlying asset currency.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quanto: Option<QuantoSpec>,
    /// Optional payment date (defaults to last observation date)
    #[schemars(with = "Option<String>")]
    pub payment_date: Option<Date>,
    /// Number of past observations that were in range (for mid-life valuations).
    /// If None, past observations are not included in the accrual calculation.
    pub past_fixings_in_range: Option<usize>,
    /// Total number of past observations (for mid-life valuations).
    /// Must be provided if `past_fixings_in_range` is set.
    pub total_past_observations: Option<usize>,
}

impl RangeAccrual {
    /// Create a canonical example range accrual (monthly observations).
    ///
    /// This example uses relative bounds (95%-105% of initial spot) which is
    /// typical for equity-linked range accruals.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::DayCount;
        use time::Month;
        let observation_dates = vec![
            Date::from_calendar_date(2024, Month::January, 31).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::February, 29).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::March, 31).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::April, 30).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::May, 31).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::June, 30).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::July, 31).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::August, 31).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::September, 30).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::October, 31).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::November, 30).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::December, 31).expect("Valid example date"),
        ];
        RangeAccrual::builder()
            .id(InstrumentId::new("RANGE-SPX-1Y"))
            .underlying_ticker("SPX".to_string())
            .observation_dates(observation_dates)
            .lower_bound(0.95) // 95% of initial spot
            .upper_bound(1.05) // 105% of initial spot
            .bounds_type(BoundsType::RelativeToInitialSpot)
            .coupon_rate(0.08) // 8% annual if inside range
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .payment_date_opt(None)
            .past_fixings_in_range_opt(None)
            .total_past_observations_opt(None)
            .build()
            .expect("Example RangeAccrual construction should not fail")
    }

    /// Create an example with absolute bounds (typical for rate-linked range accruals).
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example_absolute_bounds() -> Self {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::DayCount;
        use time::Month;
        let observation_dates = vec![
            Date::from_calendar_date(2024, Month::January, 31).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::February, 29).expect("Valid example date"),
            Date::from_calendar_date(2024, Month::March, 31).expect("Valid example date"),
        ];
        RangeAccrual::builder()
            .id(InstrumentId::new("RANGE-SOFR-3M"))
            .underlying_ticker("SOFR".to_string())
            .observation_dates(observation_dates)
            .lower_bound(0.04) // 4% lower bound
            .upper_bound(0.06) // 6% upper bound
            .bounds_type(BoundsType::Absolute)
            .coupon_rate(0.05) // 5% annual if inside range
            .notional(Money::new(1_000_000.0, Currency::USD))
            .day_count(DayCount::Act360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SOFR-RATE".into())
            .vol_surface_id(CurveId::new("SOFR-VOL"))
            .div_yield_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .payment_date_opt(None)
            .past_fixings_in_range_opt(None)
            .total_past_observations_opt(None)
            .build()
            .expect("Example RangeAccrual construction should not fail")
    }

    /// Validate the range accrual parameters.
    ///
    /// Checks:
    /// - At least one observation date exists
    /// - Observation dates are sorted in ascending order
    /// - Lower bound is strictly less than upper bound
    /// - Coupon rate is non-negative
    /// - Quanto fields are consistent (if correlation is set, fx_vol_surface must be set)
    /// - Past fixing fields are consistent
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        // Check observation dates
        validation::require_with(!self.observation_dates.is_empty(), || {
            "RangeAccrual requires at least one observation date".to_string()
        })?;

        // Check observation dates are sorted
        validation::validate_sorted_strict(
            &self.observation_dates,
            "RangeAccrual observation_dates",
        )?;

        // Check bound ordering
        validation::require_with(self.lower_bound < self.upper_bound, || {
            format!(
                "RangeAccrual lower_bound ({}) must be strictly less than upper_bound ({})",
                self.lower_bound, self.upper_bound
            )
        })?;

        // Check coupon rate
        validation::require_with(self.coupon_rate >= 0.0, || {
            format!(
                "RangeAccrual coupon_rate ({}) must be non-negative",
                self.coupon_rate
            )
        })?;

        // Check past fixing field consistency
        match (self.past_fixings_in_range, self.total_past_observations) {
            (Some(in_range), Some(total)) => {
                if in_range > total {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "RangeAccrual past_fixings_in_range ({}) cannot exceed total_past_observations ({})",
                        in_range, total
                    )));
                }
            }
            (Some(_), None) => {
                return Err(finstack_quant_core::Error::Validation(
                    "RangeAccrual past_fixings_in_range requires total_past_observations to be set"
                        .to_string(),
                ));
            }
            (None, Some(_)) => {
                return Err(finstack_quant_core::Error::Validation(
                    "RangeAccrual total_past_observations requires past_fixings_in_range to be set"
                        .to_string(),
                ));
            }
            (None, None) => {} // Both unset is valid
        }

        Ok(())
    }

    /// Get the effective lower bound for a given initial spot.
    ///
    /// For `Absolute` bounds, returns the bound as-is.
    /// For `RelativeToInitialSpot`, returns `initial_spot * lower_bound`.
    pub fn effective_lower_bound(&self, initial_spot: f64) -> f64 {
        match self.bounds_type {
            BoundsType::Absolute => self.lower_bound,
            BoundsType::RelativeToInitialSpot => initial_spot * self.lower_bound,
        }
    }

    /// Get the effective upper bound for a given initial spot.
    ///
    /// For `Absolute` bounds, returns the bound as-is.
    /// For `RelativeToInitialSpot`, returns `initial_spot * upper_bound`.
    pub fn effective_upper_bound(&self, initial_spot: f64) -> f64 {
        match self.bounds_type {
            BoundsType::Absolute => self.upper_bound,
            BoundsType::RelativeToInitialSpot => initial_spot * self.upper_bound,
        }
    }
}

impl RangeAccrualBuilder {
    /// Set the coupon rate using a typed rate.
    pub fn coupon_rate_rate(mut self, rate: Rate) -> Self {
        self.coupon_rate = Some(rate.as_decimal());
        self
    }
}

impl crate::instruments::common_impl::traits::Instrument for RangeAccrual {
    impl_instrument_base!(crate::pricer::InstrumentType::RangeAccrual);

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::StaticReplication
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        crate::instruments::common_impl::dependencies::MarketDependencies::from_curves_and_equity(
            self,
        )
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        self.validate()?;
        crate::instruments::rates::range_accrual::pricer::compute_pv(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        self.observation_dates.first().copied()
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

// Implement CurveDependencies for DV01 calculator
impl crate::instruments::common_impl::traits::CurveDependencies for RangeAccrual {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
    }
}

impl crate::instruments::common_impl::traits::EquityDependencies for RangeAccrual {
    fn equity_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::EquityInstrumentDeps>
    {
        crate::instruments::common_impl::traits::EquityInstrumentDeps::builder()
            .spot(self.spot_id.as_str())
            .vol_surface(self.vol_surface_id.as_str())
            .build()
    }
}

impl crate::metrics::HasPricingOverrides for RangeAccrual {
    fn pricing_overrides_mut(&mut self) -> &mut crate::instruments::PricingOverrides {
        &mut self.pricing_overrides
    }
}

impl crate::metrics::HasExpiry for RangeAccrual {
    fn expiry(&self) -> finstack_quant_core::dates::Date {
        self.observation_dates
            .last()
            .copied()
            .or(self.payment_date)
            .unwrap_or(Date::MIN)
    }
}

impl crate::metrics::HasDayCount for RangeAccrual {
    fn day_count(&self) -> finstack_quant_core::dates::DayCount {
        self.day_count
    }
}

crate::impl_empty_cashflow_provider!(
    RangeAccrual,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);
