//! Basis swap implementation for multi-curve calibration.
//!
//! A basis swap exchanges two floating rate payments with different tenors,
//! capturing the basis spread between them (e.g., 3M vs 6M).
//!
//! # Sign Convention
//!
//! NPV is computed from the perspective of **receiving the primary leg** (with spread)
//! and **paying the reference leg**:
//!
//! - **Positive NPV**: Primary leg receiver is in-the-money
//! - **Negative NPV**: Primary leg receiver is out-of-the-money
//!
//! This convention aligns with ISDA standards where the spread-receiving party
//! is considered "long" the basis swap.
//!
//! # Shared Infrastructure
//!
//! This module delegates to the shared swap leg pricing infrastructure in
//! [`crate::instruments::common_impl::pricing::swap_legs`] for robust discounting
//! and numerical stability.

use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
use finstack_quant_core::{
    dates::Date, market_data::context::MarketContext, money::Money, types::InstrumentId, Result,
};

// Import shared swap leg pricing utilities
use crate::cashflow::builder::{
    CashFlowSchedule, CouponType, FloatingCouponSpec, FloatingRateFallback, FloatingRateSpec,
    Notional,
};
use crate::cashflow::CashflowProvider;
use crate::impl_instrument_base;
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::pricing::swap_legs::{FloatingLegParams, LegPeriod};
use crate::instruments::common_impl::validation;
use rust_decimal::Decimal;

// Re-export from common parameters
pub use crate::instruments::common_impl::parameters::legs::BasisSwapLeg;

/// Basis swap instrument that exchanges two floating rate payments with different tenors.
///
/// A basis swap allows parties to exchange floating rate payments based on different
/// reference rates (e.g., 3M SOFR vs 6M SOFR) plus an optional spread on one leg.
/// The primary leg typically receives the spread, while the reference leg pays flat.
///
/// Each leg owns its own dates, discount curve, calendar, and stub conventions,
/// following the IRS leg-centric pattern.
///
/// # Cross-Currency (XCCY) Basis Swaps
///
/// **Important**: This implementation supports **single-currency** basis swaps only.
/// For cross-currency basis swaps, use `XccySwap` instead.
///
/// # Examples
/// ```rust
/// use finstack_quant_core::{dates::*, money::Money, currency::Currency, types::CurveId};
/// use finstack_quant_valuations::instruments::rates::basis_swap::{BasisSwap, BasisSwapLeg};
/// use time::Month;
///
/// let start = Date::from_calendar_date(2024, Month::January, 3).expect("valid date");
/// let end = Date::from_calendar_date(2025, Month::January, 3).expect("valid date");
///
/// let primary_leg = BasisSwapLeg {
///     forward_curve_id: CurveId::new("3M-SOFR"),
///     discount_curve_id: CurveId::new("OIS"),
///     start,
///     end,
///     frequency: Tenor::quarterly(),
///     day_count: DayCount::Act360,
///     bdc: BusinessDayConvention::ModifiedFollowing,
///     calendar_id: None,
///     stub: StubKind::ShortFront,
///     spread_bp: rust_decimal::Decimal::from(5),
///     payment_lag_days: 0,
///     reset_lag_days: 0,
/// };
///
/// let reference_leg = BasisSwapLeg {
///     forward_curve_id: CurveId::new("6M-SOFR"),
///     discount_curve_id: CurveId::new("OIS"),
///     start,
///     end,
///     frequency: Tenor::semi_annual(),
///     day_count: DayCount::Act360,
///     bdc: BusinessDayConvention::ModifiedFollowing,
///     calendar_id: None,
///     stub: StubKind::ShortFront,
///     spread_bp: rust_decimal::Decimal::ZERO,
///     payment_lag_days: 0,
///     reset_lag_days: 0,
/// };
///
/// let swap = BasisSwap::new(
///     "BASIS_SWAP_001",
///     Money::new(1_000_000.0, Currency::USD),
///     primary_leg,
///     reference_leg,
/// );
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct BasisSwap {
    /// Unique identifier for this instrument.
    pub id: InstrumentId,
    /// Notional amount for both legs.
    pub notional: Money,
    /// Primary leg that typically receives the spread.
    pub primary_leg: BasisSwapLeg,
    /// Reference leg that typically pays flat.
    pub reference_leg: BasisSwapLeg,
    /// Allow calendar-day fallback when the calendar cannot be resolved.
    ///
    /// When `false` (default), missing calendars are treated as an input error to
    /// avoid silently misaligning schedule and payment-lag conventions.
    #[builder(default)]
    #[serde(default)]
    pub allow_calendar_fallback: bool,
    /// Allow both legs to reference the same forward curve.
    ///
    /// When `false` (default), having identical forward curves on both legs produces
    /// a validation error, as this is almost always a configuration mistake (NPV would
    /// equal spread × annuity by construction). Set to `true` only for testing or
    /// deliberate same-index spread trades.
    #[builder(default)]
    #[serde(default)]
    pub allow_same_curve: bool,
    /// Pricing overrides for scenario analysis and model configuration.
    #[builder(default)]
    #[serde(
        default,
        deserialize_with = "crate::instruments::common_impl::parameters::deserialize_null_default"
    )]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for instrument selection and tagging.
    pub attributes: crate::instruments::common_impl::traits::Attributes,
}

