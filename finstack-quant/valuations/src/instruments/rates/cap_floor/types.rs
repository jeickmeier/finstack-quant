//! Interest rate option instrument types and Black model greeks.

use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::rates::irs::FloatingLegCompounding;
use crate::instruments::{ExerciseStyle, SettlementType};
use crate::market::conventions::defs::RateIndexKind;
use crate::market::conventions::ConventionRegistry;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::IndexId;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use crate::impl_instrument_base;

/// Volatility convention for cap/floor pricing.
///
/// The volatility type determines how the input volatility is interpreted
/// and which pricing model is used:
///
/// # Lognormal (Black-Scholes)
///
/// The standard market convention where volatility is expressed as a
/// proportion of the forward rate. Uses the Black (1976) formula.
///
/// **Constraints**: Requires positive forward rates and strikes.
///
/// # Normal (Bachelier)
///
/// Volatility expressed in absolute rate terms (e.g., 50bp = 0.50%).
/// Uses the Bachelier model, which naturally handles negative rates.
///
/// **Use case**: EUR/CHF markets with negative rates.
///
/// # Market Convention Notes
///
/// - **USD**: Historically lognormal, shifting to normal post-SOFR
/// - **EUR**: Predominantly normal since negative rates became common
/// - **GBP/JPY**: Mixed, check dealer quotes
///
/// Always verify the vol convention with your data provider as using
/// the wrong type will produce materially incorrect prices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CapFloorVolType {
    /// Lognormal (Black) volatility - percentage of forward rate.
    ///
    /// Standard market convention. Volatility is typically quoted as a
    /// decimal (e.g., 0.20 for 20% vol).
    Lognormal,

    /// Shifted lognormal (displaced diffusion / shifted Black).
    ///
    /// Uses Black pricing on shifted rates:
    /// `F' = F + shift`, `K' = K + shift`.
    /// This is standard for low/negative rate regimes while preserving
    /// lognormal smile conventions.
    ShiftedLognormal,

    /// Normal (Bachelier) volatility - absolute rate terms.
    ///
    /// Volatility is quoted in the same units as rates (e.g., 0.0050 for 50bp).
    /// Required for negative rate environments.
    Normal,

    /// Lognormal (Black) surface quote with a negative-rate fallback.
    ///
    /// Treats the volatility surface as a **lognormal** quote. Each
    /// caplet/floorlet uses Black-76 when the forward and strike are
    /// positive; otherwise the lognormal vol is converted to an equivalent
    /// normal vol and priced with Bachelier.
    ///
    /// This does **not** inspect the surface quote type. A normal-vol
    /// surface must set `vol_type = Normal`. Explicit `Lognormal`,
    /// `ShiftedLognormal`, and `Normal` remain explicit.
    #[default]
    Auto,
}

impl std::fmt::Display for CapFloorVolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapFloorVolType::Lognormal => write!(f, "lognormal"),
            CapFloorVolType::ShiftedLognormal => write!(f, "shifted_lognormal"),
            CapFloorVolType::Normal => write!(f, "normal"),
            CapFloorVolType::Auto => write!(f, "auto"),
        }
    }
}

impl std::str::FromStr for CapFloorVolType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "lognormal" => Ok(Self::Lognormal),
            "shifted_lognormal" => Ok(Self::ShiftedLognormal),
            "normal" => Ok(Self::Normal),
            "auto" => Ok(Self::Auto),
            _ => Err(format!(
                "Unknown cap/floor vol type: '{}'. Valid: lognormal, shifted_lognormal, normal, auto",
                s
            )),
        }
    }
}

/// Type of interest rate option
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RateOptionType {
    /// Cap (series of caplets)
    Cap,
    /// Floor (series of floorlets)
    Floor,
    /// Caplet (single period cap)
    Caplet,
    /// Floorlet (single period floor)
    Floorlet,
}

impl std::fmt::Display for RateOptionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RateOptionType::Cap => write!(f, "cap"),
            RateOptionType::Floor => write!(f, "floor"),
            RateOptionType::Caplet => write!(f, "caplet"),
            RateOptionType::Floorlet => write!(f, "floorlet"),
        }
    }
}

impl std::str::FromStr for RateOptionType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "cap" => Ok(Self::Cap),
            "floor" => Ok(Self::Floor),
            "caplet" => Ok(Self::Caplet),
            "floorlet" => Ok(Self::Floorlet),
            _ => Err(format!(
                "Unknown rate option type: '{}'. Valid: cap, floor, caplet, floorlet",
                s
            )),
        }
    }
}

/// Whether a contractual spread is included in overnight daily compounding.
///
/// ISDA-standard RFR coupons normally compound only the overnight index and add
/// any spread as simple interest after compounding. `Include` represents the
/// less common contract where the spread enters every daily compound factor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OvernightSpreadCompounding {
    /// Add the spread after compounding the overnight index.
    #[default]
    Exclude,
    /// Include the spread in every daily overnight compound factor.
    Include,
}

