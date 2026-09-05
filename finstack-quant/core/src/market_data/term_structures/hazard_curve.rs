//! Credit hazard rate curves for default probability modeling.
//!
//! A hazard curve represents the instantaneous probability of default (credit
//! event) for a corporate or sovereign issuer. These curves are fundamental
//! for pricing credit default swaps (CDS), corporate bonds, and credit derivatives.
//!
//! # Financial Concept
//!
//! The hazard rate λ(t) represents the instantaneous default intensity:
//! ```text
//! Survival probability: S(t) = P(τ > t) = exp(-∫₀ᵗ λ(s)ds)
//! Default probability: Q(t) = 1 - S(t)
//!
//! For piecewise-constant λ:
//! S(t) = exp(-Σ λᵢ * Δtᵢ)
//! ```
//!
//! # Market Construction
//!
//! Hazard curves are typically bootstrapped from:
//! - **CDS spreads**: Single-name CDS par spreads (market standard)
//! - **Bond spreads**: Credit spread over risk-free benchmark
//! - **Loan spreads**: Primary or secondary market loan pricing
//! - **Recovery assumptions**: Typically 40% for senior unsecured
//!
//! # Piecewise-Constant Model
//!
//! This implementation assumes constant hazard rates between knots, which:
//! - Provides analytical survival probabilities (no numerical integration)
//! - Ensures positive default probabilities (λ ≥ 0)
//! - Matches ISDA Standard CDS Model convention
//!
//! # Use Cases
//!
//! - **CDS pricing**: Protection and premium leg valuation
//! - **Corporate bond pricing**: Credit spread decomposition
//! - **CVA calculation**: Counterparty credit risk adjustment
//! - **CDO/CLO pricing**: Constituent credit curves for tranches
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::market_data::term_structures::HazardCurve;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let hc = HazardCurve::builder("USD-CREDIT")
//!     .base_date(base)
//!     .recovery_rate(0.40)
//!     .knots([(1.0, 0.01), (10.0, 0.015)])
//!     .build()
//!     .expect("HazardCurve builder should succeed");
//! assert!(hc.sp(5.0) < 1.0); // Survival probability < 1
//! ```
//!
//! # References
//!
//! - **CDS Pricing**:
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*.
//!   Wiley Finance. Chapters 3-5. `docs/REFERENCES.md#o-kane-2008`
//! - ISDA (2009). "ISDA CDS Standard Model." Version 1.8.2. `docs/REFERENCES.md#isda-cds-standard-model`
//!
//!
//! - **Hazard Rate Models**:
//!   - Duffie, D., & Singleton, K. J. (1999). "Modeling Term Structures of Defaultable
//!   Bonds." *Review of Financial Studies*, 12(4), 687-720. `docs/REFERENCES.md#duffie-singleton-1999`
//!   - Lando, D. (1998). "On Cox Processes and Credit Risky Securities."
//!   *Review of Derivatives Research*, 2(2-3), 99-120. `docs/REFERENCES.md#lando-1998`
//!
//! - **Industry Practice**:
//!   - Markit (2009). "CDS Curve Bootstrapping Guide."
//!   - Bloomberg (2013). "Credit Curve Construction and CDS Pricing Guide."

use crate::{
    currency::Currency,
    dates::{Date, DayCount, DayCountContext},
    error::InputError,
    market_data::traits::Survival,
    math::interp::{
        strategies::{LinearStrategy, LogLinearStrategy},
        types::Interp,
        ExtrapolationPolicy, InterpStyle, InterpolationStrategy,
    },
    types::CurveId,
};

/// Piecewise-constant credit hazard curve for default probability modeling.
///
/// Represents the instantaneous default intensity λ(t) for a credit issuer.
/// Assumes constant hazard rate between knots, providing analytical survival
/// probabilities without numerical integration.
///
/// # Mathematical Model
///
/// ```text
/// λ(t) = piecewise-constant hazard rate
/// S(t) = exp(-∫₀ᵗ λ(s)ds) = exp(-Σ λᵢ * Δtᵢ)
/// Q(t) = 1 - S(t) = cumulative default probability
/// ```
///
/// # Invariants
///
/// - All hazard rates λᵢ ≥ 0 (enforced at construction)
/// - Survival probability S(t) is monotonically decreasing
/// - Recovery rate ∈ [0, 1] (typically 40% for senior unsecured)
///
/// # Thread Safety
///
/// Immutable after construction; safe to share via `Arc<HazardCurve>`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawHazardCurve", into = "RawHazardCurve")]
pub struct HazardCurve {
    id: CurveId,
    base: Date,
    /// Time grid in years from base date; strictly increasing and non-negative.
    knots: Box<[f64]>,
    /// Piecewise-constant hazard rates λ ≥ 0; same length as `knots`.
    lambdas: Box<[f64]>,
    /// Recovery rate used during calibration/reporting (metadata)
    recovery_rate: f64,
    /// Optional issuer metadata
    issuer: Option<String>,
    /// Debt seniority
    pub seniority: Option<Seniority>,
    /// Currency of protection leg (metadata)
    currency: Option<Currency>,
    /// Day count convention for converting dates→times (metadata)
    day_count: DayCount,
    /// Stored market par spreads used to bootstrap this curve (for reporting)
    par_tenors: Box<[f64]>,
    /// Par spreads in basis points at `par_tenors`
    par_spreads_bp: Box<[f64]>,
    /// Default interpolation for par spreads
    par_interp: ParInterp,
    /// Interpolation style for survival probabilities between pillars
    /// (LogLinear ⇒ piecewise-constant hazard).
    survival_interp_style: InterpStyle,
    /// Exact typed recipe used to replay calibration after quote shocks.
    hazard_calibration: Option<super::HazardCalibrationRecipe>,
    /// Interpolator for survival probabilities
    interp: Interp,
    /// Opaque FX policy stamp; see [`super::DiscountCurve::fx_policy`].
    fx_policy: Option<String>,
}

/// Raw serializable state of a HazardCurve
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawHazardCurve {
    /// Curve identifier
    pub id: String,
    /// Base date
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub base: Date,
    /// Time/value pairs used to construct the curve
    pub knot_points: Vec<(f64, f64)>,
    /// Recovery rate
    pub recovery_rate: f64,
    /// Optional issuer
    pub issuer: Option<String>,
    /// Seniority
    pub seniority: Option<Seniority>,
    /// Currency
    pub currency: Option<Currency>,
    /// Day count convention
    pub day_count: DayCount,
    /// Par spread points for reporting
    pub par_points: Vec<(f64, f64)>,
    /// Par interpolation method
    #[serde(default = "default_par_interp")]
    pub par_interp: ParInterp,
    /// Survival-probability interpolation style between pillars
    #[serde(default = "default_survival_interp")]
    pub survival_interp: InterpStyle,
    /// Exact calibration replay inputs.
    #[serde(default)]
    pub hazard_calibration: Option<super::HazardCalibrationRecipe>,
    /// Opaque FX policy stamp; see [`super::DiscountCurve::fx_policy`].
    #[serde(default)]
    pub fx_policy: Option<String>,
}

fn default_par_interp() -> ParInterp {
    ParInterp::Linear
}

fn default_survival_interp() -> InterpStyle {
    InterpStyle::LogLinear
}

impl From<HazardCurve> for RawHazardCurve {
    fn from(curve: HazardCurve) -> Self {
        let knot_points: Vec<(f64, f64)> = curve
            .knots
            .iter()
            .zip(curve.lambdas.iter())
            .map(|(&t, &lambda)| (t, lambda))
            .collect();
        let par_points: Vec<(f64, f64)> = curve
            .par_tenors
            .iter()
            .zip(curve.par_spreads_bp.iter())
            .map(|(&t, &spread)| (t, spread))
            .collect();

        RawHazardCurve {
            id: curve.id.to_string(),
            base: curve.base,
            knot_points,
            recovery_rate: curve.recovery_rate,
            issuer: curve.issuer,
            seniority: curve.seniority,
            currency: curve.currency,
            day_count: curve.day_count,
            par_points,
            par_interp: curve.par_interp,
            survival_interp: curve.survival_interp_style,
            hazard_calibration: curve.hazard_calibration,
            fx_policy: curve.fx_policy,
        }
    }
}

