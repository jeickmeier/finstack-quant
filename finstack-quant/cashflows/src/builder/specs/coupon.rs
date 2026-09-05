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
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum CouponType {
    /// Entire coupon paid in cash.
    #[default]
    Cash,
    /// Entire coupon capitalized into principal.
    Pik,
    /// Coupon split between cash and PIK.
    Split {
        /// Fraction of the coupon paid in cash, expressed as a decimal share in
        /// `[0, 1]`.
        #[serde(with = "finstack_quant_core::wire::decimal")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DecimalWire")
        )]
        cash_pct: Decimal,
        /// Fraction of the coupon capitalized as PIK, expressed as a decimal
        /// share in `[0, 1]`.
        #[serde(with = "finstack_quant_core::wire::decimal")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DecimalWire")
        )]
        pik_pct: Decimal,
    },
}

impl CouponType {
    /// Returns (cash_fraction, pik_fraction) as Decimal values.
    pub(crate) fn split_parts(self) -> finstack_quant_core::Result<(Decimal, Decimal)> {
        match self {
            CouponType::Cash => Ok((Decimal::ONE, Decimal::ZERO)),
            CouponType::Pik => Ok((Decimal::ZERO, Decimal::ONE)),
            CouponType::Split { cash_pct, pik_pct } => {
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(from = "RawFixedCouponSpec")]
#[cfg_attr(feature = "json-schema", schemars(!from))]
#[cfg_attr(feature = "json-schema", schemars(deny_unknown_fields))]
pub struct FixedCouponSpec {
    /// Coupon settlement behavior: cash, PIK, or an explicit split of the
    /// coupon amount.
    #[serde(default)]
    pub coupon_type: CouponType,
    /// Coupon rate as a decimal (e.g., 0.05 for 5%). Uses Decimal for exact representation.
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub rate: Decimal,
    /// Accrual and payment schedule conventions.
    #[serde(flatten)]
    pub schedule: ScheduleParams,
}

#[derive(serde::Deserialize)]
struct RawFixedCouponSpec {
    #[serde(default)]
    coupon_type: CouponType,
    rate: Decimal,
    #[serde(flatten)]
    schedule: ScheduleParams,
    #[serde(flatten)]
    _unknown_fields: finstack_quant_core::serde_guard::UnknownFieldGuard,
}

impl From<RawFixedCouponSpec> for FixedCouponSpec {
    fn from(raw: RawFixedCouponSpec) -> Self {
        let RawFixedCouponSpec {
            coupon_type,
            rate,
            schedule,
            _unknown_fields: _,
        } = raw;
        Self {
            coupon_type,
            rate,
            schedule,
        }
    }
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
/// - ARRC (2020). "SOFR: A User's Guide." Federal Reserve Bank of New York. `docs/REFERENCES.md#arrc-sofr-users-guide`
/// - `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
/// - `docs/REFERENCES.md#isda-2006-definitions`
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
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
    /// Freeze the last `lockout_days` business-day observations at the fixing
    /// immediately preceding those days. For fixings `b_1..b_n`, the source is
    /// `b_{n-lockout}`. A positive lockout must leave a preceding fixing.
    ///
    /// Reference: ARRC, December 3, 2021 Statement, Appendix A, definition of
    /// "Lockout": <https://www.newyorkfed.org/medialibrary/Microsites/arrc/files/2021/ARRC-Statement-LIBOR-tenors-Legislation.pdf>.
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
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
    FixedRate(
        #[serde(with = "finstack_quant_core::wire::decimal")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DecimalWire")
        )]
        rust_decimal::Decimal,
    ),
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
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
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
///     reset_frequency: Tenor::quarterly(),
///     index_tenor: None,
///     reset_lag_days: 2,
///     fixing_calendar_id: None,
///     overnight_compounding: None,
///     overnight_basis: None,
///     fallback: Default::default(),
/// };
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FloatingRateSpec {
    /// Forward curve identifier (e.g., "USD-SOFR-3M", "EUR-EURIBOR-6M").
    pub index_id: CurveId,

    /// Spread/margin over index in basis points. Uses Decimal for exact representation.
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub spread_bp: Decimal,

    /// Gearing/leverage multiplier applied to the all-in rate (default: 1.0).
    ///
    /// Example: gearing = 2.0 means the rate is doubled.
    ///
    /// **Restriction:** gearing must be strictly positive (`gearing > 0`);
    /// projection rejects zero or negative gearing, so inverse floaters
    /// (negative gearing) are not currently expressible with this field.
    #[serde(default = "default_gearing")]
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
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
    #[serde(default)]
    #[serde(with = "finstack_quant_core::wire::optional_decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DecimalWire>")
    )]
    pub index_floor_bp: Option<Decimal>,

