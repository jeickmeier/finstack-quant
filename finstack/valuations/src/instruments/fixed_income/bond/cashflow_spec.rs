//! Thin facade for bond cashflow specification.
//!
//! This module provides a clean, ergonomic API for bonds by wrapping the canonical
//! builder coupon specs (`FixedCouponSpec`, `FloatingCouponSpec`) with convenience
//! constructors that apply sensible defaults.
//!
//! # Features
//!
//! - Fixed-rate bonds with configurable coupon rates and frequencies
//! - Floating-rate notes (FRNs) with index spreads and margins
//! - Amortizing bonds with custom principal repayment schedules
//! - Step-up/step-down coupon bonds with scheduled rate changes
//! - Full parity with builder coupon specs (floors/caps, BDC, calendars, PIK, etc.)
//!
//! # Examples
//!
//! ```rust
//! use finstack_valuations::instruments::fixed_income::bond::CashflowSpec;
//! use finstack_core::dates::{Tenor, DayCount};
//! use finstack_core::types::Bps;
//!
//! // Fixed-rate bond: 5% annual coupon, semi-annual payments
//! let fixed = CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Thirty360);
//!
//! // Floating-rate note: SOFR + 200bps, quarterly payments
//! let floating = CashflowSpec::floating_bps(
//!     "USD-SOFR-3M".into(),
//!     Bps::new(200),  // margin in basis points
//!     Tenor::quarterly(),
//!     DayCount::Act360,
//! );
//! ```
//!
//! # See Also
//!
//! - [`crate::instruments::fixed_income::bond::Bond`] for bond construction using cashflow specs
//! - [`crate::cashflow::builder::specs`] for full builder coupon specifications

use crate::cashflow::builder::specs::{
    CouponType, FixedCouponSpec, FloatingCouponSpec, FloatingRateSpec, StepUpCouponSpec,
};
use crate::cashflow::builder::AmortizationSpec;
use crate::market::conventions::ids::IndexId;
use crate::market::conventions::ConventionRegistry;
use crate::market::conventions::RateIndexConventions;
use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_core::types::{Bps, CurveId, Rate};
use rust_decimal::Decimal;

fn rate_index_defaults(index_id: &CurveId) -> Option<RateIndexConventions> {
    let registry = ConventionRegistry::try_global().ok()?;
    let id = IndexId::new(index_id.as_str());
    registry.require_rate_index(&id).ok().cloned()
}

/// Parameters for [`CashflowSpec::from_bond_builder_params`].
///
/// Bundles all the flat fields from the Python bond builder so that the
/// function signature stays within the `too_many_arguments` threshold.
pub struct BondBuilderParams {
    /// Fixed coupon rate (used when `forward_curve` is `None`).
    pub coupon_rate: Decimal,
    /// Cash vs PIK coupon split.
    pub coupon_type: CouponType,
    /// Payment / reset frequency.
    pub frequency: Tenor,
    /// Day-count convention for accrual.
    pub day_count: DayCount,
    /// Business-day convention for date adjustment.
    pub bdc: BusinessDayConvention,
    /// Stub period treatment.
    pub stub: StubKind,
    /// Holiday calendar identifier; falls back to `"weekends_only"` when `None`.
    pub calendar_id: Option<String>,
    /// Forward curve id; when `Some`, the bond is treated as floating-rate.
    pub forward_curve: Option<CurveId>,
    /// Floating-leg margin in basis points (ignored for fixed-rate bonds).
    pub float_margin_bp: Decimal,
    /// Floating-leg gearing / multiplier (ignored for fixed-rate bonds).
    pub float_gearing: Decimal,
    /// Business-day lag between fixing and accrual start (ignored for fixed-rate bonds).
    pub float_reset_lag_days: i32,
    /// Optional amortization schedule; wraps the base spec when `Some`.
    pub amortization: Option<AmortizationSpec>,
}

