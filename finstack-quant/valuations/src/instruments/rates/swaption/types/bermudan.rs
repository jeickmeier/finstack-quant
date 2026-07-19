//! Pricing and metric helpers for interest-rate instruments.
//!
use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::rates::irs::{FixedLegSpec, FloatLegSpec, InterestRateSwap, PayReceive};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use finstack_quant_core::{Error, Result};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use super::definitions::{
    BermudanSchedule, BermudanType, CashSettlementMethod, SwaptionExercise, SwaptionSettlement,
    VolatilityModel,
};
use super::swaption::{
    normalize_underlier, underlier_wire_schema, vanilla_underlier, LegacySwaptionUnderlier,
    Swaption, VanillaSwaptionUnderlier,
};

// ============================================================================
// Bermudan Swaption Instrument
// ============================================================================

/// Bermudan swaption with multiple exercise dates.
///
/// A Bermudan swaption gives the holder the right to enter into an interest rate
/// swap at any of a set of predetermined exercise dates. This is the most common
/// type of exotic swaption in the market, used extensively for:
///
/// - Callable bond hedging
/// - Mortgage prepayment risk management
/// - Structured product hedging
///
/// # Pricing Methods
///
/// Bermudan swaptions require numerical methods for pricing:
/// - **Hull-White Tree**: Industry standard, calibrated to swaption volatility
/// - **LSMC**: Longstaff-Schwartz Monte Carlo for validation
///
/// # Example
///
/// ```ignore
/// use finstack_quant_valuations::instruments::rates::swaption::{
///     BermudanSwaption, BermudanSchedule, BermudanType, SwaptionSettlement,
/// };
///
/// // Create a 10NC2 (10-year swap, callable after 2 years)
/// let swaption = BermudanSwaption::example();
/// ```
#[derive(Debug, Clone)]
pub struct BermudanSwaption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Option type (payer = Call, receiver = Put)
    pub option_type: OptionType,
    /// Notional amount of underlying swap
    pub notional: Money,
    /// Settlement method (physical or cash)
    pub settlement: SwaptionSettlement,
    /// Volatility surface ID for calibration
    pub vol_surface_id: CurveId,
    /// Bermudan exercise schedule
    pub bermudan_schedule: BermudanSchedule,
    /// Co-terminal or non-co-terminal exercise
    pub bermudan_type: BermudanType,
    /// Complete fixed leg of the underlying swap.
    pub underlying_fixed_leg: FixedLegSpec,
    /// Complete floating leg of the underlying swap.
    pub underlying_float_leg: FloatLegSpec,
    /// Instrument-owned pricing inputs.
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

#[derive(Clone, Debug, finstack_quant_valuations_macros::FocusedPricingOverrides)]
#[serde(deny_unknown_fields)]
struct BermudanSwaptionWire {
    id: InstrumentId,
    option_type: OptionType,
    notional: Money,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    strike: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    swap_start: Option<Date>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    swap_end: Option<Date>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fixed_freq: Option<Tenor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    float_freq: Option<Tenor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    day_count: Option<DayCount>,
    settlement: SwaptionSettlement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    discount_curve_id: Option<CurveId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    forward_curve_id: Option<CurveId>,
    vol_surface_id: CurveId,
    bermudan_schedule: BermudanSchedule,
    bermudan_type: BermudanType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calendar_id: Option<CalendarId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    underlying_fixed_leg: Option<FixedLegSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    underlying_float_leg: Option<FloatLegSpec>,
    instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    #[serde(default)]
    attributes: Attributes,
}

impl TryFrom<BermudanSwaptionWire> for BermudanSwaption {
    type Error = Error;

    fn try_from(wire: BermudanSwaptionWire) -> Result<Self> {
        let (underlying_fixed_leg, underlying_float_leg) = normalize_underlier(
            wire.underlying_fixed_leg,
            wire.underlying_float_leg,
            LegacySwaptionUnderlier {
                strike: wire.strike,
                swap_start: wire.swap_start,
                swap_end: wire.swap_end,
                fixed_freq: wire.fixed_freq,
                float_freq: wire.float_freq,
                day_count: wire.day_count,
                discount_curve_id: wire.discount_curve_id,
                forward_curve_id: wire.forward_curve_id,
                calendar_id: wire.calendar_id,
            },
        )?;
        let swaption = Self {
            id: wire.id,
            option_type: wire.option_type,
            notional: wire.notional,
            settlement: wire.settlement,
            vol_surface_id: wire.vol_surface_id,
            bermudan_schedule: wire.bermudan_schedule,
            bermudan_type: wire.bermudan_type,
            underlying_fixed_leg,
            underlying_float_leg,
            instrument_pricing_overrides: wire.instrument_pricing_overrides,
            metric_pricing_overrides: wire.metric_pricing_overrides,
            scenario_pricing_overrides: wire.scenario_pricing_overrides,
            attributes: wire.attributes,
        };
        swaption.validate()?;
        Ok(swaption)
    }
}

