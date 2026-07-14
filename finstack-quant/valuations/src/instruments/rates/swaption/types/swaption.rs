use crate::impl_instrument_base;
use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::instruments::pricing_overrides::VolSurfaceExtrapolation;
use crate::instruments::rates::irs::{
    FixedLegSpec, FloatLegSpec, FloatingLegCompounding, InterestRateSwap, PayReceive,
};
use crate::instruments::PricingOverrides;
use crate::models::SABRModel;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use finstack_quant_core::{Error, Result};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use super::super::parameters::SwaptionParams;
use super::definitions::{
    CashSettlementMethod, SABRParameters, SwaptionExercise, SwaptionSettlement, VolatilityModel,
};

/// Swaption instrument
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Swaption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Option type (payer or receiver swaption)
    pub option_type: OptionType,
    /// Notional amount of underlying swap
    pub notional: Money,
    /// Strike (fixed rate on underlying swap)
    pub strike: Decimal,
    /// Option expiry date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Underlying swap start date
    #[schemars(with = "String")]
    pub swap_start: Date,
    /// Underlying swap end date
    #[schemars(with = "String")]
    pub swap_end: Date,
    /// Fixed leg payment frequency
    pub fixed_freq: Tenor,
    /// Floating leg payment frequency
    pub float_freq: Tenor,
    /// Day count convention
    pub day_count: DayCount,
    /// Exercise style (European, Bermudan, American). Defaults to European.
    #[serde(default)]
    #[builder(default)]
    pub exercise_style: SwaptionExercise,
    /// Settlement method (physical or cash)
    pub settlement: SwaptionSettlement,
    /// Cash settlement annuity method (only used when settlement = Cash).
    ///
    /// - `ParYield` (default): Fast approximation using flat forward rate
    /// - `IsdaParPar`: Uses actual swap annuity from discount curve (ISDA compliant)
    /// - `ZeroCoupon`: Discounts to swap maturity (rarely used)
    #[serde(default)]
    pub cash_settlement_method: CashSettlementMethod,
    /// Volatility model (Black or Normal)
    #[serde(default)]
    pub vol_model: VolatilityModel,
    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Forward curve ID for floating rate projections
    pub forward_curve_id: CurveId,
    /// Volatility surface ID for option pricing
    pub vol_surface_id: CurveId,
    /// Holiday calendar ID for schedule generation.
    ///
    /// Controls business day adjustment and payment date calculation for the
    /// underlying swap schedule. When `None`, uses weekends-only calendar
    /// (no holiday adjustments). For production use, set to the appropriate
    /// currency calendar (e.g., `"nyse"` for USD, `"target"` for EUR).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let swaption = Swaption::example()
    ///     .with_calendar("nyse");
    /// ```
    #[serde(default)]
    pub calendar_id: Option<CalendarId>,
    /// Optional full fixed-leg specification for the underlying swap.
    ///
    /// When omitted, the pricer derives a vanilla fixed leg from the legacy
    /// swaption fields (`fixed_freq`, `day_count`, dates, calendar, discount
    /// curve). Use this for Bloomberg-style underliers that need explicit
    /// payment lag, stub, calendar, or day-count conventions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underlying_fixed_leg: Option<FixedLegSpec>,
    /// Optional full floating-leg specification for the underlying swap.
    ///
    /// When omitted, the pricer derives a simple term-rate floating leg from the
    /// legacy swaption fields. Use this for compounded SOFR/SONIA underliers
    /// that need reset calendars, payment lag, cutoff, or observation-shift
    /// conventions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underlying_float_leg: Option<FloatLegSpec>,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Optional SABR volatility model parameters
    pub sabr_params: Option<SABRParameters>,
    /// Attributes for scenario selection and grouping
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

impl Swaption {
    pub(crate) fn strike_f64(&self) -> Result<f64> {
        self.strike.to_f64().ok_or_else(|| {
            Error::Validation("Swaption strike could not be converted to f64".to_string())
        })
    }

    /// Validate structural invariants.
    ///
    /// Checks date ordering (expiry <= swap_start < swap_end), notional
    /// finiteness and positivity, and strike finiteness and magnitude.
    pub fn validate(&self) -> Result<()> {
        validation::validate_money_finite(self.notional, "swaption notional")?;
        validation::validate_money_gt(self.notional, 0.0, "swaption notional")?;

        validation::validate_date_range_non_strict(
            self.expiry,
            self.swap_start,
            "swaption expiry vs swap_start",
        )?;
        validation::validate_date_range_strict(
            self.swap_start,
            self.swap_end,
            "swaption swap_start vs swap_end",
        )?;

        let strike = self.strike_f64()?;
        validation::validate_f64_finite(strike, "swaption strike")?;
        validation::validate_f64_abs_le(strike, 2.0, "swaption strike", Some(" (rate)"))?;

        if self.underlying_fixed_leg.is_some() != self.underlying_float_leg.is_some() {
            return Err(Error::Validation(
                "swaption underlier must provide both fixed and floating leg specifications"
                    .to_string(),
            ));
        }

        Ok(())
    }