/// Parameters for [`CashflowSpec::floating_with_conventions`].
pub struct FloatingConventionParams {
    /// Forward curve / rate index identifier (e.g. `"USD-SOFR-3M"`).
    pub index_id: CurveId,
    /// Spread over the index in basis points.
    pub spread_bp: Decimal,
    /// Index rate multiplier (1.0 = no leverage).
    pub gearing: Decimal,
    /// Business days between observation and accrual start.
    pub reset_lag_days: i32,
    /// Cash vs PIK coupon split.
    pub coupon_type: CouponType,
    /// Payment / reset frequency.
    pub freq: Tenor,
    /// Day-count convention for accrual.
    pub dc: DayCount,
    /// Business-day convention for date adjustment.
    pub bdc: BusinessDayConvention,
    /// Holiday calendar identifier.
    pub calendar_id: String,
    /// Stub period treatment.
    pub stub: StubKind,
}

/// Thin facade over canonical builder coupon specs for bond cashflows.
///
/// Wraps `FixedCouponSpec` and `FloatingCouponSpec` from the cashflow builder,
/// providing convenience constructors with sensible defaults for common bond use cases.
/// This ensures parity with all builder features (floors/caps, BDC, calendars, PIK, etc.)
/// while keeping the bond API simple.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum CashflowSpec {
    /// Fixed-rate bond using the canonical `FixedCouponSpec`.
    Fixed(FixedCouponSpec),

    /// Floating-rate note using the canonical `FloatingCouponSpec`.
    Floating(FloatingCouponSpec),

    /// Step-up/step-down coupon bond with scheduled rate changes.
    StepUp(StepUpCouponSpec),

    /// Amortizing bond (principal payments during life).
    Amortizing {
        /// Base cashflow specification (fixed or floating).
        base: Box<CashflowSpec>,
        /// Amortization schedule.
        schedule: AmortizationSpec,
    },
}

