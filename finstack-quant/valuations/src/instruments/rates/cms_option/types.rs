//! CMS option instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::IRSConvention;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::OptionType;
use crate::instruments::PricingOverrides;
use finstack_quant_core::dates::{CalendarRegistry, Date, DateExt, DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// CMS option instrument (cap/floor on CMS rates).
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = CmsOption::validate)]
#[serde(deny_unknown_fields)]
pub struct CmsOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Strike (fixed rate for CMS option)
    pub strike: Decimal,
    /// Tenor of the CMS swap in years (e.g., 10.0 for 10Y)
    pub cms_tenor: f64,
    /// Observation/fixing dates for CMS rate
    #[schemars(with = "Vec<String>")]
    pub fixing_dates: Vec<Date>,
    /// Payment dates for each period (usually fixing date + lag or period end)
    #[schemars(with = "Vec<String>")]
    pub payment_dates: Vec<Date>,
    /// Accrual fractions for each period
    pub accrual_fractions: Vec<f64>,
    /// Option type (call or put on CMS rate)
    pub option_type: OptionType,
    /// Notional amount
    pub notional: Money,
    /// Day count convention for the option accrual
    pub day_count: DayCount,

    // --- Underlying Swap Conventions ---
    /// IRS convention for the underlying swap (e.g., `USDStandard`).
    ///
    /// When set, provides default values for `swap_fixed_freq`, `swap_float_freq`,
    /// `swap_day_count`, and `swap_float_day_count`. Individual fields still
    /// override the convention when explicitly set.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_convention: Option<IRSConvention>,
    /// Fixed leg frequency of the underlying swap (overrides convention if set)
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_fixed_freq: Option<Tenor>,
    /// Floating leg frequency of the underlying swap (overrides convention if set)
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_float_freq: Option<Tenor>,
    /// Day count convention of the underlying swap fixed leg (overrides convention if set)
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_day_count: Option<DayCount>,
    /// Optional day count convention of the underlying swap floating leg
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_float_day_count: Option<DayCount>,

    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Forward/projection curve ID for CMS rate projection
    pub forward_curve_id: CurveId,
    /// Volatility surface ID for CMS rates
    pub vol_surface_id: CurveId,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection and grouping
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

