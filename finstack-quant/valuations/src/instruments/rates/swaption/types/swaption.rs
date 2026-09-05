//! Pricing and metric helpers for interest-rate instruments.
//!
use crate::impl_instrument_base;
use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::instruments::pricing_overrides::VolSurfaceExtrapolation;
use crate::instruments::rates::irs::{
    FixedLegSpec, FloatLegSpec, FloatingLegCompounding, InterestRateSwap, PayReceive,
};
use crate::market::resolve_vol_source;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use finstack_quant_core::{Error, Result};
use finstack_quant_models::SabrModel;
use finstack_quant_models::SabrParameters;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use super::super::parameters::SwaptionParams;
use super::definitions::{
    CashSettlementMethod, SwaptionExercise, SwaptionSettlement, VolatilityModel,
};

/// Swaption instrument
///
/// # Exercise lifecycle boundary
///
/// `Instrument::value` prices the option claim through expiry. At expiry it
/// returns model-free intrinsic value; after expiry it returns zero. For
/// physical settlement, trade lifecycle infrastructure must materialize the
/// delivered [`InterestRateSwap`] from `underlying_fixed_leg`,
/// `underlying_float_leg`, `notional`, and `option_type`. This instrument does
/// not retain an exercised swap position after expiry.
#[derive(
    Clone,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    finstack_quant_valuations_macros::FinancialBuilder,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Swaption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Option type (payer or receiver swaption)
    pub option_type: OptionType,
    /// Notional amount of underlying swap
    pub notional: Money,
    /// Option expiry date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// Exercise style (European, Bermudan, American). Defaults to European.
    #[builder(default)]
    pub exercise_style: SwaptionExercise,
    /// Settlement method (physical or cash)
    pub settlement: SwaptionSettlement,
    /// Cash settlement annuity method (only used when settlement = Cash).
    ///
    /// - `CollateralizedCashPrice` (default): Actual collateral-discounted fixed-leg annuity
    /// - `ParYield`: Legacy flat-yield cash annuity
    /// - `IsdaParPar`: Legacy par-par annuity from the discount curve
    /// - `ZeroCoupon`: Single discount to swap maturity
    pub cash_settlement_method: CashSettlementMethod,
    /// Volatility model (Black or Normal)
    pub vol_model: VolatilityModel,
    /// Volatility surface ID for option pricing
    pub vol_surface_id: CurveId,
    /// Complete fixed leg of the underlying swap.
    pub underlying_fixed_leg: FixedLegSpec,
    /// Complete floating leg of the underlying swap.
    pub underlying_float_leg: FloatLegSpec,
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
    /// Optional SABR volatility model parameters
    pub sabr_params: Option<SabrParameters>,
    /// Attributes for scenario selection and grouping
    #[builder(default)]
    pub attributes: Attributes,
}

pub(super) struct VanillaSwaptionUnderlier {
    pub strike: Decimal,
    pub swap_start: Date,
    pub swap_end: Date,
    pub fixed_frequency: Tenor,
    pub float_frequency: Tenor,
    pub fixed_day_count: DayCount,
    pub float_day_count: DayCount,
    pub discount_curve_id: CurveId,
    pub forward_curve_id: CurveId,
    pub calendar_id: Option<CalendarId>,
}

impl VanillaSwaptionUnderlier {
    /// Vanilla underlier with the USD market-standard leg conventions:
    /// semi-annual 30/360 fixed leg versus quarterly ACT/360 floating leg,
    /// no holiday calendar.
    pub(super) fn standard(
        strike: Decimal,
        swap_start: Date,
        swap_end: Date,
        discount_curve_id: CurveId,
        forward_curve_id: CurveId,
    ) -> Self {
        Self {
            strike,
            swap_start,
            swap_end,
            fixed_frequency: Tenor::semi_annual(),
            float_frequency: Tenor::quarterly(),
            fixed_day_count: DayCount::Thirty360,
            float_day_count: DayCount::Act360,
            discount_curve_id,
            forward_curve_id,
            calendar_id: None,
        }
    }
}

