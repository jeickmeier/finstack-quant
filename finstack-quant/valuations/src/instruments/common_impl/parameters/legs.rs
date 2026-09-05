//! Common leg specification types for interest rate and credit instruments.

use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::types::{CurveId, Percentage};
use rust_decimal::Decimal;

use serde::{Deserialize, Serialize};

/// Direction for instrument legs (universal for IRS, CDS, variance swaps, etc.)
///
/// For interest rate swaps: Pay = pay fixed/receive floating, Receive = receive fixed/pay floating
/// For credit default swaps: Pay = buy protection (pay premium), Receive = sell protection (receive premium)
/// For variance swaps: Pay = short variance, Receive = long variance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PayReceive {
    /// Pay the primary leg (fixed rate in IRS, protection premium in CDS, short variance)
    Pay,
    /// Receive the primary leg (fixed rate in IRS, protection premium in CDS, long variance)
    Receive,
}

impl PayReceive {
    /// Check if this is the payer side
    pub fn is_payer(&self) -> bool {
        matches!(self, Self::Pay)
    }

    /// Check if this is the receiver side
    pub fn is_receiver(&self) -> bool {
        matches!(self, Self::Receive)
    }

    /// Returns the sign multiplier (+1.0 for Receive, -1.0 for Pay).
    pub fn sign(&self) -> f64 {
        match self {
            PayReceive::Pay => -1.0,
            PayReceive::Receive => 1.0,
        }
    }
}

impl std::fmt::Display for PayReceive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayReceive::Pay => write!(f, "pay"),
            PayReceive::Receive => write!(f, "receive"),
        }
    }
}

impl std::str::FromStr for PayReceive {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pay" => Ok(PayReceive::Pay),
            "receive" => Ok(PayReceive::Receive),
            _ => Err(format!("Unknown pay/receive: '{}'. Valid: pay, receive", s)),
        }
    }
}

/// Method for calculating par rates in swaps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ParRateMethod {
    /// Use forward-curve based float PV over the schedule (market standard)
    ForwardBased,
    /// Use discount-curve ratio for bootstrapping
    DiscountRatio,
}

impl std::fmt::Display for ParRateMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParRateMethod::ForwardBased => write!(f, "forward_based"),
            ParRateMethod::DiscountRatio => write!(f, "discount_ratio"),
        }
    }
}

impl std::str::FromStr for ParRateMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "forward_based" => Ok(Self::ForwardBased),
            "discount_ratio" => Ok(Self::DiscountRatio),
            _ => Err(format!(
                "Unknown par rate method: '{}'. Valid: forward_based, discount_ratio",
                s
            )),
        }
    }
}

/// Specification for fixed rate legs in interest rate swaps
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FixedLegSpec {
    /// Discount curve identifier for pricing
    pub discount_curve_id: CurveId,
    /// Fixed rate (e.g., 0.05 for 5%)
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub rate: Decimal,
    /// Payment frequency
    pub frequency: Tenor,
    /// Day count convention for accrual
    pub day_count: DayCount,
    /// Business day convention for payment dates
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Optional calendar for business day adjustments
    pub calendar_id: Option<String>,
    /// Stub period handling rule
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Start date of the fixed leg
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// End date of the fixed leg
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
    /// Optional par-rate calculation method override
    pub par_method: Option<ParRateMethod>,
    /// If true, use simple interest on accrual fraction
    pub compounding_simple: bool,
    /// Payment lag in business days after period end (default: 0).
    ///
    /// Bloomberg OIS swaps typically use 2 business days payment lag.
    /// The actual payment date is adjusted from the period end date by
    /// this many business days using the leg's calendar.
    #[serde(default)]
    pub payment_lag_days: i32,
    /// End-of-month roll convention (default: false).
    ///
    /// When `true`, if the start date falls on the last business day of a month,
    /// all subsequent roll dates will also fall on the last business day of their
    /// respective months. This matches QuantLib's `MakeOIS` default behavior.
    ///
    /// # Market Standard
    ///
    /// Per ISDA 2006 Definitions Section 4.18, the End-of-Month convention should
    /// be applied when the effective date is the last business day of a month.
    /// Most professional systems (QuantLib, Bloomberg SWDF) default to `true`.
    #[serde(default)]
    pub end_of_month: bool,
}