impl CmsOption {
    /// Effective date of the reference swap observed on `fixing_date`.
    pub(crate) fn reference_swap_start(
        &self,
        fixing_date: Date,
    ) -> finstack_quant_core::Result<Date> {
        let convention = match (self.swap_convention, self.notional.currency()) {
            (Some(convention), _) => convention,
            (None, finstack_quant_core::currency::Currency::USD) => IRSConvention::USDStandard,
            (None, finstack_quant_core::currency::Currency::EUR) => IRSConvention::EURStandard,
            (None, finstack_quant_core::currency::Currency::GBP) => IRSConvention::GBPStandard,
            (None, finstack_quant_core::currency::Currency::JPY) => IRSConvention::JPYStandard,
            (None, currency) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CMS option '{}' requires swap_convention for currency {}",
                    self.id, currency
                )))
            }
        };
        let calendar_id = convention.calendar_id().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "CMS option '{}' convention has no reference calendar",
                self.id
            ))
        })?;
        let calendar = CalendarRegistry::global()
            .resolve_str(&calendar_id)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "CMS option '{}' reference calendar '{}' is not registered",
                    self.id, calendar_id
                ))
            })?;
        fixing_date.add_business_days(convention.reset_lag_days(), calendar)
    }

    /// Validate CMS option schedule vectors.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if self.fixing_dates.len() != self.payment_dates.len()
            || self.fixing_dates.len() != self.accrual_fractions.len()
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CMS option vectors must have equal length: fixing_dates={}, payment_dates={}, accrual_fractions={}",
                self.fixing_dates.len(),
                self.payment_dates.len(),
                self.accrual_fractions.len(),
            )));
        }
        Ok(())
    }

    pub(crate) fn strike_f64(&self) -> finstack_quant_core::Result<f64> {
        self.strike
            .to_f64()
            .ok_or(finstack_quant_core::InputError::ConversionOverflow.into())
    }

    /// Override the fixed leg frequency of the underlying swap.
    pub fn with_swap_fixed_freq(mut self, freq: Tenor) -> Self {
        self.swap_fixed_freq = Some(freq);
        self
    }

    /// Override the floating leg frequency of the underlying swap.
    pub fn with_swap_float_freq(mut self, freq: Tenor) -> Self {
        self.swap_float_freq = Some(freq);
        self
    }

    /// Override the fixed leg day count of the underlying swap.
    pub fn with_swap_day_count(mut self, dc: DayCount) -> Self {
        self.swap_day_count = Some(dc);
        self
    }

    /// Override the floating leg day count of the underlying swap.
    pub fn with_swap_float_day_count(mut self, dc: DayCount) -> Self {
        self.swap_float_day_count = Some(dc);
        self
    }

    /// Resolved fixed leg frequency (explicit field > convention > default semi-annual).
    pub fn resolved_swap_fixed_freq(&self) -> Tenor {
        self.swap_fixed_freq
            .or_else(|| self.swap_convention.map(|c| c.fixed_frequency()))
            .unwrap_or_else(Tenor::semi_annual)
    }

    /// Resolved float leg frequency (explicit field > convention > default quarterly).
    pub fn resolved_swap_float_freq(&self) -> Tenor {
        self.swap_float_freq
            .or_else(|| self.swap_convention.map(|c| c.float_frequency()))
            .unwrap_or_else(Tenor::quarterly)
    }

    /// Resolved fixed leg day count (explicit field > convention > default 30/360).
    pub fn resolved_swap_day_count(&self) -> DayCount {
        self.swap_day_count
            .or_else(|| self.swap_convention.map(|c| c.fixed_day_count()))
            .unwrap_or(DayCount::Thirty360)
    }

    /// Resolved float leg day count (explicit field > convention float > ACT/360).
    pub fn resolved_swap_float_day_count(&self) -> DayCount {
        self.swap_float_day_count
            .or_else(|| self.swap_convention.map(|c| c.float_day_count()))
            .unwrap_or(DayCount::Act360)
    }

    /// Create a CMS option from a schedule specification.
    ///
    /// Generates fixing and payment dates from `start_date`, `maturity`, and `frequency`
    /// using the calendar and reset lag from `swap_convention`.
    /// This is the preferred way to construct standard CMS cap/floor instruments.
    ///
    /// Swap convention fields (`swap_fixed_freq`, `swap_float_freq`, `swap_day_count`)
    /// are provided via `swap_convention`. Set individual fields to override the convention.
    ///
    /// # Errors
    ///
    /// Returns an error if the generated schedule is empty (e.g. `maturity <= start_date`).
    #[allow(clippy::too_many_arguments)]
    pub fn from_schedule(
        id: impl Into<InstrumentId>,
        start_date: Date,
        maturity: Date,
        frequency: Tenor,
        cms_tenor: f64,
        strike: Decimal,
        option_type: OptionType,
        notional: finstack_quant_core::money::Money,
        day_count: finstack_quant_core::dates::DayCount,
        swap_convention: IRSConvention,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        use crate::cashflow::builder::periods::{build_periods, BuildPeriodsParams};
        use finstack_quant_core::dates::{BusinessDayConvention, StubKind};

        let calendar_id = swap_convention.calendar_id().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "CMS option convention has no reference calendar".to_string(),
            )
        })?;
        let reset_lag_days = swap_convention.reset_lag_days();

        let periods = build_periods(BuildPeriodsParams {
            start: start_date,
            end: maturity,
            frequency,
            stub: StubKind::ShortFront,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: &calendar_id,
            end_of_month: false,
            day_count,
            payment_lag_days: 0,
            reset_lag_days: Some(reset_lag_days),
            adjust_accrual_dates: false,
        })?;

        if periods.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid,
            ));
        }

        let fixing_dates: Vec<Date> = periods
            .iter()
            .map(|p| p.reset_date.unwrap_or(p.accrual_start))
            .collect();
        let payment_dates: Vec<Date> = periods.iter().map(|p| p.payment_date).collect();
        let accrual_fractions: Vec<f64> = periods.iter().map(|p| p.accrual_year_fraction).collect();

        CmsOption::builder()
            .id(id.into())
            .strike(strike)
            .cms_tenor(cms_tenor)
            .fixing_dates(fixing_dates)
            .payment_dates(payment_dates)
            .accrual_fractions(accrual_fractions)
            .option_type(option_type)
            .notional(notional)
            .day_count(day_count)
            .swap_convention_opt(Some(swap_convention))
            .discount_curve_id(discount_curve_id.into())
            .forward_curve_id(forward_curve_id.into())
            .vol_surface_id(vol_surface_id.into())
            .build()
            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))
    }

    /// Create a canonical example CMS option (10Y CMS caplet style).
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        use finstack_quant_core::currency::Currency;
        use time::Month;

        let fixing_dates = vec![
            Date::from_calendar_date(2025, Month::March, 20).expect("Valid example date"),
            Date::from_calendar_date(2025, Month::June, 20).expect("Valid example date"),
            Date::from_calendar_date(2025, Month::September, 22).expect("Valid example date"),
            Date::from_calendar_date(2025, Month::December, 22).expect("Valid example date"),
        ];
        let payment_dates = vec![
            Date::from_calendar_date(2025, Month::June, 20).expect("Valid example date"),
            Date::from_calendar_date(2025, Month::September, 22).expect("Valid example date"),
            Date::from_calendar_date(2025, Month::December, 22).expect("Valid example date"),
            Date::from_calendar_date(2026, Month::March, 20).expect("Valid example date"),
        ];
        let accrual_fractions = vec![0.25, 0.25, 0.25, 0.25];

        CmsOption::builder()
            .id(InstrumentId::new("CMSOPT-10Y-USD"))
            .strike(Decimal::try_from(0.025).expect("valid decimal"))
            .cms_tenor(10.0)
            .fixing_dates(fixing_dates)
            .payment_dates(payment_dates)
            .accrual_fractions(accrual_fractions)
            .option_type(crate::instruments::OptionType::Call)
            .notional(Money::new(10_000_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .swap_convention_opt(Some(IRSConvention::USDStandard))
            .swap_float_day_count_opt(Some(DayCount::Act360))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("Example CmsOption construction should not fail")
    }
}