pub(super) fn vanilla_underlier(
    underlier: VanillaSwaptionUnderlier,
) -> (FixedLegSpec, FloatLegSpec) {
    let calendar = underlier.calendar_id.as_ref().map(ToString::to_string);
    let fixed = FixedLegSpec {
        discount_curve_id: underlier.discount_curve_id.clone(),
        rate: underlier.strike,
        frequency: underlier.fixed_frequency,
        day_count: underlier.fixed_day_count,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: calendar.clone(),
        stub: StubKind::None,
        start: underlier.swap_start,
        end: underlier.swap_end,
        par_method: None,
        compounding_simple: true,
        payment_lag_days: 0,
        end_of_month: false,
    };
    let compounding =
        crate::instruments::common_impl::pricing::overnight_conventions::compounding_from_index_id(
            underlier.forward_curve_id.as_str(),
        )
        .ok()
        .flatten()
        .unwrap_or(FloatingLegCompounding::Simple);
    let float = FloatLegSpec {
        discount_curve_id: underlier.discount_curve_id,
        forward_curve_id: underlier.forward_curve_id,
        spread_bp: Decimal::ZERO,
        frequency: underlier.float_frequency,
        // Floating accrual follows the referenced index and can differ from the
        // fixed-leg convention (for example SONIA uses ACT/365F).
        day_count: underlier.float_day_count,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: calendar.clone(),
        stub: StubKind::None,
        reset_lag_days: 0,
        fixing_calendar_id: calendar,
        start: underlier.swap_start,
        end: underlier.swap_end,
        compounding,
        payment_lag_days: 0,
        end_of_month: false,
    };
    (fixed, float)
}

impl Swaption {
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