impl TryFrom<RawHazardCurve> for HazardCurve {
    type Error = crate::Error;

    fn try_from(state: RawHazardCurve) -> crate::Result<Self> {
        HazardCurve::builder(state.id)
            .base_date(state.base)
            .recovery_rate(state.recovery_rate)
            .day_count(state.day_count)
            .knots(state.knot_points)
            .par_spreads(state.par_points)
            .par_interp(state.par_interp)
            .interp(state.survival_interp)
            .hazard_calibration_opt(state.hazard_calibration)
            .issuer_opt(state.issuer)
            .seniority_opt(state.seniority)
            .currency_opt(state.currency)
            .fx_policy_opt(state.fx_policy)
            .build()
    }
}

impl HazardCurve {
    /// Start building a hazard curve with identifier `id`.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> HazardCurveBuilder {
        let base =
            Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN);
        HazardCurveBuilder {
            id: id.into(),
            base,
            points: Vec::new(),
            recovery_rate: None,
            issuer: None,
            seniority: None,
            currency: None,
            day_count: DayCount::Act365F,
            par_points: Vec::new(),
            par_interp: ParInterp::Linear,
            survival_interp: InterpStyle::LogLinear,
            max_hazard_rate: 10.0,
            hazard_calibration: None,
            fx_policy: None,
        }
    }

    /// Construct a flat (constant-intensity) hazard curve.
    ///
    /// The curve stores a single knot at `t = 1` carrying `hazard_rate`; with
    /// flat-forward extrapolation this gives `S(t) = exp(-hazard_rate * t)`
    /// for every non-negative maturity.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"ACME-HZD"`).
    /// * `base_date` - Valuation date anchoring `t = 0`; the curve day count
    ///   is `Act/365F`.
    /// * `hazard_rate` - Constant annual default intensity as a decimal
    ///   fraction (`0.02` is 2% per year); must be finite and non-negative.
    /// * `recovery_rate` - Assumed recovery on default as a decimal fraction
    ///   in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `hazard_rate` is non-finite, negative or
    /// exceeds the default `max_hazard_rate` of `10.0`, or if `recovery_rate`
    /// is outside `[0, 1]`.
    pub fn flat(
        id: impl Into<CurveId>,
        base_date: Date,
        hazard_rate: f64,
        recovery_rate: f64,
    ) -> crate::Result<Self> {
        if !hazard_rate.is_finite() {
            return Err(crate::Error::Validation(format!(
                "HazardCurve::flat hazard_rate must be finite, got {hazard_rate}"
            )));
        }
        Self::builder(id)
            .base_date(base_date)
            .knots([(1.0, hazard_rate)])
            .recovery_rate(recovery_rate)
            .build()
    }

    /// Construct a hazard curve from survival-probability pillars.
    ///
    /// Each pillar `(t, S(t))` is converted to the piecewise-constant hazard
    /// rate of the segment ending at `t`:
    /// `λᵢ = -ln(S(tᵢ) / S(tᵢ₋₁)) / (tᵢ - tᵢ₋₁)` with `S(0) = 1`. A pillar at
    /// `t = 0` is accepted only when its survival probability is `1`.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier.
    /// * `base_date` - Valuation date anchoring `t = 0`; the curve day count
    ///   is `Act/365F`.
    /// * `points` - `(time_years, survival_probability)` pillars. Times must be
    ///   finite and non-negative and may be supplied in any order; survival
    ///   probabilities must lie in `(0, 1]` and be non-increasing in time.
    /// * `recovery_rate` - Assumed recovery on default as a decimal fraction
    ///   in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `points` is empty, a time is negative or
    /// duplicated, a survival probability is outside `(0, 1]` or increases
    /// with time, or `recovery_rate` is outside `[0, 1]`.
    pub fn from_survival_probs(
        id: impl Into<CurveId>,
        base_date: Date,
        points: &[(f64, f64)],
        recovery_rate: f64,
    ) -> crate::Result<Self> {
        if points.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }
        let mut sorted = points.to_vec();
        sorted.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut knots = Vec::with_capacity(sorted.len());
        let mut prev_t = 0.0_f64;
        let mut prev_sp = 1.0_f64;
        for &(t, sp) in &sorted {
            if !t.is_finite() || t < 0.0 {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve::from_survival_probs time must be finite and non-negative, got t={t}"
                )));
            }
            if !sp.is_finite() || sp <= 0.0 || sp > 1.0 {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve::from_survival_probs survival probability must be in (0, 1], got {sp} at t={t}"
                )));
            }
            if t <= 1e-9 {
                if (sp - 1.0).abs() > 1e-12 {
                    return Err(crate::Error::Validation(format!(
                        "HazardCurve::from_survival_probs survival probability at t=0 must be 1, got {sp}"
                    )));
                }
                continue;
            }
            if t <= prev_t {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve::from_survival_probs times must be strictly increasing, got duplicate t={t}"
                )));
            }
            if sp > prev_sp {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve::from_survival_probs survival probability must be non-increasing, got {sp} at t={t} after {prev_sp}"
                )));
            }
            let lambda = -(sp / prev_sp).ln() / (t - prev_t);
            knots.push((t, lambda));
            prev_t = t;
            prev_sp = sp;
        }
        if knots.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }
        Self::builder(id)
            .base_date(base_date)
            .knots(knots)
            .recovery_rate(recovery_rate)
            .build()
    }

    /// Survival probability S(t) up to time `t` (in **years**).
    ///
    /// # Arguments
    ///
    /// * `t` - Year-fraction time from the curve or surface base date to the query point
    #[must_use]
    pub fn sp(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 1.0;
        }
        if let Some(&last_t) = self.knots.last() {
            if t > last_t {
                let survival_at_last = self.interp.interp(last_t);
                let tail_hazard = self.lambdas[self.lambdas.len() - 1];
                return survival_at_last * (-tail_hazard * (t - last_t)).exp();
            }
        }
        self.interp.interp(t)
    }

    /// Default probability between `t1` and `t2`.
    ///
    /// Returns `S(t1) - S(t2)`, the probability of default occurring
    /// in the interval `[t1, t2]`.
    ///
    /// # Errors
    ///
    /// Returns an error if `t2 < t1`.
    ///
    /// # Arguments
    ///
    /// * `t1` - Start year-fraction of the forward or rate interval being queried
    /// * `t2` - End year-fraction of the forward or rate interval being queried
    #[must_use = "computed default probability should not be discarded"]
    pub fn default_prob(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        if t2 < t1 {
            return Err(crate::Error::Validation(format!(
                "default_prob requires t2 >= t1 (t1={t1}, t2={t2})"
            )));
        }
        let sp1 = self.sp(t1);
        let sp2 = self.sp(t2);
        Ok(sp1 - sp2)
    }

    /// Instantaneous hazard rate λ(t) at time `t`.
    ///
    /// For piecewise-constant hazard curves, this returns the lambda value
    /// corresponding to the interval containing `t`.
    ///
    /// Hazards are right-continuous at knot boundaries. λᵢ applies to the
    /// segment *ending* at knot tᵢ (ISDA-style): `hazard_rate(tᵢ) = λᵢ` and
    /// the next segment starts immediately after tᵢ. An explicit t≈0 knot is
    /// ignored for this lookup except at `t <= 0`, which returns the first
    /// stored lambda.
    ///
    /// # Arguments
    ///
    /// * `t` - Year-fraction from the curve base date to the query point; values
    ///   at or below zero return the first segment's hazard rate
    ///
    /// # Panics
    ///
    /// Panics only if the curve's internal invariant is violated and it has no
    /// hazard-rate segment. Public constructors reject that state.
    #[must_use]
    pub fn hazard_rate(&self, t: f64) -> f64 {
        // A valid hazard curve always has at least one lambda.
        assert!(
            !self.lambdas.is_empty(),
            "HazardCurve invariant violated: empty lambdas"
        );
        if t <= 0.0 {
            return self.lambdas[0];
        }

        let idx = self.knots.partition_point(|&k| k < t);
        let idx = idx.min(self.lambdas.len() - 1);
        self.lambdas[idx]
    }

    /// Survival probability on a specific calendar date using the curve's day-count.
    ///
    /// This is the date-based equivalent of [`sp`](Self::sp), consistent with
    /// `DiscountCurve::df_on_date_curve` and `ForwardCurve::df_on_date_curve`.
    ///
    /// # Errors
    ///
    /// Returns an error if the year fraction calculation fails.
    #[inline]
    #[must_use = "computed survival probability should not be discarded"]
    pub fn sp_on_date(&self, date: Date) -> crate::Result<f64> {
        let t = self.year_fraction_to(date)?;
        Ok(self.sp(t))
    }

    /// Hazard rate on a specific calendar date using the curve's day-count.
    ///
    /// This is the date-based equivalent of [`hazard_rate`](Self::hazard_rate).
    ///
    /// # Errors
    ///
    /// Returns an error if the year fraction calculation fails.
    #[inline]
    #[must_use = "computed hazard rate should not be discarded"]
    pub fn hazard_rate_on_date(&self, date: Date) -> crate::Result<f64> {
        let t = self.year_fraction_to(date)?;
        Ok(self.hazard_rate(t))
    }

    /// Evaluate survival probabilities at the provided calendar dates.
    ///
    /// Each date is converted to a year fraction from [`Self::base_date`] with
    /// this curve's [`Self::day_count`], then evaluated with [`Self::sp`].
    /// Results preserve the input order, and dates on or before the base date
    /// therefore return `1.0`. Values are clamped to `[0, 1]` as a final
    /// numerical safeguard.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected day-count convention cannot calculate
    /// a year fraction for any supplied date. No partial vector is returned.
    #[must_use = "computed survival probabilities should not be discarded"]
    pub fn survival_at_dates(&self, dates: &[Date]) -> crate::Result<Vec<f64>> {
        let base = self.base_date();
        let day_count = self.day_count();
        let mut survival = Vec::with_capacity(dates.len());

        for &date in dates {
            let t = day_count.year_fraction(base, date, DayCountContext::default())?;
            let sp = self.sp(t).clamp(0.0, 1.0);
            survival.push(sp);
        }

        Ok(survival)
    }

    /// Unique identifier used to register and resolve this hazard curve.
    pub fn id(&self) -> &CurveId {
        &self.id
    }
    /// Curve valuation **base date**.
    pub fn base_date(&self) -> Date {
        self.base
    }

    /// Recovery rate metadata used when mapping spreads↔hazards during bootstrap.
    pub fn recovery_rate(&self) -> f64 {
        self.recovery_rate
    }

    /// Day count convention associated with this curve's time axis.
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Get the currency of the protection leg.
    pub fn currency(&self) -> Option<Currency> {
        self.currency
    }

    /// Get the issuer name.
    pub fn issuer(&self) -> Option<&str> {
        self.issuer.as_deref()
    }

    /// Opaque FX policy stamp set by the curve constructor; see
    /// [`super::DiscountCurve::fx_policy`] for the contract.
    #[inline]
    pub fn fx_policy(&self) -> Option<&str> {
        self.fx_policy.as_deref()
    }

    /// Access the knot points (time, lambda) for inspection or modification.
    pub fn knot_points(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.knots
            .iter()
            .zip(self.lambdas.iter())
            .map(|(&t, &lambda)| (t, lambda))
    }

    /// Access the par spread points for inspection.
    pub fn par_spread_points(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.par_tenors
            .iter()
            .zip(self.par_spreads_bp.iter())
            .map(|(&t, &spread)| (t, spread))
    }
    /// Get the default interpolation method for par spreads.
    pub fn par_interp(&self) -> ParInterp {
        self.par_interp
    }
    /// Exact inputs used to calibrate this curve, when available.
    #[inline]
    #[must_use]
    pub fn hazard_calibration(&self) -> Option<&super::HazardCalibrationRecipe> {
        self.hazard_calibration.as_ref()
    }

    /// Number of knot points in the curve.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.knots.len()
    }

    /// Returns `true` if the curve has no knot points.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.knots.is_empty()
    }

    /// Create a copy of this curve with its `recovery_rate` metadata overridden.
    ///
    /// All hazard knots (λ), the survival interpolator, day-count, base date,
    /// par spreads and other metadata are preserved **unchanged** — only the
    /// recovery-rate field is replaced. The recovery rate is calibration/
    /// reporting metadata (consulted by the recovery-consistency guard and the
    /// spread↔hazard bootstrap mapping); it is *not* a direct input to the
    /// survival-probability or protection-leg PV math. Overriding it therefore
    /// realigns a "frozen-curve" curve with a trade carrying a different
    /// recovery without altering any priced quantity.
    ///
    /// # Errors
    ///
    /// Returns an error if `recovery_rate` is outside `[0, 1]`.
    pub fn with_recovery_rate(&self, recovery_rate: f64) -> crate::Result<HazardCurve> {
        super::common::validate_unit_range(recovery_rate, "recovery_rate")?;
        Ok(HazardCurve {
            id: self.id.clone(),
            base: self.base,
            knots: self.knots.clone(),
            lambdas: self.lambdas.clone(),
            recovery_rate,
            issuer: self.issuer.clone(),
            seniority: self.seniority,
            currency: self.currency,
            day_count: self.day_count,
            par_tenors: self.par_tenors.clone(),
            par_spreads_bp: self.par_spreads_bp.clone(),
            par_interp: self.par_interp,
            survival_interp_style: self.survival_interp_style,
            hazard_calibration: None,
            interp: self.interp.clone(),
            fx_policy: self.fx_policy.clone(),
        })
    }

    /// Create a builder with this curve's parameters, using a new ID.
    /// Useful for creating modified versions of the curve.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> HazardCurveBuilder {
        self.metadata_builder(new_id)
            .knots(self.knot_points())
            .par_spreads(self.par_spread_points())
    }

    /// Builder pre-populated with this curve's full metadata but **no** knots
    /// or par spreads. Shared by all rebuild-style operations (bumps, rolls)
    /// so that no metadata field (issuer, seniority, currency, day-count,
    /// par interpolation, survival interpolation style, fx_policy) is dropped.
    pub(crate) fn metadata_builder(&self, new_id: impl Into<CurveId>) -> HazardCurveBuilder {
        HazardCurve::builder(new_id)
            .base_date(self.base)
            .recovery_rate(self.recovery_rate)
            .day_count(self.day_count)
            .par_interp(self.par_interp)
            .interp(self.survival_interp_style)
            .issuer_opt(self.issuer.clone())
            .seniority_opt(self.seniority)
            .currency_opt(self.currency)
            .fx_policy_opt(self.fx_policy.clone())
    }

    /// Clone the curve under `id` and apply `spec` through [`Self::bump_in_place`],
    /// the single bump implementation; the `with_*` helpers and `Bumpable`
    /// are thin wrappers over this.
    pub(crate) fn bumped_with_id(
        &self,
        id: CurveId,
        spec: &crate::market_data::bumps::BumpSpec,
    ) -> crate::Result<Self> {
        let mut bumped = self.clone();
        bumped.id = id;
        bumped.bump_in_place(spec)?;
        Ok(bumped)
    }

    /// Apply a bump specification in-place, mutating lambda values and rebuilding the interpolator.
    pub(crate) fn bump_in_place(
        &mut self,
        spec: &crate::market_data::bumps::BumpSpec,
    ) -> crate::Result<()> {
        use crate::market_data::bumps::BumpType;

        spec.validate_finite()?;
        if !matches!(spec.bump_type, BumpType::Parallel) {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: "HazardCurve only supports Parallel bumps, not key-rate bumps".to_string(),
            }
            .into());
        }

        // Recovery must be within [0, 1) for the par spread ⇢ hazard
        // conversion below; recovery == 1.0 would divide by zero and yield an
        // infinite shift. Mirrors the guard in `Bumpable::apply_bump`.
        let recovery = self.recovery_rate;
        if !recovery.is_finite() || !(0.0..1.0).contains(&recovery) {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: format!(
                    "HazardCurve bump requires recovery rate in [0, 1), got {}",
                    recovery
                ),
            }
            .into());
        }
        let (spread, is_multiplicative) = spec.resolve_standard_values().ok_or_else(|| {
            crate::error::InputError::UnsupportedBump {
                reason: format!(
                    "HazardCurve only supports Additive bumps, got {:?}/{:?}",
                    spec.mode, spec.units
                ),
            }
        })?;
        if is_multiplicative {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: "HazardCurve does not support Multiplicative bumps".to_string(),
            }
            .into());
        }

        let shift = spread / (1.0 - recovery);
        // Clone the hazard array only, not the whole curve. Early returns below
        // still leave `self` untouched, and nothing is committed until the
        // fallible interpolator build succeeds.
        let mut lambdas = self.lambdas.clone();
        for lambda in lambdas.iter_mut() {
            let shifted = *lambda + shift;
            if !shifted.is_finite() || shifted < 0.0 {
                return Err(crate::error::InputError::UnsupportedBump {
                    reason: "non-finite or negative hazard rate after bump".to_string(),
                }
                .into());
            }
            *lambda = shifted;
        }
        let (interp_knots, interp_sp) = survival_pillars(&self.knots, &lambdas);
        let interp = super::common::build_interp(
            self.survival_interp_style,
            interp_knots.into_boxed_slice(),
            interp_sp.into_boxed_slice(),
            ExtrapolationPolicy::FlatForward,
        )?;
        self.lambdas = lambdas;
        // The stored par-spread quotes were calibrated to the *unbumped*
        // hazards; keeping them would make `cds_quote_bp` report stale quotes.
        // Clear them so `cds_quote_bp` falls back to the hazard-based
        // approximation λ·(1−R)·1e4, which reflects the bumped curve.
        self.par_tenors = Box::new([]);
        self.par_spreads_bp = Box::new([]);
        self.hazard_calibration = None;
        self.interp = interp;
        Ok(())
    }
    /// Create a curve with every model hazard rate shifted in basis-point units.
    ///
    /// This is the hazard-curve equivalent of `DiscountCurve::with_parallel_bump`:
    /// a direct intensity shock that keeps the curve ID and all credit
    /// metadata; it does not replay CDS par quotes. Stored par-spread and
    /// calibration recipes are intentionally removed because they describe
    /// the pre-bump calibration, so [`Self::cds_quote_bp`] reports its
    /// hazard-based approximation for the bumped curve.
    ///
    /// # Arguments
    ///
    /// * `bump_bp` - Additive hazard-rate shift in basis points, where one
    ///   basis point is `1e-4` in decimal intensity units. Negative shifts
    ///   that would make any hazard rate negative are rejected.
    ///
    /// # Errors
    ///
    /// Returns an error when the shock is non-finite, makes a hazard rate
    /// negative, or makes it exceed the builder's maximum hazard-rate limit
    /// (10.0 unless the source curve was built with a different limit, which
    /// is not retained by this method).
    pub fn with_parallel_hazard_rate_bump_bp(&self, bump_bp: f64) -> crate::Result<Self> {
        if !bump_bp.is_finite() {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: "hazard-rate basis-point bump must be finite".to_string(),
            }
            .into());
        }
        let shift = bump_bp * 1e-4;
        let mut shifted_points = Vec::with_capacity(self.knots.len());
        for (t, lambda) in self.knot_points() {
            let shifted = lambda + shift;
            if shifted < 0.0 {
                return Err(crate::error::InputError::UnsupportedBump {
                    reason: "negative hazard rate after bump".to_string(),
                }
                .into());
            }
            shifted_points.push((t, shifted));
        }
        self.metadata_builder(self.id.clone())
            .knots(shifted_points)
            .build()
    }

    /// Create a curve with direct hazard-rate shocks at tenor segments.
    ///
    /// An exact knot match shocks that knot. Otherwise the segment containing
    /// the requested tenor is shocked; targets beyond the final knot are a
    /// no-op. Requests are applied in order, so repeated targets are additive.
    /// Stored par-spread and calibration recipes are intentionally removed.
    ///
    /// # Arguments
    ///
    /// * `targets_bp` - Ordered `(tenor_years, bump_bp)` shocks. Tenors are
    ///   measured from the curve base date and bumps are additive hazard-rate
    ///   basis points.
    ///
    /// # Errors
    ///
    /// Returns an error when a tenor or bump is non-finite, the curve is
    /// malformed, or rebuilding the shocked curve fails validation.
    pub fn with_tenor_hazard_rate_bumps_bp(
        &self,
        targets_bp: &[(f64, f64)],
    ) -> crate::Result<Self> {
        let knots: Vec<f64> = self.knot_points().map(|(tenor, _)| tenor).collect();
        let mut hazard_rates: Vec<f64> = self
            .knot_points()
            .map(|(_, hazard_rate)| hazard_rate)
            .collect();
        if knots.len() < 2 {
            let total_bp = targets_bp.iter().try_fold(0.0, |total, (tenor, bump_bp)| {
                if !tenor.is_finite() || !bump_bp.is_finite() {
                    return Err(crate::error::InputError::UnsupportedBump {
                        reason: "hazard-rate tenor and basis-point bump must be finite".to_string(),
                    });
                }
                Ok(total + bump_bp)
            })?;
            return self.with_parallel_hazard_rate_bump_bp(total_bp);
        }

        let last_knot = knots[knots.len() - 1];
        for (tenor, bump_bp) in targets_bp {
            if !tenor.is_finite() || !bump_bp.is_finite() {
                return Err(crate::error::InputError::UnsupportedBump {
                    reason: "hazard-rate tenor and basis-point bump must be finite".to_string(),
                }
                .into());
            }
            if *tenor > last_knot + 1e-6 {
                continue;
            }
            let mut target_index = knots
                .iter()
                .position(|knot| (*knot - tenor).abs() <= 1e-6)
                .unwrap_or(0);
            if target_index == 0 {
                if *tenor <= knots[0] {
                    target_index = 0;
                } else if *tenor >= last_knot {
                    target_index = knots.len() - 1;
                } else if let Some(index) = (0..knots.len() - 1)
                    .find(|index| *tenor > knots[*index] && *tenor < knots[*index + 1])
                {
                    target_index = index;
                }
            }
            hazard_rates[target_index] = (hazard_rates[target_index] + bump_bp * 1e-4).max(0.0);
        }

        self.metadata_builder(self.id.clone())
            .knots(knots.into_iter().zip(hazard_rates))
            .build()
    }

    /// Roll the curve forward by a specified number of days.
    ///
    /// This creates a new curve with:
    /// - Base date advanced by `days`
    /// - Knot times shifted backwards (t' = t - dt_years)
    /// - Points with t' <= 0 are filtered out (expired)
    /// - Hazard rates are preserved (no carry/theta adjustment)
    ///
    /// # Arguments
    /// * `days` - Number of days to roll forward
    ///
    /// # Returns
    /// A new hazard curve with updated base date and shifted knots.
    ///
    /// # Errors
    ///
    /// Returns an error if the day-count calculation fails, fewer than two
    /// knot points remain after the roll, or the rebuilt curve violates a
    /// construction invariant. `days` is signed; a negative value moves the
    /// base date backward rather than rejecting the request.
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        let new_base = self.base + time::Duration::days(days);
        let dt_years =
            self.day_count
                .year_fraction(self.base, new_base, DayCountContext::default())?;

        // Anchor the active post-roll hazard at the new origin, then retain
        // future hazard changes. This preserves conditional survival rather
        // than re-attributing the first surviving lambda to the whole front
        // segment.
        let mut rolled_points = Vec::with_capacity(self.knots.len() + 1);
        rolled_points.push((0.0, self.hazard_rate(dt_years)));
        rolled_points.extend(super::common::roll_knots(
            &self.knots,
            &self.lambdas,
            dt_years,
        ));

        if rolled_points.len() < 2 {
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        let rolled_par_points =
            super::common::roll_knots(&self.par_tenors, &self.par_spreads_bp, dt_years);

        let mut builder = self
            .metadata_builder(self.id.clone())
            .base_date(new_base)
            .knots(rolled_points);

        if !rolled_par_points.is_empty() {
            builder = builder.par_spreads(rolled_par_points);
        }

        builder.build()
    }

    /// Helper: compute year fraction from base date to target date using the curve's day-count.
    #[inline]
    fn year_fraction_to(&self, date: Date) -> crate::Result<f64> {
        super::common::year_fraction_to(self.base, date, self.day_count)
    }

    /// Return an interpolated par spread in basis points for reporting.
    /// Linear interpolation in spread, with log-linear fallback when values are positive and requested.
    ///
    /// # Arguments
    ///
    /// * `t` - Year fraction from the curve base date to the quoted CDS horizon.
    /// * `method` - Interpolation of stored par-spread quotes; log-linear falls
    ///   back to linear when a quote is non-positive.
    #[must_use]
    pub fn cds_quote_bp(&self, t: f64, method: ParInterp) -> f64 {
        if self.par_tenors.len() < 2 || self.par_tenors.len() != self.par_spreads_bp.len() {
            let lambda = self.hazard_rate(t.max(0.0));
            return (lambda * (1.0 - self.recovery_rate) * 10_000.0).max(0.0);
        }

        match method {
            ParInterp::Linear => {
                let strat = LinearStrategy;
                strat.interp(
                    t,
                    &self.par_tenors,
                    &self.par_spreads_bp,
                    ExtrapolationPolicy::FlatForward,
                )
            }
            ParInterp::LogLinear => {
                if let Ok(strat) = LogLinearStrategy::from_raw(
                    &self.par_tenors,
                    &self.par_spreads_bp,
                    ExtrapolationPolicy::FlatForward,
                ) {
                    strat.interp(
                        t,
                        &self.par_tenors,
                        &self.par_spreads_bp,
                        ExtrapolationPolicy::FlatForward,
                    )
                } else {
                    let strat = LinearStrategy;
                    strat.interp(
                        t,
                        &self.par_tenors,
                        &self.par_spreads_bp,
                        ExtrapolationPolicy::FlatForward,
                    )
                }
            }
        }
    }
}