impl FixedLegSpec {
    /// Validate the structural invariants of this fixed-leg specification.
    ///
    /// Enforces that the accrual period is well-formed: `start < end`. A leg
    /// with `start >= end` produces an empty or reversed schedule, which yields
    /// a silently wrong (typically zero) PV instead of an error.
    ///
    /// The `rate` field is a [`Decimal`], which is finite by construction
    /// (`rust_decimal` has no `NaN`/`infinity` representation), so no separate
    /// finiteness check is required for it.
    ///
    /// This struct is normally built from its public fields (e.g. via serde),
    /// so call `validate` after construction to enforce the invariant that a
    /// constructor would otherwise guarantee.
    ///
    /// # Errors
    /// Returns an error stating both dates when `start >= end`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        crate::instruments::common_impl::validation::validate_date_range_strict(
            self.start,
            self.end,
            "FixedLegSpec",
        )
    }
}

/// Specification for floating rate legs in interest rate swaps
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FloatLegSpec {
    /// Discount curve identifier for pricing
    pub discount_curve_id: CurveId,
    /// Forward curve identifier for rate projections
    pub forward_curve_id: CurveId,
    /// Spread in basis points added to the forward rate
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub spread_bp: Decimal,
    /// Payment frequency
    pub frequency: Tenor,
    /// Day count convention for accrual
    pub day_count: DayCount,
    /// Business day convention for payment dates
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Optional calendar for business day adjustments
    pub calendar_id: Option<String>,
    /// Stub period handling rule
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Reset lag in business days for floating rate fixing (default: 0).
    ///
    /// - **0** (default): fixing on the accrual start date. A swap whose first
    ///   accrual period starts on or after the valuation date then prices off
    ///   the forward curve without historical fixings.
    /// - **Positive** (e.g., 2): T-2 fixing (2 business days before accrual
    ///   start). Use `RateIndexConventions::default_reset_lag_days` (or
    ///   `InterestRateSwap::from_conventions`) for the market default of a
    ///   given index.
    /// - **Negative** (e.g., -1): resolved at pricing time to the registered
    ///   index convention default; rejected when the forward curve id is not a
    ///   registered rate index.
    #[serde(default)]
    pub reset_lag_days: i32,
    /// Optional calendar for rate fixing (reset lag)
    #[serde(default)]
    pub fixing_calendar_id: Option<String>,
    /// Start date of the floating leg
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// End date of the floating leg
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
    /// Compounding method for floating coupons.
    ///
    /// Determines how floating rate coupons are calculated:
    /// - `Simple` (default): LIBOR-style simple interest
    /// - `CompoundedInArrears`: SOFR/SONIA-style daily compounding
    ///
    /// # Implementation Notes
    ///
    /// Compounded-in-arrears is implemented for IRS pricing in `instruments::irs` with
    /// support for lookback and observation shift conventions. For seasoned (already
    /// started) compounded swaps, pricing requires explicit fixings for observation
    /// dates prior to `as_of`.
    #[serde(default)]
    pub compounding: crate::instruments::rates::irs::FloatingLegCompounding,
    /// Payment lag in business days after period end (default: 0).
    ///
    /// Bloomberg OIS swaps typically use 2 business days payment lag.
    /// The actual payment date is adjusted from the period end date by
    /// this many business days using the leg's calendar.
    #[serde(default)]
    pub payment_lag_days: i32,
    /// End-of-month roll convention (default: false).
    ///
    /// When `true`, if the start date falls on the last business day of a month,
    /// all subsequent roll dates will also fall on the last business day of their
    /// respective months. This matches QuantLib's `MakeOIS` default behavior.
    ///
    /// # Market Standard
    ///
    /// Per ISDA 2006 Definitions Section 4.18, the End-of-Month convention should
    /// be applied when the effective date is the last business day of a month.
    /// Most professional systems (QuantLib, Bloomberg SWDF) default to `true`.
    #[serde(default)]
    pub end_of_month: bool,
}

