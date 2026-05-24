//! Asian option instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::OptionType;
use crate::instruments::PricingOverrides;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId, PriceId};

/// Averaging method for Asian options.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AveragingMethod {
    /// Arithmetic average: (1/n) Σ S_i
    Arithmetic,
    /// Geometric average: (Π S_i)^(1/n)
    Geometric,
}

impl std::fmt::Display for AveragingMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arithmetic => write!(f, "arithmetic"),
            Self::Geometric => write!(f, "geometric"),
        }
    }
}

impl std::str::FromStr for AveragingMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "arithmetic" => Ok(Self::Arithmetic),
            "geometric" => Ok(Self::Geometric),
            other => Err(format!(
                "Unknown averaging method: '{}'. Valid: arithmetic, geometric",
                other
            )),
        }
    }
}

/// Asian option instrument.
///
/// Asian options depend on the average price over a period rather than
/// just the terminal price. Supports both call and put options with
/// arithmetic or geometric averaging.
///
/// # Averaging Methods
///
/// - **Arithmetic**: `A = (1/n) × Σ S(t_i)` - Market standard, approximated via Turnbull-Wakeman
/// - **Geometric**: `G = [Π S(t_i)]^(1/n)` - Closed-form solution available (Kemna-Vorst)
///
/// # Fixing Dates and Business Day Conventions
///
/// **Important**: The `fixing_dates` field expects dates that have already been adjusted
/// for business day conventions. In production use, callers should:
///
/// 1. Generate the schedule of observation dates (e.g., monthly end-of-month)
/// 2. Apply the appropriate business day convention (typically Modified Following)
/// 3. Adjust for the relevant holiday calendar (based on underlying asset's market)
///
/// Common conventions by market:
/// - **US Equity (SPX)**: NYSE calendar, Modified Following
/// - **FX Options**: Joint calendar of currency pair, Modified Following
/// - **Commodities**: Exchange-specific calendar
///
/// The number of fixing dates directly affects the averaging calculation and pricing.
/// Typical configurations:
/// - Daily averaging: ~252 dates per year (trading days)
/// - Weekly averaging: ~52 dates per year
/// - Monthly averaging: 12 dates per year (typically month-end)
///
/// # Pricing Models
///
/// | Averaging | Model | Accuracy |
/// |-----------|-------|----------|
/// | Geometric | Kemna-Vorst (1990) | Exact closed-form |
/// | Arithmetic | Turnbull-Wakeman (1991) | ~1% vs Monte Carlo |
/// | Either | Monte Carlo | Configurable accuracy |
///
/// # Example
///
/// ```ignore
/// // For production: pre-adjust fixing dates using business day logic
/// let fixing_dates = generate_monthly_schedule(start, end)
///     .into_iter()
///     .map(|d| adjust_business_day(d, ModifiedFollowing, &nyse_calendar))
///     .collect();
/// ```
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct AsianOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Underlying asset ticker symbol
    pub underlying_ticker: String,
    /// Strike price
    pub strike: f64,
    /// Option type (call or put)
    pub option_type: OptionType,
    /// Averaging method (arithmetic or geometric)
    pub averaging_method: AveragingMethod,
    /// Option expiry date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Dates on which underlying is observed for averaging.
    ///
    /// **Note**: These dates should be pre-adjusted for business day conventions.
    /// The pricer uses these dates directly without further adjustment.
    /// See struct-level documentation for business day convention guidance.
    #[schemars(with = "Vec<String>")]
    pub fixing_dates: Vec<Date>,
    /// Notional amount
    pub notional: Money,
    /// Day count convention
    pub day_count: finstack_core::dates::DayCount,
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
    /// Past fixings for seasoned options (date, observed price pairs).
    ///
    /// For seasoned options where some averaging observations have already occurred,
    /// provide the historical fixings here. Only fixings that match dates in
    /// `fixing_dates` and are on or before the valuation date are considered.
    #[builder(default)]
    #[serde(default)]
    #[schemars(with = "Vec<(String, f64)>")]
    pub past_fixings: Vec<(Date, f64)>,
}

impl AsianOption {
    /// Create a canonical example Asian option (arithmetic average).
    pub fn example() -> finstack_core::Result<Self> {
        use finstack_core::currency::Currency;
        use finstack_core::dates::DayCount;
        use time::macros::date;
        let fixing_dates = vec![
            date!(2024 - 01 - 31),
            date!(2024 - 02 - 29),
            date!(2024 - 03 - 31),
            date!(2024 - 04 - 30),
            date!(2024 - 05 - 31),
            date!(2024 - 06 - 30),
        ];
        AsianOption::builder()
            .id(InstrumentId::new("ASIAN-SPX-ARITH-6M"))
            .underlying_ticker("SPX".to_string())
            .strike(4500.0)
            .option_type(crate::instruments::OptionType::Call)
            .averaging_method(AveragingMethod::Arithmetic)
            .expiry(date!(2024 - 06 - 30))
            .fixing_dates(fixing_dates)
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
    }