impl Survival for HazardCurve {
    #[inline]
    fn id(&self) -> &CurveId {
        &self.id
    }

    #[inline]
    fn sp(&self, t: f64) -> f64 {
        self.sp(t)
    }

    #[inline]
    fn base_date(&self) -> Option<Date> {
        Some(self.base_date())
    }

    #[inline]
    fn day_count(&self) -> DayCount {
        self.day_count()
    }
}

/// Fluent builder for [`HazardCurve`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::term_structures::HazardCurve;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = HazardCurve::builder("USD-CREDIT")
///     .base_date(base)
///     .recovery_rate(0.40)
///     .knots([(1.0, 0.01), (5.0, 0.015), (10.0, 0.02)])
///     .build()
///     .expect("HazardCurve builder should succeed");
/// assert!(curve.sp(5.0) < 1.0);
/// ```
pub struct HazardCurveBuilder {
    id: CurveId,
    base: Date,
    points: Vec<(f64, f64)>, // (t, lambda)
    recovery_rate: Option<f64>,
    issuer: Option<String>,
    seniority: Option<Seniority>,
    currency: Option<Currency>,
    day_count: DayCount,
    par_points: Vec<(f64, f64)>, // (t, spread_bp)
    par_interp: ParInterp,
    /// Survival-probability interpolation style (default LogLinear).
    survival_interp: InterpStyle,
    /// Maximum allowed hazard rate (default 10.0).
    /// Rates above this trigger an error in `build()`.
    max_hazard_rate: f64,
    hazard_calibration: Option<super::HazardCalibrationRecipe>,
    fx_policy: Option<String>,
}