impl FloatLegSpec {
    /// Validate the structural invariants of this floating-leg specification.
    ///
    /// Enforces that the accrual period is well-formed: `start < end`. A leg
    /// with `start >= end` produces an empty or reversed schedule, which yields
    /// a silently wrong (typically zero) PV instead of an error.
    ///
    /// The `spread_bp` field is a [`Decimal`], which is finite by construction
    /// (`rust_decimal` has no `NaN`/`infinity` representation), so no separate
    /// finiteness check is required for it.
    ///
    /// This struct is normally built from its public fields (e.g. via serde),
    /// so call `validate` after construction to enforce the invariant that a
    /// constructor would otherwise guarantee.
    ///
    /// # Errors
    /// Returns an error stating both dates when `start >= end`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        crate::instruments::common_impl::validation::validate_date_range_strict(
            self.start,
            self.end,
            "FloatLegSpec",
        )
    }
}

/// Specification for basis swap legs (floating vs floating)
///
/// A basis swap leg represents one side of a floating-for-floating interest rate swap,
/// where two parties exchange payments linked to different floating rate indices
/// (e.g., 3M SOFR vs 6M SOFR).
///
/// Each leg owns its own dates, discount curve, schedule conventions, and calendar,
/// following the IRS leg-centric pattern used by `FixedLegSpec` and `FloatLegSpec`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BasisSwapLeg {
    /// Forward curve identifier for this leg
    pub forward_curve_id: CurveId,
    /// Discount curve identifier for present value calculations
    pub discount_curve_id: CurveId,
    /// Start date of the leg
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// End date of the leg
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
    /// Payment frequency for the leg
    pub frequency: Tenor,
    /// Day count convention for accrual calculations
    pub day_count: DayCount,
    /// Business day convention for date adjustments
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Optional calendar identifier for business day adjustments
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<String>,
    /// Stub period handling rule
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Spread added to the floating rate, in **basis points**.
    ///
    /// # Units
    ///
    /// - `Decimal::from(5)` represents 5 basis points (5bp)
    /// - `Decimal::from(100)` represents 100 basis points (1%)
    /// - `Decimal::from(-10)` represents -10 basis points
    ///
    /// This is consistent with `FloatLegSpec::spread_bp` and `PremiumLegSpec::spread_bp`.
    ///
    /// # Typical Market Range
    ///
    /// Basis spreads in liquid markets typically range from -50bp to +50bp.
    /// Values outside ±5000bp are considered extreme and
    /// will trigger a validation warning during pricing.
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub spread_bp: Decimal,
    /// Payment lag in business days after period end (default: 0).
    ///
    /// E.g., `payment_lag_days: 2` means payment occurs 2 business days after the
    /// accrual period end date.
    #[serde(default)]
    pub payment_lag_days: i32,
    /// Reset lag in business days before period start (default: 0).
    ///
    /// E.g., `reset_lag_days: 2` means the rate fixing occurs 2 business days before
    /// the accrual period start date. This follows standard market convention where
    /// fixing typically precedes the accrual period.
    #[serde(default)]
    pub reset_lag_days: i32,
    /// Overnight vs term compounding for this floating leg.
    ///
    /// Defaults to [`crate::instruments::rates::irs::FloatingLegCompounding::Simple`] so tenor-basis swaps
    /// (3s1s, EURIBOR 6s3s) keep term projection. Set a compounded variant
    /// for an overnight RFR leg such as SOFR OIS or €STR.
    #[serde(default)]
    pub compounding: crate::instruments::rates::irs::FloatingLegCompounding,
}

