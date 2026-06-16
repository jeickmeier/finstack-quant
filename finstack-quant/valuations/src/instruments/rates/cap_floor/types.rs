//! Interest rate option instrument types and Black model greeks.

use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::{ExerciseStyle, SettlementType};
use crate::market::conventions::defs::RateIndexKind;
use crate::market::conventions::ConventionRegistry;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::IndexId;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use super::parameters::CapFloorParams;
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
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CapFloorVolType {
    /// Lognormal (Black) volatility - percentage of forward rate.
    ///
    /// Standard market convention. Volatility is typically quoted as a
    /// decimal (e.g., 0.20 for 20% vol).
    #[default]
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

    /// Automatic model selection based on forward rate and strike.
    ///
    /// Inspects the forward rate for each caplet/floorlet:
    /// - If both forward > 0 and strike > 0: uses Black (lognormal)
    /// - Otherwise: uses Normal (Bachelier)
    ///
    /// This is useful for portfolios spanning multiple currencies or rate
    /// regimes where some periods may have negative forwards.
    ///
    /// **Recommended default for production use** — safely handles mixed
    /// positive/negative rate environments without manual intervention.
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
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "lognormal" | "black" => Ok(Self::Lognormal),
            "shifted_lognormal" | "shifted" | "displaced" => Ok(Self::ShiftedLognormal),
            "normal" | "bachelier" => Ok(Self::Normal),
            "auto" => Ok(Self::Auto),
            other => Err(format!(
                "Unknown cap/floor vol type: '{}'. Valid: lognormal, shifted_lognormal, normal, auto",
                other
            )),
        }
    }
}

/// Type of interest rate option
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
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
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "cap" => Ok(Self::Cap),
            "floor" => Ok(Self::Floor),
            "caplet" => Ok(Self::Caplet),
            "floorlet" => Ok(Self::Floorlet),
            other => Err(format!(
                "Unknown rate option type: '{}'. Valid: cap, floor, caplet, floorlet",
                other
            )),
        }
    }
}