impl HazardCurveBuilder {
    /// Set the **base date** for the curve.
    pub fn base_date(mut self, d: Date) -> Self {
        self.base = d;
        self
    }
    /// Set issuer metadata.
    pub fn issuer(mut self, name: impl Into<String>) -> Self {
        self.issuer = Some(name.into());
        self
    }
    /// Set seniority metadata.
    pub fn seniority(mut self, s: Seniority) -> Self {
        self.seniority = Some(s);
        self
    }
    /// Set currency metadata.
    pub fn currency(mut self, ccy: Currency) -> Self {
        self.currency = Some(ccy);
        self
    }
    /// Set day-count convention for the curve time axis.
    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = day_count;
        self
    }
    /// Set recovery rate metadata.
    pub fn recovery_rate(mut self, r: f64) -> Self {
        self.recovery_rate = Some(r);
        self
    }
    /// Supply knot points `(t, λ)` where λ is the hazard rate.
    pub fn knots<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(pts);
        self
    }
    /// Store the market par spreads used for bootstrap for reporting.
    pub fn par_spreads<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.par_points.extend(pts);
        self
    }
    /// Set the interpolation method for par spreads.
    pub fn par_interp(mut self, method: ParInterp) -> Self {
        self.par_interp = method;
        self
    }

    /// Set the interpolation style for survival probabilities between
    /// pillars. The default [`InterpStyle::LogLinear`] is the market
    /// standard and the only supported style: it preserves consistency with
    /// the stored piecewise-constant hazard rates.
    ///
    /// # Arguments
    ///
    /// * `style` - Survival interpolation; must be [`InterpStyle::LogLinear`].
    ///   Other styles are rejected by [`build`](Self::build).
    pub fn interp(mut self, style: InterpStyle) -> Self {
        self.survival_interp = style;
        self
    }
    /// Attach the exact inputs used to calibrate this curve.
    ///
    /// # Arguments
    ///
    /// * `calibration` - Original hazard parameters, typed CDS quotes, and
    ///   solver policy required for deterministic quote-shock replay.
    pub fn hazard_calibration(mut self, calibration: super::HazardCalibrationRecipe) -> Self {
        self.hazard_calibration = Some(calibration);
        self
    }

    /// Optionally attach exact calibration replay inputs.
    ///
    /// # Arguments
    ///
    /// * `calibration` - Replay inputs to retain, or `None` for a curve that
    ///   was not produced by the calibration engine.
    pub fn hazard_calibration_opt(
        mut self,
        calibration: Option<super::HazardCalibrationRecipe>,
    ) -> Self {
        self.hazard_calibration = calibration;
        self
    }

    /// Set the maximum allowed hazard rate.
    ///
    /// During `build()`, any hazard rate exceeding this value triggers an error.
    /// The default is `10.0` (implies >99.995% 1Y default probability).
    pub fn max_hazard_rate(mut self, max: f64) -> Self {
        self.max_hazard_rate = max;
        self
    }

    /// Optionally set issuer metadata (no-op if `None`).
    pub fn issuer_opt(mut self, name: Option<impl Into<String>>) -> Self {
        self.issuer = name.map(Into::into);
        self
    }

    /// Optionally set seniority metadata (no-op if `None`).
    pub fn seniority_opt(mut self, s: Option<Seniority>) -> Self {
        self.seniority = s;
        self
    }

    /// Optionally set currency metadata (no-op if `None`).
    pub fn currency_opt(mut self, ccy: Option<Currency>) -> Self {
        self.currency = ccy;
        self
    }

    /// Stamp an opaque FX policy on the curve. See [`HazardCurve::fx_policy`].
    pub fn fx_policy(mut self, policy: impl Into<String>) -> Self {
        self.fx_policy = Some(policy.into());
        self
    }

    /// Optionally stamp an FX policy; `None` is a no-op. Used by the serde
    /// round-trip path and by curve builders propagating metadata.
    pub fn fx_policy_opt(mut self, policy: Option<String>) -> Self {
        self.fx_policy = policy;
        self
    }

    /// Validate input and build the [`HazardCurve`].
    ///
    /// # Validation
    ///
    /// - Base date must be explicitly set (not the default 1970-01-01)
    /// - At least one knot point required
    /// - All hazard rates must be non-negative and finite
    /// - Hazard rates > `max_hazard_rate` (default 10.0) trigger an error
    /// - Recovery rate must be supplied explicitly and lie in [0, 1]
    /// - Knot times must be strictly increasing after sorting by time
    /// - Stored par-spread tenors must be finite and non-negative, and spreads
    ///   must be finite; they are retained for reporting rather than used to
    ///   re-bootstrap hazards
    ///
    /// The only supported survival interpolation is log-linear, which corresponds to
    /// piecewise-constant hazards between pillars. A zero-time knot is allowed;
    /// its hazard applies from the base date onward.
    ///
    /// # Errors
    ///
    /// Returns an error when the base date was not explicitly set, no knots
    /// are supplied, a time, rate, recovery rate, or stored par spread is
    /// invalid, knot times are duplicated, or a hazard rate exceeds the
    /// configured maximum, or survival interpolation is not log-linear.
    /// Input points are sorted by time before the curve is
    /// constructed; callers need not pre-sort them.
    pub fn build(self) -> crate::Result<HazardCurve> {
        if self.survival_interp != InterpStyle::LogLinear {
            return Err(crate::Error::Validation(
                "HazardCurve requires log-linear survival interpolation for piecewise-constant hazards".to_string(),
            ));
        }
        // Require explicit base_date to avoid accidentally anchoring to 1970-01-01
        let default_base =
            Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN);
        if self.base == default_base {
            return Err(InputError::Invalid.into());
        }
        if self.points.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }

        let recovery_rate = self.recovery_rate.ok_or_else(|| {
            crate::Error::Validation(
                "HazardCurve requires an explicit recovery_rate in [0, 1]".to_string(),
            )
        })?;

        // Validate knot times and hazard rates: times must be finite/non-negative;
        // rates non-negative and finite; a zero-time anchor is allowed, but all
        // subsequent knots must increase strictly.
        for &(t, lambda) in &self.points {
            if !t.is_finite() || t < 0.0 {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve knot time must be finite and non-negative, got t={t}"
                )));
            }
            if lambda < 0.0 {
                return Err(InputError::NegativeValue.into());
            }
            if !lambda.is_finite() {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve hazard rate must be finite, got lambda={lambda} at t={t}"
                )));
            }
            // Sanity check: λ exceeding max_hazard_rate is almost certainly a
            // data error (units confusion, etc.).  Default limit is 10.0 which
            // implies >99.995% 1Y default probability.
            if lambda > self.max_hazard_rate {
                return Err(crate::Error::Validation(format!(
                    "Hazard rate {lambda:.4} at t={t:.4}y exceeds maximum {:.4}. \
                     Use .allow_high_hazard_rates() or .max_hazard_rate() to override.",
                    self.max_hazard_rate
                )));
            }
        }

        super::common::validate_unit_range(recovery_rate, "recovery_rate")?;

        let mut points = self.points;
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (kvec, lvec): (Vec<f64>, Vec<f64>) = points.into_iter().unzip();
        if kvec.len() > 1 {
            for i in 1..kvec.len() {
                if kvec[i] <= kvec[i - 1] {
                    return Err(InputError::Invalid.into());
                }
            }
        }
        let mut par_pts = self.par_points;
        for &(t, spread) in &par_pts {
            if !t.is_finite() || t < 0.0 || !spread.is_finite() {
                return Err(InputError::Invalid.into());
            }
        }
        par_pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (p_ten, p_spd): (Vec<f64>, Vec<f64>) = par_pts.into_iter().unzip();
        if let Some(recipe) = &self.hazard_calibration {
            recipe.validate()?;
        }

        // Convert hazard rates to survival probabilities for interpolation
        // using the single canonical λ-attribution convention shared with
        // `rebuild_interp` (see `survival_pillars`).
        let (interp_kvec, interp_svec) = survival_pillars(&kvec, &lvec);

        // Build interpolator over survival probabilities. The default
        // LogLinear style implies a piecewise-constant hazard rate.
        // Extrapolate with FlatForward (constant hazard rate at tail).
        let interp = super::common::build_interp(
            self.survival_interp,
            interp_kvec.into_boxed_slice(),
            interp_svec.into_boxed_slice(),
            ExtrapolationPolicy::FlatForward,
        )?;

        Ok(HazardCurve {
            id: self.id,
            base: self.base,
            knots: kvec.into_boxed_slice(),
            lambdas: lvec.into_boxed_slice(),
            recovery_rate,
            issuer: self.issuer,
            seniority: self.seniority,
            currency: self.currency,
            day_count: self.day_count,
            par_tenors: p_ten.into_boxed_slice(),
            par_spreads_bp: p_spd.into_boxed_slice(),
            par_interp: self.par_interp,
            survival_interp_style: self.survival_interp,
            hazard_calibration: self.hazard_calibration,
            interp,
            fx_policy: self.fx_policy,
        })
    }
}