    /// Floor on all-in rate in basis points (Min Coupon).
    ///
    /// Applied to the final calculated rate after gearing and spread.
    #[serde(default)]
    #[serde(with = "finstack_quant_core::wire::optional_decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DecimalWire>")
    )]
    pub all_in_floor_bp: Option<Decimal>,

    /// Cap on all-in rate in basis points (applied after spread and gearing).
    ///
    /// Example: all_in_cap_bp = Some(1000.0) ensures all-in rate <= 10%.
    #[serde(default)]
    #[serde(with = "finstack_quant_core::wire::optional_decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DecimalWire>")
    )]
    pub all_in_cap_bp: Option<Decimal>,

    /// Cap on index rate in basis points (applied to index component).
    #[serde(default)]
    #[serde(with = "finstack_quant_core::wire::optional_decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DecimalWire>")
    )]
    pub index_cap_bp: Option<Decimal>,

    /// Index floor/cap application policy for overnight-compounded coupons.
    #[serde(
        default,
        skip_serializing_if = "OvernightIndexConstraintApplication::is_default"
    )]
    pub overnight_index_constraints: OvernightIndexConstraintApplication,

    /// Reset frequency for rate fixings.
    ///
    /// This is the cadence at which the rate refixes. When
    /// [`Self::index_tenor`] is `None`, it is also the tenor used only to
    /// build the diagnostic index-maturity date in projection error context.
    pub reset_frequency: Tenor,

    /// Diagnostic tenor for term-index projection error context.
    ///
    /// The named forward curve is already the term index (for example a 3M
    /// EURIBOR curve). Projection is `fwd.rate(reset_date)`, not a FRA-style
    /// average over `[reset, reset + tenor]`. This field (or
    /// [`Self::reset_frequency`] when `None`) is used only to compute
    /// `index_maturity` for error messages. Ignored for overnight-compounded
    /// legs. When set, the builder warns at build time if it disagrees with
    /// the resolved curve's tenor by more than 10% — the curve remains
    /// authoritative.
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
    /// overnight fixings (e.g., 360 for SOFR/€STR/TONA, 365 for SONIA).
    /// It is independent of the leg's accrual day count when set explicitly.
    ///
    /// When `None` and `overnight_compounding` is set, the coupon
    /// `schedule.day_count` is used if it is `Act360` or `Act365F`. Other
    /// coupon day counts (for example `Thirty360`) error unless an explicit
    /// `Act360` or `Act365F` basis is supplied. Ignored when
    /// `overnight_compounding` is `None`.
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

impl FloatingRateSpec {
    /// Shared preset skeleton: unit gearing, no floors/caps, `Error` fallback.
    fn preset(
        index_id: &str,
        spread_bp: Decimal,
        reset_frequency: Tenor,
        index_tenor: Option<Tenor>,
        reset_lag_days: i32,
        fixing_calendar_id: &str,
    ) -> Self {
        Self {
            index_id: CurveId::from(index_id),
            spread_bp,
            gearing: default_gearing(),
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_frequency,
            index_tenor,
            reset_lag_days,
            fixing_calendar_id: Some(fixing_calendar_id.to_string()),
            overnight_compounding: None,
            overnight_basis: None,
            fallback: FloatingRateFallback::Error,
        }
    }