    /// Calculate the net present value of this Asian option using Monte Carlo.
    pub fn npv_mc(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        use crate::instruments::exotics::asian_option::pricer;
        pricer::compute_pv(self, curves, as_of)
    }

    /// Get the accumulated state (sum, log_sum_product, count) for seasoned options.
    /// Only considers fixings that are in the fixing schedule and before or on as_of.
    ///
    /// # Non-positive fixings
    ///
    /// Non-positive fixings (zero or negative prices) are included in the arithmetic
    /// sum and count but excluded from the geometric log-product, which would produce
    /// `-inf` or `NaN`. If any non-positive fixings are encountered, the geometric
    /// log-product is set to `NEG_INFINITY` to signal that the geometric average is
    /// undefined, rather than silently computing an incorrect partial product.
    pub fn accumulated_state(&self, as_of: Date) -> (f64, f64, usize) {
        let mut sum = 0.0;
        let mut product_log = 0.0;
        let mut count = 0;
        let mut has_non_positive = false;

        for (d, v) in &self.past_fixings {
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

        // If any fixing was non-positive, the geometric average is undefined.
        // Set product_log to NEG_INFINITY so callers using geometric averaging
        // get a clear signal rather than a silently wrong result.
        if has_non_positive {
            product_log = f64::NEG_INFINITY;
        }

        (sum, product_log, count)
    }
}

impl crate::instruments::common_impl::traits::Instrument for AsianOption {
    impl_instrument_base!(crate::pricer::InstrumentType::AsianOption);

    fn default_model(&self) -> crate::pricer::ModelKey {
        match self.averaging_method {
            AveragingMethod::Geometric => crate::pricer::ModelKey::AsianGeometricBS,
            AveragingMethod::Arithmetic => crate::pricer::ModelKey::AsianTurnbullWakeman,
        }
    }

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
        crate::instruments::common_impl::dependencies::MarketDependencies::from_curves_and_equity(
            self,
        )
    }

    fn base_value(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        use crate::instruments::exotics::asian_option::pricer::{
            AsianOptionAnalyticalGeometricPricer, AsianOptionSemiAnalyticalTwPricer,
        };
        use crate::pricer::Pricer;

        match self.averaging_method {
            AveragingMethod::Geometric => {
                let pricer = AsianOptionAnalyticalGeometricPricer::new();
                let result = pricer
                    .price_dyn(self, market, as_of)
                    .map_err(|e| finstack_core::Error::Validation(e.to_string()))?;
                Ok(result.value)
            }
            AveragingMethod::Arithmetic => {
                let pricer = AsianOptionSemiAnalyticalTwPricer::new();
                let result = pricer
                    .price_dyn(self, market, as_of)
                    .map_err(|e| finstack_core::Error::Validation(e.to_string()))?;
                Ok(result.value)
            }
        }
    }

    fn effective_start_date(&self) -> Option<finstack_core::dates::Date> {
        self.fixing_dates.first().copied()
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
    AsianOption,
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
            Date::from_calendar_date(2024, Month::January, 1).expect("valid date"),
            Date::from_calendar_date(2024, Month::February, 1).expect("valid date"),
            Date::from_calendar_date(2024, Month::March, 1).expect("valid date"),
        ];

        let mut asian = AsianOption::example().expect("AsianOption example is valid");
        asian.fixing_dates = fixings.clone();

        // No history
        let (sum, _log_prod, count) = asian.accumulated_state(
            Date::from_calendar_date(2024, Month::April, 1).expect("valid date"),
        );
        assert_eq!(sum, 0.0);
        assert_eq!(count, 0);

        // Add history
        asian.past_fixings = vec![(fixings[0], 100.0), (fixings[1], 105.0)];

        // Check at date between Feb and Mar
        let as_of = Date::from_calendar_date(2024, Month::February, 15).expect("valid date");
        let (sum, log_prod, count) = asian.accumulated_state(as_of);

        assert_eq!(sum, 205.0);
        assert_eq!(count, 2);
        assert!((log_prod - (100.0f64.ln() + 105.0f64.ln())).abs() < 1e-10);

        // Check at date before Feb
        let as_of_early = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
        let (sum_early, _, count_early) = asian.accumulated_state(as_of_early);
        assert_eq!(sum_early, 100.0);
        assert_eq!(count_early, 1);
    }

    #[test]
    fn averaging_method_fromstr_display_roundtrip() {
        use std::str::FromStr;
        let variants = [AveragingMethod::Arithmetic, AveragingMethod::Geometric];
        for v in variants {
            let s = v.to_string();
            let parsed = AveragingMethod::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        assert!(AveragingMethod::from_str("invalid").is_err());
    }
}