impl CashflowSpec {
    /// Create a fixed-rate specification with sensible defaults.
    ///
    /// # Arguments
    ///
    /// * `coupon` - Annual coupon rate as decimal (e.g., 0.05 for 5%)
    /// * `freq` - Payment frequency (e.g., `Tenor::semi_annual()`)
    /// * `dc` - Day count convention (e.g., `DayCount::Thirty360`)
    ///
    /// # Defaults
    ///
    /// - `coupon_type`: Cash (100% cash payment)
    /// - `bdc`: Following
    /// - `stub`: None
    /// - `calendar_id`: "weekends_only"
    ///
    /// # Returns
    ///
    /// A `CashflowSpec::Fixed` variant with the specified coupon rate, frequency, and day count.
    ///
    /// # Input validation note
    ///
    /// This convenience constructor keeps existing behavior: after the debug-only
    /// finite-value assertion above, any failed `f64 -> Decimal` conversion falls
    /// back to `0`. External callers that need hard validation should prefer the
    /// typed constructors or builder APIs that accept `Decimal` directly.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::instruments::fixed_income::bond::CashflowSpec;
    /// use finstack_core::dates::{Tenor, DayCount};
    /// use finstack_core::types::Rate;
    ///
    /// // US Treasury-style: 4% coupon, semi-annual, 30/360
    /// let spec = CashflowSpec::fixed_rate(
    ///     Rate::from_percent(4.0),
    ///     Tenor::semi_annual(),
    ///     DayCount::Thirty360,
    /// );
    /// ```
    ///
    /// # See Also
    ///
    /// For full control (PIK, custom calendars, stubs), construct `FixedCouponSpec` directly
    /// and wrap in `CashflowSpec::Fixed(...)`.
    #[allow(clippy::expect_used)] // Builder with valid inputs should not fail
    pub fn fixed(coupon: f64, freq: Tenor, dc: DayCount) -> Self {
        debug_assert!(
            coupon.is_finite(),
            "CashflowSpec::fixed: coupon is not finite ({coupon})"
        );
        let rate = Decimal::try_from(coupon).unwrap_or(Decimal::ZERO);
        Self::Fixed(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate,
            freq,
            dc,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
        })
    }

    /// Create a fixed-rate specification using a typed rate.
    pub fn fixed_rate(coupon: Rate, freq: Tenor, dc: DayCount) -> Self {
        let rate = Decimal::try_from(coupon.as_decimal()).unwrap_or(Decimal::ZERO);
        Self::Fixed(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate,
            freq,
            dc,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
        })
    }

    /// Create a floating-rate specification with sensible defaults.
    ///
    /// # Arguments
    ///
    /// * `index_id` - Forward curve identifier (e.g., "USD-SOFR-3M")
    /// * `margin_bp` - Spread over index in basis points (e.g., 200.0 for 200bps)
    /// * `freq` - Payment frequency (e.g., `Tenor::quarterly()`)
    /// * `dc` - Day count convention (e.g., `DayCount::Act360`)
    ///
    /// # Defaults
    ///
    /// - `coupon_type`: Cash (100% cash payment)
    /// - `gearing`: 1.0
    /// - `reset_lag_days`: Market default from index registry (fallback: T-2)
    /// - `index_floor_bp`: None
    /// - `all_in_cap_bp`: None
    /// - `reset_freq`: Same as payment frequency
    /// - `bdc`: Following
    /// - `stub`: None
    /// - `calendar_id`: Market default from index registry (fallback: "weekends_only")
    ///
    /// # Market Conventions for Reset Lag
    ///
    /// Different indices use different reset lag conventions:
    /// - **SOFR**: T-2 (2 business days before period start)
    /// - **EURIBOR**: T-2
    /// - **LIBOR (historical)**: T-0 to T-2 depending on currency
    /// - **SONIA**: T-0 (same day)
    ///
    /// Use `floating_with_reset_lag()` to specify a non-default reset lag.
    ///
    /// # Returns
    ///
    /// A `CashflowSpec::Floating` variant with the specified index, margin, frequency, and day count.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::instruments::fixed_income::bond::CashflowSpec;
    /// use finstack_core::dates::{Tenor, DayCount};
    /// use finstack_core::types::CurveId;
    ///
    /// // FRN: 3M SOFR + 200bps, quarterly payments (default T-2 reset)
    /// let spec = CashflowSpec::floating(
    ///     CurveId::new("USD-SOFR-3M"),
    ///     200.0,  // 200 basis points
    ///     Tenor::quarterly(),
    ///     DayCount::Act360,
    /// );
    /// ```
    ///
    /// # See Also
    ///
    /// - `floating_with_reset_lag()` for custom reset lag
    /// - For full control (floors/caps/gearing), construct `FloatingCouponSpec` directly
    ///   and wrap in `CashflowSpec::Floating(...)`.
    #[allow(clippy::expect_used)] // Builder with valid inputs should not fail
    pub fn floating(index_id: CurveId, margin_bp: f64, freq: Tenor, dc: DayCount) -> Self {
        let reset_lag = rate_index_defaults(&index_id)
            .map(|conv| conv.default_reset_lag_days)
            .unwrap_or(2);
        Self::floating_with_reset_lag(index_id, margin_bp, freq, dc, reset_lag)
    }

    /// Create a floating-rate specification using a typed margin in basis points.
    pub fn floating_bps(index_id: CurveId, margin_bp: Bps, freq: Tenor, dc: DayCount) -> Self {
        let spread_bp = Decimal::from(margin_bp.as_bps());
        let defaults = rate_index_defaults(&index_id);
        let reset_lag_days = defaults
            .as_ref()
            .map(|conv| conv.default_reset_lag_days)
            .unwrap_or(2);
        let calendar_id = defaults
            .map(|conv| conv.market_calendar_id)
            .unwrap_or_else(|| "weekends_only".to_string());
        Self::Floating(FloatingCouponSpec {
            rate_spec: FloatingRateSpec {
                index_id,
                spread_bp,
                gearing: Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_cap_bp: None,
                all_in_floor_bp: None,
                index_cap_bp: None,
                reset_freq: freq,
                reset_lag_days,
                dc,
                bdc: BusinessDayConvention::Following,
                calendar_id,
                fixing_calendar_id: None,
                end_of_month: false,
                payment_lag_days: 0,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
            },
            coupon_type: CouponType::Cash,
            freq,
            stub: StubKind::ShortFront,
        })
    }

    /// Create a floating-rate specification with explicit reset lag.
    ///
    /// # Arguments
    ///
    /// * `index_id` - Forward curve identifier (e.g., "USD-SOFR-3M")
    /// * `margin_bp` - Spread over index in basis points (e.g., 200.0 for 200bps)
    /// * `freq` - Payment frequency (e.g., `Tenor::quarterly()`)
    /// * `dc` - Day count convention (e.g., `DayCount::Act360`)
    /// * `reset_lag_days` - Number of business days before period start for rate fixing
    ///
    /// # Market Conventions for Reset Lag
    ///
    /// | Index | Standard Reset Lag |
    /// |-------|-------------------|
    /// | SOFR | T-2 (2 days) |
    /// | EURIBOR | T-2 (2 days) |
    /// | SONIA | T-0 (same day) |
    /// | TONA | T-2 (2 days) |
    /// | LIBOR (historical) | T-0 to T-2 |
    ///
    /// # Returns
    ///
    /// A `CashflowSpec::Floating` variant with the specified parameters.
    ///
    /// # Input validation note
    ///
    /// This convenience constructor retains the historical zero-fallback behavior
    /// for `margin_bp` after the debug-only finite-value assertion. Callers that
    /// require strict conversion semantics should construct `FloatingCouponSpec`
    /// directly with typed `Decimal` inputs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::instruments::fixed_income::bond::CashflowSpec;
    /// use finstack_core::dates::{Tenor, DayCount};
    /// use finstack_core::types::CurveId;
    ///
    /// // SONIA-linked FRN with T-0 reset (same day fixing)
    /// let sonia_frn = CashflowSpec::floating_with_reset_lag(
    ///     CurveId::new("GBP-SONIA"),
    ///     150.0,  // 150 basis points
    ///     Tenor::quarterly(),
    ///     DayCount::Act365F,
    ///     0,  // T-0 reset for SONIA
    /// );
    ///
    /// // SOFR-linked FRN with standard T-2 reset
    /// let sofr_frn = CashflowSpec::floating_with_reset_lag(
    ///     CurveId::new("USD-SOFR-3M"),
    ///     200.0,
    ///     Tenor::quarterly(),
    ///     DayCount::Act360,
    ///     2,  // T-2 reset for SOFR
    /// );
    /// ```
    pub fn floating_with_reset_lag(
        index_id: CurveId,
        margin_bp: f64,
        freq: Tenor,
        dc: DayCount,
        reset_lag_days: i32,
    ) -> Self {
        debug_assert!(
            margin_bp.is_finite(),
            "CashflowSpec::floating_with_reset_lag: margin_bp is not finite ({margin_bp})"
        );
        let spread_bp = Decimal::try_from(margin_bp).unwrap_or(Decimal::ZERO);
        let calendar_id = rate_index_defaults(&index_id)
            .map(|conv| conv.market_calendar_id)
            .unwrap_or_else(|| "weekends_only".to_string());
        Self::Floating(FloatingCouponSpec {
            rate_spec: FloatingRateSpec {
                index_id,
                spread_bp,
                gearing: Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_cap_bp: None,
                all_in_floor_bp: None,
                index_cap_bp: None,
                reset_freq: freq,
                reset_lag_days,
                dc,
                bdc: BusinessDayConvention::Following,
                calendar_id,
                fixing_calendar_id: None,
                end_of_month: false,
                payment_lag_days: 0,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
            },
            coupon_type: CouponType::Cash,
            freq,
            stub: StubKind::ShortFront,
        })
    }

    /// Create a floating-rate specification with explicit reset lag using a typed margin.
    pub fn floating_with_reset_lag_bps(
        index_id: CurveId,
        margin_bp: Bps,
        freq: Tenor,
        dc: DayCount,
        reset_lag_days: i32,
    ) -> Self {
        let spread_bp = Decimal::from(margin_bp.as_bps());
        let calendar_id = rate_index_defaults(&index_id)
            .map(|conv| conv.market_calendar_id)
            .unwrap_or_else(|| "weekends_only".to_string());
        Self::Floating(FloatingCouponSpec {
            rate_spec: FloatingRateSpec {
                index_id,
                spread_bp,
                gearing: Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_cap_bp: None,
                all_in_floor_bp: None,
                index_cap_bp: None,
                reset_freq: freq,
                reset_lag_days,
                dc,
                bdc: BusinessDayConvention::Following,
                calendar_id,
                fixing_calendar_id: None,
                end_of_month: false,
                payment_lag_days: 0,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
            },
            coupon_type: CouponType::Cash,
            freq,
            stub: StubKind::ShortFront,
        })
    }

    /// Create a fixed-rate specification with full convention control.
    ///
    /// Unlike [`fixed()`](Self::fixed) which applies hardcoded defaults, this
    /// constructor accepts all schedule conventions (BDC, calendar, stub, coupon
    /// type) so callers can thread through user-provided values while the method
    /// still fills in implementation-detail defaults (`end_of_month`, `payment_lag_days`).
    ///
    /// # Defaults filled in
    ///
    /// - `end_of_month`: `false`
    /// - `payment_lag_days`: `0`
    pub fn fixed_with_conventions(
        rate: Decimal,
        coupon_type: CouponType,
        freq: Tenor,
        dc: DayCount,
        bdc: BusinessDayConvention,
        calendar_id: String,
        stub: StubKind,
    ) -> Self {
        Self::Fixed(FixedCouponSpec {
            coupon_type,
            rate,
            freq,
            dc,
            bdc,
            calendar_id,
            stub,
            end_of_month: false,
            payment_lag_days: 0,
        })
    }

    /// Create a floating-rate specification with full convention control.
    ///
    /// Unlike [`floating()`](Self::floating) which applies hardcoded defaults
    /// and registry look-ups, this constructor accepts all schedule conventions
    /// and floating-rate knobs (`gearing`, `reset_lag_days`, `coupon_type`, etc.)
    /// so callers can thread through user-provided values.
    ///
    /// # Defaults filled in
    ///
    /// - `gearing_includes_spread`: `true`
    /// - `index_floor_bp` / `all_in_cap_bp` / `all_in_floor_bp` / `index_cap_bp`: `None`
    /// - `fixing_calendar_id`: `None` (falls back to `calendar_id`)
    /// - `end_of_month`: `false`
    /// - `payment_lag_days`: `0`
    /// - `overnight_compounding`: `None`
    /// - `overnight_basis`: `None`
    /// - `fallback`: `FloatingRateFallback::Error`
    pub fn floating_with_conventions(params: FloatingConventionParams) -> Self {
        Self::Floating(FloatingCouponSpec {
            rate_spec: FloatingRateSpec {
                index_id: params.index_id,
                spread_bp: params.spread_bp,
                gearing: params.gearing,
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_floor_bp: None,
                all_in_cap_bp: None,
                index_cap_bp: None,
                reset_freq: params.freq,
                reset_lag_days: params.reset_lag_days,
                dc: params.dc,
                bdc: params.bdc,
                calendar_id: params.calendar_id,
                fixing_calendar_id: None,
                end_of_month: false,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
                payment_lag_days: 0,
            },
            coupon_type: params.coupon_type,
            freq: params.freq,
            stub: params.stub,
        })
    }

    /// Create a step-up coupon specification with sensible defaults.
    ///
    /// # Arguments
    ///
    /// * `initial_rate` - Initial annual coupon rate as decimal (e.g., 0.03 for 3%)
    /// * `steps` - Schedule of (date, new_rate) pairs, sorted by date
    /// * `freq` - Payment frequency
    /// * `dc` - Day count convention
    ///
    /// # Defaults
    ///
    /// - `coupon_type`: Cash (100% cash payment)
    /// - `bdc`: Following
    /// - `stub`: None
    /// - `calendar_id`: "weekends_only"
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::instruments::fixed_income::bond::CashflowSpec;
    /// use finstack_core::dates::{Date, Tenor, DayCount};
    /// use time::Month;
    ///
    /// // Bond: 3% for years 1-3, then 4.5% for years 4-5
    /// let step_date = Date::from_calendar_date(2028, Month::January, 15).unwrap();
    /// let spec = CashflowSpec::step_up(
    ///     0.03,
    ///     vec![(step_date, 0.045)],
    ///     Tenor::semi_annual(),
    ///     DayCount::Thirty360,
    /// );
    /// ```
    ///
    /// # Input validation note
    ///
    /// This helper intentionally preserves the behavior of coercing any
    /// failed `f64 -> Decimal` conversion to `0` after debug assertions. Use a
    /// typed `StepUpCouponSpec` directly when invalid external data must fail
    /// fast instead of being normalized.
    pub fn step_up(
        initial_rate: f64,
        steps: Vec<(finstack_core::dates::Date, f64)>,
        freq: Tenor,
        dc: DayCount,
    ) -> Self {
        debug_assert!(
            initial_rate.is_finite(),
            "CashflowSpec::step_up: initial_rate is not finite ({initial_rate})"
        );
        let initial = Decimal::try_from(initial_rate).unwrap_or(Decimal::ZERO);
        let step_schedule: Vec<(finstack_core::dates::Date, Decimal)> = steps
            .into_iter()
            .map(|(d, r)| {
                debug_assert!(
                    r.is_finite(),
                    "CashflowSpec::step_up: step rate is not finite ({r})"
                );
                (d, Decimal::try_from(r).unwrap_or(Decimal::ZERO))
            })
            .collect();

        Self::StepUp(StepUpCouponSpec {
            coupon_type: CouponType::Cash,
            initial_rate: initial,
            step_schedule,
            freq,
            dc,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
        })
    }

    /// Create an amortizing bond specification.
    ///
    /// Combines a base cashflow specification (fixed or floating) with an amortization
    /// schedule that specifies principal repayments during the bond's life.
    ///
    /// # Arguments
    ///
    /// * `base` - Base cashflow specification (fixed or floating) for coupon payments
    /// * `schedule` - Amortization schedule specifying principal repayment dates and amounts
    ///
    /// # Returns
    ///
    /// A `CashflowSpec::Amortizing` variant combining the base spec with the amortization schedule.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::instruments::fixed_income::bond::CashflowSpec;
    /// use finstack_cashflows::builder::AmortizationSpec;
    /// use finstack_core::dates::{Tenor, DayCount, Date};
    /// use finstack_core::money::Money;
    /// use finstack_core::currency::Currency;
    /// use time::Month;
    ///
    /// // Base fixed-rate spec
    /// let base = CashflowSpec::fixed(0.05, Tenor::annual(), DayCount::Act365F);
    ///
    /// // Amortization: 1/3 principal each year
    /// let step1 = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    /// let step2 = Date::from_calendar_date(2027, Month::January, 1).unwrap();
    /// let maturity = Date::from_calendar_date(2028, Month::January, 1).unwrap();
    /// let amort = AmortizationSpec::StepRemaining {
    ///     schedule: vec![
    ///         (step1, Money::new(333_333.33, Currency::USD)),
    ///         (step2, Money::new(666_666.67, Currency::USD)),
    ///         (maturity, Money::new(0.0, Currency::USD)),
    ///     ],
    /// };
    ///
    /// let amortizing_spec = CashflowSpec::amortizing(base, amort);
    /// ```
    pub fn amortizing(base: CashflowSpec, schedule: AmortizationSpec) -> Self {
        Self::Amortizing {
            base: Box::new(base),
            schedule,
        }
    }

    /// Build a `CashflowSpec` from the common builder parameters used by the
    /// Python-facing bond builder.
    ///
    /// This is the canonical place for the "choose fixed vs floating, apply
    /// optional amortization, apply calendar fallback" logic so that it lives
    /// in the core crate rather than in the binding layer.
    ///
    /// # Calendar fallback
    ///
    /// If `params.calendar_id` is `None`, falls back to `"weekends_only"`.
    pub fn from_bond_builder_params(params: BondBuilderParams) -> Self {
        let BondBuilderParams {
            coupon_rate,
            coupon_type,
            frequency,
            day_count,
            bdc,
            stub,
            calendar_id,
            forward_curve,
            float_margin_bp,
            float_gearing,
            float_reset_lag_days,
            amortization,
        } = params;
        let calendar = calendar_id.unwrap_or_else(|| "weekends_only".to_string());
        let spec = if let Some(fwd) = forward_curve {
            Self::floating_with_conventions(FloatingConventionParams {
                index_id: fwd,
                spread_bp: float_margin_bp,
                gearing: float_gearing,
                reset_lag_days: float_reset_lag_days,
                coupon_type,
                freq: frequency,
                dc: day_count,
                bdc,
                calendar_id: calendar,
                stub,
            })
        } else {
            Self::fixed_with_conventions(
                coupon_rate,
                coupon_type,
                frequency,
                day_count,
                bdc,
                calendar,
                stub,
            )
        };
        if let Some(amort) = amortization {
            Self::amortizing(spec, amort)
        } else {
            spec
        }
    }

    /// Get the payment frequency from this specification.
    ///
    /// # Returns
    ///
    /// The payment frequency (e.g., `Tenor::semi_annual()`).
    ///
    /// For amortizing bonds, returns the frequency from the base specification.
    pub fn frequency(&self) -> Tenor {
        match self {
            Self::Fixed(spec) => spec.freq,
            Self::Floating(spec) => spec.freq,
            Self::StepUp(spec) => spec.freq,
            Self::Amortizing { base, .. } => base.frequency(),
        }
    }

    /// Get the day count convention from this specification.
    ///
    /// # Returns
    ///
    /// The day count convention (e.g., `DayCount::Thirty360`).
    ///
    /// For amortizing bonds, returns the day count from the base specification.
    pub fn day_count(&self) -> DayCount {
        match self {
            Self::Fixed(spec) => spec.dc,
            Self::Floating(spec) => spec.rate_spec.dc,
            Self::StepUp(spec) => spec.dc,
            Self::Amortizing { base, .. } => base.day_count(),
        }
    }

    /// Get the fixed annual coupon rate, if this specification has one.
    ///
    /// # Returns
    ///
    /// - `Some(rate)` for fixed-rate bonds (the coupon as a decimal, e.g. `0.05`).
    /// - `Some(initial_rate)` for step-up bonds (the rate in force at issue).
    /// - `Some(..)` for amortizing bonds whose base leg is fixed/step-up.
    /// - `None` for floating-rate bonds, which have no contractual fixed coupon.
    ///
    /// This is used by conventions (e.g. the CME bond-future conversion factor)
    /// that require the security's stated annual coupon.
    pub fn fixed_coupon_rate(&self) -> Option<f64> {
        match self {
            Self::Fixed(spec) => f64::try_from(spec.rate).ok(),
            Self::StepUp(spec) => f64::try_from(spec.initial_rate).ok(),
            Self::Floating(_) => None,
            Self::Amortizing { base, .. } => base.fixed_coupon_rate(),
        }
    }
}