    /// USD SOFR compounded in arrears (ARRC / ISDA 2021 conventions).
    ///
    /// Index id `USD-SOFR`, quarterly resets, daily compounding in arrears on
    /// an Act/360 basis, no reset lag (the ARRC convention places the lag on
    /// payment, see [`super::ScheduleParams::usd_sofr_swap`]), USNY fixing
    /// calendar, unit gearing, no floors or caps, `Error` fallback.
    ///
    /// # Arguments
    ///
    /// * `spread_bp` - Spread over compounded SOFR in basis points
    ///   (`dec!(50)` means +50 bp per annum).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::{FloatingRateSpec, OvernightCompoundingMethod};
    /// use rust_decimal_macros::dec;
    ///
    /// let spec = FloatingRateSpec::sofr(dec!(50));
    /// assert_eq!(spec.index_id.as_str(), "USD-SOFR");
    /// assert_eq!(spec.overnight_compounding, Some(OvernightCompoundingMethod::CompoundedInArrears));
    /// assert!(spec.validate().is_ok());
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#arrc-sofr-users-guide`
    pub fn sofr(spread_bp: Decimal) -> Self {
        Self {
            overnight_compounding: Some(OvernightCompoundingMethod::CompoundedInArrears),
            overnight_basis: Some(DayCount::Act360),
            ..Self::preset("USD-SOFR", spread_bp, Tenor::quarterly(), None, 0, "usny")
        }
    }

    /// GBP SONIA compounded in arrears (BoE / ISDA 2021 conventions).
    ///
    /// Index id `GBP-SONIA`, annual resets, daily compounding in arrears on an
    /// Act/365F basis, no reset lag, GBLO fixing calendar, unit gearing, no
    /// floors or caps, `Error` fallback. Pair with
    /// [`super::ScheduleParams::gbp_sonia_swap`].
    ///
    /// # Arguments
    ///
    /// * `spread_bp` - Spread over compounded SONIA in basis points
    ///   (`dec!(25)` means +25 bp per annum).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::FloatingRateSpec;
    /// use finstack_quant_core::dates::DayCount;
    /// use rust_decimal_macros::dec;
    ///
    /// let spec = FloatingRateSpec::sonia(dec!(25));
    /// assert_eq!(spec.index_id.as_str(), "GBP-SONIA");
    /// assert_eq!(spec.overnight_basis, Some(DayCount::Act365F));
    /// ```
    pub fn sonia(spread_bp: Decimal) -> Self {
        Self {
            overnight_compounding: Some(OvernightCompoundingMethod::CompoundedInArrears),
            overnight_basis: Some(DayCount::Act365F),
            ..Self::preset("GBP-SONIA", spread_bp, Tenor::annual(), None, 0, "gblo")
        }
    }

    /// EUR 3-month EURIBOR term rate (ISDA 2006 EUR conventions).
    ///
    /// Index id `EUR-EURIBOR-3M`, quarterly resets fixed in advance with a
    /// two-business-day reset lag on the TARGET2 calendar, explicit 3M index
    /// tenor, unit gearing, no floors or caps, `Error` fallback.
    ///
    /// # Arguments
    ///
    /// * `spread_bp` - Spread over 3M EURIBOR in basis points
    ///   (`dec!(100)` means +100 bp per annum).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::FloatingRateSpec;
    /// use finstack_quant_core::dates::Tenor;
    /// use rust_decimal_macros::dec;
    ///
    /// let spec = FloatingRateSpec::euribor_3m(dec!(100));
    /// assert_eq!(spec.index_id.as_str(), "EUR-EURIBOR-3M");
    /// assert_eq!(spec.index_tenor, Some(Tenor::quarterly()));
    /// assert_eq!(spec.reset_lag_days, 2);
    /// ```
    pub fn euribor_3m(spread_bp: Decimal) -> Self {
        Self::preset(
            "EUR-EURIBOR-3M",
            spread_bp,
            Tenor::quarterly(),
            Some(Tenor::quarterly()),
            2,
            "target2",
        )
    }
}