/// Specification for CDS premium legs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PremiumLegSpec {
    /// Start date of protection
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// End date of protection
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
    /// Payment frequency
    pub frequency: Tenor,
    /// Stub convention
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Business day convention
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Holiday calendar identifier
    pub calendar_id: Option<String>,
    /// Day count convention
    pub day_count: DayCount,
    /// Fixed spread in basis points (e.g., 100 = 100bp = 1%)
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub spread_bp: Decimal,
    /// Discount curve identifier
    pub discount_curve_id: CurveId,
}

/// Specification for CDS protection legs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProtectionLegSpec {
    /// Credit curve identifier for default probabilities
    pub credit_curve_id: CurveId,
    /// Recovery rate (0.0 to 1.0)
    pub recovery_rate: f64,
    /// Settlement delay in business days
    pub settlement_delay: u16,
}

impl ProtectionLegSpec {
    /// Create a new protection leg specification with validation.
    ///
    /// # Arguments
    /// * `credit_curve_id` - Identifier for the hazard/credit curve
    /// * `recovery_rate` - Recovery rate in [0.0, 1.0] (e.g., 0.4 = 40%)
    /// * `settlement_delay` - Settlement delay in business days
    ///
    /// # Errors
    /// Returns an error if `recovery_rate` is outside [0.0, 1.0].
    pub fn new(
        credit_curve_id: impl Into<CurveId>,
        recovery_rate: f64,
        settlement_delay: u16,
    ) -> finstack_quant_core::Result<Self> {
        Self::validate_recovery_rate(recovery_rate)?;
        Ok(Self {
            credit_curve_id: credit_curve_id.into(),
            recovery_rate,
            settlement_delay,
        })
    }

    /// Create a new protection leg specification using typed percentage recovery.
    ///
    /// # Arguments
    /// * `credit_curve_id` - Identifier for the hazard/credit curve
    /// * `recovery_rate` - Recovery rate as a percentage (e.g., 40.0 = 40%)
    /// * `settlement_delay` - Settlement delay in business days
    ///
    /// # Errors
    /// Returns an error if `recovery_rate` is outside [0.0, 1.0] in decimal terms.
    pub fn new_pct(
        credit_curve_id: impl Into<CurveId>,
        recovery_rate: Percentage,
        settlement_delay: u16,
    ) -> finstack_quant_core::Result<Self> {
        let recovery_rate_decimal = recovery_rate.as_decimal();
        Self::validate_recovery_rate(recovery_rate_decimal)?;
        Ok(Self {
            credit_curve_id: credit_curve_id.into(),
            recovery_rate: recovery_rate_decimal,
            settlement_delay,
        })
    }

    /// Validate that recovery rate is within valid bounds [0, 1].
    ///
    /// Delegates to the shared internal recovery-rate validator.
    ///
    /// # Errors
    /// Returns an error if recovery rate is outside the valid range.
    pub fn validate_recovery_rate(recovery_rate: f64) -> finstack_quant_core::Result<()> {
        crate::instruments::common_impl::validation::validate_recovery_rate(recovery_rate)
    }
}

// Note: Settlement type (cash/physical/auction) is descriptive-only and does not
// impact current pricing. It has been removed from `ProtectionLegSpec` to keep
// the pricing surface minimal and consistent. If needed, store as metadata in
// instrument `Attributes`.

/// Rate-compounding convention for a TRS financing leg.
///
/// Distinguishes how each accrual period's floating rate is projected from the
/// forward curve. The two conventions differ by the daily-compounding convexity
/// — typically 12–15 bp of rate at current levels on an upward-sloping curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FinancingRateCompounding {
    /// Term-rate financing (e.g. 3M Term SOFR, EURIBOR): the period rate is the
    /// simple arithmetic-average forward over the accrual period. This is the
    /// default and matches a conventional term-rate-funded TRS.
    #[default]
    TermRate,
    /// Overnight-indexed (OIS / RFR) financing — SOFR, SONIA, €STR, TONA: the
    /// period rate daily-compounds the overnight forward,
    /// `R = (∏(1 + rᵢ·dᵢ) − 1) / τ`, picking up the compounding convexity that
    /// the simple arithmetic average drops.
    OvernightCompounded,
}

