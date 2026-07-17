//! Coupon specification types for fixed and floating rate coupons.

use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::InputError;
use rust_decimal::Decimal;

use super::schedule::ScheduleParams;

/// Coupon cashflow type for fixed/floating coupons.
///
/// - `Cash`: 100% paid in cash.
/// - `PIK`: 100% capitalized into principal.
/// - `Split { cash_pct, pik_pct }`: percentages applied to the coupon amount.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub enum CouponType {
    /// Cash variant.
    #[default]
    Cash,
    /// PIK variant.
    PIK,
    /// Split variant.
    Split {
        /// Fraction of the coupon paid in cash, expressed as a decimal share in
        /// `[0, 1]`.
        cash_pct: Decimal,
        /// Fraction of the coupon capitalized as PIK, expressed as a decimal
        /// share in `[0, 1]`.
        pik_pct: Decimal,
    },
}

impl CouponType {
    /// Returns (cash_fraction, pik_fraction) as Decimal values.
    pub(crate) fn split_parts(self) -> finstack_quant_core::Result<(Decimal, Decimal)> {
        match self {
            CouponType::Cash => Ok((Decimal::ONE, Decimal::ZERO)),
            CouponType::PIK => Ok((Decimal::ZERO, Decimal::ONE)),
            CouponType::Split { cash_pct, pik_pct } => {
                // Validate within [0,1]
                if cash_pct < Decimal::ZERO
                    || cash_pct > Decimal::ONE
                    || pik_pct < Decimal::ZERO
                    || pik_pct > Decimal::ONE
                {
                    return Err(InputError::Invalid.into());
                }
                let sum = cash_pct + pik_pct;
                let tol = Decimal::new(1, 9); // 1e-9
                let diff = if sum >= Decimal::ONE {
                    sum - Decimal::ONE
                } else {
                    Decimal::ONE - sum
                };
                if diff <= tol {
                    Ok((cash_pct, pik_pct))
                } else {
                    Err(InputError::Invalid.into())
                }
            }
        }
    }
}

/// Fixed-rate coupon specification.
///
/// This type combines the coupon quote, payment behavior, and schedule
/// conventions required to emit a fixed-rate leg.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FixedCouponSpec {
    /// Coupon settlement behavior: cash, PIK, or an explicit split of the
    /// coupon amount.
    #[serde(default)]
    pub coupon_type: CouponType,
    /// Coupon rate as a decimal (e.g., 0.05 for 5%). Uses Decimal for exact representation.
    pub rate: Decimal,
    /// Accrual and payment schedule conventions.
    #[serde(flatten)]
    pub schedule: ScheduleParams,
}

/// Compounding method for overnight rate indices (SOFR, ESTR, SONIA).
///
/// Controls how daily overnight fixings are aggregated into a period rate
/// for floating rate coupons. The choice of compounding method affects both
/// the accrued amount and the payment timing/certainty.
///
/// # Market Conventions
///
/// | Index | Standard Method | Lookback | Reference |
/// |-------|----------------|----------|-----------|
/// | USD SOFR | CompoundedInArrears | 2 BD | ISDA 2021 |
/// | EUR €STR | CompoundedWithObservationShift | 2 BD | ECB |
/// | GBP SONIA | CompoundedWithObservationShift | 5 BD | BoE |
/// | JPY TONA | CompoundedInArrears | 2 BD | BoJ |
///
/// # Reference
///
/// - ISDA (2021). "IBOR Fallbacks Supplement." Section 7.
/// - ARRC (2020). "SOFR: A User's Guide." Federal Reserve Bank of New York.
/// - `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
/// - `docs/REFERENCES.md#isda-2006-definitions`
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub enum OvernightCompoundingMethod {
    /// Arithmetic (non-compounded) average of daily overnight fixings,
    /// weighted by accrual days: `Rate = (Σ rᵢ·dᵢ) / D`.
    ///
    /// This is a fully supported convention. It is the correct choice for
    /// instruments that contractually specify a simple-average overnight
    /// index (some bilateral loans and older FRNs) rather than the
    /// compounded ISDA 2021 convention. Use [`Self::CompoundedInArrears`]
    /// for standard SOFR/ESTR/TONA legs.
    SimpleAverage,

    /// Compounded in arrears with daily compounding (ISDA 2021 standard).
    ///
    /// ```text
    /// Rate = [∏(1 + r_i × d_i/360) - 1] × 360/D
    /// ```
    #[default]
    CompoundedInArrears,

    /// Compounded in arrears with lookback (shift observation period).
    ///
    /// Uses rates from `lookback_days` business days before each accrual date.
    CompoundedWithLookback {
        /// Number of business days to look back for rate observations.
        lookback_days: u32,
    },

    /// Compounded in arrears with lockout (rate cut-off near end of period).
    ///
    /// Uses the rate from `lockout_days` business days before period end for all
    /// remaining days in the period. With fixings `b_1..b_n` (where `b_n` is one
    /// business day before the exclusive period end), the cut-off fixing is
    /// `b_{n-lockout+1}` per ISDA 2021 Definitions §7 (rate cut-off) and the
    /// ARRC SOFR FRN conventions.
    CompoundedWithLockout {
        /// Number of business days before period end to freeze the rate.
        lockout_days: u32,
    },

    /// Compounded in arrears with observation shift.
    ///
    /// Both observation dates AND weights are shifted back by `shift_days`
    /// business days. This is the ISDA 2021 recommended convention for SOFR
    /// and the standard for GBP SONIA and EUR €STR.
    CompoundedWithObservationShift {
        /// Number of business days to shift observations.
        shift_days: u32,
    },
}