/// Floating coupon specification (composes FloatingRateSpec).
///
/// Used by the cashflow builder for instruments with floating rate coupons.
/// Embeds the canonical `FloatingRateSpec` for rate projection and adds
/// coupon-specific settings like payment frequency and PIK behavior.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(from = "RawFloatingCouponSpec")]
#[cfg_attr(feature = "json-schema", schemars(!from))]
#[cfg_attr(feature = "json-schema", schemars(deny_unknown_fields))]
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

#[derive(serde::Deserialize)]
struct RawFloatingCouponSpec {
    rate_spec: FloatingRateSpec,
    #[serde(default)]
    coupon_type: CouponType,
    #[serde(flatten)]
    schedule: ScheduleParams,
    #[serde(flatten)]
    _unknown_fields: finstack_quant_core::serde_guard::UnknownFieldGuard,
}

impl From<RawFloatingCouponSpec> for FloatingCouponSpec {
    fn from(raw: RawFloatingCouponSpec) -> Self {
        let RawFloatingCouponSpec {
            rate_spec,
            coupon_type,
            schedule,
            _unknown_fields: _,
        } = raw;
        Self {
            rate_spec,
            coupon_type,
            schedule,
        }
    }
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
///         frequency: Tenor::semi_annual(),
///         day_count: DayCount::Thirty360,
///         business_day_convention: BusinessDayConvention::Following,
///         calendar_id: "weekends_only".to_string(),
///         stub: StubKind::None,
///         end_of_month: false,
///         payment_lag_days: 0,
///         adjust_accrual_dates: false,
///         roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
///     },
/// };
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(from = "RawStepUpCouponSpec")]
#[cfg_attr(feature = "json-schema", schemars(!from))]
#[cfg_attr(feature = "json-schema", schemars(deny_unknown_fields))]
pub struct StepUpCouponSpec {
    /// Coupon type (Cash/PIK/Split).
    #[serde(default)]
    pub coupon_type: CouponType,
    /// Initial coupon rate (annual, decimal). Used until the first step date.
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub initial_rate: Decimal,
    /// Step schedule: (effective_date, new_rate). Must be sorted by date.
    /// Each entry sets the rate from that date forward until the next step.
    ///
    /// **Date convention:** `effective_date` is compared against each
    /// accrual period's *unadjusted* `accrual_start`. Specify dates as
    /// unadjusted accrual-period boundaries (typically the issue date plus
    /// integer multiples of `frequency`); business-day adjustment is not
    /// applied here. The rate is set at accrual start (per market
    /// convention for step-up bonds).
    #[cfg_attr(
        feature = "json-schema",
        schemars(
            with = "Vec<(finstack_quant_core::wire::DateWire, finstack_quant_core::wire::DecimalWire)>"
        )
    )]
    pub step_schedule: Vec<(Date, Decimal)>,
    /// Accrual and payment schedule conventions.
    #[serde(flatten)]
    pub schedule: ScheduleParams,
}

#[derive(serde::Deserialize)]
struct RawStepUpCouponSpec {
    #[serde(default)]
    coupon_type: CouponType,
    initial_rate: Decimal,
    step_schedule: Vec<(Date, Decimal)>,
    #[serde(flatten)]
    schedule: ScheduleParams,
    #[serde(flatten)]
    _unknown_fields: finstack_quant_core::serde_guard::UnknownFieldGuard,
}

impl From<RawStepUpCouponSpec> for StepUpCouponSpec {
    fn from(raw: RawStepUpCouponSpec) -> Self {
        let RawStepUpCouponSpec {
            coupon_type,
            initial_rate,
            step_schedule,
            schedule,
            _unknown_fields: _,
        } = raw;
        Self {
            coupon_type,
            initial_rate,
            step_schedule,
            schedule,
        }
    }
}