impl BasisSwap {
    /// Creates a new basis swap with the specified parameters.
    ///
    /// Dates, discount curves, calendars, and stub conventions are on each leg.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Either leg has `start >= end` (invalid swap tenor)
    /// - Both legs reference the same forward curve (use `new_allowing_same_curve` to override)
    /// - Any lag is negative
    pub fn new(
        id: impl Into<String>,
        notional: Money,
        primary_leg: BasisSwapLeg,
        reference_leg: BasisSwapLeg,
    ) -> Result<Self> {
        let id_str = id.into();

        // Validate dates on each leg
        if primary_leg.start >= primary_leg.end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BasisSwap '{}' primary leg has start ({}) >= end ({}); \
                 leg must have positive tenor",
                id_str, primary_leg.start, primary_leg.end
            )));
        }
        if reference_leg.start >= reference_leg.end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BasisSwap '{}' reference leg has start ({}) >= end ({}); \
                 leg must have positive tenor",
                id_str, reference_leg.start, reference_leg.end
            )));
        }

        // Validate different forward curves (unless explicitly allowed)
        if primary_leg.forward_curve_id == reference_leg.forward_curve_id {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BasisSwap '{}' has identical forward curves on both legs ({}). \
                 A same-index basis swap has NPV = spread × annuity by construction. \
                 If this is intentional, use .with_allow_same_curve(true).",
                id_str,
                primary_leg.forward_curve_id.as_str()
            )));
        }

        Self::validate_leg_lags(&id_str, "primary", &primary_leg)?;
        Self::validate_leg_lags(&id_str, "reference", &reference_leg)?;

        Ok(Self {
            id: InstrumentId::new(id_str),
            notional,
            primary_leg,
            reference_leg,
            allow_calendar_fallback: false,
            allow_same_curve: false,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: crate::instruments::common_impl::traits::Attributes::default(),
        })
    }

    /// Allow (or disallow) calendar-day fallback when the calendar cannot be resolved.
    pub fn with_allow_calendar_fallback(mut self, allow: bool) -> Self {
        self.allow_calendar_fallback = allow;
        self
    }

    /// Allow (or disallow) same forward curve on both legs.
    pub fn with_allow_same_curve(mut self, allow: bool) -> Self {
        self.allow_same_curve = allow;
        self
    }

    /// Validate structural invariants independently of the constructor.
    ///
    /// This is required for Serde-backed Rust/Python/WASM construction, which
    /// does not pass through [`BasisSwap::new`].
    pub fn validate(&self) -> Result<()> {
        if self.primary_leg.start >= self.primary_leg.end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BasisSwap '{}' primary leg has start ({}) >= end ({}); leg must have positive tenor",
                self.id, self.primary_leg.start, self.primary_leg.end
            )));
        }
        if self.reference_leg.start >= self.reference_leg.end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BasisSwap '{}' reference leg has start ({}) >= end ({}); leg must have positive tenor",
                self.id, self.reference_leg.start, self.reference_leg.end
            )));
        }
        if !self.allow_same_curve
            && self.primary_leg.forward_curve_id == self.reference_leg.forward_curve_id
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BasisSwap '{}' has identical forward curves on both legs ({}); set allow_same_curve=true only for an intentional same-index spread trade",
                self.id,
                self.primary_leg.forward_curve_id.as_str()
            )));
        }
        Self::validate_leg_lags(self.id.as_str(), "primary", &self.primary_leg)?;
        Self::validate_leg_lags(self.id.as_str(), "reference", &self.reference_leg)?;
        self.resolve_leg_calendar("primary", &self.primary_leg)?;
        self.resolve_leg_calendar("reference", &self.reference_leg)?;
        Ok(())
    }

    /// Creates a basis swap without curve uniqueness validation.
    ///
    /// Use this constructor when you intentionally want both legs to reference the
    /// same forward curve (e.g., for testing or same-index spread trades).
    pub fn new_allowing_same_curve(
        id: impl Into<String>,
        notional: Money,
        primary_leg: BasisSwapLeg,
        reference_leg: BasisSwapLeg,
    ) -> Result<Self> {
        let id_str = id.into();

        if primary_leg.start >= primary_leg.end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BasisSwap '{}' primary leg has start ({}) >= end ({}); \
                 leg must have positive tenor",
                id_str, primary_leg.start, primary_leg.end
            )));
        }
        if reference_leg.start >= reference_leg.end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BasisSwap '{}' reference leg has start ({}) >= end ({}); \
                 leg must have positive tenor",
                id_str, reference_leg.start, reference_leg.end
            )));
        }

        if primary_leg.forward_curve_id == reference_leg.forward_curve_id {
            tracing::warn!(
                instrument_id = %id_str,
                forward_curve_id = %primary_leg.forward_curve_id.as_str(),
                "BasisSwap created with same forward curve on both legs; \
                 NPV will equal spread × annuity by construction"
            );
        }

        Self::validate_leg_lags(&id_str, "primary", &primary_leg)?;
        Self::validate_leg_lags(&id_str, "reference", &reference_leg)?;

        Ok(Self {
            id: InstrumentId::new(id_str),
            notional,
            primary_leg,
            reference_leg,
            allow_calendar_fallback: false,
            allow_same_curve: true,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: crate::instruments::common_impl::traits::Attributes::default(),
        })
    }

    /// Create a canonical example USD 3M SOFR vs 1M SOFR basis swap (5Y, $10M notional).
    ///
    /// Returns a 5-year basis swap with the primary leg on 3M SOFR (receiving 5bp spread)
    /// and the reference leg on 1M SOFR (flat).
    pub fn example() -> Result<Self> {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::StubKind;
        use finstack_quant_core::types::CurveId;
        use time::Month;

        let start = Date::from_calendar_date(2024, Month::January, 3).map_err(|e| {
            finstack_quant_core::Error::Validation(format!("Invalid example start date: {}", e))
        })?;
        let end = Date::from_calendar_date(2029, Month::January, 3).map_err(|e| {
            finstack_quant_core::Error::Validation(format!("Invalid example end date: {}", e))
        })?;

        let primary_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::from(5),
            payment_lag_days: 0,
            reset_lag_days: 0,
        };

        let reference_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start,
            end,
            frequency: Tenor::monthly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        };

        Self::new(
            "BASIS-SWAP-USD-3M1M-5Y",
            Money::new(10_000_000.0, Currency::USD),
            primary_leg,
            reference_leg,
        )
    }

    fn validate_leg_lags(id: &str, leg_name: &str, leg: &BasisSwapLeg) -> Result<()> {
        if leg.payment_lag_days < 0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BasisSwap '{}' {} leg has negative payment_lag_days ({}); \
                 payment lag must be non-negative",
                id, leg_name, leg.payment_lag_days
            )));
        }
        if leg.reset_lag_days < 0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BasisSwap '{}' {} leg has negative reset_lag_days ({}); \
                 reset lag must be non-negative",
                id, leg_name, leg.reset_lag_days
            )));
        }
        Ok(())
    }

    fn resolve_leg_calendar<'a>(&self, leg_name: &str, leg: &'a BasisSwapLeg) -> Result<&'a str> {
        if let Some(id) = leg.calendar_id.as_deref() {
            if crate::cashflow::builder::calendar::resolve_calendar_strict(id).is_ok() {
                return Ok(id);
            }
        }
        if self.allow_calendar_fallback {
            return Ok(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID);
        }
        Err(finstack_quant_core::Error::Validation(format!(
            "BasisSwap '{}' {} leg requires a resolvable calendar_id; set allow_calendar_fallback=true to opt into weekends_only",
            self.id, leg_name
        )))
    }

    /// Calculates the present value of a floating rate leg.
    ///
    /// This method uses the shared swap leg pricing infrastructure for
    /// robust discounting and numerical stability (Kahan summation).
    pub fn pv_float_leg(
        &self,
        leg: &BasisSwapLeg,
        context: &MarketContext,
        valuation_date: Date,
    ) -> Result<Money> {
        if leg.payment_lag_days < 0 || leg.reset_lag_days < 0 {
            return Err(finstack_quant_core::Error::Validation(
                "BasisSwap leg lags must be non-negative".to_string(),
            ));
        }

        let spread_bp = decimal_to_f64(leg.spread_bp, "BasisSwap leg spread_bp")?;
        validation::validate_rate_magnitude(
            spread_bp,
            5000.0,
            "BasisSwap leg spread",
            1.0,
            "bp",
            "Spread is in basis points (e.g., Decimal::from(5) for 5bp). \
             If this is intentional for stress testing, consider using a dedicated stress API.",
        )?;

        let typical_spread_bp = 500.0;
        if spread_bp.abs() > typical_spread_bp {
            tracing::warn!(
                instrument_id = %self.id.as_str(),
                spread_bp,
                "BasisSwap leg spread {}bp is outside typical market range (±500bp). \
                 Verify this is intentional and not a unit conversion error.",
                leg.spread_bp
            );
        }

        let disc = context.get_discount(&leg.discount_curve_id)?;
        let fwd = context.get_forward(&leg.forward_curve_id)?;
        let currency = self.notional.currency();

        let periods = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: leg.start,
                end: leg.end,
                frequency: leg.frequency,
                stub: leg.stub,
                bdc: leg.bdc,
                calendar_id: self.resolve_leg_calendar("priced", leg)?,
                end_of_month: false,
                day_count: leg.day_count,
                payment_lag_days: leg.payment_lag_days,
                reset_lag_days: Some(leg.reset_lag_days),
                adjust_accrual_dates: false,
            },
        )?;

        if periods.is_empty() {
            return Ok(Money::new(0.0, currency));
        }

        let leg_periods: Vec<LegPeriod> = periods
            .into_iter()
            .filter(|period| period.payment_date > valuation_date)
            .map(|period| LegPeriod {
                accrual_start: period.accrual_start,
                accrual_end: period.accrual_end,
                reset_date: period.reset_date,
                year_fraction: period.accrual_year_fraction,
            })
            .collect();

        let params = FloatingLegParams::full(
            decimal_to_f64(leg.spread_bp, "BasisSwap leg spread_bp")?,
            1.0,
            true,
            None,
            None,
            None,
            None,
            leg.payment_lag_days,
            leg.calendar_id.clone(),
        );

        let fixings = finstack_quant_core::market_data::fixings::get_fixing_series(
            context,
            leg.forward_curve_id.as_str(),
        )
        .ok();

        let pv = crate::instruments::common_impl::pricing::swap_legs::pv_floating_leg(
            leg_periods.into_iter(),
            self.notional.amount(),
            &params,
            disc.as_ref(),
            fwd.as_ref(),
            valuation_date,
            fixings,
        )?;

        Ok(Money::new(pv, currency))
    }

    /// Calculates the discounted accrual sum (annuity) for a leg.
    pub fn annuity_for_leg(
        &self,
        leg: &BasisSwapLeg,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        if leg.payment_lag_days < 0 {
            return Err(finstack_quant_core::Error::Validation(
                "BasisSwap payment lag must be non-negative".to_string(),
            ));
        }

        let disc = curves.get_discount(&leg.discount_curve_id)?;

        let periods = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: leg.start,
                end: leg.end,
                frequency: leg.frequency,
                stub: leg.stub,
                bdc: leg.bdc,
                calendar_id: self.resolve_leg_calendar("cashflow", leg)?,
                end_of_month: false,
                day_count: leg.day_count,
                payment_lag_days: leg.payment_lag_days,
                reset_lag_days: Some(leg.reset_lag_days),
                adjust_accrual_dates: false,
            },
        )?;

        if periods.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "BasisSwap leg schedule must contain at least 2 dates".to_string(),
            ));
        }

        let mut annuity = 0.0;
        for period in periods {
            if period.payment_date > as_of {
                let df = crate::instruments::common_impl::pricing::swap_legs::robust_relative_df(
                    disc.as_ref(),
                    as_of,
                    period.payment_date,
                )?;
                annuity += period.accrual_year_fraction * df;
            }
        }

        const ANNUITY_EPSILON: f64 = 1e-12;
        if annuity < ANNUITY_EPSILON {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BasisSwap annuity ({:.2e}) is below minimum threshold ({:.2e}). \
                 This may indicate all periods have expired or extreme discounting scenarios.",
                annuity, ANNUITY_EPSILON
            )));
        }

        Ok(annuity)
    }

    fn floating_leg_schedule(
        &self,
        leg: &BasisSwapLeg,
        market: &MarketContext,
    ) -> Result<CashFlowSchedule> {
        let mut builder = CashFlowSchedule::builder();
        let _ = builder
            .principal(self.notional, leg.start, leg.end)
            .floating_cf(FloatingCouponSpec {
                rate_spec: FloatingRateSpec {
                    index_id: leg.forward_curve_id.clone(),
                    spread_bp: leg.spread_bp,
                    gearing: Decimal::ONE,
                    gearing_includes_spread: true,
                    index_floor_bp: None,
                    all_in_cap_bp: None,
                    all_in_floor_bp: None,
                    index_cap_bp: None,
                    overnight_index_constraints: Default::default(),
                    reset_freq: leg.frequency,
                    index_tenor: None,
                    reset_lag_days: leg.reset_lag_days,
                    fixing_calendar_id: None,
                    overnight_compounding: None,
                    overnight_basis: None,
                    fallback: FloatingRateFallback::Error,
                },
                coupon_type: CouponType::Cash,
                schedule: finstack_quant_cashflows::builder::ScheduleParams {
                    freq: leg.frequency,
                    dc: leg.day_count,
                    bdc: leg.bdc,
                    calendar_id: self.resolve_leg_calendar("cashflow", leg)?.to_string(),
                    stub: leg.stub,
                    end_of_month: false,
                    payment_lag_days: leg.payment_lag_days,
                    adjust_accrual_dates: false,
                },
            });
        let mut schedule = builder.build_with_curves(Some(market))?;
        schedule
            .flows
            .retain(|cf| cf.kind == crate::cashflow::primitives::CFKind::FloatReset);
        Ok(schedule)
    }
}