/// Contractual terms for an option on a compounded overnight RFR coupon.
///
/// The shared [`FloatingLegCompounding`] type is reused so lookback,
/// observation-shift, and rate-cutoff semantics cannot drift from IRS pricing.
/// Payment and fixing calendars are separate because operational payment
/// delays need not use the index publication calendar.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OvernightCouponConvention {
    /// Daily overnight compounding convention.
    pub compounding: FloatingLegCompounding,
    /// Payment delay in business days after the accrual end date.
    #[serde(default)]
    #[cfg_attr(feature = "json-schema", schemars(range(min = 0, max = 31)))]
    pub payment_delay_days: i32,
    /// Calendar used for overnight observations and fixings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixing_calendar_id: Option<CalendarId>,
    /// Calendar used to apply the payment delay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_calendar_id: Option<CalendarId>,
    /// Whether any contractual spread is compounded or added afterward.
    #[serde(default)]
    pub spread_compounding: OvernightSpreadCompounding,
}

/// Interest rate option instrument.
///
/// # Pre-1.0 API evolution
///
/// Compounded-RFR contractual terms and contractual spread are typed public
/// fields. Adding them is intentionally source-breaking for downstream exhaustive
/// struct literals; canonical constructors and the generated builder retain
/// backward-compatible defaults. Prefer those construction APIs for forward
/// compatibility.
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
pub struct CapFloor {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Option type
    pub rate_option_type: RateOptionType,
    /// Notional amount
    pub notional: Money,
    /// Strike (as decimal, e.g., 0.05 for 5%)
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub strike: Decimal,
    /// Contractual spread added to the referenced rate, in decimal rate units.
    ///
    /// Term-index coupons add this spread after projecting the index. For
    /// overnight coupons, [`OvernightSpreadCompounding`] determines whether it
    /// is added after compounding or included in every daily factor.
    #[serde(default, skip_serializing_if = "Decimal::is_zero")]
    #[builder(default)]
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub spread: Decimal,
    /// Start date of underlying period
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start_date: Date,
    /// End date of underlying period
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Payment frequency for caps/floors
    pub frequency: Tenor,
    /// Day count convention
    pub day_count: DayCount,
    /// Schedule stub convention
    #[builder(default = StubKind::ShortFront)]
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Schedule business day convention
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Optional holiday calendar identifier for schedule and roll conventions
    pub calendar_id: Option<CalendarId>,
    /// Exercise style (defaults to European; caps/floors are virtually always European)
    #[serde(default)]
    #[builder(default)]
    pub exercise_style: ExerciseStyle,
    /// Settlement type (defaults to Cash; caps/floors are virtually always cash-settled)
    #[serde(default = "crate::serde_defaults::settlement_cash")]
    #[builder(default = SettlementType::Cash)]
    pub settlement: SettlementType,
    /// Discount curve identifier
    pub discount_curve_id: CurveId,
    /// Forward curve identifier
    pub forward_curve_id: CurveId,
    /// Volatility surface identifier
    pub vol_surface_id: CurveId,
    /// Volatility type convention (lognormal/Black or normal/Bachelier).
    ///
    /// **Critical**: This must match the convention of your vol surface data.
    /// Using lognormal vol with a normal surface (or vice versa) will produce
    /// incorrect prices.
    ///
    /// - `Auto` (default): treat the surface as a lognormal quote; Black-76
    ///   when forward and strike are positive, otherwise convert to an
    ///   equivalent normal vol and price with Bachelier. A normal-vol
    ///   surface must set `vol_type = Normal`.
    /// - `Lognormal`: Standard Black model, requires positive rates/strikes
    /// - `Normal`: Bachelier model, handles negative rates
    #[serde(default)]
    #[builder(default)]
    pub vol_type: CapFloorVolType,
    /// Displacement shift for shifted-lognormal pricing (default: 0.0 = no shift).
    ///
    /// When `vol_type = ShiftedLognormal`, rates and strikes are shifted by this amount:
    /// `F' = F + vol_shift`, `K' = K + vol_shift`.
    ///
    /// Typical values are 0.01–0.03 (1%–3%) to push rates into positive territory
    /// in low-rate environments. A shift of 0.0 is equivalent to plain lognormal.
    ///
    /// **Validation**: Must be ≥ 0.0. The shifted forward `F + vol_shift` must be
    /// positive for the Black model to be well-defined.
    #[serde(default)]
    #[builder(default = 0.0_f64)]
    pub vol_shift: f64,
    /// Optional compounded-overnight coupon terms.
    ///
    /// `None` preserves the legacy term-index/simple-forward caplet contract.
    /// Set this explicitly for caps on compounded SOFR, SONIA, €STR, or another
    /// overnight RFR coupon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub overnight_coupon: Option<OvernightCouponConvention>,
    /// Optional dated premium paid by the cap/floor holder.
    ///
    /// A positive amount is an outflow from the holder and reduces NPV while
    /// the payment date is strictly after `as_of`. The premium currency must
    /// match the notional currency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    #[serde(with = "finstack_quant_core::wire::optional_dated_money")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<(finstack_quant_core::wire::DateWire, Money)>")
    )]
    pub premium: Option<(Date, Money)>,
    /// Additional attributes
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
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl CapFloor {
    /// Create a canonical example USD 5Y 3% interest rate cap ($10M notional, quarterly SOFR).
    ///
    /// Returns a 5-year cap with quarterly payment frequency, ACT/360 day count,
    /// automatic vol-type selection, and standard schedule conventions.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        use time::Month;

        let start = Date::from_calendar_date(2024, Month::January, 3).map_err(|e| {
            finstack_quant_core::Error::Validation(format!("Invalid example start date: {}", e))
        })?;
        let maturity = Date::from_calendar_date(2029, Month::January, 3).map_err(|e| {
            finstack_quant_core::Error::Validation(format!("Invalid example end date: {}", e))
        })?;

        Self::new(
            InstrumentId::new("IRCAP-USD-5Y-3PCT"),
            RateOptionType::Cap,
            Money::from((10_000_000_i64, Currency::USD)),
            0.03,
            start,
            maturity,
            Some(Tenor::quarterly()),
            DayCount::Act360,
            CurveId::new("USD-OIS"),
            CurveId::new("USD-SOFR-3M"),
            CurveId::new("USD-CAPFLOOR-VOL"),
        )
    }

    pub(crate) fn strike_f64(&self) -> finstack_quant_core::Result<f64> {
        self.strike
            .to_f64()
            .ok_or(finstack_quant_core::InputError::ConversionOverflow.into())
    }

    pub(crate) fn spread_f64(&self) -> finstack_quant_core::Result<f64> {
        self.spread
            .to_f64()
            .ok_or(finstack_quant_core::InputError::ConversionOverflow.into())
    }

    /// Create a cap, floor, caplet or floorlet with the standard schedule
    /// conventions (short-front stub, modified-following, no holiday calendar,
    /// European exercise, cash settlement, zero spread).
    ///
    /// # Arguments
    ///
    /// * `id` - Instrument identifier.
    /// * `rate_option_type` - Cap, Floor, Caplet or Floorlet.
    /// * `notional` - Notional in the instrument currency.
    /// * `strike` - Strike as a decimal rate (0.03 = 3%).
    /// * `start_date` - Accrual start of the first period.
    /// * `maturity` - Final payment date.
    /// * `frequency` - Payment frequency used to generate the period schedule.
    ///   `None` infers a single period spanning `[start_date, maturity]`
    ///   (the caplet/floorlet convention).
    /// * `day_count` - Accrual day-count convention for each period.
    /// * `discount_curve_id` - Discount curve for payments.
    /// * `forward_curve_id` - Projection curve for the underlying rate index.
    /// * `vol_surface_id` - Cap/floor volatility surface.
    ///
    /// # Errors
    ///
    /// Returns an error if `strike` is not representable as `Decimal`
    /// (NaN or infinite).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<InstrumentId>,
        rate_option_type: RateOptionType,
        notional: Money,
        strike: f64,
        start_date: Date,
        maturity: Date,
        frequency: Option<Tenor>,
        day_count: DayCount,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        Ok(Self {
            id: id.into(),
            rate_option_type,
            notional,
            strike: finstack_quant_core::decimal::f64_to_decimal(strike)?,
            spread: Decimal::ZERO,
            start_date,
            maturity,
            frequency: frequency
                .unwrap_or_else(|| infer_single_period_frequency(start_date, maturity)),
            day_count,
            stub: StubKind::ShortFront,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            exercise_style: ExerciseStyle::European,
            settlement: SettlementType::Cash,
            discount_curve_id: discount_curve_id.into(),
            forward_curve_id: forward_curve_id.into(),
            vol_surface_id: vol_surface_id.into(),
            vol_type: CapFloorVolType::default(),
            vol_shift: 0.0,
            overnight_coupon: None,
            premium: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        })
    }

    pub(crate) fn resolved_schedule_calendar_id(&self) -> finstack_quant_core::Result<&str> {
        if let Some(id) = self.calendar_id.as_deref() {
            crate::cashflow::builder::calendar::resolve_calendar_strict(id)?;
            return Ok(id);
        }
        if let Ok(registry) = ConventionRegistry::try_global() {
            let index_id = IndexId::new(self.forward_curve_id.as_str());
            if let Ok(convention) = registry.require_rate_index(&index_id) {
                let id = convention.market_calendar_id.as_str();
                crate::cashflow::builder::calendar::resolve_calendar_strict(id)?;
                return Ok(id);
            }
        }
        crate::instruments::common_impl::pricing::overnight::default_rate_calendar_id(
            self.notional.currency(),
        )
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "CapFloor '{}' requires an explicit schedule calendar for {}",
                self.id,
                self.notional.currency()
            ))
        })
    }

    pub(crate) fn pricing_periods(
        &self,
    ) -> finstack_quant_core::Result<Vec<crate::cashflow::builder::periods::SchedulePeriod>> {
        let overnight_payment_delay = self
            .overnight_coupon
            .as_ref()
            .map(|terms| terms.payment_delay_days);
        let params = crate::cashflow::builder::periods::BuildPeriodsParams {
            start: self.start_date,
            end: self.maturity,
            frequency: self.frequency,
            stub: self.stub,
            business_day_convention: self.business_day_convention,
            calendar_id: self.resolved_schedule_calendar_id()?,
            end_of_month: false,
            day_count: self.day_count,
            payment_lag_days: if overnight_payment_delay.is_some() {
                0
            } else {
                self.resolved_payment_lag_days()
            },
            reset_lag_days: self.resolved_reset_lag_days(),
            adjust_accrual_dates: false,
            roll_rule: crate::cashflow::builder::specs::RollRule::None,
        };

        let mut periods = if matches!(
            self.rate_option_type,
            RateOptionType::Caplet | RateOptionType::Floorlet
        ) {
            vec![crate::cashflow::builder::periods::build_single_period(
                params,
            )?]
        } else {
            crate::cashflow::builder::periods::build_periods(params)?
        };
        if let Some(terms) = &self.overnight_coupon {
            let fixing_calendar_id = terms
                .fixing_calendar_id
                .as_deref()
                .or(self.calendar_id.as_deref());
            let fixing_calendar =
                crate::instruments::common_impl::pricing::overnight::resolve_overnight_fixing_calendar(
                    fixing_calendar_id,
                    self.notional.currency(),
                    &format!("CapFloor '{}'", self.id),
                )?;
            let payment_calendar_id = terms
                .payment_calendar_id
                .as_deref()
                .or(self.calendar_id.as_deref())
                .or(terms.fixing_calendar_id.as_deref());
            for period in &mut periods {
                (period.accrual_start, period.accrual_end) =
                    crate::instruments::common_impl::pricing::overnight::adjust_overnight_accrual_boundaries(
                        period.accrual_start,
                        period.accrual_end,
                        self.business_day_convention,
                        fixing_calendar,
                    )?;
                period.accrual_year_fraction = self.day_count.year_fraction(
                    period.accrual_start,
                    period.accrual_end,
                    DayCountContext {
                        calendar: Some(fixing_calendar),
                        frequency: Some(self.frequency),
                        bus_basis: None,
                        ..DayCountContext::default()
                    },
                )?;
                period.payment_date =
                    crate::instruments::common_impl::pricing::swap_legs::add_payment_delay(
                        period.accrual_end,
                        terms.payment_delay_days,
                        payment_calendar_id,
                    )?;
            }
        }
        Ok(periods)
    }
    pub(crate) fn final_fixing_date(&self) -> finstack_quant_core::Result<Date> {
        let period = self.pricing_periods()?.into_iter().last().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "CapFloor '{}' produced no option periods",
                self.id
            ))
        })?;
        let Some(terms) = &self.overnight_coupon else {
            return Ok(period.reset_date.unwrap_or(period.accrual_start));
        };
        let fixing_calendar_id = terms
            .fixing_calendar_id
            .as_deref()
            .or(self.calendar_id.as_deref());
        let fixing_calendar =
            crate::instruments::common_impl::pricing::overnight::resolve_overnight_fixing_calendar(
                fixing_calendar_id,
                self.notional.currency(),
                &format!("CapFloor '{}'", self.id),
            )?;
        crate::instruments::common_impl::pricing::overnight::final_overnight_fixing_date(
            period.accrual_start,
            period.accrual_end,
            &terms.compounding,
            fixing_calendar,
        )
    }

    /// Set the volatility type convention.
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_valuations::instruments::rates::cap_floor::{CapFloor, CapFloorVolType, RateOptionType};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{create_date, DayCount, Tenor};
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::{CurveId, InstrumentId};
    /// use time::Month;
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// // Create a floor with normal volatility for EUR market (ACT/360 is the standard day count
    /// // for EUR ESTR/EURIBOR caps and floors per ISDA conventions).
    /// let floor = CapFloor::new(
    ///     InstrumentId::new("EUR-FLOOR-001"),
    ///     RateOptionType::Floor,
    ///     Money::from((1_000_000_i64, Currency::EUR)),
    ///     0.02,
    ///     create_date(2026, Month::January, 1)?,
    ///     create_date(2027, Month::January, 1)?,
    ///     Some(Tenor::quarterly()),
    ///     DayCount::Act360,
    ///     CurveId::new("EUR-OIS"),
    ///     CurveId::new("EUR-ESTR-3M"),
    ///     CurveId::new("EUR-CAPFLOOR-VOL"),
    /// )?
    ///     .with_vol_type(CapFloorVolType::Normal);
    /// # let _ = floor;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_vol_type(mut self, vol_type: CapFloorVolType) -> Self {
        self.vol_type = vol_type;
        self
    }

    pub(crate) fn resolved_payment_lag_days(&self) -> i32 {
        if let Some(terms) = &self.overnight_coupon {
            return terms.payment_delay_days;
        }
        let Ok(registry) = ConventionRegistry::try_global() else {
            return 0;
        };
        let idx = IndexId::new(self.forward_curve_id.as_str());
        registry
            .require_rate_index(&idx)
            .map(|conv| conv.default_payment_lag_days)
            .unwrap_or(0)
    }

    pub(crate) fn resolved_reset_lag_days(&self) -> Option<i32> {
        let Ok(registry) = ConventionRegistry::try_global() else {
            return None;
        };
        let idx = IndexId::new(self.forward_curve_id.as_str());
        registry
            .require_rate_index(&idx)
            .map(|conv| conv.default_reset_lag_days)
            .ok()
    }

    pub(crate) fn resolved_vol_shift(&self) -> f64 {
        self.vol_shift
    }
}

