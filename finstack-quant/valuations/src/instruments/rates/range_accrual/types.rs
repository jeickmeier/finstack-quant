//! Range accrual instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::QuantoSpec;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, IndexId, InstrumentId, PriceId, Rate};

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
    /// Bounds are absolute levels (e.g., 4500.0 for SPX, or an absolute rate
    /// band for a rate-linked note priced via
    /// [`CallableRangeAccrual`](crate::instruments::rates::callable_range_accrual)).
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
///
/// # Rate-Linked Underlyings: Pricing Routing
///
/// The rate-linked fields (`rate_index_id`, `projection_curve_id`,
/// `reference_tenor`) describe the contract but are **not priceable by the
/// standalone `RangeAccrual` pricers**, which support equity/FX (GBM)
/// underlyings only and return a validation error when these fields are set.
/// Price rate-linked range accrual notes through
/// [`CallableRangeAccrual`](crate::instruments::rates::callable_range_accrual)
/// (with an empty call schedule for a non-callable note), which models the
/// reference rate under HW1F and reconstructs the term rate per observation.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    finstack_quant_valuations_macros::FocusedPricingOverrides,
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
    /// Contractual accrual-period start date.
    ///
    /// When omitted, the legacy representation infers the start by stepping
    /// one first-observation interval backward. Single-observation contracts
    /// must provide this field explicitly.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub accrual_start_date: Option<Date>,
    /// Explicit rate index for rate-linked range accruals.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_index_id: Option<IndexId>,
    /// Projection curve for a rate-linked range accrual.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection_curve_id: Option<CurveId>,
    /// Contractual tenor of the observed reference rate.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_tenor: Option<finstack_quant_core::dates::Tenor>,
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
    /// Instrument-owned pricing inputs.
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(default)]
    #[builder(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(default)]
    #[builder(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
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
    /// Return the contractual accrual factor applied to the annual coupon.
    pub fn accrual_year_fraction(&self) -> finstack_quant_core::Result<f64> {
        let accrual_end = self.observation_dates.last().copied().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "RangeAccrual requires at least one observation date".to_string(),
            )
        })?;
        let accrual_start = if let Some(start) = self.accrual_start_date {
            start
        } else if self.observation_dates.len() >= 2 {
            let first = self.observation_dates[0];
            let second = self.observation_dates[1];
            first - (second - first)
        } else {
            return Err(finstack_quant_core::Error::Validation(
                "RangeAccrual with a single observation requires accrual_start_date".to_string(),
            ));
        };
        if accrual_start >= accrual_end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RangeAccrual accrual_start_date ({accrual_start}) must precede final observation ({accrual_end})"
            )));
        }
        self.day_count.year_fraction(
            accrual_start,
            accrual_end,
            finstack_quant_core::dates::DayCountContext::default(),
        )
    }

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

        validation::validate_f64_finite(self.lower_bound, "RangeAccrual lower_bound")?;
        validation::validate_f64_finite(self.upper_bound, "RangeAccrual upper_bound")?;
        validation::validate_f64_finite(self.coupon_rate, "RangeAccrual coupon_rate")?;
        validation::validate_money_finite(self.notional, "RangeAccrual notional")?;
        validation::require_with(self.notional.amount() > 0.0, || {
            format!(
                "RangeAccrual notional must be positive, got {}",
                self.notional.amount()
            )
        })?;

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

        if let (Some(payment_date), Some(last_observation)) =
            (self.payment_date, self.observation_dates.last().copied())
        {
            validation::require_with(payment_date >= last_observation, || {
                format!(
                    "RangeAccrual payment_date ({payment_date}) must be on or after the final observation date ({last_observation})"
                )
            })?;
        }
        let accrual_factor = self.accrual_year_fraction()?;
        validation::require_with(accrual_factor.is_finite() && accrual_factor > 0.0, || {
            format!("RangeAccrual accrual factor must be finite and positive, got {accrual_factor}")
        })?;
        let rate_field_count = usize::from(self.rate_index_id.is_some())
            + usize::from(self.projection_curve_id.is_some())
            + usize::from(self.reference_tenor.is_some());
        validation::require_with(rate_field_count == 0 || rate_field_count == 3, || {
            "RangeAccrual rate_index_id, projection_curve_id, and reference_tenor must be supplied together"
                .to_string()
        })?;

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

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        RangeAccrual::validate(self)
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::StaticReplication
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if let Some(projection_curve) = &self.projection_curve_id {
            deps.add_forward_curve(projection_curve.clone());
        }
        deps.add_spot_id(self.spot_id.as_str());
        for strike in [self.lower_bound, self.upper_bound] {
            deps.add_volatility_dependency(
                crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                    self.vol_surface_id.clone(),
                    Some(self.spot_id.clone()),
                    Some(strike),
                ),
            );
        }
        if let Some(dividend_yield) = &self.div_yield_id {
            deps.add_spot_id(dividend_yield.as_str());
        }
        Ok(deps)
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

    crate::impl_focused_pricing_overrides!();
}

impl crate::metrics::HasExpiry for RangeAccrual {
    fn expiry(&self) -> finstack_quant_core::dates::Date {
        self.payment_date
            .or_else(|| self.observation_dates.last().copied())
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

#[cfg(test)]
mod audit_regression_tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn accrual_factor_uses_explicit_contractual_period() {
        let mut range = RangeAccrual::example();
        range.day_count = finstack_quant_core::dates::DayCount::Act360;
        range.accrual_start_date = Some(date!(2024 - 01 - 01));
        range.observation_dates = vec![date!(2024 - 01 - 31), date!(2024 - 04 - 01)];
        range.payment_date = Some(date!(2024 - 04 - 03));
        let factor = range.accrual_year_fraction().expect("accrual factor");
        assert!((factor - 91.0 / 360.0).abs() < 1e-12);
        range.validate().expect("valid contractual period");
    }

    #[test]
    fn rate_contract_fields_are_all_or_none() {
        let mut range = RangeAccrual::example();
        range.rate_index_id = Some(IndexId::new("SOFR"));
        let err = range.validate().expect_err("partial rate spec must fail");
        assert!(err.to_string().contains("must be supplied together"));
    }

    #[test]
    fn payment_cannot_precede_final_observation() {
        let mut range = RangeAccrual::example();
        range.payment_date = Some(date!(2024 - 06 - 01));
        let err = range.validate().expect_err("early payment must fail");
        assert!(err.to_string().contains("final observation"));
    }

    #[test]
    fn canonical_dependencies_keep_both_range_strikes() {
        let range = RangeAccrual::example();
        let deps =
            crate::instruments::Instrument::market_dependencies(&range).expect("dependencies");

        assert_eq!(deps.volatility_dependencies.len(), 2);
        assert_eq!(
            deps.volatility_dependencies
                .iter()
                .map(|dependency| dependency.reference_strike)
                .collect::<Vec<_>>(),
            vec![Some(range.lower_bound), Some(range.upper_bound)]
        );
        assert_eq!(deps.unique_vol_surface_ids(), vec![range.vol_surface_id]);
        let mut expected_spots = vec![range.spot_id.as_str().to_string()];
        expected_spots.extend(range.div_yield_id.iter().map(|id| id.as_str().to_string()));
        assert_eq!(deps.spot_ids, expected_spots);
        assert!(deps.series_ids.is_empty());
    }
}