impl From<&BermudanSwaption> for BermudanSwaptionWire {
    fn from(value: &BermudanSwaption) -> Self {
        Self {
            id: value.id.clone(),
            option_type: value.option_type,
            notional: value.notional,
            strike: None,
            swap_start: None,
            swap_end: None,
            fixed_freq: None,
            float_freq: None,
            day_count: None,
            settlement: value.settlement,
            discount_curve_id: None,
            forward_curve_id: None,
            vol_surface_id: value.vol_surface_id.clone(),
            bermudan_schedule: value.bermudan_schedule.clone(),
            bermudan_type: value.bermudan_type,
            calendar_id: None,
            underlying_fixed_leg: Some(value.underlying_fixed_leg.clone()),
            underlying_float_leg: Some(value.underlying_float_leg.clone()),
            instrument_pricing_overrides: value.instrument_pricing_overrides.clone(),
            metric_pricing_overrides: value.metric_pricing_overrides.clone(),
            scenario_pricing_overrides: value.scenario_pricing_overrides.clone(),
            attributes: value.attributes.clone(),
        }
    }
}

impl serde::Serialize for BermudanSwaption {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde::Serialize::serialize(&BermudanSwaptionWire::from(self), serializer)
    }
}

impl<'de> serde::Deserialize<'de> for BermudanSwaption {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = <BermudanSwaptionWire as serde::Deserialize>::deserialize(deserializer)?;
        Self::try_from(wire).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for BermudanSwaption {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("BermudanSwaption")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        underlier_wire_schema(<BermudanSwaptionWire as schemars::JsonSchema>::json_schema(
            generator,
        ))
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use crate::instruments::rates::irs::FloatingLegCompounding;
    use finstack_quant_core::dates::{BusinessDayConvention, StubKind};
    use serde_json::Value;

    fn add_matching_legacy_underlier(value: &mut Value) {
        let fixed = value["underlying_fixed_leg"].clone();
        let float = value["underlying_float_leg"].clone();
        let object = value.as_object_mut().expect("Bermudan JSON object");
        object.insert("strike".to_string(), fixed["rate"].clone());
        object.insert("swap_start".to_string(), fixed["start"].clone());
        object.insert("swap_end".to_string(), fixed["end"].clone());
        object.insert("fixed_freq".to_string(), fixed["frequency"].clone());
        object.insert("float_freq".to_string(), float["frequency"].clone());
        object.insert("day_count".to_string(), fixed["day_count"].clone());
        object.insert(
            "discount_curve_id".to_string(),
            fixed["discount_curve_id"].clone(),
        );
        object.insert(
            "forward_curve_id".to_string(),
            float["forward_curve_id"].clone(),
        );
    }

    #[test]
    fn legacy_bermudan_input_serializes_as_canonical_legs() {
        let canonical =
            serde_json::to_value(BermudanSwaption::example()).expect("canonical Bermudan JSON");
        let mut legacy = canonical.clone();
        add_matching_legacy_underlier(&mut legacy);
        let object = legacy.as_object_mut().expect("legacy Bermudan JSON object");
        object.remove("underlying_fixed_leg");
        object.remove("underlying_float_leg");

        let normalized: BermudanSwaption =
            serde_json::from_value(legacy).expect("legacy Bermudan input");
        assert_eq!(
            serde_json::to_value(normalized).expect("canonical Bermudan output"),
            canonical
        );
    }

    #[test]
    fn mixed_bermudan_input_requires_complete_matching_representations() {
        let canonical =
            serde_json::to_value(BermudanSwaption::example()).expect("canonical Bermudan JSON");

        let mut mixed = canonical.clone();
        add_matching_legacy_underlier(&mut mixed);
        let normalized: BermudanSwaption =
            serde_json::from_value(mixed.clone()).expect("complete matching mixed input");
        assert_eq!(
            serde_json::to_value(normalized).expect("canonical Bermudan output"),
            canonical
        );

        let mut incomplete_legacy = mixed.clone();
        incomplete_legacy
            .as_object_mut()
            .expect("mixed object")
            .remove("float_freq");
        let incomplete_legacy_error = serde_json::from_value::<BermudanSwaption>(incomplete_legacy)
            .expect_err("mixed input requires every legacy scalar field")
            .to_string();
        assert!(incomplete_legacy_error.contains("requires `float_freq`"));

        let mut incomplete_legs = mixed.clone();
        incomplete_legs
            .as_object_mut()
            .expect("mixed object")
            .remove("underlying_float_leg");
        let incomplete_legs_error = serde_json::from_value::<BermudanSwaption>(incomplete_legs)
            .expect_err("mixed input requires both canonical legs")
            .to_string();
        assert!(incomplete_legs_error.contains("must provide both fixed and floating"));

        let mut conflicting = mixed;
        conflicting.as_object_mut().expect("mixed object").insert(
            "fixed_freq".to_string(),
            serde_json::to_value(Tenor::quarterly()).expect("quarterly tenor JSON"),
        );
        let conflict_error = serde_json::from_value::<BermudanSwaption>(conflicting)
            .expect_err("mixed input requires matching representations")
            .to_string();
        assert!(conflict_error.contains("fixed_freq conflicts"));
    }

    #[test]
    fn bermudan_schedule_uses_complete_fixed_leg_conventions() {
        let mut bermudan = BermudanSwaption::example();
        bermudan.underlying_fixed_leg.frequency = Tenor::annual();
        bermudan.underlying_fixed_leg.day_count = DayCount::Act365F;
        bermudan.underlying_fixed_leg.bdc = BusinessDayConvention::Preceding;
        bermudan.underlying_fixed_leg.calendar_id =
            Some(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID.to_string());
        bermudan.underlying_fixed_leg.stub = StubKind::ShortBack;
        bermudan.underlying_fixed_leg.payment_lag_days = 2;
        bermudan.underlying_fixed_leg.end_of_month = true;
        let fixed = bermudan.underlying_fixed_leg.clone();

        let actual = bermudan
            .fixed_schedule_periods()
            .expect("canonical fixed-leg schedule");
        let expected = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: fixed.start,
                end: fixed.end,
                frequency: fixed.frequency,
                stub: fixed.stub,
                bdc: fixed.bdc,
                calendar_id: fixed.calendar_id.as_deref().expect("calendar"),
                end_of_month: fixed.end_of_month,
                day_count: fixed.day_count,
                payment_lag_days: fixed.payment_lag_days,
                reset_lag_days: None,
                adjust_accrual_dates: false,
            },
        )
        .expect("direct fixed-leg schedule");