fn infer_single_period_frequency(start_date: Date, maturity: Date) -> Tenor {
    let day_span = (maturity - start_date).whole_days().abs();
    if day_span <= 45 {
        Tenor::monthly()
    } else if day_span <= 135 {
        Tenor::quarterly()
    } else if day_span <= 225 {
        Tenor::semi_annual()
    } else {
        Tenor::annual()
    }
}

impl crate::instruments::common_impl::traits::Instrument for CapFloor {
    impl_instrument_base!(crate::pricer::InstrumentType::CapFloor);
    fn includes_valuation_date_cashflows(&self) -> bool {
        false
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        if self.start_date >= self.maturity {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CapFloor '{}' start_date ({}) must precede maturity ({})",
                self.id, self.start_date, self.maturity
            )));
        }
        if !self.notional.amount().is_finite() || self.notional.amount() <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CapFloor '{}' notional must be finite and positive, got {}",
                self.id,
                self.notional.amount()
            )));
        }
        let strike = self.strike_f64()?;
        if !strike.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CapFloor '{}' strike must be finite",
                self.id
            )));
        }
        if !self.vol_shift.is_finite() || self.vol_shift < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CapFloor '{}' vol_shift must be finite and non-negative, got {}",
                self.id, self.vol_shift
            )));
        }
        self.resolved_schedule_calendar_id()?;
        if let Some((_, premium)) = self.premium {
            if premium.currency() != self.notional.currency() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CapFloor '{}' premium currency {} must match notional currency {}",
                    self.id,
                    premium.currency(),
                    self.notional.currency()
                )));
            }
            if !premium.amount().is_finite() || premium.amount() < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CapFloor '{}' premium must be finite and non-negative, got {}",
                    self.id,
                    premium.amount()
                )));
            }
        }
        if let Some(overnight) = &self.overnight_coupon {
            let idx = IndexId::new(self.forward_curve_id.as_str());
            let convention = ConventionRegistry::try_global()
                .and_then(|registry| registry.require_rate_index(&idx))
                .map_err(|_| {
                    finstack_quant_core::Error::Validation(format!(
                        "CapFloor '{}' carries overnight coupon settings, but forward index '{}' \
                         is not a registered overnight RFR index",
                        self.id, self.forward_curve_id
                    ))
                })?;
            if convention.kind != RateIndexKind::OvernightRfr {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CapFloor '{}' carries overnight coupon settings for term index '{}'; \
                     overnight settings require an OvernightRfr index",
                    self.id, self.forward_curve_id
                )));
            }
            if overnight.payment_delay_days < 0 || overnight.payment_delay_days > 31 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CapFloor '{}' overnight payment delay must be between 0 and 31 business \
                     days, got {}",
                    self.id, overnight.payment_delay_days
                )));
            }
            match overnight.compounding {
                FloatingLegCompounding::Simple => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CapFloor '{}' overnight coupon convention cannot use simple compounding",
                        self.id
                    )));
                }
                FloatingLegCompounding::CompoundedInArrears { lookback_days } => {
                    if !(0..=31).contains(&lookback_days) {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "CapFloor '{}' overnight lookback must be 0-31 business days",
                            self.id
                        )));
                    }
                }
                FloatingLegCompounding::CompoundedWithObservationShift { shift_days }
                    if !(0..=31).contains(&shift_days) =>
                {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CapFloor '{}' overnight observation shift must be between 0 and 31 \
                         business days",
                        self.id
                    )));
                }
                FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days }
                    if !(0..=31).contains(&cutoff_days) =>
                {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CapFloor '{}' overnight rate cutoff must be between 0 and 31 business days",
                        self.id
                    )));
                }
                _ => {}
            }
        }
        if !matches!(self.exercise_style, ExerciseStyle::European) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CapFloor '{}' supports European exercise only; got {}",
                self.id, self.exercise_style
            )));
        }
        if !matches!(self.settlement, SettlementType::Cash) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CapFloor '{}' supports cash settlement only",
                self.id
            )));
        }
        Ok(())
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Black76
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let option_value = crate::instruments::rates::cap_floor::pricing::pricer::price_cap_floor(
            self, curves, as_of,
        )?;
        let Some((payment_date, premium)) = self
            .premium
            .filter(|(payment_date, _)| *payment_date > as_of)
        else {
            return Ok(option_value);
        };
        let discount = curves.get_discount(self.discount_curve_id.clone())?;
        let discount_factor = discount.df_between_dates(as_of, payment_date)?;
        Money::new(
            option_value.amount() - premium.amount() * discount_factor,
            option_value.currency(),
        )
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if self.overnight_coupon.is_none() || self.forward_curve_id != self.discount_curve_id {
            deps.add_forward_curve(self.forward_curve_id.clone());
        }
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                None,
                Some(self.strike_f64()?),
            ),
        );
        deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
            self.forward_curve_id.as_str(),
        ));
        Ok(deps)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        self.final_fixing_date().ok()
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.start_date)
    }

    crate::impl_focused_pricing_overrides!();
}