/// Convert piecewise-constant hazard knots `(tᵢ, λᵢ)` into survival-probability
/// interpolation pillars `(tᵢ, S(tᵢ))` anchored at `(0, 1)`.
///
/// This is the **single canonical λ-attribution convention** used by both the
/// builder (`HazardCurveBuilder::build`) and in-place rebuilds
/// (`HazardCurve::rebuild_interp`, the `MarketContext::bump` / CS01 path):
///
/// - **Ending-segment attribution**: λᵢ applies to the segment *ending* at tᵢ
///   (segment `(tᵢ₋₁, tᵢ]` accrues `λᵢ·(tᵢ − tᵢ₋₁)`; segment `(0, t₁]` uses λ₁).
///   A redundant t≈0 knot is ignored for the integral; its lambda is not
///   applied to `(0, t₁]`.
///
/// Sharing this function guarantees that bumping a curve in place with a
/// zero-size bump is an exact no-op (no silent re-attribution of base hazards
/// into spurious CS01 components).
fn survival_pillars(knots: &[f64], lambdas: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut interp_knots = Vec::with_capacity(knots.len() + 1);
    let mut interp_sp = Vec::with_capacity(knots.len() + 1);
    interp_knots.push(0.0);
    interp_sp.push(1.0);

    let mut accum = 0.0;
    let mut prev_t = 0.0;

    for (&t, &lambda) in knots.iter().zip(lambdas.iter()) {
        if t <= 1e-9 {
            continue;
        }
        accum += lambda * (t - prev_t);
        interp_knots.push(t);
        interp_sp.push((-accum).exp());
        prev_t = t;
    }

    (interp_knots, interp_sp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_data::term_structures::{HazardCalibrationInput, HazardCalibrationRecipe};
    use time::Month;

    fn test_hazard_recipe(base: Date) -> HazardCalibrationRecipe {
        let input = HazardCalibrationInput {
            quote: serde_json::json!({
                "type": "cds_par_spread",
                "id": "CDS-5Y"
            }),
            pillar_date: base + time::Duration::days(365),
            pillar_time: 1.0,
        };
        HazardCalibrationRecipe::new(
            serde_json::json!({"curve_id": "RECIPE"}),
            vec![input.clone()],
            vec![input],
            serde_json::json!({"fail_on_bad_fit": true}),
        )
        .expect("valid hazard calibration recipe")
    }

    #[test]
    fn hazard_recipe_invalidation_covers_synthetic_transformations() {
        use crate::market_data::bumps::BumpSpec;

        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let source = HazardCurve::builder("RECIPE")
            .base_date(base)
            .recovery_rate(0.4)
            .knots([(1.0, 0.01), (5.0, 0.02), (10.0, 0.03)])
            .hazard_calibration(test_hazard_recipe(base))
            .build()
            .expect("recipe-backed curve");

        let recovery = source.with_recovery_rate(0.35).expect("recovery override");
        assert!(recovery.hazard_calibration().is_none());

        let parallel = source
            .with_parallel_hazard_rate_bump_bp(1.0)
            .expect("parallel bump");
        assert!(parallel.hazard_calibration().is_none());

        let rolled = source.roll_forward(30).expect("curve roll");
        assert!(rolled.hazard_calibration().is_none());

        let rebuilt = source
            .to_builder_with_id("REBUILT")
            .build()
            .expect("generic rebuild");
        assert!(rebuilt.hazard_calibration().is_none());

        let mut in_place = source;
        in_place
            .bump_in_place(&BumpSpec::parallel_bp(1.0))
            .expect("in-place bump");
        assert!(in_place.hazard_calibration().is_none());
    }

    #[test]
    fn survival_interpolation_preserves_hazard_consistency() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let builder = || {
            HazardCurve::builder("HZ")
                .base_date(base)
                .knots([(1.0, 0.02), (2.0, 1.0)])
                .recovery_rate(0.4)
        };
        assert!(builder().interp(InterpStyle::Linear).build().is_err());
        let curve = builder().build().expect("valid hazard curve");
        for t in [0.5, 1.5, 3.0] {
            let eps = 1e-5;
            let implied = -(curve.sp(t + eps).ln() - curve.sp(t - eps).ln()) / (2.0 * eps);
            assert!((implied - curve.hazard_rate(t)).abs() < 1e-9);
        }
        let mut state = serde_json::to_value(&curve).expect("serialize");
        state["survival_interp"] = serde_json::to_value(InterpStyle::Linear).expect("style");
        assert!(serde_json::from_value::<HazardCurve>(state).is_err());
    }

    #[test]
    fn survival_monotone_decreasing() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let hc = HazardCurve::builder("USD-CREDIT")
            .base_date(base)
            .knots([(1.0, 0.01), (5.0, 0.02)])
            .recovery_rate(0.40)
            .build()
            .expect("HazardCurve builder should succeed with valid test data");
        assert!(hc.sp(1.0) < 1.0);
        assert!(hc.sp(6.0) < hc.sp(1.0));
    }

    #[test]
    fn default_prob_positive() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let hc = HazardCurve::builder("USD")
            .base_date(base)
            .knots([(1.0, 0.01), (10.0, 0.015)])
            .recovery_rate(0.40)
            .build()
            .expect("HazardCurve builder should succeed with valid test data");
        let dp = hc
            .default_prob(2.0, 4.0)
            .expect("default_prob should succeed with valid inputs");
        assert!(dp >= 0.0);
    }

    #[test]
    fn zero_anchored_tail_uses_last_hazard() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let curve = HazardCurve::builder("TAIL")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 0.01), (5.0, 0.02), (10.0, 0.03)])
            .recovery_rate(0.40)
            .build()
            .expect("valid hazard curve");

        let implied_tail_hazard = -(curve.sp(11.0) / curve.sp(10.0)).ln();
        assert!((implied_tail_hazard - 0.03).abs() < 1e-12);
        assert!((curve.hazard_rate(10.0) - 0.03).abs() < 1e-12);
    }

    #[test]
    fn unanchored_hazard_changes_immediately_after_end_knot() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let curve = HazardCurve::builder("BOUNDARY")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(1.0, 0.01), (2.0, 0.02), (3.0, 0.03)])
            .recovery_rate(0.40)
            .build()
            .expect("valid hazard curve");

        assert!((curve.hazard_rate(1.0) - 0.01).abs() < 1e-12);
        assert!((curve.hazard_rate(1.0 + 1e-12) - 0.02).abs() < 1e-12);
    }

    #[test]
    fn roll_forward_preserves_conditional_survival() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let curve = HazardCurve::builder("ROLL")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 0.01), (5.0, 0.02), (10.0, 0.03)])
            .recovery_rate(0.40)
            .build()
            .expect("valid hazard curve");

        let rolled = curve
            .roll_forward(365)
            .expect("one-year roll should succeed");
        for t in [0.5, 1.0, 4.0, 5.0, 8.0] {
            let expected = curve.sp(t + 1.0) / curve.sp(1.0);
            assert!(
                (rolled.sp(t) - expected).abs() < 1e-12,
                "t={t}: rolled={}, expected={expected}",
                rolled.sp(t)
            );
        }
    }

    #[test]
    fn quoted_spread_interpolation_linear() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let hc = HazardCurve::builder("TEST")
            .base_date(base)
            .knots([(1.0, 0.02)])
            .par_spreads([(1.0, 100.0), (3.0, 200.0)])
            .recovery_rate(0.40)
            .build()
            .expect("HazardCurve builder should succeed with valid test data");
        assert!((hc.cds_quote_bp(2.0, ParInterp::Linear) - 150.0).abs() < 1e-9);
    }

    #[test]
    fn roll_forward_works() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let _hc = HazardCurve::builder("TEST-ROLL")
            .base_date(base)
            .day_count(DayCount::Act365F) // Use Act365F for simple math
            .knots([(0.5, 0.01), (1.5, 0.02)])
            .recovery_rate(0.40)
            .build()
            .expect("Builder works");

        let hc = HazardCurve::builder("TEST-ROLL")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.5, 0.01), (1.5, 0.02), (2.5, 0.03)])
            .recovery_rate(0.40)
            .build()
            .expect("Builder works");

        let rolled = hc.roll_forward(183).expect("Roll should succeed"); // > 0.5 years

        assert_eq!(rolled.base_date(), base + time::Duration::days(183));

        let knots: Vec<f64> = rolled.knot_points().map(|(t, _)| t).collect();
        assert_eq!(knots.len(), 3);
        assert!(
            knots[0].abs() < 1e-12,
            "rolled curve must be anchored at zero"
        );
        // 1.5 - (183/365) = 1.5 - 0.50137 = 0.9986
        // 2.5 - (183/365) = 1.9986
        assert!(knots[1] < 1.0 && knots[1] > 0.99);
    }

    #[test]
    fn builder_allows_explicit_zero_time_knot() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let result = HazardCurve::builder("USD-CREDIT")
            .base_date(base)
            .knots([(0.0, 0.01), (5.0, 0.02)])
            .recovery_rate(0.40)
            .build();

        assert!(result.is_ok(), "t=0 hazard knots should be accepted");
    }

    #[test]
    fn hazard_rate_is_available_for_valid_built_curves() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let hc = HazardCurve::builder("USD-CREDIT")
            .base_date(base)
            .knots([(0.0, 0.01), (5.0, 0.02)])
            .recovery_rate(0.40)
            .build()
            .expect("HazardCurve builder should succeed with valid test data");

        assert_eq!(hc.hazard_rate(-1.0), 0.01);
        assert_eq!(hc.hazard_rate(0.0), 0.01);
        assert_eq!(hc.hazard_rate(10.0), 0.02);
    }

    /// Regression test (2026-06-09 "Major — market data"
    /// item 2): `build()` and `rebuild_interp` (the `MarketContext::bump` /
    /// CS01 path via `bump_in_place`) must share one λ-segment attribution
    /// convention. A zero-size bump must be an exact no-op even for curves
    /// with an explicit t=0 anchor knot.
    #[test]
    fn zero_size_bump_in_place_is_noop_for_zero_anchored_curve() {
        use crate::market_data::bumps::BumpSpec;

        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let curve = HazardCurve::builder("ZERO-ANCHORED")
            .base_date(base)
            .recovery_rate(0.40)
            .knots([(0.0, 0.01), (1.0, 0.02), (5.0, 0.015)])
            .build()
            .expect("zero-anchored hazard curve builds");

        let mut bumped = curve.clone();
        bumped
            .bump_in_place(&BumpSpec::parallel_bp(0.0))
            .expect("zero bump succeeds");

        for t in [0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 7.0] {
            assert!(
                (bumped.sp(t) - curve.sp(t)).abs() < 1e-15,
                "zero bump must not change survival at t={t}: \
                 bumped {} vs base {}",
                bumped.sp(t),
                curve.sp(t)
            );
        }
    }

    /// A small parallel spread bump on a zero-anchored curve must shift the
    /// average hazard −ln(S(t))/t by exactly spread/(1−R) at every t inside
    /// the knot range — with no spurious re-attribution of base hazards.
    #[test]
    fn small_bump_in_place_shifts_average_hazard_uniformly() {
        use crate::market_data::bumps::BumpSpec;

        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let recovery = 0.40;
        let curve = HazardCurve::builder("ZERO-ANCHORED-SMALL")
            .base_date(base)
            .recovery_rate(recovery)
            .knots([(0.0, 0.01), (1.0, 0.02), (5.0, 0.015)])
            .build()
            .expect("zero-anchored hazard curve builds");

        let spread_bp = 10.0;
        let expected_shift = (spread_bp / 10_000.0) / (1.0 - recovery);

        let mut bumped = curve.clone();
        bumped
            .bump_in_place(&BumpSpec::parallel_bp(spread_bp))
            .expect("small bump succeeds");

        for t in [0.5, 1.0, 2.0, 3.0, 5.0] {
            let base_avg = -curve.sp(t).ln() / t;
            let bumped_avg = -bumped.sp(t).ln() / t;
            assert!(
                (bumped_avg - base_avg - expected_shift).abs() < 1e-12,
                "average hazard change at t={t} must equal the bump: \
                 got {}, expected {}",
                bumped_avg - base_avg,
                expected_shift
            );
        }
    }

    /// `bump_in_place` clears stored par-spread quotes (they were calibrated
    /// to the unbumped hazards); `cds_quote_bp` then falls back to the
    /// hazard-based approximation λ·(1−R)·1e4 reflecting the bumped curve.
    #[test]
    fn bump_in_place_clears_stale_par_spread_quotes() {
        use crate::market_data::bumps::BumpSpec;

        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let recovery = 0.40;
        let mut curve = HazardCurve::builder("QUOTED")
            .base_date(base)
            .recovery_rate(recovery)
            .knots([(1.0, 0.01), (5.0, 0.01)])
            .par_spreads([(1.0, 60.0), (5.0, 60.0)])
            .build()
            .expect("quoted hazard curve builds");

        curve
            .bump_in_place(&BumpSpec::parallel_bp(10.0))
            .expect("bump succeeds");

        assert_eq!(
            curve.par_spread_points().count(),
            0,
            "stale par quotes must be cleared on bump"
        );
        // Fallback quote reflects the bumped hazard: (0.01 + 0.001/0.6)·0.6·1e4 = 70bp.
        let quote = curve.cds_quote_bp(3.0, ParInterp::Linear);
        assert!(
            (quote - 70.0).abs() < 1e-9,
            "fallback quote must reflect bumped hazards, got {quote}"
        );
    }

    #[test]
    fn direct_hazard_rate_parallel_bump_is_additive() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let curve = HazardCurve::builder("DIRECT-PARALLEL")
            .base_date(base)
            .recovery_rate(0.40)
            .knots([(1.0, 0.010), (3.0, 0.015), (5.0, 0.020)])
            .par_spreads([(1.0, 60.0), (3.0, 90.0), (5.0, 120.0)])
            .build()
            .expect("valid hazard curve");

        let up = curve
            .with_parallel_hazard_rate_bump_bp(25.0)
            .expect("parallel direct shock");
        let round_trip = up
            .with_parallel_hazard_rate_bump_bp(-25.0)
            .expect("reverse direct shock");

        for (tenor, base_rate) in curve.knot_points() {
            assert!((up.hazard_rate(tenor) - base_rate - 25e-4).abs() < 1e-12);
            assert!((round_trip.hazard_rate(tenor) - base_rate).abs() < 1e-12);
        }
        assert_eq!(up.id(), curve.id());
        assert_eq!(up.par_spread_points().count(), 0);
        assert!(up.hazard_calibration().is_none());
    }

    #[test]
    fn direct_hazard_rate_tenor_bumps_preserve_segment_matching_and_additivity() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let curve = HazardCurve::builder("DIRECT-TENORS")
            .base_date(base)
            .recovery_rate(0.40)
            .knots([(1.0, 0.010), (3.0, 0.015), (5.0, 0.020), (10.0, 0.025)])
            .build()
            .expect("valid hazard curve");

        let bumped = curve
            .with_tenor_hazard_rate_bumps_bp(&[
                (4.0, 3.0),
                (4.0, 5.0),
                (3.0, 2.0),
                (10.0 + 2e-6, 100.0),
            ])
            .expect("tenor direct shocks");
        let expected = [0.010, 0.016, 0.020, 0.025];
        for ((tenor, rate), expected_rate) in bumped.knot_points().zip(expected) {
            assert!(
                (rate - expected_rate).abs() < 1e-12,
                "unexpected direct hazard rate at {tenor}: {rate}"
            );
        }
    }

    #[test]
    fn failed_bump_in_place_is_atomic() {
        use crate::market_data::bumps::BumpSpec;

        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let original = HazardCurve::builder("ATOMIC")
            .base_date(base)
            .recovery_rate(0.40)
            .knots([(1.0, 0.010), (5.0, 0.001)])
            .build()
            .expect("valid hazard curve");
        let mut attempted = original.clone();

        attempted
            .bump_in_place(&BumpSpec::parallel_bp(-30.0))
            .expect_err("second hazard node would become negative");

        assert_eq!(
            attempted.knot_points().collect::<Vec<_>>(),
            original.knot_points().collect::<Vec<_>>()
        );
        for t in [0.5, 1.0, 3.0, 5.0] {
            assert_eq!(attempted.sp(t), original.sp(t));
        }
    }

    #[test]
    fn parallel_bump_rejects_negative_shift_that_crosses_zero_hazard() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let curve = HazardCurve::builder("HY")
            .base_date(base)
            .knots([(1.0, 0.001), (5.0, 0.002)])
            .recovery_rate(0.40)
            .build()
            .expect("valid hazard curve");

        let err = curve
            .with_parallel_hazard_rate_bump_bp(-15.0)
            .expect_err("negative shifted hazard rate must be rejected");

        assert!(
            err.to_string().contains("negative hazard rate after bump"),
            "unexpected error: {err}"
        );
    }
}