/// Default gearing for floating rates.
fn default_gearing() -> Decimal {
    Decimal::ONE
}

/// Default reset lag for floating rates (T-2 standard).
fn default_reset_lag() -> i32 {
    2
}

/// Policy for handling floating rate projection failures.
///
/// Controls what happens when a forward curve lookup fails during
/// cashflow emission. The default (`Error`) surfaces failures explicitly;
/// the other variants are explicit opt-in degradation modes for callers
/// that intentionally want a projected schedule without a forward curve.
///
/// # References
///
/// - `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
/// - `docs/REFERENCES.md#hull-options-futures`
#[derive(
    Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub enum FloatingRateFallback {
    /// Return an error with curve ID and reset date (strictest, safest).
    #[default]
    Error,
    /// Treat the index component as zero, so the coupon rate is the spread
    /// (plus any floors/caps/gearing). An explicit opt-in for spread-only
    /// projection when no forward curve is available; emits `warn!`.
    SpreadOnly,
    /// Use a fixed rate as the index component. Emits `info!`.
    ///
    /// The value is a **decimal annual rate**, not basis points: `0.045`
    /// means 4.5%. This differs from the bp-denominated spread/floor/cap
    /// fields on [`FloatingRateSpec`] because it substitutes directly for
    /// the projected index rate.
    FixedRate(rust_decimal::Decimal),
}

impl FloatingRateFallback {
    /// Returns `true` when the variant is the default (`Error`).
    ///
    /// Used by serde `skip_serializing_if` to omit the field from JSON
    /// when it carries the default value.
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Error)
    }
}

/// Where overnight index floors/caps are applied for daily-compounded rates.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub enum OvernightIndexConstraintApplication {
    /// Apply index floors/caps to each sampled daily fixing before compounding.
    ///
    /// This matches common floored SOFR loan conventions.
    #[default]
    Daily,
    /// Apply index floors/caps once to the compounded period index rate.
    ///
    /// This preserves the historical period-level behavior for contracts that
    /// explicitly define floors/caps on the period index rate.
    Period,
}

impl OvernightIndexConstraintApplication {
    /// Returns true for the default daily application mode.
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Daily)
    }
}