/// Specification for TRS financing legs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FinancingLegSpec {
    /// Discount curve identifier for present value calculations
    pub discount_curve_id: CurveId,
    /// Forward curve identifier (e.g., USD-SOFR-3M)
    pub forward_curve_id: CurveId,
    /// Spread in basis points over the floating rate (e.g., 50 = 50bp = 0.5%)
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub spread_bp: Decimal,
    /// Day count convention for accrual calculations
    pub day_count: DayCount,
    /// Rate-compounding convention (term-rate vs overnight-compounded).
    ///
    /// Defaults to [`FinancingRateCompounding::TermRate`]. Set to
    /// [`FinancingRateCompounding::OvernightCompounded`] for SOFR/SONIA/€STR
    /// overnight-funded TRS so the financing rate captures daily-compounding
    /// convexity.
    #[serde(default)]
    pub compounding: FinancingRateCompounding,
}

impl FinancingLegSpec {
    /// Create a new financing leg specification.
    ///
    /// The compounding convention defaults to [`FinancingRateCompounding::TermRate`]
    /// for term indices and unknown ids. A registered overnight RFR forward id
    /// (for example `USD-SOFR-OIS`) is upgraded to
    /// [`FinancingRateCompounding::OvernightCompounded`]. Use
    /// [`FinancingLegSpec::with_compounding`] to override.
    ///
    /// # Arguments
    ///
    /// * `discount_curve_id` - Discount curve used to PV financing coupons.
    /// * `forward_curve_id` - Term or overnight financing index / forward curve.
    /// * `spread_bp` - Contractual financing spread in basis points.
    /// * `day_count` - Accrual day-count for the financing coupon.
    pub fn new(
        discount_curve_id: impl Into<String>,
        forward_curve_id: impl Into<String>,
        spread_bp: Decimal,
        day_count: DayCount,
    ) -> Self {
        let forward_curve_id = CurveId::new(forward_curve_id);
        let compounding = match crate::instruments::common_impl::pricing::overnight_conventions::compounding_from_index_id(
            forward_curve_id.as_str(),
        ) {
            Ok(Some(crate::instruments::rates::irs::FloatingLegCompounding::Simple))
            | Ok(None)
            | Err(_) => FinancingRateCompounding::TermRate,
            Ok(Some(_)) => FinancingRateCompounding::OvernightCompounded,
        };
        Self {
            discount_curve_id: CurveId::new(discount_curve_id),
            forward_curve_id,
            spread_bp,
            day_count,
            compounding,
        }
    }

    /// Set the rate-compounding convention (consuming builder style).
    ///
    /// # Arguments
    ///
    /// * `compounding` - Term-rate versus overnight-compounded convention applied when
    ///   projecting the TRS funding leg.
    pub fn with_compounding(mut self, compounding: FinancingRateCompounding) -> Self {
        self.compounding = compounding;
        self
    }

    /// Validate the curve identifiers required by the financing leg.
    ///
    /// # Arguments
    ///
    /// * `context` - Instrument name included in validation diagnostics.
    pub(crate) fn validate(&self, context: &str) -> finstack_quant_core::Result<()> {
        if self.discount_curve_id.as_str().trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} requires a non-empty financing discount_curve_id"
            )));
        }
        if self.forward_curve_id.as_str().trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} requires a non-empty financing forward_curve_id"
            )));
        }
        if matches!(self.compounding, FinancingRateCompounding::TermRate) {
            crate::instruments::common_impl::pricing::overnight_conventions::reject_simple_overnight(
                self.forward_curve_id.as_str(),
                &crate::instruments::rates::irs::FloatingLegCompounding::Simple,
            )?;
        }
        Ok(())
    }
}

