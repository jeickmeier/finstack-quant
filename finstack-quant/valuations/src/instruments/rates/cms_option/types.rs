//! CMS option instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::IRSConvention;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::rates::cms_common::CmsReferenceSwap;
use crate::instruments::OptionType;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// CMS option instrument (cap/floor on CMS rates).
#[derive(
    PartialEq,
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CmsOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Strike (fixed rate for CMS option)
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub strike: Decimal,
    /// Tenor of the CMS swap in years (e.g., 10.0 for 10Y)
    pub cms_tenor: f64,
    /// Observation/fixing dates for CMS rate
    #[serde(with = "finstack_quant_core::wire::dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub fixing_dates: Vec<Date>,
    /// Payment dates for each period (usually fixing date + lag or period end)
    #[serde(with = "finstack_quant_core::wire::dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub payment_dates: Vec<Date>,
    /// Accrual fractions for each period
    pub accrual_fractions: Vec<f64>,
    /// Option type (call or put on CMS rate)
    pub option_type: OptionType,
    /// Notional amount
    pub notional: Money,
    /// Day count convention for the option accrual
    pub day_count: DayCount,

    /// IRS convention for the underlying swap (e.g., `UsdSofr`).
    ///
    /// When set, provides default values for `swap_fixed_frequency`, `swap_float_frequency`,
    /// `swap_day_count`, and `swap_float_day_count`. Individual fields still
    /// override the convention when explicitly set.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_convention: Option<IRSConvention>,
    /// Fixed leg frequency of the underlying swap (overrides convention if set)
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_fixed_frequency: Option<Tenor>,
    /// Floating leg frequency of the underlying swap (overrides convention if set)
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_float_frequency: Option<Tenor>,
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
    #[builder(default)]
    /// Instrument-owned pricing inputs.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and grouping
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

impl CmsOption {
    /// Reference swap observed by this instrument's CMS fixings.
    pub fn reference_swap(&self) -> CmsReferenceSwap<'_> {
        CmsReferenceSwap {
            label: "CMS option",
            currency: self.notional.currency(),
            swap_convention: self.swap_convention,
            swap_fixed_frequency: self.swap_fixed_frequency,
            swap_float_frequency: self.swap_float_frequency,
            swap_day_count: self.swap_day_count,
            swap_float_day_count: self.swap_float_day_count,
            discount_curve_id: &self.discount_curve_id,
            forward_curve_id: &self.forward_curve_id,
        }
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
            .notional(Money::from((10_000_000_i64, Currency::USD)))
            .day_count(DayCount::Act365F)
            .swap_convention_opt(Some(IRSConvention::UsdSofr))
            .swap_float_day_count_opt(Some(DayCount::Act360))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"))
            .attributes(Attributes::new())
            .build()
            .expect("Example CmsOption construction should not fail")
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

    crate::impl_focused_pricing_overrides!();
}

// Declare canonical market dependencies for the DV01 calculator.
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
            .notional(Money::from((1_000_000_i64, Currency::USD)))
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