        let actual_dates = actual
            .iter()
            .map(|period| {
                (
                    period.accrual_start,
                    period.accrual_end,
                    period.payment_date,
                    period.accrual_year_fraction,
                )
            })
            .collect::<Vec<_>>();
        let expected_dates = expected
            .iter()
            .map(|period| {
                (
                    period.accrual_start,
                    period.accrual_end,
                    period.payment_date,
                    period.accrual_year_fraction,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(actual_dates, expected_dates);

        let (payment_dates, accruals) = bermudan
            .build_swap_schedule(fixed.start)
            .expect("public fixed-leg schedule");
        assert_eq!(
            payment_dates,
            expected
                .iter()
                .map(|period| period.payment_date)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            accruals,
            expected
                .iter()
                .map(|period| period.accrual_year_fraction)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn exercise_underlier_preserves_both_leg_conventions() {
        let mut bermudan = BermudanSwaption::example();
        bermudan.underlying_fixed_leg.frequency = Tenor::annual();
        bermudan.underlying_fixed_leg.day_count = DayCount::Act365F;
        bermudan.underlying_fixed_leg.bdc = BusinessDayConvention::Preceding;
        bermudan.underlying_fixed_leg.stub = StubKind::LongBack;
        bermudan.underlying_fixed_leg.payment_lag_days = 2;
        bermudan.underlying_fixed_leg.end_of_month = true;
        bermudan.underlying_float_leg.frequency = Tenor::semi_annual();
        bermudan.underlying_float_leg.day_count = DayCount::Act360;
        bermudan.underlying_float_leg.bdc = BusinessDayConvention::Following;
        bermudan.underlying_float_leg.stub = StubKind::ShortBack;
        bermudan.underlying_float_leg.reset_lag_days = 3;
        bermudan.underlying_float_leg.payment_lag_days = 4;
        bermudan.underlying_float_leg.end_of_month = true;
        bermudan.underlying_float_leg.compounding = FloatingLegCompounding::sofr();

        let exercise = bermudan.first_exercise().expect("first exercise");
        let underlier = bermudan
            .underlying_irs_at(exercise)
            .expect("canonical exercise underlier");
        let mut expected_fixed = bermudan.underlying_fixed_leg.clone();
        expected_fixed.start = exercise;
        expected_fixed.rate = Decimal::ZERO;
        let mut expected_float = bermudan.underlying_float_leg;
        expected_float.start = exercise;

        assert_eq!(
            serde_json::to_value(underlier.fixed).expect("fixed JSON"),
            serde_json::to_value(expected_fixed).expect("expected fixed JSON")
        );
        assert_eq!(
            serde_json::to_value(underlier.float).expect("float JSON"),
            serde_json::to_value(expected_float).expect("expected float JSON")
        );
    }

    #[test]
    fn to_european_preserves_complete_leg_conventions() {
        let mut bermudan = BermudanSwaption::example();
        bermudan.underlying_fixed_leg.frequency = Tenor::annual();
        bermudan.underlying_fixed_leg.day_count = DayCount::Act365F;
        bermudan.underlying_fixed_leg.bdc = BusinessDayConvention::Preceding;
        bermudan.underlying_fixed_leg.calendar_id = Some("nyse".to_string());
        bermudan.underlying_fixed_leg.stub = StubKind::ShortFront;
        bermudan.underlying_fixed_leg.compounding_simple = false;
        bermudan.underlying_fixed_leg.payment_lag_days = 2;
        bermudan.underlying_fixed_leg.end_of_month = true;

        bermudan.underlying_float_leg.frequency = Tenor::semi_annual();
        bermudan.underlying_float_leg.day_count = DayCount::Act360;
        bermudan.underlying_float_leg.bdc = BusinessDayConvention::Following;
        bermudan.underlying_float_leg.calendar_id = Some("target".to_string());
        bermudan.underlying_float_leg.fixing_calendar_id = Some("nyse".to_string());
        bermudan.underlying_float_leg.stub = StubKind::LongBack;
        bermudan.underlying_float_leg.reset_lag_days = 2;
        bermudan.underlying_float_leg.compounding = FloatingLegCompounding::sofr();
        bermudan.underlying_float_leg.payment_lag_days = 3;
        bermudan.underlying_float_leg.end_of_month = true;

        let first_exercise = bermudan.first_exercise().expect("first exercise");
        let mut expected_fixed = bermudan.underlying_fixed_leg.clone();
        expected_fixed.start = first_exercise;
        let mut expected_float = bermudan.underlying_float_leg.clone();
        expected_float.start = first_exercise;

        let european = bermudan.to_european().expect("European conversion");
        assert_eq!(european.expiry, first_exercise);
        assert_eq!(
            serde_json::to_value(european.underlying_fixed_leg).expect("fixed leg JSON"),
            serde_json::to_value(expected_fixed).expect("expected fixed leg JSON")
        );
        assert_eq!(
            serde_json::to_value(european.underlying_float_leg).expect("float leg JSON"),
            serde_json::to_value(expected_float).expect("expected float leg JSON")
        );
    }
}

impl BermudanSwaption {
    /// Validate the underlying swap and the complete exercise schedule.
    pub fn validate(&self) -> Result<()> {
        let context = format!("Bermudan swaption '{}'", self.id.as_str());
        if !self.notional.amount().is_finite() || self.notional.amount() <= 0.0 {
            return Err(Error::Validation(format!(
                "{context} notional must be positive and finite"
            )));
        }
        if self.vol_surface_id.as_str().trim().is_empty() {
            return Err(Error::Validation(format!(
                "{context} requires a non-empty vol_surface_id"
            )));
        }
        self.underlying_fixed_leg.validate()?;
        self.underlying_float_leg.validate()?;
        if self.underlying_fixed_leg.start != self.underlying_float_leg.start
            || self.underlying_fixed_leg.end != self.underlying_float_leg.end
        {
            return Err(Error::Validation(format!(
                "{context} underlying fixed and floating leg dates must match"
            )));
        }
        if self.underlying_fixed_leg.discount_curve_id
            != self.underlying_float_leg.discount_curve_id
        {
            return Err(Error::Validation(format!(
                "{context} underlying legs must use the same discount curve"
            )));
        }
        if self.bermudan_schedule.exercise_dates.is_empty() {
            return Err(Error::Validation(format!(
                "{context} requires at least one exercise date"
            )));
        }
        for dates in self.bermudan_schedule.exercise_dates.windows(2) {
            if dates[0] >= dates[1] {
                return Err(Error::Validation(format!(
                    "{context} exercise dates must be strictly increasing"
                )));
            }
        }
        let swap_start = self.get_swap_start();
        let swap_end = self.get_swap_end();
        if self
            .bermudan_schedule
            .exercise_dates
            .iter()
            .any(|date| *date < swap_start || *date >= swap_end)
        {
            return Err(Error::Validation(format!(
                "{context} exercise dates must lie in [{swap_start}, {swap_end})"
            )));
        }
        if self
            .bermudan_schedule
            .lockout_end
            .is_some_and(|lockout| lockout >= swap_end)
        {
            return Err(Error::Validation(format!(
                "{context} lockout_end must precede swap maturity"
            )));
        }
        if self.bermudan_schedule.effective_dates().is_empty() {
            return Err(Error::Validation(format!(
                "{context} lockout removes every exercise opportunity"
            )));
        }
        self.strike_f64()?;
        Ok(())
    }

    /// Create a canonical example Bermudan swaption for testing.
    ///
    /// Returns a 10NC2 payer swaption (10-year swap, callable quarterly after 2 years).
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        let swap_start =
            Date::from_calendar_date(2027, time::Month::January, 17).expect("Valid example date");
        let swap_end =
            Date::from_calendar_date(2037, time::Month::January, 17).expect("Valid example date");
        let first_exercise =
            Date::from_calendar_date(2029, time::Month::January, 17).expect("Valid example date");
        let strike = Decimal::try_from(0.03).expect("valid decimal");
        let (underlying_fixed_leg, underlying_float_leg) =
            vanilla_underlier(VanillaSwaptionUnderlier {
                strike,
                swap_start,
                swap_end,
                fixed_freq: Tenor::semi_annual(),
                float_freq: Tenor::quarterly(),
                day_count: DayCount::Thirty360,
                discount_curve_id: CurveId::new("USD-OIS"),
                forward_curve_id: CurveId::new("USD-OIS"),
                calendar_id: None,
            });

        Self {
            id: InstrumentId::new("BERM-10NC2-USD"),
            option_type: OptionType::Call,
            notional: Money::new(10_000_000.0, Currency::USD),
            settlement: SwaptionSettlement::Physical,
            vol_surface_id: CurveId::new("USD-SWPNVOL"),
            bermudan_schedule: BermudanSchedule::co_terminal(
                first_exercise,
                swap_end,
                Tenor::semi_annual(),
            )
            .expect("valid Bermudan schedule"),
            bermudan_type: BermudanType::CoTerminal,
            underlying_fixed_leg,
            underlying_float_leg,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        }
    }

    /// Create a new Bermudan payer swaption (right to pay fixed).
    ///
    /// Returns an error if the strike value is not representable as `Decimal` (e.g., NaN or Inf).
    #[allow(clippy::too_many_arguments)]
    pub fn new_payer(
        id: impl Into<InstrumentId>,
        notional: Money,
        strike: f64,
        swap_start: Date,
        swap_end: Date,
        bermudan_schedule: BermudanSchedule,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let strike = finstack_quant_core::decimal::f64_to_decimal(strike)?;
        let (underlying_fixed_leg, underlying_float_leg) =
            vanilla_underlier(VanillaSwaptionUnderlier {
                strike,
                swap_start,
                swap_end,
                fixed_freq: Tenor::semi_annual(),
                float_freq: Tenor::quarterly(),
                day_count: DayCount::Thirty360,
                discount_curve_id: discount_curve_id.into(),
                forward_curve_id: forward_curve_id.into(),
                calendar_id: None,
            });
        let swaption = Self {
            id: id.into(),
            option_type: OptionType::Call,
            notional,
            settlement: SwaptionSettlement::Physical,
            vol_surface_id: vol_surface_id.into(),
            bermudan_schedule,
            bermudan_type: BermudanType::CoTerminal,
            underlying_fixed_leg,
            underlying_float_leg,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::default(),
        };
        swaption.validate()?;
        Ok(swaption)
    }

    /// Create a new Bermudan receiver swaption (right to receive fixed).
    ///
    /// Returns an error if the strike value is not representable as `Decimal` (e.g., NaN or Inf).
    #[allow(clippy::too_many_arguments)]
    pub fn new_receiver(
        id: impl Into<InstrumentId>,
        notional: Money,
        strike: f64,
        swap_start: Date,
        swap_end: Date,
        bermudan_schedule: BermudanSchedule,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let strike = finstack_quant_core::decimal::f64_to_decimal(strike)?;
        let (underlying_fixed_leg, underlying_float_leg) =
            vanilla_underlier(VanillaSwaptionUnderlier {
                strike,
                swap_start,
                swap_end,
                fixed_freq: Tenor::semi_annual(),
                float_freq: Tenor::quarterly(),
                day_count: DayCount::Thirty360,
                discount_curve_id: discount_curve_id.into(),
                forward_curve_id: forward_curve_id.into(),
                calendar_id: None,
            });
        let swaption = Self {
            id: id.into(),
            option_type: OptionType::Put,
            notional,
            settlement: SwaptionSettlement::Physical,
            vol_surface_id: vol_surface_id.into(),
            bermudan_schedule,
            bermudan_type: BermudanType::CoTerminal,
            underlying_fixed_leg,
            underlying_float_leg,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::default(),
        };
        swaption.validate()?;
        Ok(swaption)
    }

    /// Fixed rate of the underlying swap.
    pub fn get_strike(&self) -> Decimal {
        self.underlying_fixed_leg.rate
    }

    /// Start date shared by both underlying legs.
    pub fn get_swap_start(&self) -> Date {
        self.underlying_fixed_leg.start
    }

    /// End date shared by both underlying legs.
    pub fn get_swap_end(&self) -> Date {
        self.underlying_fixed_leg.end
    }

    /// Fixed-leg payment frequency.
    pub fn get_fixed_freq(&self) -> Tenor {
        self.underlying_fixed_leg.frequency
    }

    /// Floating-leg payment frequency.
    pub fn get_float_freq(&self) -> Tenor {
        self.underlying_float_leg.frequency
    }

    /// Fixed-leg accrual convention.
    pub fn get_day_count(&self) -> DayCount {
        self.underlying_fixed_leg.day_count
    }

    /// Discount curve selected by the underlying legs.
    pub fn get_discount_curve_id(&self) -> &CurveId {
        &self.underlying_fixed_leg.discount_curve_id
    }

    /// Forward curve selected by the floating leg.
    pub fn get_forward_curve_id(&self) -> &CurveId {
        &self.underlying_float_leg.forward_curve_id
    }

    /// Schedule calendar selected by the fixed leg.
    pub fn get_calendar_id(&self) -> Option<&str> {
        self.underlying_fixed_leg.calendar_id.as_deref()
    }

    /// Set fixed leg frequency.
    pub fn with_fixed_freq(mut self, freq: Tenor) -> Self {
        self.underlying_fixed_leg.frequency = freq;
        self
    }

    /// Set floating leg frequency.
    pub fn with_float_freq(mut self, freq: Tenor) -> Self {
        self.underlying_float_leg.frequency = freq;
        self
    }

    /// Set day count convention.
    pub fn with_day_count(mut self, dc: DayCount) -> Self {
        self.underlying_fixed_leg.day_count = dc;
        self.underlying_float_leg.day_count = dc;
        self
    }

    /// Set settlement method.
    pub fn with_settlement(mut self, settlement: SwaptionSettlement) -> Self {
        self.settlement = settlement;
        self
    }

    /// Set Bermudan type (co-terminal or non-co-terminal).
    pub fn with_bermudan_type(mut self, bermudan_type: BermudanType) -> Self {
        self.bermudan_type = bermudan_type;
        self
    }

    /// Set the holiday calendar for schedule generation.
    pub fn with_calendar(mut self, calendar_id: impl Into<CalendarId>) -> Self {
        let calendar_id = calendar_id.into().to_string();
        self.underlying_fixed_leg.calendar_id = Some(calendar_id.clone());
        self.underlying_float_leg.calendar_id = Some(calendar_id.clone());
        self.underlying_float_leg.fixing_calendar_id = Some(calendar_id);
        self
    }

    /// Get the first exercise date.
    pub fn first_exercise(&self) -> Option<Date> {
        self.bermudan_schedule.effective_dates().first().copied()
    }

    /// Get the last exercise date.
    pub fn last_exercise(&self) -> Option<Date> {
        self.bermudan_schedule.effective_dates().last().copied()
    }

    /// Calculate time to first exercise in years.
    pub fn time_to_first_exercise(&self, as_of: Date) -> Result<f64> {
        match self.first_exercise() {
            Some(first) => {
                if as_of >= first {
                    return Ok(0.0);
                }
                self.get_day_count().year_fraction(
                    as_of,
                    first,
                    finstack_quant_core::dates::DayCountContext::default(),
                )
            }
            None => Err(Error::Validation("No exercise dates".into())),
        }
    }

    /// Calculate time to swap maturity in years.
    pub fn time_to_maturity(&self, as_of: Date) -> Result<f64> {
        if as_of >= self.get_swap_end() {
            return Ok(0.0);
        }
        self.get_day_count().year_fraction(
            as_of,
            self.get_swap_end(),
            finstack_quant_core::dates::DayCountContext::default(),
        )
    }

    /// Get exercise dates as year fractions from valuation date.
    pub fn exercise_times(&self, as_of: Date) -> Result<Vec<f64>> {
        let times = self
            .bermudan_schedule
            .exercise_times(as_of, self.get_day_count())?;
        if times.is_empty() {
            return Ok(times);
        }
        Ok(times)
    }

    /// Build the underlying swap payment schedule.
    ///
    /// Returns (payment_dates, accrual_fractions) for the fixed leg.
    pub fn build_swap_schedule(&self, _as_of: Date) -> Result<(Vec<Date>, Vec<f64>)> {
        let periods = self.fixed_schedule_periods()?;

        if periods.is_empty() {
            return Err(Error::Validation(
                "Swap schedule has fewer than 2 dates".into(),
            ));
        }

        let dates: Vec<Date> = periods.iter().map(|p| p.payment_date).collect();
        let accruals: Vec<f64> = periods.iter().map(|p| p.accrual_year_fraction).collect();

        Ok((dates, accruals))
    }

    /// Build the canonical fixed-leg periods used by Bermudan pricing paths.
    pub(crate) fn fixed_schedule_periods(
        &self,
    ) -> Result<Vec<crate::cashflow::builder::periods::SchedulePeriod>> {
        self.fixed_schedule_periods_at(self.get_swap_start())
    }

    fn fixed_schedule_periods_at(
        &self,
        start: Date,
    ) -> Result<Vec<crate::cashflow::builder::periods::SchedulePeriod>> {
        let underlier = self.underlying_irs_at(start)?;
        let fixed = underlier.resolved_fixed_leg()?;
        crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: fixed.start,
                end: fixed.end,
                frequency: fixed.frequency,
                stub: fixed.stub,
                bdc: fixed.bdc,
                calendar_id: fixed
                    .calendar_id
                    .as_deref()
                    .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
                end_of_month: fixed.end_of_month,
                day_count: fixed.day_count,
                payment_lag_days: fixed.payment_lag_days,
                reset_lag_days: None,
                adjust_accrual_dates: false,
            },
        )
    }

    fn underlying_irs_at(&self, start: Date) -> Result<InterestRateSwap> {
        let mut fixed = self.underlying_fixed_leg.clone();
        fixed.start = start;
        fixed.rate = Decimal::ZERO;
        let mut float = self.underlying_float_leg.clone();
        float.start = start;
        let underlier = InterestRateSwap::builder()
            .id(InstrumentId::new(format!("{}:UNDERLIER", self.id.as_str())))
            .notional(self.notional)
            .side(PayReceive::Pay)
            .fixed(fixed)
            .float(float)
            .build()?;
        underlier.validate()?;
        Ok(underlier)
    }

    /// Convert payment dates to year fractions.
    pub fn payment_times(&self, as_of: Date) -> Result<Vec<f64>> {
        let (dates, _) = self.build_swap_schedule(as_of)?;
        let ctx = finstack_quant_core::dates::DayCountContext::default();
        dates
            .iter()
            .map(|&d| self.get_day_count().year_fraction(as_of, d, ctx))
            .collect()
    }

    pub(crate) fn strike_f64(&self) -> Result<f64> {
        self.get_strike().to_f64().ok_or_else(|| {
            Error::Validation("BermudanSwaption strike could not be converted to f64".into())
        })
    }

    /// Forward swap rate at a given exercise date (multi-curve).
    ///
    /// For co-terminal swaptions, the swap always matures at `swap_end`.
    /// For non-co-terminal, each exercise date may have different remaining tenor.
    ///
    /// # Time Basis
    ///
    /// Uses curve-consistent time mapping:
    /// - Discount factors use the discount curve's own base_date/day_count
    /// - Forward rates use the forward curve's own base_date/day_count
    pub fn forward_swap_rate(
        &self,
        curves: &MarketContext,
        as_of: Date,
        exercise_date: Date,
    ) -> Result<f64> {
        let disc = curves.get_discount(self.get_discount_curve_id().as_ref())?;
        let annuity = self.remaining_annuity(disc.as_ref(), as_of, exercise_date)?;

        if annuity.abs() < 1e-10 {
            return Ok(0.0);
        }

        let underlier = self.underlying_irs_at(exercise_date)?;
        let pv_float =
            crate::instruments::rates::irs::pricer::compute_pv_raw(&underlier, curves, as_of)?;
        Ok(pv_float / (self.notional.amount() * annuity))
    }

    /// Calculate annuity for remaining swap payments after exercise date.
    ///
    /// # Time Basis
    ///
    /// Uses curve-consistent relative discount factors:
    /// - DF from `as_of` to each payment date computed using the discount curve's
    ///   own base_date and day_count.
    /// - Accrual fractions use the instrument's day_count (correct for coupon calculation).
    pub fn remaining_annuity(
        &self,
        disc: &dyn Discounting,
        as_of: Date,
        exercise_date: Date,
    ) -> Result<f64> {
        use crate::instruments::common_impl::pricing::time::relative_df_discounting;

        if exercise_date >= self.get_swap_end() {
            return Ok(0.0);
        }
        let periods = self.fixed_schedule_periods_at(exercise_date)?;

        let mut annuity = 0.0;
        for period in periods {
            if period.payment_date > exercise_date {
                let df = relative_df_discounting(disc, as_of, period.payment_date)?;
                annuity += period.accrual_year_fraction * df;
            }
        }

        Ok(annuity)
    }

    /// Convert to European swaption for the first exercise date.
    ///
    /// Useful for calibration and testing.
    pub fn to_european(&self) -> Result<Swaption> {
        let first_ex = self
            .first_exercise()
            .ok_or_else(|| Error::Validation("No exercise dates".into()))?;
        let mut underlying_fixed_leg = self.underlying_fixed_leg.clone();
        underlying_fixed_leg.start = first_ex;
        let mut underlying_float_leg = self.underlying_float_leg.clone();
        underlying_float_leg.start = first_ex;
        underlying_fixed_leg.validate()?;
        underlying_float_leg.validate()?;

        Ok(Swaption {
            id: InstrumentId::new(format!("{}-EURO", self.id.as_str())),
            option_type: self.option_type,
            notional: self.notional,
            expiry: first_ex,
            exercise_style: SwaptionExercise::European,
            settlement: self.settlement,
            cash_settlement_method: CashSettlementMethod::default(),
            vol_model: VolatilityModel::Black,
            vol_surface_id: self.vol_surface_id.clone(),
            underlying_fixed_leg,
            underlying_float_leg,
            instrument_pricing_overrides: self.instrument_pricing_overrides.clone(),
            metric_pricing_overrides: self.metric_pricing_overrides.clone(),
            scenario_pricing_overrides: self.scenario_pricing_overrides.clone(),
            sabr_params: None,
            attributes: self.attributes.clone(),
        })
    }
}

impl crate::instruments::common_impl::traits::Instrument for BermudanSwaption {
    impl_instrument_base!(crate::pricer::InstrumentType::BermudanSwaption);