/// Specification for TRS total return legs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct TotalReturnLegSpec {
    /// Reference index or asset identifier
    pub reference_id: String,
    /// Initial price/level (if known, otherwise fetched from market)
    pub initial_level: Option<f64>,
    /// Whether to include dividends/distributions in the return calculation
    pub include_distributions: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    fn fixed_leg(start: Date, end: Date) -> FixedLegSpec {
        FixedLegSpec {
            discount_curve_id: CurveId::new("USD-OIS"),
            rate: Decimal::new(5, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            start,
            end,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        }
    }

    fn float_leg(start: Date, end: Date) -> FloatLegSpec {
        FloatLegSpec {
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-SOFR"),
            spread_bp: Decimal::ZERO,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            start,
            end,
            compounding: crate::instruments::rates::irs::FloatingLegCompounding::default(),
            payment_lag_days: 0,
            end_of_month: false,
        }
    }

    #[test]
    fn fixed_leg_validate_rejects_non_increasing_dates() {
        // Failure mode: a leg with start >= end produces an empty or reversed
        // schedule and a silently wrong (typically zero) PV.
        let start = date!(2025 - 03 - 15);
        let end = date!(2030 - 03 - 15);

        // start == end
        let equal = fixed_leg(start, start)
            .validate()
            .expect_err("start == end must be rejected");
        assert!(
            equal.to_string().contains("FixedLegSpec"),
            "error should name the leg: {equal}"
        );

        // start > end (reversed)
        assert!(fixed_leg(end, start).validate().is_err());

        // well-formed leg passes
        assert!(fixed_leg(start, end).validate().is_ok());
    }

    #[test]
    fn float_leg_validate_rejects_non_increasing_dates() {
        // Failure mode: same as the fixed leg — a reversed/empty accrual window.
        let start = date!(2025 - 03 - 15);
        let end = date!(2030 - 03 - 15);

        let equal = float_leg(start, start)
            .validate()
            .expect_err("start == end must be rejected");
        assert!(
            equal.to_string().contains("FloatLegSpec"),
            "error should name the leg: {equal}"
        );

        assert!(float_leg(end, start).validate().is_err());
        assert!(float_leg(start, end).validate().is_ok());
    }

    #[test]
    fn float_leg_reset_lag_days_defaults_to_zero_on_the_wire() {
        // Failure mode: a leg written without `reset_lag_days` must fix on the
        // accrual start (0), never on a negative registry sentinel.
        let json = r#"{
            "discount_curve_id": "USD-OIS",
            "forward_curve_id": "USD-SOFR-3M",
            "spread_bp": "0",
            "frequency": {"count": 3, "unit": "months"},
            "day_count": "act_360",
            "calendar_id": null,
            "start": "2025-01-15",
            "end": "2030-01-15"
        }"#;
        let leg: FloatLegSpec = serde_json::from_str(json).expect("float leg without reset lag");
        assert_eq!(leg.reset_lag_days, 0);
        assert_eq!(leg.payment_lag_days, 0);
        assert!(!leg.end_of_month);
    }

    #[test]
    fn financing_new_upgrades_overnight_index() {
        let ois = FinancingLegSpec::new("USD-OIS", "USD-SOFR-OIS", Decimal::ZERO, DayCount::Act360);
        assert_eq!(
            ois.compounding,
            FinancingRateCompounding::OvernightCompounded
        );
        let term = FinancingLegSpec::new("USD-OIS", "USD-SOFR-3M", Decimal::ZERO, DayCount::Act360);
        assert_eq!(term.compounding, FinancingRateCompounding::TermRate);
    }

    #[test]
    fn financing_validate_rejects_term_rate_on_overnight_index() {
        let spec = FinancingLegSpec {
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-SOFR-OIS"),
            spread_bp: Decimal::ZERO,
            day_count: DayCount::Act360,
            compounding: FinancingRateCompounding::TermRate,
        };
        let err = spec
            .validate("TRS")
            .expect_err("TermRate on USD-SOFR-OIS must fail");
        assert!(
            format!("{err}").contains("Overnight RFR"),
            "expected overnight/TermRate rejection, got {err}"
        );
    }
}