impl Default for CashflowSpec {
    /// Default to semi-annual fixed bond with 30/360 day count (US convention).
    fn default() -> Self {
        Self::fixed(0.0, Tenor::semi_annual(), DayCount::Thirty360)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::Date;
    use time::Month;

    #[test]
    fn test_step_up_serde_roundtrip() {
        let original = CashflowSpec::step_up(
            0.03,
            vec![
                (
                    Date::from_calendar_date(2027, Month::January, 15).unwrap(),
                    0.045,
                ),
                (
                    Date::from_calendar_date(2029, Month::January, 15).unwrap(),
                    0.05,
                ),
            ],
            Tenor::semi_annual(),
            DayCount::Thirty360,
        );

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: CashflowSpec = serde_json::from_str(&json).expect("deserialize");

        // Verify the roundtrip preserves the variant and key properties
        match &deserialized {
            CashflowSpec::StepUp(spec) => {
                assert_eq!(spec.initial_rate, Decimal::try_from(0.03).unwrap());
                assert_eq!(spec.step_schedule.len(), 2);
                assert_eq!(spec.freq, Tenor::semi_annual());
                assert_eq!(spec.dc, DayCount::Thirty360);
            }
            other => panic!("Expected StepUp variant, got {:?}", other),
        }
    }

    #[test]
    fn test_step_up_convenience_constructor() {
        let spec = CashflowSpec::step_up(
            0.03,
            vec![(
                Date::from_calendar_date(2028, Month::January, 15).unwrap(),
                0.045,
            )],
            Tenor::semi_annual(),
            DayCount::Thirty360,
        );

        assert_eq!(spec.frequency(), Tenor::semi_annual());
        assert_eq!(spec.day_count(), DayCount::Thirty360);

        match &spec {
            CashflowSpec::StepUp(s) => {
                assert_eq!(s.initial_rate, Decimal::try_from(0.03).unwrap());
                assert_eq!(s.step_schedule.len(), 1);
                assert_eq!(s.coupon_type, CouponType::Cash);
            }
            other => panic!("Expected StepUp variant, got {:?}", other),
        }
    }
}
