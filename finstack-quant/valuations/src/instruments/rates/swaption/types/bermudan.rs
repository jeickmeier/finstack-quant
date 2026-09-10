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
use super::swaption::{vanilla_underlier, Swaption, VanillaSwaptionUnderlier};

// Bermudan Swaption Instrument

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
/// - **Hull-White Tree** (default): Industry standard, calibrated to swaption
///   volatility. Official PV and risk use this path unless a caller selects
///   [`crate::pricer::ModelKey::MonteCarloHullWhite1F`].
/// - **LSMC**: Longstaff-Schwartz Monte Carlo, opt-in for path-dependent
///   validation and model comparison.
///
/// # Example
///
/// ```
/// use finstack_quant_valuations::instruments::rates::swaption::{
///     BermudanSwaption, BermudanSchedule, BermudanType, SwaptionSettlement,
/// };
///
/// // Create a 10NC2 (10-year swap, callable after 2 years)
/// let swaption = BermudanSwaption::example();
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
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
            vanilla_underlier(VanillaSwaptionUnderlier::standard(
                strike,
                swap_start,
                swap_end,
                CurveId::new("USD-OIS"),
                CurveId::new("USD-OIS"),
            ));

        Self {
            id: InstrumentId::new("BERM-10NC2-USD"),
            option_type: OptionType::Call,
            notional: Money::from((10_000_000_i64, Currency::USD)),
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

    /// Create a co-terminal Bermudan swaption with USD-standard leg conventions
    /// (semi-annual 30/360 fixed versus quarterly ACT/360 floating).
    ///
    /// # Arguments
    ///
    /// * `id` - Instrument identifier.
    /// * `option_type` - [`OptionType::Call`] for a payer (right to pay fixed),
    ///   [`OptionType::Put`] for a receiver (right to receive fixed).
    /// * `notional` - Swap notional in the instrument currency.
    /// * `strike` - Fixed rate of the underlying swap as a decimal (0.03 = 3%).
    /// * `swap_start` - Effective date of the underlying swap.
    /// * `swap_end` - Maturity of the underlying swap (co-terminal for every exercise).
    /// * `bermudan_schedule` - Exercise dates.
    /// * `discount_curve_id` - Discount curve for both legs.
    /// * `forward_curve_id` - Projection curve for the floating leg.
    /// * `vol_surface_id` - Swaption volatility cube.
    ///
    /// # Errors
    ///
    /// Returns an error if `strike` is not representable as `Decimal` (NaN or
    /// infinite) or the resulting instrument fails [`BermudanSwaption::validate`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<InstrumentId>,
        option_type: OptionType,
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
            vanilla_underlier(VanillaSwaptionUnderlier::standard(
                strike,
                swap_start,
                swap_end,
                discount_curve_id.into(),
                forward_curve_id.into(),
            ));
        let swaption = Self {
            id: id.into(),
            option_type,
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
    pub fn get_fixed_frequency(&self) -> Tenor {
        self.underlying_fixed_leg.frequency
    }

    /// Floating-leg payment frequency.
    pub fn get_float_frequency(&self) -> Tenor {
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
    pub fn with_fixed_frequency(mut self, frequency: Tenor) -> Self {
        self.underlying_fixed_leg.frequency = frequency;
        self
    }

    /// Set floating leg frequency.
    pub fn with_float_frequency(mut self, frequency: Tenor) -> Self {
        self.underlying_float_leg.frequency = frequency;
        self
    }

    /// Set the fixed-leg day count convention.
    pub fn with_fixed_day_count(mut self, day_count: DayCount) -> Self {
        self.underlying_fixed_leg.day_count = day_count;
        self
    }

    /// Set the floating-leg day count convention.
    pub fn with_float_day_count(mut self, day_count: DayCount) -> Self {
        self.underlying_float_leg.day_count = day_count;
        self
    }

    /// Set settlement method.
    pub fn with_settlement(mut self, settlement: SwaptionSettlement) -> Self {
        self.settlement = settlement;
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

    /// Calculate ACT/365F model time to the first exercise.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date; elapsed first exercises return zero.
    pub fn time_to_first_exercise(&self, as_of: Date) -> Result<f64> {
        match self.first_exercise() {
            Some(first) => {
                if as_of >= first {
                    return Ok(0.0);
                }
                Ok(finstack_quant_models::rates::clock::model_time(
                    as_of, first,
                ))
            }
            None => Err(Error::Validation("No exercise dates".into())),
        }
    }

    /// Calculate the ACT/365F model horizon covering remaining contractual payments.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date defining model time zero; matured swaps return zero.
    pub fn time_to_maturity(&self, as_of: Date) -> Result<f64> {
        if as_of >= self.get_swap_end() {
            return Ok(0.0);
        }
        let flows =
            crate::instruments::rates::swaption::pricing::hw_cashflows::HwSwaptionCashflows::new(
                &self.underlying_fixed_leg,
                &self.underlying_float_leg,
                as_of,
                as_of,
            )?;
        Ok(flows.horizon())
    }

    /// Get future exercise dates as ACT/365F model times.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date defining model time zero; past exercises are excluded.
    pub fn exercise_times(&self, as_of: Date) -> Result<Vec<f64>> {
        self.bermudan_schedule
            .exercise_times(as_of, finstack_quant_core::dates::DayCount::Act365F)
    }

    /// Build the underlying swap payment schedule.
    ///
    /// Returns (payment_dates, accrual_fractions) for the fixed leg.
    pub fn build_swap_schedule(&self) -> Result<(Vec<Date>, Vec<f64>)> {
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
                business_day_convention: fixed.business_day_convention,
                calendar_id: fixed
                    .calendar_id
                    .as_deref()
                    .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
                end_of_month: fixed.end_of_month,
                day_count: fixed.day_count,
                payment_lag_days: fixed.payment_lag_days,
                reset_lag_days: None,
                adjust_accrual_dates: false,
                roll_rule: crate::cashflow::builder::specs::RollRule::None,
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
        let (dates, _) = self.build_swap_schedule()?;
        Ok(dates
            .iter()
            .map(|&d| finstack_quant_models::rates::clock::model_time(as_of, d))
            .collect())
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
        if exercise_date >= self.get_swap_end() {
            return Ok(0.0);
        }
        let periods = self.fixed_schedule_periods_at(exercise_date)?;
        crate::instruments::rates::irs::cashflow::fixed_leg_annuity(
            &periods,
            disc,
            as_of,
            exercise_date,
        )
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
        crate::pricer::ModelKey::HullWhite1F
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
        Err(Error::Validation(
            "BermudanSwaption requires tree or LSMC pricing via BermudanSwaptionPricer".into(),
        ))
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.get_swap_start())
    }

    crate::impl_focused_pricing_overrides!();
}

crate::impl_empty_cashflow_provider!(
    BermudanSwaption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::Instrument;
    use crate::pricer::ModelKey;

    #[test]
    fn default_model_is_hull_white_tree() {
        let swaption = BermudanSwaption::example();
        assert_eq!(swaption.default_model(), ModelKey::HullWhite1F);
    }

    #[test]
    fn value_requires_explicit_pricer() {
        use finstack_quant_core::market_data::context::MarketContext;
        use time::macros::date;

        let swaption = BermudanSwaption::example();
        let market = MarketContext::default();
        let as_of = date!(2025 - 01 - 01);
        let err = swaption
            .value(&market, as_of)
            .expect_err("value must require BermudanSwaptionPricer");
        let msg = err.to_string();
        assert!(
            msg.contains("BermudanSwaptionPricer"),
            "expected explicit pricer in error message, got: {msg}"
        );
    }
}