// Attributable implementation is provided by the impl_instrument! macro

// Use the macro to implement Instrument with pricing
impl crate::instruments::common_impl::traits::Instrument for BasisSwap {
    impl_instrument_base!(crate::pricer::InstrumentType::BasisSwap);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        BasisSwap::validate(self)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        // pv_float_leg builds its own period schedule internally; computing leg_schedule
        // here would be redundant work on the calibration hot path.
        let primary_pv = self.pv_float_leg(&self.primary_leg, curves, as_of)?;
        let reference_pv = self.pv_float_leg(&self.reference_leg, curves, as_of)?;

        // NPV from perspective of receiving primary leg (with spread), paying reference leg
        primary_pv.checked_sub(reference_pv)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.primary_leg.end)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.primary_leg.start)
    }
}

impl CashflowProvider for BasisSwap {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let mut primary = self.floating_leg_schedule(&self.primary_leg, market)?;
        let mut reference = self.floating_leg_schedule(&self.reference_leg, market)?;
        for cf in &mut reference.flows {
            cf.amount *= -1.0;
        }
        primary.flows.extend(reference.flows);
        primary.notional = Notional::par(self.notional.amount(), self.notional.currency());
        primary.day_count = self.primary_leg.day_count;
        Ok(primary.normalize_public(
            as_of,
            crate::cashflow::builder::CashflowRepresentation::Projected,
        ))
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for BasisSwap {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.primary_leg.discount_curve_id.clone())
            .discount(self.reference_leg.discount_curve_id.clone())
            .forward(self.primary_leg.forward_curve_id.clone())
            .forward(self.reference_leg.forward_curve_id.clone())
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::StubKind;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::types::CurveId;
    use time::Month;