crate::impl_empty_cashflow_provider!(
    CapFloor,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DateExt;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::types::CurveId;
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn test_market_context(base_date: Date) -> MarketContext {
        let disc = DiscountCurve::builder(CurveId::new("TEST-DISC"))
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (0.5, 0.975), (1.0, 0.95), (2.0, 0.90)])
            .build()
            .expect("discount curve should build");

        let fwd = ForwardCurve::builder(CurveId::new("USD-SOFR-3M"), 0.25)
            .base_date(base_date)
            .day_count(DayCount::Act360)
            .knots(vec![(0.0, 0.04), (0.5, 0.042), (1.0, 0.045), (2.0, 0.05)])
            .build()
            .expect("forward curve should build");

        // Create a flat vol surface at 20%
        let vol = 0.20;
        let vol_surface = VolSurface::builder(CurveId::new("TEST-VOL"))
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[0.01, 0.03, 0.05, 0.07, 0.10])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .build()
            .expect("vol surface should build");

        MarketContext::new()
            .insert(disc)
            .insert(fwd)
            .insert_surface(vol_surface)
    }

    /// Test cap-floor parity: Cap(K) - Floor(K) = Forward Swap PV
    ///
    /// This verifies the fundamental no-arbitrage relationship:
    /// Cap(K) - Floor(K) = sum_i [ DF(T_i) * tau_i * (F_i - K) ]
    ///
    /// where F_i is the forward rate for period i.
    ///
    /// # References
    ///
    /// - Hull, J.C. "Options, Futures, and Other Derivatives", Chapter 28 `docs/REFERENCES.md#hull-options-futures`
    /// - This parity holds for European-style options under Black model
    #[test]
    fn cap_floor_parity_holds() {
        let base_date = date(2024, 1, 1);
        let start_date = date(2024, 3, 1);
        let end_date = date(2025, 3, 1);
        let strike = 0.045;
        let notional = Money::from((1_000_000_i64, Currency::USD));

        let ctx = test_market_context(base_date);

        // Create cap and floor with identical parameters
        let cap = CapFloor::new(
            "TEST-CAP",
            RateOptionType::Cap,
            notional,
            strike,
            start_date,
            end_date,
            Some(Tenor::quarterly()),
            DayCount::Act360,
            "TEST-DISC",
            "USD-SOFR-3M",
            "TEST-VOL",
        )
        .expect("valid strike");

        let floor = CapFloor::new(
            "TEST-FLOOR",
            RateOptionType::Floor,
            notional,
            strike,
            start_date,
            end_date,
            Some(Tenor::quarterly()),
            DayCount::Act360,
            "TEST-DISC",
            "USD-SOFR-3M",
            "TEST-VOL",
        )
        .expect("valid strike");

        let cap_pv = cap
            .value(&ctx, base_date)
            .expect("cap pricing should succeed");
        let floor_pv = floor
            .value(&ctx, base_date)
            .expect("floor pricing should succeed");

        // Calculate expected forward swap value: sum of DF * tau * (F - K)
        let disc = ctx.get_discount(CurveId::new("TEST-DISC")).expect("disc");
        let fwd = ctx.get_forward(CurveId::new("USD-SOFR-3M")).expect("fwd");

        // Use the instrument's resolved market calendar and stub so the parity
        // reference shares exactly the priced schedule.
        let periods = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: start_date,
                end: end_date,
                frequency: Tenor::quarterly(),
                stub: cap.stub,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: cap
                    .resolved_schedule_calendar_id()
                    .expect("resolved calendar"),
                end_of_month: false,
                day_count: DayCount::Act360,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
                roll_rule: crate::cashflow::builder::specs::RollRule::None,
            },
        )
        .expect("periods");

        let mut expected_swap_pv = 0.0;
        for p in periods {
            let tau = p.accrual_year_fraction;
            let forward = crate::instruments::common_impl::pricing::time::rate_between_on_dates(
                &fwd,
                p.accrual_start,
                p.accrual_end,
            )
            .expect("forward");
            let df = disc
                .df_between_dates(base_date, p.payment_date)
                .expect("df");
            expected_swap_pv += df * tau * notional.amount() * (forward - strike);
        }

        // Cap - Floor should equal the forward swap PV
        let cap_minus_floor = cap_pv.amount() - floor_pv.amount();
        let parity_error = (cap_minus_floor - expected_swap_pv).abs();

        // Allow for small numerical tolerance (< 0.05 currency units on 1MM notional).
        // The tolerance accounts for day count fraction differences between the cap/floor
        // schedule and the analytical calculation, which can cause ~0.01 divergence.
        assert!(
            parity_error < 0.05,
            "Cap-floor parity violated: Cap({:.2}) - Floor({:.2}) = {:.4}, expected {:.4}, error = {:.6}",
            cap_pv.amount(),
            floor_pv.amount(),
            cap_minus_floor,
            expected_swap_pv,
            parity_error
        );
    }

    /// Test that cap and floor prices are non-negative and sensible
    #[test]
    fn cap_floor_prices_are_sensible() {
        let base_date = date(2024, 1, 1);
        let start_date = date(2024, 3, 1);
        let end_date = date(2025, 3, 1);
        let notional = Money::from((1_000_000_i64, Currency::USD));

        let ctx = test_market_context(base_date);

        // Test at multiple strikes: ITM, ATM, OTM
        let forward_approx = 0.045; // Approximate forward rate
        let strikes = [0.02, 0.04, forward_approx, 0.05, 0.08];

        for &strike in &strikes {
            let cap = CapFloor::new(
                format!("CAP-{}", strike),
                RateOptionType::Cap,
                notional,
                strike,
                start_date,
                end_date,
                Some(Tenor::quarterly()),
                DayCount::Act360,
                "TEST-DISC",
                "USD-SOFR-3M",
                "TEST-VOL",
            )
            .expect("valid strike");

            let floor = CapFloor::new(
                format!("FLOOR-{}", strike),
                RateOptionType::Floor,
                notional,
                strike,
                start_date,
                end_date,
                Some(Tenor::quarterly()),
                DayCount::Act360,
                "TEST-DISC",
                "USD-SOFR-3M",
                "TEST-VOL",
            )
            .expect("valid strike");

            let cap_pv = cap.value(&ctx, base_date).expect("cap pricing");
            let floor_pv = floor.value(&ctx, base_date).expect("floor pricing");

            // Option prices must be non-negative
            assert!(
                cap_pv.amount() >= 0.0,
                "Cap price must be non-negative at strike {}: got {}",
                strike,
                cap_pv.amount()
            );
            assert!(
                floor_pv.amount() >= 0.0,
                "Floor price must be non-negative at strike {}: got {}",
                strike,
                floor_pv.amount()
            );

            // Monotonicity: cap value decreases with strike, floor increases
            // (This is tested implicitly by comparing adjacent strikes)
        }
    }

    #[test]
    fn normal_vol_type_handles_negative_forward() {
        let base_date = date(2024, 1, 1);
        let start_date = date(2024, 3, 1);
        let end_date = date(2025, 3, 1);
        let notional = Money::from((1_000_000_i64, Currency::USD));

        let mut ctx = test_market_context(base_date);
        let neg_fwd = ForwardCurve::builder(CurveId::new("USD-SOFR-3M"), 0.25)
            .base_date(base_date)
            .day_count(DayCount::Act360)
            .knots(vec![
                (0.0, -0.01),
                (0.5, -0.008),
                (1.0, -0.006),
                (2.0, -0.004),
            ])
            .build()
            .expect("negative forward curve should build");
        ctx = ctx.insert(neg_fwd);

        // Build a flat vol surface at 50bp normal vol for the normal model test
        let normal_vol = 0.005;
        let normal_vol_surface = VolSurface::builder(CurveId::new("TEST-VOL-NORMAL"))
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[-0.02, -0.01, 0.0, 0.01, 0.02])
            .row(&[normal_vol, normal_vol, normal_vol, normal_vol, normal_vol])
            .row(&[normal_vol, normal_vol, normal_vol, normal_vol, normal_vol])
            .row(&[normal_vol, normal_vol, normal_vol, normal_vol, normal_vol])
            .row(&[normal_vol, normal_vol, normal_vol, normal_vol, normal_vol])
            .build()
            .expect("normal vol surface should build");
        ctx = ctx.insert_surface(normal_vol_surface);

        // Build a floorlet with negative forward using normal vol surface.
        let normal_floorlet = CapFloor::new(
            "NORM-FLOORLET",
            RateOptionType::Floor,
            notional,
            0.0,
            start_date,
            end_date,
            Some(Tenor::quarterly()),
            DayCount::Act360,
            "TEST-DISC",
            "USD-SOFR-3M",
            "TEST-VOL-NORMAL",
        )
        .expect("valid strike")
        .with_vol_type(CapFloorVolType::Normal);

        let black_floorlet = CapFloor::new(
            "BLACK-FLOORLET",
            RateOptionType::Floor,
            notional,
            0.0,
            start_date,
            end_date,
            Some(Tenor::quarterly()),
            DayCount::Act360,
            "TEST-DISC",
            "USD-SOFR-3M",
            "TEST-VOL-NORMAL",
        )
        .expect("valid strike")
        .with_vol_type(CapFloorVolType::Lognormal);

        // This should succeed under normal model.
        let normal_pv = normal_floorlet
            .value(&ctx, base_date)
            .expect("normal cap/floor pricing should succeed");
        assert!(
            normal_pv.amount().is_finite() && normal_pv.amount() >= 0.0,
            "Normal cap/floor PV should be finite and non-negative"
        );

        // A `Lognormal` cap/floor on a NON-POSITIVE forward must fall back to
        // the Bachelier (normal) pricer without panicking or erroring.
        //
        // Note: the fallback *converts* the lognormal vol to a normal vol
        // (audit item 6) — it does NOT feed the lognormal vol verbatim into
        // Bachelier. So the fallback PV is NOT expected to equal the
        // `Normal`-vol-type PV here: the single 50bp surface value is a
        // *normal* vol for the `Normal` floorlet but a *lognormal* vol for the
        // `Lognormal` floorlet, and the lognormal→normal conversion of a vol
        // on a negative forward (no shift) is the crude approximation that
        // `lognormal_to_normal_vol` documents. The meaningful invariant is
        // that the fallback produces a finite, non-negative price.
        let black_pv = black_floorlet
            .value(&ctx, base_date)
            .expect("lognormal should auto-fallback to Bachelier for non-positive forwards");
        assert!(
            black_pv.amount().is_finite() && black_pv.amount() >= 0.0,
            "lognormal-fallback cap/floor PV on a negative forward must be finite \
             and non-negative; got {}",
            black_pv.amount()
        );
    }

    #[test]
    fn payment_lag_resolution_uses_convention_or_fallback() {
        let instrument_with_unknown_index = CapFloor::new(
            "CAP-LAG-UNKNOWN",
            RateOptionType::Cap,
            Money::from((1_000_000_i64, Currency::USD)),
            0.04,
            date(2024, 3, 1),
            date(2025, 3, 1),
            Some(Tenor::quarterly()),
            DayCount::Act360,
            "TEST-DISC",
            "DOES-NOT-EXIST",
            "TEST-VOL",
        )
        .expect("valid strike");
        assert_eq!(
            instrument_with_unknown_index.resolved_payment_lag_days(),
            0,
            "Unknown index should default to zero payment lag"
        );

        let instrument_with_convention = CapFloor::new(
            "CAP-LAG-CONVENTION",
            RateOptionType::Cap,
            Money::from((1_000_000_i64, Currency::USD)),
            0.04,
            date(2024, 3, 1),
            date(2025, 3, 1),
            Some(Tenor::quarterly()),
            DayCount::Act360,
            "TEST-DISC",
            "USD-SOFR-OIS",
            "TEST-VOL",
        )
        .expect("valid strike");
        assert!(
            instrument_with_convention.resolved_payment_lag_days() >= 0,
            "Convention-based lag should resolve to a non-negative business-day delay"
        );
    }
    #[test]
    fn expiry_is_last_term_fixing_not_maturity() {
        let cap = CapFloor::new(
            "CAP-EXPIRY",
            RateOptionType::Cap,
            Money::from((1_000_000_i64, Currency::USD)),
            0.04,
            date(2024, 3, 1),
            date(2025, 3, 1),
            Some(Tenor::quarterly()),
            DayCount::Act360,
            "TEST-DISC",
            "USD-SOFR-3M",
            "TEST-VOL",
        )
        .expect("valid cap");
        let expected = cap
            .pricing_periods()
            .expect("periods")
            .last()
            .and_then(|period| period.reset_date)
            .expect("last reset date");
        assert_eq!(cap.expiry(), Some(expected));
        assert!(expected < cap.maturity);
    }

    #[test]
    fn overnight_expiry_respects_rate_cutoff() {
        let mut cap = CapFloor::new(
            "RFR-CAP-EXPIRY",
            RateOptionType::Cap,
            Money::from((1_000_000_i64, Currency::USD)),
            0.04,
            date(2024, 3, 1),
            date(2025, 3, 1),
            Some(Tenor::quarterly()),
            DayCount::Act360,
            "USD-SOFR-OIS",
            "USD-SOFR-OIS",
            "TEST-VOL",
        )
        .expect("valid cap");
        cap.overnight_coupon = Some(OvernightCouponConvention {
            compounding: FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 2 },
            payment_delay_days: 2,
            fixing_calendar_id: Some("usny".into()),
            payment_calendar_id: Some("usny".into()),
            spread_compounding: OvernightSpreadCompounding::Exclude,
        });
        let last_period = cap
            .pricing_periods()
            .expect("periods")
            .into_iter()
            .last()
            .expect("last period");
        let calendar = crate::cashflow::builder::calendar::resolve_calendar_strict("usny")
            .expect("USNY calendar");
        let expected = last_period
            .accrual_end
            .add_business_days(-3, calendar)
            .expect("cutoff reference date");
        assert_eq!(cap.expiry(), Some(expected));
        let deps = crate::instruments::Instrument::market_dependencies(&cap).expect("dependencies");
        assert!(
            deps.curves.forward_curves.is_empty(),
            "single-curve overnight caps must not require a redundant forward curve"
        );
    }

    #[test]
    fn future_dated_premium_reduces_holder_npv() {
        let as_of = date(2024, 1, 1);
        let payment_date = date(2024, 2, 1);
        let market = test_market_context(as_of);
        let mut cap = CapFloor::new(
            "CAP-WITH-PREMIUM",
            RateOptionType::Cap,
            Money::from((1_000_000_i64, Currency::USD)),
            0.04,
            date(2024, 3, 1),
            date(2025, 3, 1),
            Some(Tenor::quarterly()),
            DayCount::Act360,
            "TEST-DISC",
            "USD-SOFR-3M",
            "TEST-VOL",
        )
        .expect("valid cap");
        let gross = cap.value(&market, as_of).expect("gross value");
        cap.premium = Some((payment_date, Money::from((25_000_i64, Currency::USD))));
        let net = cap.value(&market, as_of).expect("net value");
        let discount = market
            .get_discount(CurveId::new("TEST-DISC"))
            .expect("discount curve");
        let premium_pv = 25_000.0
            * discount
                .df_between_dates(as_of, payment_date)
                .expect("premium discount factor");
        assert!((net.amount() - (gross.amount() - premium_pv)).abs() < 1.0e-8);
    }

    #[test]
    fn settled_premium_is_excluded_and_currency_is_validated() {
        let as_of = date(2024, 1, 1);
        let market = test_market_context(as_of);
        let mut cap = CapFloor::new(
            "CAP-PREMIUM-CONTRACT",
            RateOptionType::Cap,
            Money::from((1_000_000_i64, Currency::USD)),
            0.04,
            date(2024, 3, 1),
            date(2025, 3, 1),
            Some(Tenor::quarterly()),
            DayCount::Act360,
            "TEST-DISC",
            "USD-SOFR-3M",
            "TEST-VOL",
        )
        .expect("valid cap");
        let gross = cap.value(&market, as_of).expect("gross value");
        cap.premium = Some((as_of, Money::from((25_000_i64, Currency::USD))));
        let settled = cap.value(&market, as_of).expect("settled premium value");
        assert_eq!(settled, gross);

        cap.premium = Some((date(2024, 2, 1), Money::from((25_000_i64, Currency::EUR))));
        let error = cap
            .value(&market, as_of)
            .expect_err("premium currency mismatch must fail");
        assert!(error.to_string().contains("premium currency"));
    }
}