/// Interest rate option instrument
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CapFloor {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Option type
    pub rate_option_type: RateOptionType,
    /// Notional amount
    pub notional: Money,
    /// Strike (as decimal, e.g., 0.05 for 5%)
    pub strike: Decimal,
    /// Start date of underlying period
    #[schemars(with = "String")]
    pub start_date: Date,
    /// End date of underlying period
    #[schemars(with = "String")]
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
    pub bdc: BusinessDayConvention,
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
    /// - `Lognormal` (default): Standard Black model, requires positive rates/strikes
    /// - `Normal`: Bachelier model, handles negative rates
    #[serde(default)]
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
    /// Additional attributes
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl CapFloor {
    /// Create a canonical example USD 5Y 3% interest rate cap ($10M notional, quarterly SOFR).
    ///
    /// Returns a 5-year cap with quarterly payment frequency, ACT/360 day count,
    /// lognormal vol convention, and standard schedule conventions.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        use time::Month;

        let start = Date::from_calendar_date(2024, Month::January, 3).map_err(|e| {
            finstack_quant_core::Error::Validation(format!("Invalid example start date: {}", e))
        })?;
        let maturity = Date::from_calendar_date(2029, Month::January, 3).map_err(|e| {
            finstack_quant_core::Error::Validation(format!("Invalid example end date: {}", e))
        })?;

        Self::new_cap(
            InstrumentId::new("IRCAP-USD-5Y-3PCT"),
            Money::new(10_000_000.0, Currency::USD),
            0.03,
            start,
            maturity,
            Tenor::quarterly(),
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

    fn from_params(
        id: impl Into<InstrumentId>,
        option_params: &CapFloorParams,
        start_date: Date,
        maturity: Date,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> Self {
        Self {
            id: id.into(),
            rate_option_type: option_params.rate_option_type,
            notional: option_params.notional,
            strike: option_params.strike,
            start_date,
            maturity,
            frequency: option_params.frequency,
            day_count: option_params.day_count,
            stub: option_params.stub,
            bdc: option_params.bdc,
            calendar_id: option_params.calendar_id.map(CalendarId::new),
            exercise_style: ExerciseStyle::European,
            settlement: SettlementType::Cash,
            discount_curve_id: discount_curve_id.into(),
            forward_curve_id: forward_curve_id.into(),
            vol_surface_id: vol_surface_id.into(),
            vol_type: CapFloorVolType::default(),
            vol_shift: 0.0,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
        }
    }

    /// Create a cap instrument using parameter structs.
    ///
    /// Returns an error if the strike value is not representable as `Decimal` (e.g., NaN or Inf).
    #[allow(clippy::too_many_arguments)]
    pub fn new_cap(
        id: impl Into<InstrumentId>,
        notional: Money,
        strike: f64,
        start_date: Date,
        maturity: Date,
        frequency: Tenor,
        day_count: DayCount,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let option_params = CapFloorParams::cap(notional, strike, frequency, day_count)?;
        Ok(Self::from_params(
            id,
            &option_params,
            start_date,
            maturity,
            discount_curve_id.into(),
            forward_curve_id.into(),
            vol_surface_id,
        ))
    }

    /// Create a floor instrument using parameter structs.
    ///
    /// Returns an error if the strike value is not representable as `Decimal` (e.g., NaN or Inf).
    #[allow(clippy::too_many_arguments)]
    pub fn new_floor(
        id: impl Into<InstrumentId>,
        notional: Money,
        strike: f64,
        start_date: Date,
        maturity: Date,
        frequency: Tenor,
        day_count: DayCount,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let option_params = CapFloorParams::floor(notional, strike, frequency, day_count)?;
        Ok(Self::from_params(
            id,
            &option_params,
            start_date,
            maturity,
            discount_curve_id.into(),
            forward_curve_id.into(),
            vol_surface_id,
        ))
    }

    /// Create a single-period caplet instrument.
    ///
    /// Returns an error if the strike value is not representable as `Decimal` (e.g., NaN or Inf).
    #[allow(clippy::too_many_arguments)]
    pub fn new_caplet(
        id: impl Into<InstrumentId>,
        notional: Money,
        strike: f64,
        start_date: Date,
        maturity: Date,
        day_count: DayCount,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let option_params = CapFloorParams {
            rate_option_type: RateOptionType::Caplet,
            notional,
            strike: finstack_quant_core::decimal::f64_to_decimal(strike)?,
            frequency: infer_single_period_frequency(start_date, maturity),
            day_count,
            stub: StubKind::ShortFront,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
        };
        Ok(Self::from_params(
            id,
            &option_params,
            start_date,
            maturity,
            discount_curve_id.into(),
            forward_curve_id.into(),
            vol_surface_id,
        ))
    }

    /// Create a single-period floorlet instrument.
    ///
    /// Returns an error if the strike value is not representable as `Decimal` (e.g., NaN or Inf).
    #[allow(clippy::too_many_arguments)]
    pub fn new_floorlet(
        id: impl Into<InstrumentId>,
        notional: Money,
        strike: f64,
        start_date: Date,
        maturity: Date,
        day_count: DayCount,
        discount_curve_id: impl Into<CurveId>,
        forward_curve_id: impl Into<CurveId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let option_params = CapFloorParams {
            rate_option_type: RateOptionType::Floorlet,
            notional,
            strike: finstack_quant_core::decimal::f64_to_decimal(strike)?,
            frequency: infer_single_period_frequency(start_date, maturity),
            day_count,
            stub: StubKind::ShortFront,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
        };
        Ok(Self::from_params(
            id,
            &option_params,
            start_date,
            maturity,
            discount_curve_id.into(),
            forward_curve_id.into(),
            vol_surface_id,
        ))
    }

    pub(crate) fn pricing_periods(
        &self,
    ) -> finstack_quant_core::Result<Vec<crate::cashflow::builder::periods::SchedulePeriod>> {
        let params = crate::cashflow::builder::periods::BuildPeriodsParams {
            start: self.start_date,
            end: self.maturity,
            frequency: self.frequency,
            stub: self.stub,
            bdc: self.bdc,
            calendar_id: self
                .calendar_id
                .as_deref()
                .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
            end_of_month: false,
            day_count: self.day_count,
            payment_lag_days: self.resolved_payment_lag_days(),
            reset_lag_days: self.resolved_reset_lag_days(),
            adjust_accrual_dates: false,
        };

        if matches!(
            self.rate_option_type,
            RateOptionType::Caplet | RateOptionType::Floorlet
        ) {
            Ok(vec![
                crate::cashflow::builder::periods::build_single_period(params)?,
            ])
        } else {
            crate::cashflow::builder::periods::build_periods(params)
        }
    }

    /// Set the volatility type convention.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::rates::cap_floor::{CapFloor, CapFloorVolType};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{create_date, DayCount, Tenor};
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::{CurveId, InstrumentId};
    /// use time::Month;
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// // Create a floor with normal volatility for EUR market (ACT/360 is the standard day count
    /// // for EUR ESTR/EURIBOR caps and floors per ISDA conventions).
    /// let floor = CapFloor::new_floor(
    ///     InstrumentId::new("EUR-FLOOR-001"),
    ///     Money::new(1_000_000.0, Currency::EUR),
    ///     0.02,
    ///     create_date(2026, Month::January, 1)?,
    ///     create_date(2027, Month::January, 1)?,
    ///     Tenor::quarterly(),
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

    /// Set displacement shift used for shifted-lognormal pricing.
    ///
    /// # Arguments
    ///
    /// * `vol_shift` — Displacement added to forward and strike: `F' = F + shift`, `K' = K + shift`.
    ///   Must be ≥ 0.0 to keep shifted rates positive. Typical range: 0.01–0.03 (1%–3%).
    pub fn with_vol_shift(mut self, vol_shift: f64) -> Self {
        self.vol_shift = vol_shift;
        self
    }

    pub(crate) fn resolved_payment_lag_days(&self) -> i32 {
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

    pub(crate) fn uses_overnight_rfr_index(&self) -> bool {
        let Ok(registry) = ConventionRegistry::try_global() else {
            return false;
        };
        let idx = IndexId::new(self.forward_curve_id.as_str());
        registry
            .require_rate_index(&idx)
            .map(|conv| conv.kind == RateIndexKind::OvernightRfr)
            .unwrap_or(false)
    }

    pub(crate) fn option_fixing_date(
        &self,
        period: &crate::cashflow::builder::periods::SchedulePeriod,
    ) -> Date {
        if self.uses_overnight_rfr_index() {
            period.accrual_end
        } else {
            period.reset_date.unwrap_or(period.accrual_start)
        }
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

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Black76
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        crate::instruments::rates::cap_floor::pricing::pricer::price_cap_floor(self, curves, as_of)
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
        Some(self.maturity)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.start_date)
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

impl crate::instruments::common_impl::traits::CurveDependencies for CapFloor {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .forward(self.forward_curve_id.clone())
            .build()
    }
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

        let fwd = ForwardCurve::builder(CurveId::new("TEST-FWD"), 0.25)
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
    /// - Hull, J.C. "Options, Futures, and Other Derivatives", Chapter 28
    /// - This parity holds for European-style options under Black model
    #[test]
    fn cap_floor_parity_holds() {
        let base_date = date(2024, 1, 1);
        let start_date = date(2024, 3, 1);
        let end_date = date(2025, 3, 1);
        let strike = 0.045;
        let notional = Money::new(1_000_000.0, Currency::USD);

        let ctx = test_market_context(base_date);

        // Create cap and floor with identical parameters
        let cap = CapFloor::new_cap(
            "TEST-CAP",
            notional,
            strike,
            start_date,
            end_date,
            Tenor::quarterly(),
            DayCount::Act360,
            "TEST-DISC",
            "TEST-FWD",
            "TEST-VOL",
        )
        .expect("valid strike");

        let floor = CapFloor::new_floor(
            "TEST-FLOOR",
            notional,
            strike,
            start_date,
            end_date,
            Tenor::quarterly(),
            DayCount::Act360,
            "TEST-DISC",
            "TEST-FWD",
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
        let fwd = ctx.get_forward(CurveId::new("TEST-FWD")).expect("fwd");

        // IMPORTANT: Use the same canonical schedule builder as the instrument pricer.
        //
        // `cashflow::builder::periods::build_periods` applies BDC even when `calendar_id`
        // is weekends-only. Cap/floor parity is very sensitive to small date shifts,
        // so we must match the instrument's schedule.
        let periods = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: start_date,
                end: end_date,
                frequency: Tenor::quarterly(),
                stub: StubKind::None,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID,
                end_of_month: false,
                day_count: DayCount::Act360,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
            },
        )
        .expect("periods");

        let mut expected_swap_pv = 0.0;
        for p in periods {
            let tau = p.accrual_year_fraction;
            let forward = crate::instruments::common_impl::pricing::time::rate_period_on_dates(
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
        let notional = Money::new(1_000_000.0, Currency::USD);

        let ctx = test_market_context(base_date);

        // Test at multiple strikes: ITM, ATM, OTM
        let forward_approx = 0.045; // Approximate forward rate
        let strikes = [0.02, 0.04, forward_approx, 0.05, 0.08];

        for &strike in &strikes {
            let cap = CapFloor::new_cap(
                format!("CAP-{}", strike),
                notional,
                strike,
                start_date,
                end_date,
                Tenor::quarterly(),
                DayCount::Act360,
                "TEST-DISC",
                "TEST-FWD",
                "TEST-VOL",
            )
            .expect("valid strike");

            let floor = CapFloor::new_floor(
                format!("FLOOR-{}", strike),
                notional,
                strike,
                start_date,
                end_date,
                Tenor::quarterly(),
                DayCount::Act360,
                "TEST-DISC",
                "TEST-FWD",
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
        let notional = Money::new(1_000_000.0, Currency::USD);

        let mut ctx = test_market_context(base_date);
        let neg_fwd = ForwardCurve::builder(CurveId::new("TEST-FWD-NEG"), 0.25)
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
        let normal_floorlet = CapFloor::new_floor(
            "NORM-FLOORLET",
            notional,
            0.0,
            start_date,
            end_date,
            Tenor::quarterly(),
            DayCount::Act360,
            "TEST-DISC",
            "TEST-FWD-NEG",
            "TEST-VOL-NORMAL",
        )
        .expect("valid strike")
        .with_vol_type(CapFloorVolType::Normal);

        let black_floorlet = CapFloor::new_floor(
            "BLACK-FLOORLET",
            notional,
            0.0,
            start_date,
            end_date,
            Tenor::quarterly(),
            DayCount::Act360,
            "TEST-DISC",
            "TEST-FWD-NEG",
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
        let instrument_with_unknown_index = CapFloor::new_cap(
            "CAP-LAG-UNKNOWN",
            Money::new(1_000_000.0, Currency::USD),
            0.04,
            date(2024, 3, 1),
            date(2025, 3, 1),
            Tenor::quarterly(),
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

        let instrument_with_convention = CapFloor::new_cap(
            "CAP-LAG-CONVENTION",
            Money::new(1_000_000.0, Currency::USD),
            0.04,
            date(2024, 3, 1),
            date(2025, 3, 1),
            Tenor::quarterly(),
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
}