/// Canonical floating rate specification for all instruments.
///
/// Used by bonds, swaps, credit facilities, and structured products.
/// All instruments should compose this type rather than defining their own
/// floating rate specifications.
///
/// # Rate Calculation
///
/// The all-in rate is computed as:
/// 1. Look up forward rate from `index_id` curve for the accrual period
/// 2. Apply `index_floor_bp` to index rate (if specified) - applied BEFORE adding spread
/// 3. Add `spread_bp` to get base rate
/// 4. Multiply by `gearing` (typically 1.0)
/// 5. Apply `all_in_cap_bp` to final rate (if specified) - applied AFTER spread and gearing
///
/// Formula: `cap(gearing * (floor(index) + spread))`
///
/// # Negative Rate Handling
///
/// Negative index rates are supported and will flow through calculations
/// unless constrained by floors. For markets with negative rates (EUR, JPY, CHF):
///
/// - Set `index_floor_bp: Some(0.0)` to floor the index at zero
/// - Set `all_in_floor_bp: Some(0.0)` to floor the total coupon at zero
/// - Omit floors to allow negative coupons (rare but valid in some structures)
///
/// The implementation does not reject negative rates; the policy is controlled
/// by the floor configuration.
///
/// # Seasoned Instruments (Historical Fixings)
///
/// Historical fixings **are supported** via the `MarketContext`: store a
/// `ScalarTimeSeries` under the canonical id `FIXING:{index_id}` (see
/// `finstack_quant_core::market_data::fixings`) containing realized index
/// observations. Observation dates strictly before the forward curve base
/// date then resolve from that series instead of the curve:
///
/// - **Overnight observations** (compounded/averaged paths) use LOCF lookup
///   (last observation carried forward), matching RFR publication
///   conventions where a fixing carries over non-publication days
///   (ARRC 2020 SOFR conventions; ISDA 2021 Supp. 70 §7.1(g)). A partially
///   seasoned compounding window seamlessly mixes realized fixings and
///   curve-projected forwards with identical `(rate, days)` weighting.
/// - **Term-rate resets** use exact-date lookup on the (business-day
///   adjusted) reset date — a term rate fixes on a specific published date.
///   The fixing is the index rate only; gearing/spread/floors/caps apply on
///   top exactly as for projected rates.
///
/// An observation exactly on the curve base date prefers a published
/// same-day fixing when the series has one, otherwise projects from `t = 0`.
///
/// The [`FloatingRateFallback`] policy applies only when **no** fixing
/// series is provided: `Error` (the default) fails the build with a
/// descriptive message naming the date, index, and expected series id;
/// `FixedRate(r)` uses `r` as the index rate for the affected coupon;
/// `SpreadOnly` projects spread-only.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::dates::Tenor;
/// use finstack_quant_cashflows::builder::{FloatingRateSpec, OvernightIndexConstraintApplication};
/// use rust_decimal_macros::dec;
///
/// // 3M SOFR + 200bps with 0% floor
/// let spec = FloatingRateSpec {
///     index_id: "USD-SOFR-3M".into(),
///     spread_bp: dec!(200.0),
///     gearing: dec!(1.0),
///     gearing_includes_spread: true,
///     index_floor_bp: Some(dec!(0.0)),
///     all_in_floor_bp: None,
///     all_in_cap_bp: None,
///     index_cap_bp: None,
///     overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
///     reset_freq: Tenor::quarterly(),
///     index_tenor: None,
///     reset_lag_days: 2,
///     fixing_calendar_id: None,
///     overnight_compounding: None,
///     overnight_basis: None,
///     fallback: Default::default(),
/// };
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FloatingRateSpec {
    /// Forward curve identifier (e.g., "USD-SOFR-3M", "EUR-EURIBOR-6M").
    pub index_id: CurveId,

    /// Spread/margin over index in basis points. Uses Decimal for exact representation.
    pub spread_bp: Decimal,

    /// Gearing/leverage multiplier applied to the all-in rate (default: 1.0).
    ///
    /// Example: gearing = 2.0 means the rate is doubled.
    ///
    /// **Restriction:** gearing must be strictly positive (`gearing > 0`);
    /// projection rejects zero or negative gearing, so inverse floaters
    /// (negative gearing) are not currently expressible with this field.
    #[serde(default = "default_gearing")]
    pub gearing: Decimal,

    /// Whether gearing includes the spread (default: true).
    ///
    /// - `true`: `rate = (index + spread) * gearing`
    /// - `false`: `rate = (index * gearing) + spread` (Affine model)
    #[serde(default = "default_gearing_includes_spread")]
    pub gearing_includes_spread: bool,

    /// Floor on index rate in basis points (applied to index component).
    ///
    /// Example: index_floor_bp = Some(0.0) ensures index rate >= 0%.
    #[serde(default, alias = "floor_bp")]
    pub index_floor_bp: Option<Decimal>,

    /// Floor on all-in rate in basis points (Min Coupon).
    ///
    /// Applied to the final calculated rate after gearing and spread.
    #[serde(default)]
    pub all_in_floor_bp: Option<Decimal>,

    /// Cap on all-in rate in basis points (applied after spread and gearing).
    ///
    /// Example: all_in_cap_bp = Some(1000.0) ensures all-in rate <= 10%.
    #[serde(default, alias = "cap_bp")]
    pub all_in_cap_bp: Option<Decimal>,

    /// Cap on index rate in basis points (applied to index component).
    #[serde(default)]
    pub index_cap_bp: Option<Decimal>,

    /// Index floor/cap application policy for overnight-compounded coupons.
    #[serde(
        default,
        skip_serializing_if = "OvernightIndexConstraintApplication::is_default"
    )]
    pub overnight_index_constraints: OvernightIndexConstraintApplication,

    /// Reset frequency for rate fixings.
    ///
    /// This is the cadence at which the rate refixes; it also serves as the
    /// default index tenor when [`Self::index_tenor`] is `None`.
    pub reset_freq: Tenor,

    /// Underlying index tenor used to project the forward rate (term rates).
    ///
    /// The forward rate is projected over
    /// `[accrual_start, accrual_start + index_tenor]`. When `None` (the serde
    /// default), the index tenor falls back to [`Self::reset_freq`]. Set this
    /// explicitly when the reset cadence differs from the index's underlying
    /// deposit period (e.g. a monthly-paying leg referencing a 3M index).
    /// Ignored for overnight-compounded legs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_tenor: Option<Tenor>,

    /// Reset lag in business days (e.g., 2 for T-2 SOFR convention).
    #[serde(default = "default_reset_lag")]
    pub reset_lag_days: i32,

    /// Optional calendar for rate fixing (reset lag).
    ///
    /// If not provided, defaults to the coupon schedule calendar.
    #[serde(default)]
    pub fixing_calendar_id: Option<String>,

    /// Overnight compounding method for overnight rate indices (SOFR, ESTR, SONIA).
    ///
    /// When set to `Some(method)`, the rate for each accrual period is computed
    /// by compounding daily overnight fixings according to the specified method,
    /// rather than looking up a single forward rate for the period.
    ///
    /// Leave as `None` for term rates (e.g., 3M EURIBOR, 6M LIBOR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overnight_compounding: Option<OvernightCompoundingMethod>,

    /// Day-count basis for the overnight compounding denominator.
    ///
    /// This controls the annualization factor used when compounding daily
    /// overnight fixings (e.g., 360 for SOFR/ESTR/TONA, 365 for SONIA).
    /// It is independent of the leg's accrual day count (`dc`), which
    /// governs the coupon year fraction.
    ///
    /// Defaults to `Act/360` when `None`, matching SOFR/ESTR/TONA
    /// convention. Set to `Act/365F` for SONIA.
    /// Ignored when `overnight_compounding` is `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overnight_basis: Option<DayCount>,

    /// Policy when forward curve lookup fails during emission.
    ///
    /// Defaults to `Error`, which surfaces curve lookup failures.
    /// Set to `SpreadOnly` for spread-only projection, or `FixedRate(r)`
    /// to use a fixed index rate.
    #[serde(default, skip_serializing_if = "FloatingRateFallback::is_default")]
    pub fallback: FloatingRateFallback,
}