/// Seniority level for credit exposures.
///
/// Used to tag a hazard curve with the seniority of the issuer's debt
/// observed by the curve. Drives the recovery-rate prior in default
/// modelling and selects the right LGD prior in
/// `finstack_quant_models::credit::lgd::seniority`.
///
/// Order is **not** total — `SeniorSecured` is strictly senior to `Senior`,
/// `Subordinated`, and `Junior`, but the relative ordering of `Subordinated`
/// vs. `Junior` is jurisdiction-dependent. Do not rely on `Ord` semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Seniority {
    /// Senior secured debt
    SeniorSecured,
    /// Senior unsecured debt
    Senior,
    /// Subordinated debt
    Subordinated,
    /// Junior/mezzanine debt
    Junior,
}

impl core::fmt::Display for Seniority {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Seniority::SeniorSecured => write!(f, "senior_secured"),
            Seniority::Senior => write!(f, "senior"),
            Seniority::Subordinated => write!(f, "subordinated"),
            Seniority::Junior => write!(f, "junior"),
        }
    }
}

impl core::str::FromStr for Seniority {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        match s {
            "senior_secured" => Ok(Self::SeniorSecured),
            "senior" => Ok(Self::Senior),
            "subordinated" => Ok(Self::Subordinated),
            "junior" => Ok(Self::Junior),
            _ => Err(crate::error::InputError::Invalid.into()),
        }
    }
}

