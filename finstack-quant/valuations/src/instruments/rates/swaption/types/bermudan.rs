use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
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
use super::swaption::Swaption;

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
#[derive(Debug, Clone, finstack_quant_valuations_macros::FocusedPricingOverrides)]
#[serde(deny_unknown_fields)]
pub struct BermudanSwaption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Option type (payer = Call, receiver = Put)
    pub option_type: OptionType,
    /// Notional amount of underlying swap
    pub notional: Money,
    /// Strike (fixed rate on underlying swap)
    pub strike: Decimal,
    /// Underlying swap start date (first accrual start)
    #[schemars(with = "String")]
    pub swap_start: Date,
    /// Underlying swap end date (final payment)
    #[schemars(with = "String")]
    pub swap_end: Date,
    /// Fixed leg payment frequency
    pub fixed_freq: Tenor,
    /// Floating leg payment frequency
    pub float_freq: Tenor,
    /// Day count convention for fixed leg
    pub day_count: DayCount,
    /// Settlement method (physical or cash)
    pub settlement: SwaptionSettlement,
    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Forward curve ID for floating rate projections
    pub forward_curve_id: CurveId,
    /// Volatility surface ID for calibration
    pub vol_surface_id: CurveId,
    /// Bermudan exercise schedule
    pub bermudan_schedule: BermudanSchedule,
    /// Co-terminal or non-co-terminal exercise
    pub bermudan_type: BermudanType,
    /// Holiday calendar ID for schedule generation.
    ///
    /// Controls business day adjustment for the underlying swap schedule.
    /// When `None`, uses weekends-only calendar. For production use, set to
    /// the appropriate currency calendar (e.g., `"nyse"` for USD).
    #[serde(default)]
    pub calendar_id: Option<CalendarId>,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    /// Instrument-owned pricing inputs.
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and grouping
    #[serde(default)]
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl BermudanSwaption {
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

        Self {
            id: InstrumentId::new("BERM-10NC2-USD"),
            option_type: OptionType::Call,
            notional: Money::new(10_000_000.0, Currency::USD),
            strike: Decimal::try_from(0.03).expect("valid decimal"),
            swap_start,
            swap_end,
            fixed_freq: Tenor::semi_annual(),
            float_freq: Tenor::quarterly(),
            day_count: DayCount::Thirty360,
            settlement: SwaptionSettlement::Physical,
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: CurveId::new("USD-SWPNVOL"),
            bermudan_schedule: BermudanSchedule::co_terminal(
                first_exercise,
                swap_end,
                Tenor::semi_annual(),
            )
            .expect("valid Bermudan schedule"),
            bermudan_type: BermudanType::CoTerminal,
            calendar_id: None,
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
        Ok(Self {
            id: id.into(),
            option_type: OptionType::Call,
            notional,
            strike: finstack_quant_core::decimal::f64_to_decimal(strike)?,
            swap_start,
            swap_end,
            fixed_freq: Tenor::semi_annual(),
            float_freq: Tenor::quarterly(),
            day_count: DayCount::Thirty360,
            settlement: SwaptionSettlement::Physical,
            discount_curve_id: discount_curve_id.into(),
            forward_curve_id: forward_curve_id.into(),
            vol_surface_id: vol_surface_id.into(),
            bermudan_schedule,
            bermudan_type: BermudanType::CoTerminal,
            calendar_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::default(),
        })
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
        Ok(Self {
            id: id.into(),
            option_type: OptionType::Put,
            notional,
            strike: finstack_quant_core::decimal::f64_to_decimal(strike)?,
            swap_start,
            swap_end,
            fixed_freq: Tenor::semi_annual(),
            float_freq: Tenor::quarterly(),
            day_count: DayCount::Thirty360,
            settlement: SwaptionSettlement::Physical,
            discount_curve_id: discount_curve_id.into(),
            forward_curve_id: forward_curve_id.into(),
            vol_surface_id: vol_surface_id.into(),
            bermudan_schedule,
            bermudan_type: BermudanType::CoTerminal,
            calendar_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::default(),
        })
    }

    /// Set fixed leg frequency.
    pub fn with_fixed_freq(mut self, freq: Tenor) -> Self {
        self.fixed_freq = freq;
        self
    }

    /// Set floating leg frequency.
    pub fn with_float_freq(mut self, freq: Tenor) -> Self {
        self.float_freq = freq;
        self
    }

    /// Set day count convention.
    pub fn with_day_count(mut self, dc: DayCount) -> Self {
        self.day_count = dc;
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
        self.calendar_id = Some(calendar_id.into());
        self
    }

    /// Resolve the effective calendar ID for schedule generation.
    fn effective_calendar_id(&self) -> &str {
        self.calendar_id
            .as_deref()
            .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID)
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
                self.day_count.year_fraction(
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
        if as_of >= self.swap_end {
            return Ok(0.0);
        }
        self.day_count.year_fraction(
            as_of,
            self.swap_end,
            finstack_quant_core::dates::DayCountContext::default(),
        )
    }

    /// Get exercise dates as year fractions from valuation date.
    pub fn exercise_times(&self, as_of: Date) -> Result<Vec<f64>> {
        let times = self
            .bermudan_schedule
            .exercise_times(as_of, self.day_count)?;
        if times.is_empty() {
            return Ok(times);
        }
        Ok(times)
    }

    /// Build the underlying swap payment schedule.
    ///
    /// Returns (payment_dates, accrual_fractions) for the fixed leg.
    pub fn build_swap_schedule(&self, _as_of: Date) -> Result<(Vec<Date>, Vec<f64>)> {
        let periods = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: self.swap_start,
                end: self.swap_end,
                frequency: self.fixed_freq,
                stub: StubKind::None,
                bdc: BusinessDayConvention::ModifiedFollowing, // Market standard per ISDA
                calendar_id: self.effective_calendar_id(),
                end_of_month: false,
                day_count: self.day_count,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
            },
        )?;

        if periods.is_empty() {
            return Err(Error::Validation(
                "Swap schedule has fewer than 2 dates".into(),
            ));
        }

        let dates: Vec<Date> = periods.iter().map(|p| p.payment_date).collect();
        let accruals: Vec<f64> = periods.iter().map(|p| p.accrual_year_fraction).collect();

        Ok((dates, accruals))
    }

    /// Convert payment dates to year fractions.
    pub fn payment_times(&self, as_of: Date) -> Result<Vec<f64>> {
        let (dates, _) = self.build_swap_schedule(as_of)?;
        let ctx = finstack_quant_core::dates::DayCountContext::default();
        dates
            .iter()
            .map(|&d| self.day_count.year_fraction(as_of, d, ctx))
            .collect()
    }

    pub(crate) fn strike_f64(&self) -> Result<f64> {
        self.strike.to_f64().ok_or_else(|| {
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
        use crate::instruments::common_impl::pricing::time::{
            rate_period_on_dates, relative_df_discounting,
        };

        let disc = curves.get_discount(self.discount_curve_id.as_ref())?;
        let annuity = self.remaining_annuity(disc.as_ref(), as_of, exercise_date)?;

        if annuity.abs() < 1e-10 {
            return Ok(0.0);
        }

        // Single-curve optimization
        if self.forward_curve_id == self.discount_curve_id {
            let df_start = relative_df_discounting(disc.as_ref(), as_of, exercise_date)?;
            let df_end = relative_df_discounting(disc.as_ref(), as_of, self.swap_end)?;
            return Ok((df_start - df_end) / annuity);
        }

        let fwd = curves.get_forward(self.forward_curve_id.as_ref())?;
        let fwd_dc = fwd.day_count();
        let periods = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: exercise_date,
                end: self.swap_end,
                frequency: self.float_freq,
                stub: StubKind::None,
                bdc: BusinessDayConvention::ModifiedFollowing, // Market standard per ISDA
                calendar_id: self.effective_calendar_id(),
                end_of_month: false,
                day_count: fwd_dc,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
            },
        )?;

        let mut pv_float = 0.0;
        for period in periods {
            let fwd_rate =
                rate_period_on_dates(fwd.as_ref(), period.accrual_start, period.accrual_end)?;
            let df = relative_df_discounting(disc.as_ref(), as_of, period.payment_date)?;
            pv_float += period.accrual_year_fraction * fwd_rate * df;
        }

        Ok(pv_float / annuity)
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

        let (dates, accruals) = self.build_swap_schedule(as_of)?;

        let mut annuity = 0.0;
        for (d, tau) in dates.iter().zip(accruals.iter()) {
            if *d > exercise_date {
                let df = relative_df_discounting(disc, as_of, *d)?;
                annuity += tau * df;
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

        Ok(Swaption {
            id: InstrumentId::new(format!("{}-EURO", self.id.as_str())),
            option_type: self.option_type,
            notional: self.notional,
            strike: self.strike,
            expiry: first_ex,
            swap_start: first_ex,
            swap_end: self.swap_end,
            fixed_freq: self.fixed_freq,
            float_freq: self.float_freq,
            day_count: self.day_count,
            exercise_style: SwaptionExercise::European,
            settlement: self.settlement,
            cash_settlement_method: CashSettlementMethod::default(),
            vol_model: VolatilityModel::Black,
            discount_curve_id: self.discount_curve_id.clone(),
            forward_curve_id: self.forward_curve_id.clone(),
            vol_surface_id: self.vol_surface_id.clone(),
            calendar_id: self.calendar_id.clone(),
            underlying_fixed_leg: None,
            underlying_float_leg: None,
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

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::MonteCarloHullWhite1F
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
        Some(self.swap_start)
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