    /// Create a canonical example swaption for testing and documentation.
    ///
    /// Returns a 1Y x 5Y payer swaption (1 year to expiry, 5 year swap tenor).
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        Self {
            id: InstrumentId::new("SWPN-1Yx5Y-USD"),
            option_type: OptionType::Call,
            notional: Money::new(10_000_000.0, Currency::USD),
            strike: Decimal::try_from(0.03).expect("valid decimal"),
            expiry: Date::from_calendar_date(2027, time::Month::January, 15)
                .expect("Valid example date"),
            swap_start: Date::from_calendar_date(2027, time::Month::January, 17)
                .expect("Valid example date"),
            swap_end: Date::from_calendar_date(2032, time::Month::January, 17)
                .expect("Valid example date"),
            fixed_freq: Tenor::semi_annual(),
            float_freq: Tenor::quarterly(),
            day_count: DayCount::Thirty360,
            exercise_style: SwaptionExercise::European,
            settlement: SwaptionSettlement::Cash,
            cash_settlement_method: CashSettlementMethod::default(),
            vol_model: VolatilityModel::Black,
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: CurveId::new("USD-SWPNVOL"),
            calendar_id: None,
            underlying_fixed_leg: None,
            underlying_float_leg: None,
            pricing_overrides: PricingOverrides::default(),
            sabr_params: None,
            attributes: Attributes::new(),
        }
    }

    /// Create a Bermudan-style swaption example for testing and documentation.
    ///
    /// Returns a 5NC1 payer swaption (5-year swap, Bermudan exercise after 1 year)
    /// with physical settlement, Normal vol model, and SABR parameters populated.
    /// Exercise dates are semi-annual, aligned with swap coupon dates.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example_bermudan() -> Self {
        let swap_start =
            Date::from_calendar_date(2027, time::Month::January, 17).expect("Valid example date");
        let swap_end =
            Date::from_calendar_date(2032, time::Month::January, 17).expect("Valid example date");
        // First exercise 1 year after swap start
        let first_exercise =
            Date::from_calendar_date(2028, time::Month::January, 17).expect("Valid example date");
        Self {
            id: InstrumentId::new("SWPN-5NC1-BERM-USD"),
            option_type: OptionType::Call,
            notional: Money::new(10_000_000.0, Currency::USD),
            strike: Decimal::try_from(0.035).expect("valid decimal"),
            expiry: first_exercise,
            swap_start,
            swap_end,
            fixed_freq: Tenor::semi_annual(),
            float_freq: Tenor::quarterly(),
            day_count: DayCount::Act360,
            exercise_style: SwaptionExercise::Bermudan,
            settlement: SwaptionSettlement::Physical,
            cash_settlement_method: CashSettlementMethod::default(),
            vol_model: VolatilityModel::Normal,
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: CurveId::new("USD-SWPNVOL"),
            calendar_id: None,
            underlying_fixed_leg: None,
            underlying_float_leg: None,
            pricing_overrides: PricingOverrides::default(),
            sabr_params: Some(SABRParameters {
                alpha: 0.025,
                beta: 0.5,
                nu: 0.40,
                rho: -0.30,
                shift: None,
            }),
            attributes: Attributes::new(),
        }
    }

    /// Create a new payer swaption using parameter structs.
    pub fn new_payer(
        id: impl Into<InstrumentId>,
        params: &SwaptionParams,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> Self {
        let mut s = Self {
            id: id.into(),
            option_type: OptionType::Call,
            notional: params.notional,
            strike: params.strike,
            expiry: params.expiry,
            swap_start: params.swap_start,
            swap_end: params.swap_end,
            fixed_freq: Tenor::semi_annual(),
            float_freq: Tenor::quarterly(),
            day_count: DayCount::Thirty360,
            exercise_style: SwaptionExercise::European,
            settlement: SwaptionSettlement::Physical,
            cash_settlement_method: CashSettlementMethod::default(),
            discount_curve_id: discount_curve_id.into(),
            forward_curve_id: forward_curve_id.into(),
            vol_surface_id: vol_surface_id.into(),
            calendar_id: None,
            underlying_fixed_leg: None,
            underlying_float_leg: None,
            pricing_overrides: PricingOverrides::default(),
            sabr_params: None,
            attributes: Attributes::default(),
            vol_model: Default::default(),
        };
        if let Some(f) = params.fixed_freq {
            s.fixed_freq = f;
        }
        if let Some(f) = params.float_freq {
            s.float_freq = f;
        }
        if let Some(dc) = params.day_count {
            s.day_count = dc;
        }
        if let Some(vm) = params.vol_model {
            s.vol_model = vm;
        }
        s
    }

    /// Create a new receiver swaption using parameter structs.
    pub fn new_receiver(
        id: impl Into<InstrumentId>,
        params: &SwaptionParams,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> Self {
        let mut s = Self {
            id: id.into(),
            option_type: OptionType::Put,
            notional: params.notional,
            strike: params.strike,
            expiry: params.expiry,
            swap_start: params.swap_start,
            swap_end: params.swap_end,
            fixed_freq: Tenor::semi_annual(),
            float_freq: Tenor::quarterly(),
            day_count: DayCount::Thirty360,
            exercise_style: SwaptionExercise::European,
            settlement: SwaptionSettlement::Physical,
            cash_settlement_method: CashSettlementMethod::default(),
            discount_curve_id: discount_curve_id.into(),
            forward_curve_id: forward_curve_id.into(),
            vol_surface_id: vol_surface_id.into(),
            calendar_id: None,
            underlying_fixed_leg: None,
            underlying_float_leg: None,
            pricing_overrides: PricingOverrides::default(),
            sabr_params: None,
            attributes: Attributes::default(),
            vol_model: Default::default(),
        };
        if let Some(f) = params.fixed_freq {
            s.fixed_freq = f;
        }
        if let Some(f) = params.float_freq {
            s.float_freq = f;
        }
        if let Some(dc) = params.day_count {
            s.day_count = dc;
        }
        if let Some(vm) = params.vol_model {
            s.vol_model = vm;
        }
        s
    }

    /// Attach SABR parameters to enable SABR-implied volatility pricing.
    pub fn with_sabr(mut self, params: SABRParameters) -> Self {
        self.sabr_params = Some(params);
        self
    }

    /// Override the exercise style (default: European).
    pub fn with_exercise_style(mut self, style: SwaptionExercise) -> Self {
        self.exercise_style = style;
        self
    }

    /// Override the settlement type (default: Physical).
    pub fn with_settlement(mut self, settlement: SwaptionSettlement) -> Self {
        self.settlement = settlement;
        self
    }

    /// Override the option type (Call = payer, Put = receiver).
    pub fn with_option_type(mut self, option_type: OptionType) -> Self {
        self.option_type = option_type;
        self
    }

    /// Set the holiday calendar for schedule generation.
    ///
    /// # Arguments
    /// * `calendar_id` - Calendar ID registered in `CalendarRegistry`
    ///   (e.g., `"nyse"` for USD, `"target"` for EUR)
    pub fn with_calendar(mut self, calendar_id: impl Into<CalendarId>) -> Self {
        self.calendar_id = Some(calendar_id.into());
        self
    }

    fn effective_calendar_string(&self) -> Option<String> {
        self.calendar_id
            .as_ref()
            .map(std::string::ToString::to_string)
    }

    /// Fixed-leg convention used by every pricing path.
    ///
    /// Explicit leg state is canonical. The scalar fields are retained only as
    /// the legacy wire/build input used to synthesize a vanilla leg.
    pub(crate) fn underlying_fixed_frequency(&self) -> Tenor {
        self.underlying_fixed_leg
            .as_ref()
            .map_or(self.fixed_freq, |leg| leg.frequency)
    }

    /// Accrual convention used by every pricing path.
    pub(crate) fn underlying_day_count(&self) -> DayCount {
        self.underlying_fixed_leg
            .as_ref()
            .map_or(self.day_count, |leg| leg.day_count)
    }

    /// Discount curve selected by the canonical fixed leg.
    pub(crate) fn underlying_discount_curve_id(&self) -> &CurveId {
        self.underlying_fixed_leg
            .as_ref()
            .map_or(&self.discount_curve_id, |leg| &leg.discount_curve_id)
    }

    /// Forward curve selected by the canonical floating leg.
    pub(crate) fn underlying_forward_curve_id(&self) -> &CurveId {
        self.underlying_float_leg
            .as_ref()
            .map_or(&self.forward_curve_id, |leg| &leg.forward_curve_id)
    }

    fn default_fixed_leg(&self, rate: Decimal) -> FixedLegSpec {
        FixedLegSpec {
            discount_curve_id: self.discount_curve_id.clone(),
            rate,
            frequency: self.fixed_freq,
            day_count: self.day_count,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: self.effective_calendar_string(),
            stub: StubKind::None,
            start: self.swap_start,
            end: self.swap_end,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        }
    }

    fn default_float_leg(&self) -> FloatLegSpec {
        FloatLegSpec {
            discount_curve_id: self.discount_curve_id.clone(),
            forward_curve_id: self.forward_curve_id.clone(),
            spread_bp: Decimal::ZERO,
            frequency: self.float_freq,
            day_count: self.day_count,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: self.effective_calendar_string(),
            stub: StubKind::None,
            reset_lag_days: 0,
            fixing_calendar_id: self.effective_calendar_string(),
            start: self.swap_start,
            end: self.swap_end,
            compounding: FloatingLegCompounding::Simple,
            payment_lag_days: 0,
            end_of_month: false,
        }
    }

    fn underlying_fixed_leg_with_rate(&self, rate: Decimal) -> FixedLegSpec {
        let mut fixed = self
            .underlying_fixed_leg
            .clone()
            .unwrap_or_else(|| self.default_fixed_leg(rate));
        fixed.rate = rate;
        fixed
    }

    fn underlying_irs(&self, fixed_rate: f64, side: PayReceive) -> Result<InterestRateSwap> {
        self.underlying_irs_with_float(fixed_rate, side, None)
    }

    fn underlying_irs_for_market(
        &self,
        fixed_rate: f64,
        side: PayReceive,
        curves: &MarketContext,
    ) -> Result<InterestRateSwap> {
        let float = if self.underlying_float_leg.is_some() {
            self.underlying_float_leg.clone()
        } else {
            let mut float = self.default_float_leg();
            if let Ok(forward) = curves.get_forward(self.underlying_forward_curve_id().as_ref()) {
                float.day_count = forward.day_count();
            }
            Some(float)
        };
        self.underlying_irs_with_float(fixed_rate, side, float)
    }

    fn underlying_irs_with_float(
        &self,
        fixed_rate: f64,
        side: PayReceive,
        float: Option<FloatLegSpec>,
    ) -> Result<InterestRateSwap> {
        let fixed_rate = finstack_quant_core::decimal::f64_to_decimal(fixed_rate)?;
        let fixed = self.underlying_fixed_leg_with_rate(fixed_rate);
        let float = float.unwrap_or_else(|| {
            self.underlying_float_leg
                .clone()
                .unwrap_or_else(|| self.default_float_leg())
        });

        let irs = InterestRateSwap::builder()
            .id(InstrumentId::new(format!("{}:UNDERLIER", self.id.as_str())))
            .notional(self.notional)
            .side(side)
            .fixed(fixed)
            .float(float)
            .build()?;
        irs.validate()?;
        Ok(irs)
    }

    fn underlying_tenor_years(&self) -> Result<f64> {
        if self.swap_end <= self.swap_start {
            return Err(Error::Validation(format!(
                "Swaption '{}' has non-positive underlying tenor",
                self.id
            )));
        }

        // Use a proper day-count year fraction over [swap_start, swap_end]
        // rather than an ad-hoc 30-day-month / ACT-365 mix. This value feeds
        // the vol-surface tenor axis, so it must be consistent with the rest
        // of the instrument's day-count conventions.
        year_fraction(self.underlying_day_count(), self.swap_start, self.swap_end)
    }

    /// Set the cash settlement annuity method.
    ///
    /// Only affects pricing when `settlement` is `SwaptionSettlement::Cash`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::rates::swaption::{Swaption, CashSettlementMethod};
    ///
    /// // Create a cash-settled swaption with ISDA Par-Par settlement
    /// let swaption = Swaption::example()
    ///     .with_cash_settlement_method(CashSettlementMethod::IsdaParPar);
    /// ```
    pub fn with_cash_settlement_method(mut self, method: CashSettlementMethod) -> Self {
        self.cash_settlement_method = method;
        self
    }

    // ============================================================================
    // Pricing Methods (moved from engine for direct access)
    // ============================================================================

    /// Time to option expiry in years, measured with ACT/365F.
    ///
    /// Option expiry enters the Black/Bachelier formulas as calendar time, so
    /// it uses ACT/365F regardless of the instrument's accrual `day_count`
    /// (which still governs annuity and accrual computations). Using the
    /// accrual day count (e.g. Act360) would inflate T by ~365/360.
    fn time_to_expiry(&self, as_of: Date) -> Result<f64> {
        year_fraction(DayCount::Act365F, as_of, self.expiry)
    }

    /// Helper for common pricing logic
    fn price_model_base<F>(
        &self,
        curves: &MarketContext,
        volatility: f64,
        as_of: Date,
        model_fn: F,
    ) -> Result<Money>
    where
        F: Fn(f64, f64, f64, f64, f64) -> f64, // forward, strike, vol, t, annuity -> value
    {
        let time_to_expiry = self.time_to_expiry(as_of)?;
        if time_to_expiry <= 0.0 {
            return Ok(Money::new(0.0, self.notional.currency()));
        }

        let disc = curves.get_discount(self.underlying_discount_curve_id().as_ref())?;
        let forward_rate = self.forward_swap_rate(curves, as_of)?;
        let annuity = self.annuity(disc.as_ref(), as_of, forward_rate)?;
        let strike = self.strike_f64()?;

        let value = model_fn(forward_rate, strike, volatility, time_to_expiry, annuity);

        Ok(Money::new(
            value * self.notional.amount(),
            self.notional.currency(),
        ))
    }

    /// Black (lognormal) model PV.
    pub fn price_black(
        &self,
        curves: &MarketContext,
        volatility: f64,
        as_of: Date,
    ) -> Result<Money> {
        use super::lognormal_to_normal_vol;

        let time_to_expiry = self.time_to_expiry(as_of)?;
        if time_to_expiry <= 0.0 {
            return Ok(Money::new(0.0, self.notional.currency()));
        }

        let strike = self.strike_f64()?;
        let forward = self.forward_swap_rate(curves, as_of)?;
        if forward <= 0.0 || strike <= 0.0 {
            // Black (lognormal) pricing is undefined for a non-positive forward
            // or strike. In negative-rate regimes (EUR/JPY/CHF) fall back to
            // the Bachelier (normal) model, which prices negative rates
            // natively. `volatility` here is a LOGNORMAL vol — it must be
            // converted to a normal (Bachelier) vol before the normal pricer,
            // otherwise the magnitude is wrong by roughly a factor of the
            // forward rate. Use any configured SABR shift so the conversion
            // can operate on positive shifted rates.
            let shift = self.sabr_params.as_ref().and_then(|p| p.shift);
            let normal_vol =
                lognormal_to_normal_vol(volatility, forward, strike, time_to_expiry, shift);
            return self.price_normal(curves, normal_vol, as_of);
        }

        self.price_model_base(curves, volatility, as_of, |fwd, strike, vol, t, annuity| {
            // Use stable handling if volatility is near zero
            if vol <= 0.0 || !vol.is_finite() {
                // Intrinsic value
                let val = match self.option_type {
                    OptionType::Call => (fwd - strike).max(0.0),
                    OptionType::Put => (strike - fwd).max(0.0),
                };
                return val * annuity;
            }

            use crate::models::{d1_black76, d2_black76};
            let d1 = d1_black76(fwd, strike, vol, t);
            let d2 = d2_black76(fwd, strike, vol, t);

            match self.option_type {
                OptionType::Call => {
                    annuity
                        * (fwd * finstack_quant_core::math::norm_cdf(d1)
                            - strike * finstack_quant_core::math::norm_cdf(d2))
                }
                OptionType::Put => {
                    annuity
                        * (strike * finstack_quant_core::math::norm_cdf(-d2)
                            - fwd * finstack_quant_core::math::norm_cdf(-d1))
                }
            }
        })
    }

    /// Bachelier (normal) model PV.
    pub fn price_normal(
        &self,
        curves: &MarketContext,
        volatility: f64,
        as_of: Date,
    ) -> Result<Money> {
        self.price_model_base(curves, volatility, as_of, |fwd, strike, vol, t, annuity| {
            use crate::models::volatility::normal::bachelier_price;
            bachelier_price(self.option_type, fwd, strike, vol, t, annuity)
        })
    }

    /// SABR-implied volatility PV with model-aware pricing.
    ///
    /// The SABR formula (Hagan 2002) outputs lognormal (Black) volatility by default.
    /// When `vol_model == Normal`, we convert the lognormal vol to approximate
    /// normal (Bachelier) vol using the standard approximation:
    ///
    /// ```text
    /// σ_normal ≈ σ_lognormal × forward × (1 - ε) where ε is a small correction
    /// ```
    ///
    /// For ATM options, this approximation is exact. For OTM/ITM options,
    /// the approximation is accurate to within a few basis points for typical
    /// market conditions.
    ///
    /// # Negative Rates
    ///
    /// When SABR `shift` is set, the lognormal-to-normal conversion operates on
    /// shifted rates (F + shift, K + shift) which are guaranteed positive.
    /// Without a shift, non-positive rates fall back to a crude approximation.
    /// For negative-rate currencies (EUR, JPY, CHF), always use shifted SABR
    /// via [`SABRParameters::new_with_shift`].
    ///
    /// # References
    ///
    /// - Hagan, P. et al. (2002). "Managing Smile Risk" *Wilmott Magazine*
    /// - Antonov, A. et al. (2015). "SABR/Free Sabr" for normal vol extensions
    pub fn price_sabr(&self, curves: &MarketContext, as_of: Date) -> Result<Money> {
        use super::lognormal_to_normal_vol;

        let params = self
            .sabr_params
            .as_ref()
            .ok_or_else(|| Error::internal("swaption SABR pricing requires sabr_params"))?;
        let model = SABRModel::new(params.clone());
        let time_to_expiry = self.time_to_expiry(as_of)?;
        if time_to_expiry <= 0.0 {
            return Ok(Money::new(0.0, self.notional.currency()));
        }
        let forward_rate = self.forward_swap_rate(curves, as_of)?;
        let strike = self.strike_f64()?;

        // SABR output convention is β-dependent: lognormal (Black) vol for
        // β>0, normal (Bachelier) vol for β≈0. Branch on the tag instead of
        // assuming Black — converting a Bachelier vol as if it were lognormal
        // silently misprices by orders of magnitude in rate space.
        let (sabr_vol, sabr_vol_type) =
            model.implied_volatility_with_type(forward_rate, strike, time_to_expiry)?;

        use crate::models::volatility::sabr::SabrVolType;
        match (self.vol_model, sabr_vol_type) {
            (VolatilityModel::Black, SabrVolType::Black) => {
                self.price_black(curves, sabr_vol, as_of)
            }
            (VolatilityModel::Normal, SabrVolType::Black) => {
                let sabr_normal_vol = lognormal_to_normal_vol(
                    sabr_vol,
                    forward_rate,
                    strike,
                    time_to_expiry,
                    params.shift,
                );
                self.price_normal(curves, sabr_normal_vol, as_of)
            }
            // β≈0 SABR already produces the normal vol Bachelier needs.
            (VolatilityModel::Normal, SabrVolType::Normal) => {
                self.price_normal(curves, sabr_vol, as_of)
            }
            (VolatilityModel::Black, SabrVolType::Normal) => Err(Error::Validation(format!(
                "Swaption {}: SABR with β≈0 produces a normal (Bachelier) vol, which cannot \
                 feed the Black pricing model directly. Set vol_model to Normal (the natural \
                 pairing for normal-SABR) or calibrate SABR with β>0.",
                self.id
            ))),
        }
    }

    /// Calculate annuity based on settlement type and cash settlement method.
    ///
    /// # Settlement Types
    ///
    /// - **Physical**: Always uses `swap_annuity()` (actual PV01 from discount curve)
    /// - **Cash**: Uses the method specified by `cash_settlement_method`:
    ///   - `ParYield`: Closed-form approximation (fast, less accurate for steep curves)
    ///   - `IsdaParPar`: Actual swap annuity from discount curve (ISDA compliant)
    ///   - `ZeroCoupon`: Single discount to swap maturity (rarely used)
    pub fn annuity(&self, disc: &dyn Discounting, as_of: Date, forward_rate: f64) -> Result<f64> {
        match self.settlement {
            SwaptionSettlement::Physical => self.swap_annuity(disc, as_of),
            SwaptionSettlement::Cash => match self.cash_settlement_method {
                CashSettlementMethod::ParYield => {
                    // `cash_annuity_par_yield` is the cash annuity *at expiry*;
                    // the settlement amount must be discounted back to `as_of`.
                    use crate::instruments::common_impl::pricing::time::relative_df_discounting;
                    let df = relative_df_discounting(disc, as_of, self.expiry)?;
                    Ok(self.cash_annuity_par_yield(forward_rate)? * df)
                }
                CashSettlementMethod::IsdaParPar => self.swap_annuity(disc, as_of),
                CashSettlementMethod::ZeroCoupon => self.cash_annuity_zero_coupon(disc, as_of),
            },
        }
    }

    /// Discounted fixed-leg PV01 (annuity) of the underlying swap schedule (Physical Settlement).
    ///
    /// # Time Basis
    ///
    /// Uses curve-consistent relative discount factors via `relative_df_discounting`:
    /// - DF from `as_of` to each payment date is computed using the discount curve's
    ///   own base_date and day_count (not the instrument's day_count).
    /// - Accrual fractions use the instrument's day_count (correct for coupon calculation).
    pub fn swap_annuity(&self, disc: &dyn Discounting, as_of: Date) -> Result<f64> {
        use crate::instruments::common_impl::pricing::time::relative_df_discounting;
        use finstack_quant_core::math::NeumaierAccumulator;

        let underlier = self.underlying_irs(1.0, PayReceive::Receive)?;
        let sched = crate::instruments::rates::irs::cashflow::fixed_leg_schedule(&underlier)?;
        let mut annuity = NeumaierAccumulator::new();
        for flow in sched.get_flows() {
            if flow.date <= as_of {
                continue;
            }
            let df = relative_df_discounting(disc, as_of, flow.date)?;
            annuity.add(flow.amount.amount() / self.notional.amount() * df);
        }
        Ok(annuity.total())
    }

    /// Cash settlement annuity using par yield approximation.
    ///
    /// Returns the **undiscounted at-expiry** cash annuity. Callers pricing as of
    /// an earlier date must discount by `DF(as_of → expiry)`; [`Self::annuity`]
    /// applies that discounting in the `ParYield` arm.
    ///
    /// # Formula
    ///
    /// ```text
    /// A = (1 - (1 + S/m)^(-N)) / S
    /// ```
    ///
    /// where:
    /// - S = forward swap rate (settlement rate)
    /// - m = payment frequency per year
    /// - N = total number of payment periods
    ///
    /// # Approximation Notes
    ///
    /// This formula assumes:
    /// 1. **Flat forward rate**: The swap rate S is used as a constant discount rate
    ///    across all periods. This is an approximation when the yield curve is not flat.
    /// 2. **Equal periods**: All accrual periods are assumed equal (no stubs).
    ///
    /// For production systems requiring exact ISDA compliance, use
    /// `cash_settlement_method: CashSettlementMethod::IsdaParPar` which delegates
    /// to `swap_annuity`.
    ///
    /// # Edge Cases
    ///
    /// When `forward_rate ≈ 0`, uses L'Hôpital's limit: `A → N/m` (sum of accruals).
    pub fn cash_annuity_par_yield(&self, forward_rate: f64) -> Result<f64> {
        let fixed_frequency = self.underlying_fixed_frequency();
        let freq_per_year = match fixed_frequency.unit() {
            finstack_quant_core::dates::TenorUnit::Months if fixed_frequency.count() > 0 => {
                12.0 / fixed_frequency.count() as f64
            }
            finstack_quant_core::dates::TenorUnit::Days if fixed_frequency.count() > 0 => {
                365.0 / fixed_frequency.count() as f64
            }
            finstack_quant_core::dates::TenorUnit::Years if fixed_frequency.count() > 0 => {
                1.0 / fixed_frequency.count() as f64
            }
            finstack_quant_core::dates::TenorUnit::Weeks if fixed_frequency.count() > 0 => {
                52.0 / fixed_frequency.count() as f64
            }
            _ => {
                return Err(Error::Validation(
                    "Invalid frequency in cash annuity".into(),
                ))
            }
        };

        if forward_rate.abs() < 1e-8 {
            // L'Hopital's limit for S -> 0: A = N/m (sum of accruals)
            // We need number of periods.
            let tenor = year_fraction(self.underlying_day_count(), self.swap_start, self.swap_end)?;
            let periods = freq_per_year * tenor;
            return Ok(periods / freq_per_year);
        }

        let tenor_years =
            year_fraction(self.underlying_day_count(), self.swap_start, self.swap_end)?;
        let n_periods = tenor_years * freq_per_year;

        let df_swap = (1.0 + forward_rate / freq_per_year).powf(-n_periods);
        Ok((1.0 - df_swap) / forward_rate)
    }

    /// Cash settlement annuity using zero coupon method.
    ///
    /// # Formula
    ///
    /// ```text
    /// A = τ × DF(T_swap)
    /// ```
    ///
    /// where:
    /// - τ = total swap tenor as year fraction
    /// - DF(T_swap) = discount factor to swap maturity
    ///
    /// This method treats the entire swap as a single zero-coupon payment
    /// at maturity. Rarely used in modern markets; included for completeness.
    pub fn cash_annuity_zero_coupon(&self, disc: &dyn Discounting, as_of: Date) -> Result<f64> {
        use crate::instruments::common_impl::pricing::time::relative_df_discounting;

        let tenor = year_fraction(self.underlying_day_count(), self.swap_start, self.swap_end)?;
        let df = relative_df_discounting(disc, as_of, self.swap_end)?;
        Ok(tenor * df)
    }

    /// Forward par swap rate implied by float-leg PV and fixed-leg annuity.
    ///
    /// # Time Basis
    ///
    /// Uses curve-consistent time mapping:
    /// - Discount factors use the discount curve's own base_date/day_count
    /// - Forward rates use the forward curve's own base_date/day_count
    ///
    /// # Formula
    ///
    /// ```text
    /// S = PV_float / Annuity
    /// ```
    ///
    /// where:
    /// - PV_float = Σ (accrual_i × forward_i × DF_i)
    /// - Annuity = Σ (accrual_i × DF_i) for all fixed leg payments.
    pub fn forward_swap_rate(&self, curves: &MarketContext, as_of: Date) -> Result<f64> {
        let disc = curves.get_discount(self.underlying_discount_curve_id().as_ref())?;
        if self.underlying_forward_curve_id() == self.underlying_discount_curve_id() {
            return self.single_curve_forward_from_fixed_schedule(disc.as_ref(), as_of);
        }

        let annuity = self.swap_annuity(disc.as_ref(), as_of)?;
        if annuity.abs() < 1e-10 {
            return Ok(0.0);
        }

        let underlier = self.underlying_irs_for_market(0.0, PayReceive::Receive, curves)?;
        let pv_float = underlier.pv_float_leg(curves, as_of)?;

        Ok(pv_float / (self.notional.amount() * annuity))
    }

    fn single_curve_forward_from_fixed_schedule(
        &self,
        disc: &dyn Discounting,
        as_of: Date,
    ) -> Result<f64> {
        use crate::cashflow::builder::periods::{build_periods, BuildPeriodsParams};
        use crate::instruments::common_impl::pricing::time::relative_df_discounting;
        use finstack_quant_core::math::NeumaierAccumulator;

        let fixed = self.underlying_fixed_leg_with_rate(Decimal::ONE);
        let periods = build_periods(BuildPeriodsParams {
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
        })?;

        let mut forward_leg = NeumaierAccumulator::new();
        let mut annuity = NeumaierAccumulator::new();
        for period in periods {
            if period.payment_date <= as_of {
                continue;
            }
            let tau = period.accrual_year_fraction;
            if tau.abs() <= f64::EPSILON {
                continue;
            }

            let df_start = relative_df_discounting(disc, as_of, period.accrual_start)?;
            let df_end = relative_df_discounting(disc, as_of, period.accrual_end)?;
            let df_pay = relative_df_discounting(disc, as_of, period.payment_date)?;
            let forward = (df_start / df_end - 1.0) / tau;
            forward_leg.add(tau * forward * df_pay);
            annuity.add(tau * df_pay);
        }

        let annuity = annuity.total();
        if annuity.abs() < 1e-10 {
            return Ok(0.0);
        }
        Ok(forward_leg.total() / annuity)
    }

    /// Resolve volatility from SABR parameters, pricing override, or volatility surface.
    ///
    /// This consolidates the volatility resolution logic used by Greek calculators.
    /// Priority order:
    /// 1. SABR model parameters (if set)
    /// 2. Pricing override implied volatility (if set)
    /// 3. Volatility surface lookup
    ///
    /// # Arguments
    /// * `curves` - Market context containing volatility surfaces
    /// * `forward` - Forward swap rate
    /// * `time_to_expiry` - Time to option expiry in years
    ///
    /// # Returns
    /// Resolved volatility value
    pub fn resolve_volatility(
        &self,
        curves: &MarketContext,
        forward: f64,
        time_to_expiry: f64,
    ) -> Result<f64> {
        // 1. SABR model (highest priority)
        if let Some(sabr) = &self.sabr_params {
            let model = SABRModel::new(sabr.clone());
            return model.implied_volatility(forward, self.strike_f64()?, time_to_expiry);
        }

        // 2. Pricing override
        if let Some(impl_vol) = self.pricing_overrides.market_quotes.implied_volatility {
            return Ok(impl_vol);
        }

        // 3. Volatility provider. Strike surfaces use the strike coordinate;
        // tenor surfaces and SABR cubes use the underlying swap tenor.
        let vol_provider = curves.get_vol_provider(self.vol_surface_id.as_str())?;
        let strike = self.strike_f64()?;
        let underlying_tenor = self.underlying_tenor_years()?;
        match self
            .pricing_overrides
            .model_config
            .vol_surface_extrapolation
        {
            VolSurfaceExtrapolation::Clamp | VolSurfaceExtrapolation::LinearInVariance => {
                // LinearInVariance falls back to Clamp until surface impl is ready
                Ok(vol_provider.vol_clamped(time_to_expiry, underlying_tenor, strike))
            }
            VolSurfaceExtrapolation::Error => {
                Ok(vol_provider.vol(time_to_expiry, underlying_tenor, strike)?)
            }
        }
    }

    /// Pre-compute common Greek calculation inputs.
    ///
    /// Returns `None` if the option has expired (time_to_expiry <= 0).
    /// This consolidates the setup logic shared across delta, gamma, vega, and rho calculators.
    ///
    /// # Arguments
    /// * `curves` - Market context containing curves and surfaces
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    /// `Some(GreekInputs)` containing forward, annuity, sigma, and time to expiry,
    /// or `None` if the option has expired.
    pub fn greek_inputs(&self, curves: &MarketContext, as_of: Date) -> Result<Option<GreekInputs>> {
        let disc = curves.get_discount(self.underlying_discount_curve_id().as_ref())?;
        if as_of >= self.expiry {
            return Ok(None);
        }
        let t = self.time_to_expiry(as_of)?;

        if t <= 0.0 {
            return Ok(None);
        }

        let forward = self.forward_swap_rate(curves, as_of)?;
        let annuity = self.annuity(disc.as_ref(), as_of, forward)?;
        let sigma = self.resolve_volatility(curves, forward, t)?;

        Ok(Some(GreekInputs {
            forward,
            annuity,
            sigma,
            time_to_expiry: t,
        }))
    }
}

/// Pre-computed inputs for Greek calculations.
///
/// This struct contains the common values needed by delta, gamma, vega,
/// and other Greek calculators, avoiding redundant computation.
#[derive(Debug, Clone, Copy)]
pub struct GreekInputs {
    /// Forward swap rate
    pub forward: f64,
    /// Swap annuity (PV01 or cash annuity depending on settlement)
    pub annuity: f64,
    /// Resolved volatility (from SABR, override, or surface)
    pub sigma: f64,
    /// Time to option expiry in years
    pub time_to_expiry: f64,
}

impl crate::instruments::common_impl::traits::Instrument for Swaption {
    impl_instrument_base!(crate::pricer::InstrumentType::Swaption);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        self.validate()?;
        // The default `Instrument::value()` path only implements European exercise.
        // Bermudan / American swaptions must be priced via the dedicated LMM
        // pricer (see `swaption::lmm_pricer::LmmPricer`); silently downcasting to
        // European would systematically under-price the early-exercise premium.
        match self.exercise_style {
            SwaptionExercise::European => {}
            SwaptionExercise::Bermudan | SwaptionExercise::American => {
                return Err(Error::Validation(format!(
                    "Swaption '{}' has exercise_style={}; the generic Swaption pricer only supports \
                     European exercise. Use the LMM Bermudan pricer \
                     (crate::instruments::rates::swaption::lmm_pricer) for early-exercise swaptions.",
                    self.id,
                    self.exercise_style,
                )));
            }
        }

        // 1. SABR model (if enabled) overrides basic model choice
        if self.sabr_params.is_some() {
            return self.price_sabr(curves, as_of);
        }

        let time_to_expiry = self.time_to_expiry(as_of)?;
        let forward = self.forward_swap_rate(curves, as_of)?;
        let vol = self.resolve_volatility(curves, forward, time_to_expiry)?;

        match self.vol_model {
            VolatilityModel::Black => self.price_black(curves, vol, as_of),
            VolatilityModel::Normal => self.price_normal(curves, vol, as_of),
        }
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
        deps.add_vol_surface_id(self.vol_surface_id.as_str());
        Ok(deps)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.expiry)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.swap_start)
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
impl crate::instruments::common_impl::traits::CurveDependencies for Swaption {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.underlying_discount_curve_id().clone())
            .forward(self.underlying_forward_curve_id().clone())
            .build()
    }
}

crate::impl_empty_cashflow_provider!(
    Swaption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);