/// Interpolation method for reporting par spreads stored on the curve.
///
/// Applies only to *par-spread* readouts (the spreads quoted at calibration
/// pillars), not to the underlying hazard rates. Hazard interpolation always
/// follows piecewise-constant survival. Use `LogLinear` when spreads span
/// multiple decades (e.g. high-yield issuers) so interpolation stays in
/// log-space; otherwise the default `Linear` is fine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ParInterp {
    /// Linear interpolation in spread space
    #[default]
    Linear,
    /// Log-linear interpolation when spreads are strictly positive
    LogLinear,
}

#[cfg(test)]
mod seniority_tests {
    use super::Seniority;

    #[test]
    fn test_seniority_fromstr_display_roundtrip() {
        for (input, expected) in [
            ("senior_secured", Seniority::SeniorSecured),
            ("senior", Seniority::Senior),
            ("subordinated", Seniority::Subordinated),
            ("junior", Seniority::Junior),
        ] {
            assert!(matches!(input.parse::<Seniority>(), Ok(value) if value == expected));
        }

        for variant in [
            Seniority::SeniorSecured,
            Seniority::Senior,
            Seniority::Subordinated,
            Seniority::Junior,
        ] {
            let display = variant.to_string();
            assert!(matches!(display.parse::<Seniority>(), Ok(value) if value == variant));
        }
    }

    #[test]
    fn test_seniority_fromstr_rejects_unknown() {
        for rejected in ["sub", "Senior", "senior-secured", " senior"] {
            assert!(rejected.parse::<Seniority>().is_err());
        }
    }
}