    pub(crate) fn strike_f64(&self) -> Result<f64> {
        self.get_strike().to_f64().ok_or_else(|| {
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
            self.get_swap_start(),
            "swaption expiry vs swap_start",
        )?;
        validation::validate_date_range_strict(
            self.get_swap_start(),
            self.get_swap_end(),
            "swaption swap_start vs swap_end",
        )?;

        let strike = self.strike_f64()?;
        validation::validate_f64_finite(strike, "swaption strike")?;
        validation::validate_f64_abs_le(strike, 2.0, "swaption strike", Some(" (rate)"))?;

        self.underlying_fixed_leg.validate()?;
        self.underlying_float_leg.validate()?;
        if let Some(parameters) = &self.sabr_params {
            parameters.validate()?;
        }
        if self.underlying_fixed_leg.start != self.underlying_float_leg.start
            || self.underlying_fixed_leg.end != self.underlying_float_leg.end
        {
            return Err(Error::Validation(
                "swaption fixed and floating leg spans must match".to_string(),
            ));
        }

        Ok(())
    }

    /// Create a canonical example swaption for testing and documentation.
    ///
    /// Returns a 1Y x 5Y payer swaption (1 year to expiry, 5 year swap tenor).
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        let strike = Decimal::try_from(0.03).expect("valid decimal");
        let swap_start =
            Date::from_calendar_date(2027, time::Month::January, 17).expect("Valid example date");
        let swap_end =
            Date::from_calendar_date(2032, time::Month::January, 17).expect("Valid example date");
        let discount_curve_id = CurveId::new("USD-OIS");
        let (underlying_fixed_leg, underlying_float_leg) =
            vanilla_underlier(VanillaSwaptionUnderlier::standard(
                strike,
                swap_start,
                swap_end,
                discount_curve_id,
                CurveId::new("USD-OIS"),
            ));
        Self {
            id: InstrumentId::new("SWPN-1Yx5Y-USD"),
            option_type: OptionType::Call,
            notional: Money::from((10_000_000_i64, Currency::USD)),
            expiry: Date::from_calendar_date(2027, time::Month::January, 15)
                .expect("Valid example date"),
            exercise_style: SwaptionExercise::European,
            settlement: SwaptionSettlement::Cash,
            cash_settlement_method: CashSettlementMethod::default(),
            vol_model: VolatilityModel::Black,
            vol_surface_id: CurveId::new("USD-SWPNVOL"),
            underlying_fixed_leg,
            underlying_float_leg,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
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
        let strike = Decimal::try_from(0.035).expect("valid decimal");
        let (underlying_fixed_leg, underlying_float_leg) =
            vanilla_underlier(VanillaSwaptionUnderlier {
                fixed_day_count: DayCount::Act360,
                ..VanillaSwaptionUnderlier::standard(
                    strike,
                    swap_start,
                    swap_end,
                    CurveId::new("USD-OIS"),
                    CurveId::new("USD-OIS"),
                )
            });
        Self {
            id: InstrumentId::new("SWPN-5NC1-BERM-USD"),
            option_type: OptionType::Call,
            notional: Money::from((10_000_000_i64, Currency::USD)),
            expiry: first_exercise,
            exercise_style: SwaptionExercise::Bermudan,
            settlement: SwaptionSettlement::Physical,
            cash_settlement_method: CashSettlementMethod::default(),
            vol_model: VolatilityModel::Normal,
            vol_surface_id: CurveId::new("USD-SWPNVOL"),
            underlying_fixed_leg,
            underlying_float_leg,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            sabr_params: Some(SabrParameters {
                alpha: 0.025,
                beta: 0.5,
                nu: 0.40,
                rho: -0.30,
                shift: None,
            }),
            attributes: Attributes::new(),
        }
    }

    /// Create a European swaption from a [`SwaptionParams`] specification.
    ///
    /// The payer/receiver side comes from `params.side`
    /// ([`PayReceive::Pay`] → payer, [`OptionType::Call`]; [`PayReceive::Receive`]
    /// → receiver, [`OptionType::Put`]). Leg conventions default to the USD
    /// market standard (semi-annual 30/360 fixed versus quarterly ACT/360
    /// floating) unless overridden on `params`.
    ///
    /// # Arguments
    ///
    /// * `id` - Instrument identifier.
    /// * `params` - Notional, strike, dates, side and optional leg/vol-model overrides.
    /// * `discount_curve_id` - Discount curve for both legs.
    /// * `forward_curve_id` - Projection curve for the floating leg; overnight
    ///   index ids select compounded-in-arrears accrual.
    /// * `vol_surface_id` - Swaption volatility cube used for pricing.
    pub fn new(
        id: impl Into<InstrumentId>,
        params: &SwaptionParams,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> Self {
        let (underlying_fixed_leg, underlying_float_leg) =
            vanilla_underlier(VanillaSwaptionUnderlier {
                fixed_frequency: params.fixed_frequency.unwrap_or_else(Tenor::semi_annual),
                float_frequency: params.float_frequency.unwrap_or_else(Tenor::quarterly),
                fixed_day_count: params.fixed_day_count.unwrap_or(DayCount::Thirty360),
                float_day_count: params.float_day_count.unwrap_or(DayCount::Act360),
                ..VanillaSwaptionUnderlier::standard(
                    params.strike,
                    params.swap_start,
                    params.swap_end,
                    discount_curve_id.into(),
                    forward_curve_id.into(),
                )
            });
        Self {
            id: id.into(),
            option_type: match params.side {
                PayReceive::Pay => OptionType::Call,
                PayReceive::Receive => OptionType::Put,
            },
            notional: params.notional,
            expiry: params.expiry,
            exercise_style: SwaptionExercise::European,
            settlement: SwaptionSettlement::Physical,
            cash_settlement_method: CashSettlementMethod::default(),
            vol_surface_id: vol_surface_id.into(),
            underlying_fixed_leg,
            underlying_float_leg,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            sabr_params: None,
            attributes: Attributes::default(),
            vol_model: params.vol_model.unwrap_or_default(),
        }
    }

    /// Attach SABR parameters to enable SABR-implied volatility pricing.
    pub fn with_sabr(mut self, params: SabrParameters) -> Self {
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
    /// * `calendar_id` - Calendar ID registered in `calendar_by_id`
    ///   (e.g., `"nyse"` for USD, `"target"` for EUR)
    pub fn with_calendar(mut self, calendar_id: impl Into<CalendarId>) -> Self {
        let calendar_id = calendar_id.into().to_string();
        self.underlying_fixed_leg.calendar_id = Some(calendar_id.clone());
        self.underlying_float_leg.calendar_id = Some(calendar_id.clone());
        self.underlying_float_leg.fixing_calendar_id = Some(calendar_id);
        self
    }

    fn underlying_fixed_leg_with_rate(&self, rate: Decimal) -> FixedLegSpec {
        let mut fixed = self.underlying_fixed_leg.clone();
        fixed.rate = rate;
        fixed
    }

    fn underlying_irs(&self, fixed_rate: f64, side: PayReceive) -> Result<InterestRateSwap> {
        self.underlying_irs_with_float(fixed_rate, side, self.underlying_float_leg.clone())
    }

    fn underlying_irs_with_float(
        &self,
        fixed_rate: f64,
        side: PayReceive,
        float: FloatLegSpec,
    ) -> Result<InterestRateSwap> {
        let fixed_rate = finstack_quant_core::decimal::f64_to_decimal(fixed_rate)?;
        let fixed = self.underlying_fixed_leg_with_rate(fixed_rate);
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
        super::super::contractual_swap_tenor_years(self.get_swap_start(), self.get_swap_end())
    }

    /// Set the cash settlement annuity method.
    ///
    /// Only affects pricing when `settlement` is `SwaptionSettlement::Cash`.
    ///
    /// # Example
    ///
    /// ```
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

    // Pricing Methods (moved from engine for direct access)

    /// Time to option expiry in years, measured with ACT/365F.
    ///
    /// Option expiry enters the Black/Bachelier formulas as calendar time, so
    /// it uses ACT/365F regardless of the instrument's accrual `day_count`
    /// (which still governs annuity and accrual computations). Using the
    /// accrual day count (e.g. Act360) would inflate T by ~365/360.
    fn time_to_expiry(&self, as_of: Date) -> Result<f64> {
        year_fraction(DayCount::Act365F, as_of, self.expiry)
    }

    fn validate_european_exercise(&self) -> Result<()> {
        match self.exercise_style {
            SwaptionExercise::European => Ok(()),
            SwaptionExercise::Bermudan | SwaptionExercise::American => {
                Err(Error::Validation(format!(
                    "Swaption '{}' has exercise_style={}; the generic Swaption pricer only \
                     supports European exercise. Use the LMM or Hull-White early-exercise \
                     pricer for Bermudan/American swaptions.",
                    self.id, self.exercise_style
                )))
            }
        }
    }

    /// Return the model-independent terminal value at or after expiry.
    ///
    /// Exactly at expiry this is the exercise-date intrinsic value. After
    /// expiry the option claim is zero. A physically delivered underlying swap
    /// is a separate post-exercise position and is not embedded in this value.
    ///
    /// # Arguments
    ///
    /// * `curves` - Market context used to resolve the forward swap rate and
    ///   settlement annuity exactly at expiry.
    /// * `as_of` - Valuation date compared with the contractual exercise date.
    ///
    /// # Returns
    ///
    /// `Ok(None)` before expiry, intrinsic value at expiry, or zero afterward.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported non-European exercise styles or when
    /// the forward rate or annuity required at exact expiry cannot be resolved.
    pub(crate) fn terminal_value(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<Option<Money>> {
        self.validate_european_exercise()?;

        if as_of < self.expiry {
            return Ok(None);
        }
        if as_of > self.expiry {
            return Ok(Some(Money::from((0_i64, self.notional.currency()))));
        }

        let disc = curves.get_discount(self.get_discount_curve_id().as_ref())?;
        let forward = self.forward_swap_rate(curves, as_of)?;
        let annuity = self.annuity(disc.as_ref(), as_of, forward)?;
        let strike = self.strike_f64()?;
        let intrinsic = match self.option_type {
            OptionType::Call => (forward - strike).max(0.0),
            OptionType::Put => (strike - forward).max(0.0),
        };

        Ok(Some(Money::new(
            intrinsic * annuity * self.notional.amount(),
            self.notional.currency(),
        )?))
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
        if let Some(value) = self.terminal_value(curves, as_of)? {
            return Ok(value);
        }

        let time_to_expiry = self.time_to_expiry(as_of)?;
        let disc = curves.get_discount(self.get_discount_curve_id().as_ref())?;
        let forward_rate = self.forward_swap_rate(curves, as_of)?;
        let annuity = self.annuity(disc.as_ref(), as_of, forward_rate)?;
        let strike = self.strike_f64()?;

        let value = model_fn(forward_rate, strike, volatility, time_to_expiry, annuity);

        Money::new(value * self.notional.amount(), self.notional.currency())
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
            // Delegate to the shared base path: 0 past expiry, model-free
            // intrinsic at the expiry instant (the closure is never invoked).
            return self.price_model_base(curves, volatility, as_of, |_, _, _, _, _| 0.0);
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
            use finstack_quant_models::closed_form::{black_call, black_put};
            // A non-finite vol is treated as degenerate (intrinsic), matching
            // the `vol <= 0` limit the closed forms already return.
            let vol = if vol.is_finite() { vol } else { 0.0 };
            let unit = match self.option_type {
                OptionType::Call => black_call(fwd, strike, vol, t),
                OptionType::Put => black_put(fwd, strike, vol, t),
            };
            unit * annuity
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
            use finstack_quant_models::volatility::normal::bachelier_price;
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
    /// via [`SabrParameters::new_with_shift`].
    ///
    /// # References
    ///
    /// - Hagan, P. et al. (2002). "Managing Smile Risk" *Wilmott Magazine* `docs/REFERENCES.md#hagan-2002-sabr`
    /// - Antonov, A. et al. (2015). "SABR/Free Sabr" for normal vol extensions `docs/REFERENCES.md#hagan-2002-sabr`
    pub fn price_sabr(&self, curves: &MarketContext, as_of: Date) -> Result<Money> {
        use super::lognormal_to_normal_vol;

        if let Some(value) = self.terminal_value(curves, as_of)? {
            return Ok(value);
        }

        let params = self
            .sabr_params
            .as_ref()
            .ok_or_else(|| Error::internal("swaption SABR pricing requires sabr_params"))?;
        let model = SabrModel::new(params.clone());
        let time_to_expiry = self.time_to_expiry(as_of)?;
        let forward_rate = self.forward_swap_rate(curves, as_of)?;
        let strike = self.strike_f64()?;

        // SABR output convention is β-dependent: lognormal (Black) vol for
        // β>0, normal (Bachelier) vol for β≈0. Branch on the tag instead of
        // assuming Black — converting a Bachelier vol as if it were lognormal
        // silently misprices by orders of magnitude in rate space.
        let (sabr_vol, sabr_vol_type) =
            model.implied_volatility_with_type(forward_rate, strike, time_to_expiry)?;

        use finstack_quant_models::volatility::sabr::SabrVolType;
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
    ///   - `CollateralizedCashPrice`: Actual collateral-discounted fixed-leg annuity
    ///   - `ParYield`: Legacy closed-form flat-yield annuity
    ///   - `IsdaParPar`: Legacy par-par annuity from the discount curve
    ///   - `ZeroCoupon`: Single discount to swap maturity
    ///
    /// # Arguments
    ///
    /// * `disc` - Discounting callback or curve used to present-value path cashflows
    /// * `as_of` - Valuation or observation date that anchors discounting and schedule logic
    /// * `forward_rate` - Swap rate in decimal used only for `ParYield` cash settlement
    ///   (the closed-form flat-yield annuity). Ignored for physical settlement and other
    ///   cash methods.
    pub fn annuity(&self, disc: &dyn Discounting, as_of: Date, forward_rate: f64) -> Result<f64> {
        match self.settlement {
            SwaptionSettlement::Physical => self.swap_annuity(disc, as_of),
            SwaptionSettlement::Cash => match self.cash_settlement_method {
                CashSettlementMethod::CollateralizedCashPrice
                | CashSettlementMethod::IsdaParPar => self.swap_annuity(disc, as_of),
                CashSettlementMethod::ParYield => {
                    use crate::instruments::common_impl::pricing::time::relative_df_discounting;
                    let df = relative_df_discounting(disc, as_of, self.expiry)?;
                    Ok(self.cash_annuity_par_yield(forward_rate)? * df)
                }
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
        let periods = self.fixed_schedule_periods()?;
        crate::instruments::rates::irs::cashflow::fixed_leg_annuity(&periods, disc, as_of, as_of)
    }

    /// Accrual periods of the underlying fixed leg.
    fn fixed_schedule_periods(
        &self,
    ) -> Result<Vec<crate::cashflow::builder::periods::SchedulePeriod>> {
        let fixed = &self.underlying_fixed_leg;
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
    /// This is a legacy approximation. Current confirmations that specify
    /// collateralized cash price should use
    /// [`CashSettlementMethod::CollateralizedCashPrice`].
    ///
    /// # Edge Cases
    ///
    /// When `forward_rate ≈ 0`, uses L'Hôpital's limit: `A → N/m` (sum of accruals).
    /// Payments per year follow [`Tenor::payments_per_year`] (day tenors on a
    /// 365-day year).
    pub fn cash_annuity_par_yield(&self, forward_rate: f64) -> Result<f64> {
        let tenor_years = year_fraction(
            self.get_day_count(),
            self.get_swap_start(),
            self.get_swap_end(),
        )?;
        Ok(crate::instruments::rates::cms_common::par_annuity(
            forward_rate,
            tenor_years,
            self.get_fixed_frequency().payments_per_year(),
        ))
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

        let tenor = year_fraction(
            self.get_day_count(),
            self.get_swap_start(),
            self.get_swap_end(),
        )?;
        let df = relative_df_discounting(disc, as_of, self.get_swap_end())?;
        Ok(tenor * df)
    }

    /// Whether the discount-factor telescoping shortcut exactly reproduces the
    /// configured underlying swap.
    fn can_use_single_curve_forward_shortcut(&self, as_of: Date) -> bool {
        let fixed = &self.underlying_fixed_leg;
        let float = &self.underlying_float_leg;
        let supported_compounding = matches!(
            float.compounding,
            FloatingLegCompounding::Simple
                | FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }
        );

        as_of <= fixed.start
            && float.forward_curve_id == fixed.discount_curve_id
            && float.spread_bp.is_zero()
            && fixed.payment_lag_days == 0
            && float.payment_lag_days == 0
            && fixed.frequency == float.frequency
            && fixed.day_count == float.day_count
            && fixed.business_day_convention == float.business_day_convention
            && fixed.calendar_id == float.calendar_id
            && fixed.stub == float.stub
            && fixed.end_of_month == float.end_of_month
            && supported_compounding
    }

    /// Forward par swap rate implied by float-leg PV and fixed-leg annuity.
    ///
    /// The single-curve telescoping shortcut is used only when both underlying
    /// schedules and coupon conventions are identical. Otherwise this method
    /// prices the actual floating leg through the IRS engine.
    pub fn forward_swap_rate(&self, curves: &MarketContext, as_of: Date) -> Result<f64> {
        let disc = curves.get_discount(self.get_discount_curve_id().as_ref())?;
        if self.can_use_single_curve_forward_shortcut(as_of) {
            return self.single_curve_forward_from_fixed_schedule(disc.as_ref(), as_of);
        }

        let annuity = self.swap_annuity(disc.as_ref(), as_of)?;
        if annuity.abs() < 1e-10 {
            return Ok(0.0);
        }
        let float = &self.underlying_float_leg;
        if as_of <= float.start
            && float.forward_curve_id == self.underlying_fixed_leg.discount_curve_id
            && matches!(float.compounding, FloatingLegCompounding::Simple)
            && curves.get_forward(float.forward_curve_id.as_ref()).is_err()
        {
            return self.single_curve_forward_from_float_schedule(disc.as_ref(), as_of, annuity);
        }

        let underlier = self.underlying_irs(0.0, PayReceive::Receive)?;
        let pv_float = underlier.pv_float_leg(curves, as_of)?;
        Ok(pv_float / (self.notional.amount() * annuity))
    }

    fn single_curve_forward_from_fixed_schedule(
        &self,
        disc: &dyn Discounting,
        as_of: Date,
    ) -> Result<f64> {
        let periods = self.fixed_schedule_periods()?;
        let (float_pv, annuity) = single_curve_float_leg_pv(disc, as_of, &periods, 0.0)?;
        if annuity.abs() < 1e-10 {
            return Ok(0.0);
        }
        Ok(float_pv / annuity)
    }

    fn single_curve_forward_from_float_schedule(
        &self,
        disc: &dyn Discounting,
        as_of: Date,
        fixed_annuity: f64,
    ) -> Result<f64> {
        use crate::cashflow::builder::periods::{build_periods, BuildPeriodsParams};
        use crate::instruments::common_impl::numeric::decimal_to_f64;

        let float = &self.underlying_float_leg;
        let periods = build_periods(BuildPeriodsParams {
            start: float.start,
            end: float.end,
            frequency: float.frequency,
            stub: float.stub,
            business_day_convention: float.business_day_convention,
            calendar_id: float
                .calendar_id
                .as_deref()
                .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
            end_of_month: float.end_of_month,
            day_count: float.day_count,
            payment_lag_days: float.payment_lag_days,
            reset_lag_days: Some(float.reset_lag_days),
            adjust_accrual_dates: false,
            roll_rule: crate::cashflow::builder::specs::RollRule::None,
        })?;
        let spread = decimal_to_f64(float.spread_bp, "swaption underlier spread_bp")? / 10_000.0;
        let (float_pv, _) = single_curve_float_leg_pv(disc, as_of, &periods, spread)?;
        Ok(float_pv / fixed_annuity)
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
            let model = SabrModel::new(sabr.clone());
            return model.implied_volatility(forward, self.strike_f64()?, time_to_expiry);
        }

        // 2. Pricing override
        if let Some(impl_vol) = self
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility
        {
            return Ok(impl_vol);
        }

        // 3. Volatility provider. Strike surfaces use the strike coordinate;
        // tenor surfaces and SABR cubes use the underlying swap tenor.
        let vol_source = resolve_vol_source(curves, self.vol_surface_id.as_str())?;
        let strike = self.strike_f64()?;
        let underlying_tenor = self.underlying_tenor_years()?;
        match self
            .instrument_pricing_overrides
            .model_config
            .vol_surface_extrapolation
        {
            VolSurfaceExtrapolation::Clamp | VolSurfaceExtrapolation::LinearInVariance => {
                // LinearInVariance falls back to Clamp until surface impl is ready
                Ok(vol_source.get_vol_clamped(time_to_expiry, underlying_tenor, strike))
            }
            VolSurfaceExtrapolation::Error => {
                vol_source.get_vol(time_to_expiry, underlying_tenor, strike)
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
        if as_of >= self.expiry {
            return Ok(None);
        }
        let disc = curves.get_discount(self.get_discount_curve_id().as_ref())?;
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

/// Single-curve floating-leg PV per unit notional and the fixed-style annuity
/// over the same periods, from discount-factor telescoping:
/// `Σ (DF(start)/DF(end) − 1 + spread·τ) · DF(pay)` and `Σ τ · DF(pay)`.
///
/// # Arguments
///
/// * `disc` - Discount curve used for both projection and discounting.
/// * `as_of` - Valuation date.
/// * `periods` - Accrual periods; those paid on or before `as_of` or with a
///   zero accrual fraction are skipped.
/// * `spread` - Floating spread as a decimal added to each period's forward.
fn single_curve_float_leg_pv(
    disc: &dyn Discounting,
    as_of: Date,
    periods: &[crate::cashflow::builder::periods::SchedulePeriod],
    spread: f64,
) -> Result<(f64, f64)> {
    use crate::instruments::common_impl::pricing::time::relative_df_discounting;
    use finstack_quant_core::math::NeumaierAccumulator;

    let mut float_leg = NeumaierAccumulator::new();
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
        float_leg.add((df_start / df_end - 1.0 + spread * tau) * df_pay);
        annuity.add(tau * df_pay);
    }
    Ok((float_leg.total(), annuity.total()))
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

    fn default_model(&self) -> crate::pricer::ModelKey {
        match self.vol_model {
            VolatilityModel::Black => crate::pricer::ModelKey::Black76,
            VolatilityModel::Normal => crate::pricer::ModelKey::Normal,
        }
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        self.validate()?;
        if let Some(value) = self.terminal_value(curves, as_of)? {
            return Ok(value);
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

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.expiry)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.get_swap_start())
    }

    crate::impl_focused_pricing_overrides!();
}

// Declare canonical market dependencies for the DV01 calculator.
crate::impl_empty_cashflow_provider!(
    Swaption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);