impl FloatingRateSpec {
    /// Validates the floating rate specification.
    ///
    /// # Validation Rules
    ///
    /// - `reset_lag_days` must be non-negative (fixing before accrual start)
    /// - Index floor must not exceed index cap (if both specified)
    /// - All-in floor must not exceed all-in cap (if both specified)
    ///
    /// # Errors
    ///
    /// Returns an error if the reset lag is negative, an index floor exceeds
    /// its index cap, or an all-in floor exceeds its all-in cap. Rates and
    /// spreads are quoted in basis points where the field name ends in `_bp`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if self.reset_lag_days < 0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "reset_lag_days must be non-negative; got {}",
                self.reset_lag_days
            )));
        }

        if let (Some(floor), Some(cap)) = (self.index_floor_bp, self.index_cap_bp) {
            if floor > cap {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "index_floor_bp ({}) must not exceed index_cap_bp ({})",
                    floor, cap
                )));
            }
        }

        if let (Some(floor), Some(cap)) = (self.all_in_floor_bp, self.all_in_cap_bp) {
            if floor > cap {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "all_in_floor_bp ({}) must not exceed all_in_cap_bp ({})",
                    floor, cap
                )));
            }
        }

        Ok(())
    }
}

fn default_gearing_includes_spread() -> bool {
    true
}