impl CmsOptionBuilder {
    /// Set the strike using a typed rate.
    pub fn strike_rate(mut self, rate: Rate) -> Self {
        self.strike = Decimal::try_from(rate.as_decimal()).ok();
        self
    }
}

impl crate::instruments::common_impl::traits::Instrument for CmsOption {
    impl_instrument_base!(crate::pricer::InstrumentType::CmsOption);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Black76
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_forward_curve(self.forward_curve_id.clone());
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                None,
                Some(self.strike_f64()?),
            ),
        );
        deps.add_series_id(
            finstack_quant_core::market_data::fixings::cms_fixing_series_id(
                self.forward_curve_id.as_str(),
                self.cms_tenor,
            ),
        );
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        self.validate()?;
        crate::instruments::rates::cms_option::pricer::compute_pv(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
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

// Implement CurveDependencies for DV01 calculator
impl crate::instruments::common_impl::traits::CurveDependencies for CmsOption {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        let mut builder = crate::instruments::common_impl::traits::InstrumentCurves::builder();
        builder = builder.discount(self.discount_curve_id.clone());
        builder = builder.forward(self.forward_curve_id.clone());
        builder.build()
    }
}

crate::impl_empty_cashflow_provider!(
    CmsOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod validation_tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use time::Month;

    fn test_date(month: Month, day: u8) -> Date {
        Date::from_calendar_date(2026, month, day).expect("valid date")
    }

    #[test]
    fn builder_rejects_misaligned_schedule_vectors() {
        let result = CmsOption::builder()
            .id(InstrumentId::new("CMSOPT-BAD"))
            .strike(Decimal::try_from(0.025).expect("valid decimal"))
            .cms_tenor(10.0)
            .fixing_dates(vec![
                test_date(Month::March, 20),
                test_date(Month::June, 20),
            ])
            .payment_dates(vec![test_date(Month::June, 20)])
            .accrual_fractions(vec![0.25, 0.25])
            .option_type(OptionType::Call)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"))
            .build();

        assert!(
            result.is_err(),
            "CMS option builder must reject schedule vector length mismatches"
        );
    }
}