    fn validate_invariants(&self) -> Result<()> {
        self.validate()
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::MonteCarloHullWhite1F
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.get_discount_curve_id().clone());
        deps.add_forward_curve(self.get_forward_curve_id().clone());
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                None,
                Some(self.strike_f64()?),
            ),
        );
        Ok(deps)
    }

    fn base_value(
        &self,
        _curves: &finstack_quant_core::market_data::context::MarketContext,
        _as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        // Bermudan swaptions require tree or MC pricing - delegate to pricer
        Err(Error::Validation(
            "BermudanSwaption requires tree or LSMC pricing via BermudanSwaptionPricer".into(),
        ))
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.get_swap_start())
    }

    crate::impl_focused_pricing_overrides!();
}

/// Convert lognormal (Black) volatility to normal (Bachelier) volatility.
///
/// Uses the Brenner-Subrahmanyam (1988) / Hagan (2002) approximation with
/// second-order correction. When a SABR shift is provided, the conversion
/// operates on shifted rates (F + shift, K + shift), ensuring positivity
/// even for negative-rate environments.
///
/// # Arguments
///
/// * `sigma_ln` - Lognormal (Black) volatility
/// * `forward` - Forward swap rate
/// * `strike` - Strike rate
/// * `time_to_expiry` - Time to option expiry in years
/// * `shift` - Optional SABR shift for negative rate handling
///
/// # Formula
///
/// For ATM (F = K):
/// ```text
/// σ_normal ≈ σ_lognormal × F_eff × [1 - σ²T/24]
/// ```
///
/// For general F ≠ K:
/// ```text
/// σ_normal ≈ σ_lognormal × (F_eff - K_eff) / ln(F_eff/K_eff)
///             × [1 - σ²T/24 × (1 - ln²(F_eff/K_eff)/12)]
/// ```
///
/// where F_eff = F + shift, K_eff = K + shift when shift is provided.
///
/// # References
///
/// - Brenner, M. & Subrahmanyam, M.G. (1988). "A Simple Formula to Compute
///   the Implied Standard Deviation"
/// - Hagan, P. et al. (2002). "Managing Smile Risk" Wilmott Magazine
/// - Jaeckel, P. (2017). "Let's Be Rational" for exact conversion
pub(crate) fn lognormal_to_normal_vol(
    sigma_ln: f64,
    forward: f64,
    strike: f64,
    time_to_expiry: f64,
    shift: Option<f64>,
) -> f64 {
    // Apply shift to ensure positive rates for the lognormal-to-normal mapping.
    // Shifted SABR models define F_eff = F + shift, K_eff = K + shift where
    // shift is chosen so that both are positive (e.g., shift = 3% for EUR).
    let (f, k) = match shift {
        Some(s) => (forward + s, strike + s),
        None => (forward, strike),
    };

    let variance = sigma_ln * sigma_ln * time_to_expiry;

    if f <= 0.0 || k <= 0.0 {
        // Without shift, non-positive rates can't use the lognormal approximation.
        // Fall back to linear approximation using the arithmetic mean of absolute
        // values. This is crude and will produce unreliable normal vols -- callers
        // should supply a SABR shift for negative-rate currencies instead.
        //
        // WARNING: This fallback is inherently unreliable. For negative-rate
        // currencies (EUR, JPY, CHF), always configure `SABRParameters.shift`
        // so that F + shift and K + shift are positive.
        let effective_level = ((f.abs() + k.abs()) / 2.0).max(1e-6);
        return sigma_ln * effective_level;
    }

    let log_fk = (f / k).ln();

    // Moneyness-adjusted forward level
    // For ATM: limit of (F-K)/ln(F/K) as K→F is F
    // For non-ATM: this gives the "effective" forward for normal vol
    let effective_forward = if log_fk.abs() < 1e-8 {
        // Near ATM: use Taylor expansion to avoid 0/0
        // (F-K)/ln(F/K) ≈ F × [1 - ln(F/K)/2 + ln(F/K)²/12 - ...]
        f * (1.0 - log_fk / 2.0 + log_fk * log_fk / 12.0)
    } else {
        (f - k) / log_fk
    };

    // Second-order correction from Hagan (2002):
    // The correction accounts for the difference in convexity between
    // lognormal and normal models. For typical parameters this is ~0.1-1%.
    //
    // Correction = 1 - σ²T/24 × [1 - (1/12)(ln(F/K))²]
    //
    // For extreme parameters (σ²T > 12), the raw correction becomes negative.
    // We floor at 0.5 to keep the result positive and bounded. This floor only
    // activates for unrealistic combinations (e.g., 80% vol + 30Y tenor) where
    // the second-order approximation itself has broken down anyway.
    let moneyness_factor = 1.0 - log_fk * log_fk / 12.0;
    let correction = if variance > 1e-10 {
        let raw = 1.0 - (variance / 24.0) * moneyness_factor;
        raw.max(0.5)
    } else {
        1.0
    };

    sigma_ln * effective_forward * correction
}

/// Numerical Jacobian of the normal-volatility conversion with respect to the
/// original lognormal quote.
pub(crate) fn lognormal_to_normal_vol_jacobian(
    sigma_ln: f64,
    forward: f64,
    strike: f64,
    time_to_expiry: f64,
    shift: Option<f64>,
) -> f64 {
    let bump = (sigma_ln.abs() * 1.0e-5).max(1.0e-7);
    let lower = (sigma_ln - bump).max(0.0);
    let upper = sigma_ln + bump;
    let normal_upper = lognormal_to_normal_vol(upper, forward, strike, time_to_expiry, shift);
    let normal_lower = lognormal_to_normal_vol(lower, forward, strike, time_to_expiry, shift);
    (normal_upper - normal_lower) / (upper - lower)
}

crate::impl_empty_cashflow_provider!(
    BermudanSwaption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);