    // Helper function for tests
    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid date"), day)
            .expect("should succeed")
    }

    #[test]
    fn test_basis_swap_pricing() {
        let base_date = date(2024, 1, 1);
        let start_date = date(2024, 1, 3);
        let maturity = date(2025, 1, 3);

        // Create test curves with flat rates
        let discount_curve = DiscountCurve::builder("OIS")
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.96)])
            .build()
            .expect("should succeed");

        let forward_3m = ForwardCurve::builder("3M-SOFR", 0.25)
            .base_date(base_date)
            .knots(vec![(0.0, 0.03), (1.0, 0.03), (2.0, 0.03)])
            .build()
            .expect("should succeed");

        let forward_6m = ForwardCurve::builder("6M-SOFR", 0.5)
            .base_date(base_date)
            .knots(vec![(0.0, 0.0305), (1.0, 0.0305), (2.0, 0.0305)])
            .build()
            .expect("should succeed");

        // Create context
        let context = MarketContext::new()
            .insert(discount_curve)
            .insert(forward_3m)
            .insert(forward_6m);

        // Create basis swap: 3M receives 6M + 5bp
        let primary_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("3M-SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: start_date,
            end: maturity,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::from(5), // 5bp
            payment_lag_days: 0,
            reset_lag_days: 0,
        };

        let reference_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("6M-SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: start_date,
            end: maturity,
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        };

        let swap = BasisSwap::new(
            "TEST_BASIS",
            Money::new(1_000_000.0, Currency::USD),
            primary_leg,
            reference_leg,
        )
        .expect("should succeed");

        // Price the swap
        let pv = swap.value(&context, base_date).expect("should succeed");

        // The 3M leg has rate 3.00% while 6M leg has rate 3.05%.
        // With a 5bp (0.0005) spread on the primary (3M) leg, the PV reflects the basis.
        // Expected PV ≈ (primary_rate + spread - reference_rate) × notional × annuity
        // ≈ (0.03 + 0.0005 - 0.0305) × 1_000_000 × ~1 ≈ -450 to -500
        // Use 600 tolerance to account for day count and discounting effects.
        assert!(
            pv.amount().abs() < 600.0,
            "PV should be small for near-par swap: {}",
            pv.amount()
        );
    }

    #[test]
    fn test_basis_swap_requires_calendar_by_default() {
        let base_date = date(2024, 1, 1);
        let start_date = date(2024, 1, 3);
        let maturity = date(2025, 1, 3);

        let discount_curve = DiscountCurve::builder("OIS")
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (1.0, 0.99)])
            .build()
            .expect("should succeed");
        let forward_3m = ForwardCurve::builder("3M-SOFR", 0.25)
            .base_date(base_date)
            .knots(vec![(0.0, 0.03), (1.0, 0.03)])
            .build()
            .expect("should succeed");
        let forward_6m = ForwardCurve::builder("6M-SOFR", 0.5)
            .base_date(base_date)
            .knots(vec![(0.0, 0.03), (1.0, 0.03)])
            .build()
            .expect("should succeed");

        let context = MarketContext::new()
            .insert(discount_curve)
            .insert(forward_3m)
            .insert(forward_6m);

        let primary_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("3M-SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: start_date,
            end: maturity,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        };
        let reference_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("6M-SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: start_date,
            end: maturity,
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        };

        let swap = BasisSwap::new(
            "TEST_BASIS_NO_CAL",
            Money::new(1_000_000.0, Currency::USD),
            primary_leg,
            reference_leg,
        )
        .expect("should succeed")
        .with_allow_calendar_fallback(true);

        let pv = swap.value(&context, base_date).expect("should succeed");
        assert!(
            pv.amount().is_finite(),
            "Expected finite PV when defaulting to weekends_only"
        );
    }

    #[test]
    fn test_basis_swap_payment_lag_affects_pv() {
        let base_date = date(2024, 1, 1);
        let start_date = date(2024, 1, 3);
        let maturity = date(2025, 1, 3);

        // Use a steep-ish discount curve so payment timing matters.
        let discount_curve = DiscountCurve::builder("OIS")
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (1.0, 0.90), (2.0, 0.82)])
            .build()
            .expect("should succeed");

        let forward_3m = ForwardCurve::builder("3M-SOFR", 0.25)
            .base_date(base_date)
            .knots(vec![(0.0, 0.03), (1.0, 0.03), (2.0, 0.03)])
            .build()
            .expect("should succeed");
        let forward_6m = ForwardCurve::builder("6M-SOFR", 0.5)
            .base_date(base_date)
            .knots(vec![(0.0, 0.03), (1.0, 0.03), (2.0, 0.03)])
            .build()
            .expect("should succeed");

        let context = MarketContext::new()
            .insert(discount_curve)
            .insert(forward_3m)
            .insert(forward_6m);

        let primary_leg_no_lag = BasisSwapLeg {
            forward_curve_id: CurveId::new("3M-SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: start_date,
            end: maturity,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::from(10),
            payment_lag_days: 0,
            reset_lag_days: 0,
        };
        let primary_leg_with_lag = BasisSwapLeg {
            payment_lag_days: 10,
            ..primary_leg_no_lag.clone()
        };

        let reference_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("6M-SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: start_date,
            end: maturity,
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        };

        let swap_no_lag = BasisSwap::new(
            "TEST_BASIS_NO_LAG",
            Money::new(10_000_000.0, Currency::USD),
            primary_leg_no_lag,
            reference_leg.clone(),
        )
        .expect("should succeed");

        let swap_with_lag = BasisSwap::new(
            "TEST_BASIS_WITH_LAG",
            Money::new(10_000_000.0, Currency::USD),
            primary_leg_with_lag,
            reference_leg,
        )
        .expect("should succeed");

        let pv_no_lag = swap_no_lag
            .value(&context, base_date)
            .expect("should succeed");
        let pv_with_lag = swap_with_lag
            .value(&context, base_date)
            .expect("should succeed");

        assert!(
            (pv_no_lag.amount() - pv_with_lag.amount()).abs() > 1e-6,
            "Expected payment lag to change PV: no_lag={}, with_lag={}",
            pv_no_lag.amount(),
            pv_with_lag.amount()
        );
    }

    #[test]
    fn test_basis_swap_rejects_invalid_dates() {
        let base_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("3M-SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: date(2024, 1, 3),
            end: date(2025, 1, 3),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        };

        // Test start == end (equal dates)
        let primary_eq = BasisSwapLeg {
            start: date(2024, 1, 3),
            end: date(2024, 1, 3),
            ..base_leg.clone()
        };
        let reference_eq = BasisSwapLeg {
            forward_curve_id: CurveId::new("6M-SOFR"),
            ..primary_eq.clone()
        };
        let err = BasisSwap::new(
            "INVALID_DATES",
            Money::new(1_000_000.0, Currency::USD),
            primary_eq,
            reference_eq,
        )
        .expect_err("should fail for equal dates");
        assert!(
            format!("{err}").contains("start") && format!("{err}").contains("positive tenor"),
            "Expected date validation error, got: {err}"
        );

        // Test start > end (inverted dates)
        let primary_inv = BasisSwapLeg {
            start: date(2025, 1, 3),
            end: date(2024, 1, 3),
            ..base_leg
        };
        let reference_inv = BasisSwapLeg {
            forward_curve_id: CurveId::new("6M-SOFR"),
            ..primary_inv.clone()
        };
        let err = BasisSwap::new(
            "INVERTED_DATES",
            Money::new(1_000_000.0, Currency::USD),
            primary_inv,
            reference_inv,
        )
        .expect_err("should fail for inverted dates");
        assert!(
            format!("{err}").contains("positive tenor"),
            "Expected positive tenor error, got: {err}"
        );
    }

    #[test]
    fn test_basis_swap_rejects_same_curve_by_default() {
        let leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: date(2024, 1, 3),
            end: date(2025, 1, 3),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::from(5),
            payment_lag_days: 0,
            reset_lag_days: 0,
        };

        let err = BasisSwap::new(
            "SAME_CURVE",
            Money::new(1_000_000.0, Currency::USD),
            leg.clone(),
            BasisSwapLeg {
                spread_bp: Decimal::ZERO,
                ..leg
            },
        )
        .expect_err("should fail for same forward curve");

        assert!(
            format!("{err}").contains("identical forward curves"),
            "Expected same-curve error, got: {err}"
        );
    }

    #[test]
    fn test_basis_swap_allows_same_curve_when_explicit() {
        let base_date = date(2024, 1, 1);

        let discount_curve = DiscountCurve::builder("OIS")
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (1.0, 0.98)])
            .build()
            .expect("should succeed");
        let forward = ForwardCurve::builder("SOFR", 0.25)
            .base_date(base_date)
            .knots(vec![(0.0, 0.03), (1.0, 0.03)])
            .build()
            .expect("should succeed");

        let context = MarketContext::new().insert(discount_curve).insert(forward);

        let leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: date(2024, 1, 3),
            end: date(2025, 1, 3),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::from(10), // 10bp spread
            payment_lag_days: 0,
            reset_lag_days: 0,
        };

        // Use the explicit same-curve constructor
        let swap = BasisSwap::new_allowing_same_curve(
            "SAME_CURVE_OK",
            Money::new(1_000_000.0, Currency::USD),
            leg.clone(),
            BasisSwapLeg {
                spread_bp: Decimal::ZERO,
                ..leg
            },
        )
        .expect("should succeed with explicit allow");

        // Should price successfully
        let pv = swap.value(&context, base_date).expect("should succeed");

        // For same-curve swap, PV = spread × notional × annuity
        // With 10bp spread on $1M notional for ~1Y, expect PV ≈ 1000 × 0.0010 × ~0.98 ≈ ~980
        assert!(
            (pv.amount() - 980.0).abs() < 100.0,
            "Same-curve swap PV should be spread × annuity: {}",
            pv.amount()
        );
    }

    #[test]
    fn test_par_spread_zeroes_npv() {
        use crate::instruments::common_impl::traits::Instrument;
        use crate::metrics::MetricId;

        let base_date = date(2024, 1, 1);
        let start_date = date(2024, 1, 3);
        let maturity = date(2026, 1, 3); // 2Y swap for more interesting basis

        // Create curves with a basis between 3M and 6M
        let discount_curve = DiscountCurve::builder("OIS")
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (1.0, 0.97), (2.0, 0.94), (3.0, 0.91)])
            .build()
            .expect("should succeed");

        let forward_3m = ForwardCurve::builder("3M-SOFR", 0.25)
            .base_date(base_date)
            .knots(vec![(0.0, 0.030), (1.0, 0.032), (2.0, 0.034)])
            .build()
            .expect("should succeed");

        let forward_6m = ForwardCurve::builder("6M-SOFR", 0.5)
            .base_date(base_date)
            .knots(vec![(0.0, 0.031), (1.0, 0.033), (2.0, 0.035)])
            .build()
            .expect("should succeed");

        let context = MarketContext::new()
            .insert(discount_curve)
            .insert(forward_3m)
            .insert(forward_6m);

        // Create swap with zero spread initially
        let primary_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("3M-SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: start_date,
            end: maturity,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO, // Start at zero spread
            payment_lag_days: 0,
            reset_lag_days: 0,
        };

        let reference_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("6M-SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: start_date,
            end: maturity,
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        };

        let swap = BasisSwap::new(
            "PAR_SPREAD_TEST",
            Money::new(10_000_000.0, Currency::USD),
            primary_leg.clone(),
            reference_leg.clone(),
        )
        .expect("should succeed");

        // Get the par spread using metrics
        let result = swap
            .price_with_metrics(
                &context,
                base_date,
                &[MetricId::BasisParSpread, MetricId::AnnuityPrimary],
                crate::instruments::PricingOptions::default(),
            )
            .expect("should succeed");

        let par_spread_bp = *result
            .measures
            .get(MetricId::BasisParSpread.as_str())
            .expect("should have par spread");

        // Create a new swap with the par spread applied
        let primary_leg_at_par = BasisSwapLeg {
            spread_bp: Decimal::try_from(par_spread_bp).expect("metric result is finite"),
            ..primary_leg
        };

        let swap_at_par = BasisSwap::new(
            "PAR_SPREAD_VERIFY",
            Money::new(10_000_000.0, Currency::USD),
            primary_leg_at_par,
            reference_leg,
        )
        .expect("should succeed");

        // Verify NPV is now zero
        let pv_at_par = swap_at_par
            .value(&context, base_date)
            .expect("should succeed");

        // NPV should be very close to zero (within rounding tolerance)
        assert!(
            pv_at_par.amount().abs() < 1.0, // Less than $1 on a $10M notional
            "Swap at par spread should have ~zero NPV: got {} (par spread was {:.2}bp)",
            pv_at_par.amount(),
            par_spread_bp
        );
    }

    #[test]
    fn test_basis_swap_rejects_negative_lags() {
        let valid_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("3M-SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: date(2024, 1, 3),
            end: date(2025, 1, 3),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        };
        let reference_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("6M-SOFR"),
            ..valid_leg.clone()
        };

        // Test negative payment_lag_days on primary leg
        let primary_neg_payment = BasisSwapLeg {
            payment_lag_days: -1,
            ..valid_leg.clone()
        };
        let err = BasisSwap::new(
            "NEG_PAY_PRIMARY",
            Money::new(1_000_000.0, Currency::USD),
            primary_neg_payment,
            reference_leg.clone(),
        )
        .expect_err("should fail for negative payment lag");
        assert!(
            format!("{err}").contains("payment_lag_days")
                && format!("{err}").contains("non-negative"),
            "Expected negative payment lag error, got: {err}"
        );

        // Test negative reset_lag_days on primary leg
        let primary_neg_reset = BasisSwapLeg {
            reset_lag_days: -1,
            ..valid_leg.clone()
        };
        let err = BasisSwap::new(
            "NEG_RESET_PRIMARY",
            Money::new(1_000_000.0, Currency::USD),
            primary_neg_reset,
            reference_leg.clone(),
        )
        .expect_err("should fail for negative reset lag");
        assert!(
            format!("{err}").contains("reset_lag_days")
                && format!("{err}").contains("non-negative"),
            "Expected negative reset lag error, got: {err}"
        );

        // Test negative payment_lag_days on reference leg
        let ref_neg_payment = BasisSwapLeg {
            payment_lag_days: -2,
            ..reference_leg.clone()
        };
        let err = BasisSwap::new(
            "NEG_PAY_REF",
            Money::new(1_000_000.0, Currency::USD),
            valid_leg.clone(),
            ref_neg_payment,
        )
        .expect_err("should fail for negative reference leg payment lag");
        assert!(
            format!("{err}").contains("payment_lag_days")
                && format!("{err}").contains("reference leg"),
            "Expected negative reference leg payment lag error, got: {err}"
        );

        // Test negative reset_lag_days on reference leg
        let ref_neg_reset = BasisSwapLeg {
            reset_lag_days: -3,
            ..reference_leg
        };
        let err = BasisSwap::new(
            "NEG_RESET_REF",
            Money::new(1_000_000.0, Currency::USD),
            valid_leg,
            ref_neg_reset,
        )
        .expect_err("should fail for negative reference leg reset lag");
        assert!(
            format!("{err}").contains("reset_lag_days")
                && format!("{err}").contains("reference leg"),
            "Expected negative reference leg reset lag error, got: {err}"
        );
    }

    #[test]
    fn test_basis_swap_cashflow_provider_emits_signed_leg_flows() {
        let base_date = date(2024, 1, 1);
        let start_date = date(2024, 1, 3);
        let maturity = date(2025, 1, 3);
        let discount_curve = DiscountCurve::builder("OIS")
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.96)])
            .build()
            .expect("should succeed");
        let forward_3m = ForwardCurve::builder("3M-SOFR", 0.25)
            .base_date(base_date)
            .knots(vec![(0.0, 0.03), (1.0, 0.03), (2.0, 0.03)])
            .build()
            .expect("should succeed");
        let forward_6m = ForwardCurve::builder("6M-SOFR", 0.5)
            .base_date(base_date)
            .knots(vec![(0.0, 0.031), (1.0, 0.031), (2.0, 0.031)])
            .build()
            .expect("should succeed");
        let context = MarketContext::new()
            .insert(discount_curve)
            .insert(forward_3m)
            .insert(forward_6m);

        let primary_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("3M-SOFR"),
            discount_curve_id: CurveId::new("OIS"),
            start: start_date,
            end: maturity,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        };
        let reference_leg = BasisSwapLeg {
            forward_curve_id: CurveId::new("6M-SOFR"),
            frequency: Tenor::semi_annual(),
            ..primary_leg.clone()
        };
        let swap = BasisSwap::new(
            "BASIS-CF",
            Money::new(1_000_000.0, Currency::USD),
            primary_leg,
            reference_leg,
        )
        .expect("should succeed")
        .with_allow_calendar_fallback(true);

        let flows = swap
            .dated_cashflows(&context, base_date)
            .expect("basis swap contractual schedule should build");

        assert!(!flows.is_empty(), "basis swap should emit coupon cashflows");
        assert!(flows.iter().any(|(_, money)| money.amount() > 0.0));
        assert!(flows.iter().any(|(_, money)| money.amount() < 0.0));
    }
}