/// Floating coupon specification (composes FloatingRateSpec).
///
/// Used by the cashflow builder for instruments with floating rate coupons.
/// Embeds the canonical `FloatingRateSpec` for rate projection and adds
/// coupon-specific settings like payment frequency and PIK behavior.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FloatingCouponSpec {
    /// Floating rate specification (contains index, spread, floor, cap, etc).
    pub rate_spec: FloatingRateSpec,

    /// Coupon type (Cash/PIK/Split).
    #[serde(default)]
    pub coupon_type: CouponType,

    /// Accrual and payment schedule conventions.
    #[serde(flatten)]
    pub schedule: ScheduleParams,
}

/// Step-up/step-down coupon specification.
///
/// Defines a coupon that changes rate at specified dates, commonly used
/// in bank capital instruments (AT1/Tier 2) and some agency bonds.
///
/// The rate for each coupon period is determined by the last step date
/// that falls on or before the period start date. If no step has occurred,
/// the initial rate is used.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{Date, DayCount, Tenor, BusinessDayConvention, StubKind};
/// use finstack_quant_cashflows::builder::{CouponType, ScheduleParams, StepUpCouponSpec};
/// use rust_decimal_macros::dec;
/// use time::Month;
///
/// let spec = StepUpCouponSpec {
///     coupon_type: CouponType::Cash,
///     initial_rate: dec!(0.03),
///     step_schedule: vec![
///         (Date::from_calendar_date(2027, Month::January, 1).unwrap(), dec!(0.04)),
///         (Date::from_calendar_date(2029, Month::January, 1).unwrap(), dec!(0.05)),
///     ],
///     schedule: ScheduleParams {
///         freq: Tenor::semi_annual(),
///         dc: DayCount::Thirty360,
///         bdc: BusinessDayConvention::Following,
///         calendar_id: "weekends_only".to_string(),
///         stub: StubKind::None,
///         end_of_month: false,
///         payment_lag_days: 0,
///         adjust_accrual_dates: false,
///     },
/// };
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StepUpCouponSpec {
    /// Coupon type (Cash/PIK/Split).
    #[serde(default)]
    pub coupon_type: CouponType,
    /// Initial coupon rate (annual, decimal). Used until the first step date.
    pub initial_rate: Decimal,
    /// Step schedule: (effective_date, new_rate). Must be sorted by date.
    /// Each entry sets the rate from that date forward until the next step.
    ///
    /// **Date convention:** `effective_date` is compared against each
    /// accrual period's *unadjusted* `accrual_start`. Specify dates as
    /// unadjusted accrual-period boundaries (typically the issue date plus
    /// integer multiples of `freq`); business-day adjustment is not
    /// applied here. The rate is set at accrual start (per market
    /// convention for step-up bonds).
    #[schemars(with = "Vec<(String, Decimal)>")]
    pub step_schedule: Vec<(Date, Decimal)>,
    /// Accrual and payment schedule conventions.
    #[serde(flatten)]
    pub schedule: ScheduleParams,
}

impl StepUpCouponSpec {
    /// Validate step dates and rates before compilation.
    ///
    /// # Errors
    ///
    /// Returns an error when `step_schedule` dates are duplicated or not
    /// strictly increasing. Rates are applied in their given decimal units;
    /// this method does not impose a rate floor or cap.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        for (window, current) in self
            .step_schedule
            .windows(2)
            .zip(self.step_schedule.iter().skip(1))
        {
            if window[0].0 >= current.0 {
                return Err(finstack_quant_core::Error::Validation(
                    "StepUp step_schedule dates must be strictly increasing".into(),
                ));
            }
        }
        Ok(())
    }
}
