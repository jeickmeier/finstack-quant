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
//! use finstack_core::market_data::term_structures::HazardCurve;
//! use finstack_core::dates::Date;
//! use time::Month;
//!
//! let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let hc = HazardCurve::builder("USD-CREDIT")
//!     .base_date(base)
//!     .knots([(1.0, 0.01), (10.0, 0.015)])
//!     .build()
//!     .expect("HazardCurve builder should succeed");
//! assert!(hc.sp(5.0) < 1.0); // Survival probability < 1
//! ```
//!
//! # References
//!
//! - **CDS Pricing**:
//!   - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*.
//!     Wiley Finance. Chapters 3-5.
//!   - ISDA (2009). "ISDA CDS Standard Model." Version 1.8.2.
//!
//! - **Hazard Rate Models**:
//!   - Duffie, D., & Singleton, K. J. (1999). "Modeling Term Structures of Defaultable
//!     Bonds." *Review of Financial Studies*, 12(4), 687-720.
//!   - Lando, D. (1998). "On Cox Processes and Credit Risky Securities."
//!     *Review of Derivatives Research*, 2(2-3), 99-120.
//!
//! - **Industry Practice**:
//!   - Markit (2009). "CDS Curve Bootstrapping Guide."
//!   - Bloomberg (2013). "Credit Curve Construction and CDS Pricing Guide."

use crate::{
    currency::Currency,
    dates::{Date, DayCount, DayCountContext},
    error::InputError,
    market_data::traits::{Survival, TermStructure},
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
    /// Interpolator for survival probabilities
    interp: Interp,
}

/// Raw serializable state of a HazardCurve
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHazardCurve {
    #[serde(flatten)]
    common_id: super::common::StateId,
    /// Base date
    pub base: Date,
    #[serde(flatten)]
    points: super::common::StateKnotPoints,
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
}

fn default_par_interp() -> ParInterp {
    ParInterp::Linear
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
            common_id: super::common::StateId {
                id: curve.id.to_string(),
            },
            base: curve.base,
            points: super::common::StateKnotPoints { knot_points },
            recovery_rate: curve.recovery_rate,
            issuer: curve.issuer,
            seniority: curve.seniority,
            currency: curve.currency,
            day_count: curve.day_count,
            par_points,
            par_interp: curve.par_interp,
        }
    }
}

impl TryFrom<RawHazardCurve> for HazardCurve {
    type Error = crate::Error;

    fn try_from(state: RawHazardCurve) -> crate::Result<Self> {
        HazardCurve::builder(state.common_id.id)
            .base_date(state.base)
            .recovery_rate(state.recovery_rate)
            .day_count(state.day_count)
            .knots(state.points.knot_points)
            .par_spreads(state.par_points)
            .par_interp(state.par_interp)
            .issuer_opt(state.issuer)
            .seniority_opt(state.seniority)
            .currency_opt(state.currency)
            .build()
    }
}

impl HazardCurve {
    /// Start building a hazard curve with identifier `id`.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> HazardCurveBuilder {
        // Epoch date - unwrap_or provides defensive fallback for infallible operation
        let base =
            Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN);
        HazardCurveBuilder {
            id: id.into(),
            base,
            points: Vec::new(),
            recovery_rate: crate::credit::registry::default_market_recovery_rate_or_panic(),
            issuer: None,
            seniority: None,
            currency: None,
            day_count: DayCount::Act365F,
            par_points: Vec::new(),
            par_interp: ParInterp::Linear,
            max_hazard_rate: 10.0,
        }
    }

    /// Survival probability S(t) up to time `t` (in **years**).
    #[must_use]
    pub fn sp(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 1.0;
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
    /// # Arguments
    /// * `t` - Time in years
    #[must_use]
    pub fn hazard_rate(&self, t: f64) -> f64 {
        // A valid hazard curve always has at least one lambda.
        assert!(
            !self.lambdas.is_empty(),
            "HazardCurve invariant violated: empty lambdas"
        );
        if t <= 0.0 {
            // Return first hazard rate for t<=0
            return self.lambdas[0];
        }

        let mut idx = self.knots.partition_point(|&k| k < t);
        if self.knots.first().is_some_and(|&k| k <= 1e-9) {
            idx = idx.saturating_sub(1);
        }
        idx = idx.min(self.lambdas.len() - 1);
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

    /// Default probability between two dates using the curve's day-count.
    ///
    /// This is the date-based equivalent of [`default_prob`](Self::default_prob).
    ///
    /// # Errors
    ///
    /// Returns an error if year fraction calculation fails or if `d2 < d1`.
    #[inline]
    #[must_use = "computed default probability should not be discarded"]
    pub fn default_prob_on_dates(&self, d1: Date, d2: Date) -> crate::Result<f64> {
        let t1 = self.year_fraction_to(d1)?;
        let t2 = self.year_fraction_to(d2)?;
        self.default_prob(t1, t2)
    }

    /// Evaluate survival probabilities at the provided dates using this curve's time axis.
    #[must_use = "computed survival probabilities should not be discarded"]
    pub fn survival_at_dates(&self, dates: &[Date]) -> crate::Result<Vec<f64>> {
        let base = self.base_date();
        let dc = self.day_count();
        let mut survival = Vec::with_capacity(dates.len());

        for &date in dates {
            let t = dc.year_fraction(base, date, DayCountContext::default())?;
            let sp = self.sp(t).clamp(0.0, 1.0);
            survival.push(sp);
        }

        Ok(survival)
    }

    /// Accessors
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
            interp: self.interp.clone(),
        })
    }

    /// Create a builder with this curve's parameters, using a new ID.
    /// Useful for creating modified versions of the curve.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> HazardCurveBuilder {
        HazardCurve::builder(new_id)
            .base_date(self.base)
            .recovery_rate(self.recovery_rate)
            .day_count(self.day_count)
            .par_interp(self.par_interp)
            .issuer_opt(self.issuer.clone())
            .seniority_opt(self.seniority)
            .currency_opt(self.currency)
            .knots(self.knot_points())
            .par_spreads(self.par_spread_points())
    }

    /// Recompute the survival-probability interpolator from current knots/lambdas.
    fn rebuild_interp(&mut self) -> crate::Result<()> {
        use crate::math::interp::{ExtrapolationPolicy, InterpStyle};

        let mut interp_knots = Vec::with_capacity(self.knots.len() + 1);
        let mut interp_sp = Vec::with_capacity(self.knots.len() + 1);
        interp_knots.push(0.0);
        interp_sp.push(1.0);

        let mut accum = 0.0;
        let mut prev_t = 0.0;
        for (&t, &lambda) in self.knots.iter().zip(self.lambdas.iter()) {
            if t <= 1e-9 {
                prev_t = t;
                continue;
            }
            accum += lambda * (t - prev_t);
            interp_knots.push(t);
            interp_sp.push((-accum).exp());
            prev_t = t;
        }

        self.interp = super::common::build_interp(
            InterpStyle::LogLinear,
            interp_knots.into_boxed_slice(),
            interp_sp.into_boxed_slice(),
            ExtrapolationPolicy::FlatForward,
        )?;
        Ok(())
    }

    /// Apply a bump specification in-place, mutating lambda values and rebuilding the interpolator.
    pub(crate) fn bump_in_place(
        &mut self,
        spec: &crate::market_data::bumps::BumpSpec,
    ) -> crate::Result<()> {
        use crate::market_data::bumps::BumpType;

        if !matches!(spec.bump_type, BumpType::Parallel) {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: "HazardCurve only supports Parallel bumps, not key-rate bumps".to_string(),
            }
            .into());
        }

        let recovery = self.recovery_rate;
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
        for lambda in self.lambdas.iter_mut() {
            let shifted = *lambda + shift;
            if shifted < 0.0 {
                return Err(crate::error::InputError::UnsupportedBump {
                    reason: "negative hazard rate after bump".to_string(),
                }
                .into());
            }
            *lambda = shifted;
        }
        self.rebuild_interp()
    }

    /// Create a new curve with hazard rates shifted by a constant amount.
    ///
    /// This is the hazard curve equivalent of the parallel bump applied to other
    /// term structures (`DiscountCurve::with_parallel_bump`, etc.).
    ///
    /// # Arguments
    /// * `shift` - Additive shift to all hazard rates (e.g., 0.0001 for +1bp).
    ///   Negative shifts that would make any hazard rate negative are rejected.
    pub fn with_parallel_bump(&self, shift: f64) -> crate::Result<HazardCurve> {
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

        // Create a temporary ID for the bumped curve
        // In practice, the caller will manage IDs when building market contexts
        let temp_id = format!("{}_bump_{:.4}bp", self.id(), shift * 10_000.0);

        HazardCurve::builder(temp_id)
            .base_date(self.base)
            .recovery_rate(self.recovery_rate)
            .day_count(self.day_count)
            .knots(shifted_points)
            .par_interp(self.par_interp)
            .issuer_opt(self.issuer.clone())
            .seniority_opt(self.seniority)
            .currency_opt(self.currency)
            .par_spreads(self.par_spread_points())
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
    /// Returns an error if fewer than 2 knot points remain after filtering expired points.
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        let new_base = self.base + time::Duration::days(days);
        // Use consistent day count logic (same as DiscountCurve/ForwardCurve)
        // This is a behavior change from "days/365.0" to actual day count, which is more correct.
        let dt_years =
            self.day_count
                .year_fraction(self.base, new_base, DayCountContext::default())?;

        // Shift knots and filter expired points using shared helper
        let rolled_points = super::common::roll_knots(&self.knots, &self.lambdas, dt_years);

        if rolled_points.len() < 2 {
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        // Also roll par spread points
        // Note: par_spreads also use "t" as years from base, so we can reuse roll_knots logic
        // even though they aren't "knots" for the curve itself.
        let rolled_par_points =
            super::common::roll_knots(&self.par_tenors, &self.par_spreads_bp, dt_years);

        let mut builder = HazardCurve::builder(self.id.clone())
            .base_date(new_base)
            .recovery_rate(self.recovery_rate)
            .day_count(self.day_count)
            .knots(rolled_points)
            .par_interp(self.par_interp)
            .issuer_opt(self.issuer.clone())
            .seniority_opt(self.seniority)
            .currency_opt(self.currency);

        // Add rolled par spread points if any
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
    #[must_use]
    pub fn cds_quote_bp(&self, t: f64, method: ParInterp) -> f64 {
        // If the curve was constructed without explicit par-spread quotes, fall back to a
        // simple hazard-based approximation instead of panicking inside interpolators.
        //
        // This function is used in some pricing paths (e.g., options) to obtain a
        // representative spread at a horizon, so "no quotes" must be handled gracefully.
        if self.par_tenors.len() < 2 || self.par_tenors.len() != self.par_spreads_bp.len() {
            let lambda = self.hazard_rate(t.max(0.0));
            return (lambda * (1.0 - self.recovery_rate) * 10_000.0).max(0.0);
        }

        // Use shared interpolation strategies from math::interp
        // Note: For LogLinear, we rebuild the strategy on the fly since we don't store log-values for par spreads.
        // This involves allocation but is acceptable for a reporting method.
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
                // If construction fails (e.g. non-positive values), fallback to linear to match previous behavior
                // which just did linear if y1 <= 0 || y2 <= 0.
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
                    // Fallback to linear if log-linear fails construction (e.g. 0 or negative spreads)
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

// Minimal trait implementations for polymorphism where needed

impl Survival for HazardCurve {
    #[inline]
    fn sp(&self, t: f64) -> f64 {
        self.sp(t)
    }
}

impl TermStructure for HazardCurve {
    #[inline]
    fn id(&self) -> &CurveId {
        &self.id
    }
}

/// Fluent builder for [`HazardCurve`].
///
/// # Examples
///
/// ```rust
/// use finstack_core::market_data::term_structures::HazardCurve;
/// use finstack_core::dates::Date;
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
    recovery_rate: f64,
    issuer: Option<String>,
    seniority: Option<Seniority>,
    currency: Option<Currency>,
    day_count: DayCount,
    par_points: Vec<(f64, f64)>, // (t, spread_bp)
    par_interp: ParInterp,
    /// Maximum allowed hazard rate (default 10.0).
    /// Rates above this trigger an error in `build()`.
    max_hazard_rate: f64,
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
    pub fn day_count(mut self, dc: DayCount) -> Self {
        self.day_count = dc;
        self
    }
    /// Set recovery rate metadata.
    pub fn recovery_rate(mut self, r: f64) -> Self {
        self.recovery_rate = r;
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

    /// Remove the upper bound on hazard rates (sets the limit to infinity).
    ///
    /// Useful for stress-testing or distressed-credit scenarios where very high
    /// hazard rates are intentional.
    pub fn allow_high_hazard_rates(mut self) -> Self {
        self.max_hazard_rate = f64::INFINITY;
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
    /// - Recovery rate must be in [0, 1]
    /// - Knot times must be strictly increasing
    pub fn build(self) -> crate::Result<HazardCurve> {
        // Require explicit base_date to avoid accidentally anchoring to 1970-01-01
        // unwrap_or provides defensive fallback - comparison still works correctly
        let default_base =
            Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN);
        if self.base == default_base {
            return Err(InputError::Invalid.into());
        }
        if self.points.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }

        // Validate knot times and hazard rates: times must be finite/non-negative;
        // rates non-negative and finite; a zero-time anchor is allowed, but all
        // subsequent knots must increase strictly.
        for &(t, lambda) in &self.points {
            if !t.is_finite() || t < 0.0 {
                return Err(InputError::Invalid.into());
            }
            if lambda < 0.0 {
                return Err(InputError::NegativeValue.into());
            }
            if !lambda.is_finite() {
                return Err(InputError::Invalid.into());
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

        // Validate recovery rate bounds
        super::common::validate_unit_range(self.recovery_rate, "recovery_rate")?;

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

        // Convert hazard rates to survival probabilities for interpolation
        // S(t_i) = exp(- sum(lambda_j * dt_j))
        // We anchor at (0.0, 1.0) ensuring we always have at least 2 points (if N >= 1)
        // and that S(0) = 1.
        let mut interp_kvec = Vec::with_capacity(kvec.len() + 1);
        let mut interp_svec = Vec::with_capacity(kvec.len() + 1);

        interp_kvec.push(0.0);
        interp_svec.push(1.0);

        let mut accum = 0.0;
        let mut prev_t = 0.0;
        let mut has_zero_anchor = false;
        let mut prev_lambda = None;

        for (&t, &lambda) in kvec.iter().zip(lvec.iter()) {
            if t <= 1e-9 {
                has_zero_anchor = true;
                prev_lambda = Some(lambda);
                continue;
            }
            let segment_lambda = if has_zero_anchor {
                prev_lambda.unwrap_or(lambda)
            } else {
                lambda
            };
            accum += segment_lambda * (t - prev_t);
            interp_kvec.push(t);
            interp_svec.push((-accum).exp());
            prev_t = t;
            prev_lambda = Some(lambda);
        }

        // Build interpolator: LogLinear style implies constant hazard rate
        // Extrapolate with FlatForward (constant hazard rate at tail)
        let interp = super::common::build_interp(
            InterpStyle::LogLinear,
            interp_kvec.into_boxed_slice(),
            interp_svec.into_boxed_slice(),
            ExtrapolationPolicy::FlatForward,
        )?;

        Ok(HazardCurve {
            id: self.id,
            base: self.base,
            knots: kvec.into_boxed_slice(),
            lambdas: lvec.into_boxed_slice(),
            recovery_rate: self.recovery_rate,
            issuer: self.issuer,
            seniority: self.seniority,
            currency: self.currency,
            day_count: self.day_count,
            par_tenors: p_ten.into_boxed_slice(),
            par_spreads_bp: p_spd.into_boxed_slice(),
            par_interp: self.par_interp,
            interp,
        })
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;
    #[test]
    fn survival_monotone_decreasing() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let hc = HazardCurve::builder("USD-CREDIT")
            .base_date(base)
            .knots([(1.0, 0.01), (5.0, 0.02)])
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
            .build()
            .expect("HazardCurve builder should succeed with valid test data");
        let dp = hc
            .default_prob(2.0, 4.0)
            .expect("default_prob should succeed with valid inputs");
        assert!(dp >= 0.0);
    }

    #[test]
    fn quoted_spread_interpolation_linear() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let hc = HazardCurve::builder("TEST")
            .base_date(base)
            .knots([(1.0, 0.02)])
            .par_spreads([(1.0, 100.0), (3.0, 200.0)])
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
            .build()
            .expect("Builder works");

        // Roll forward 182.5 days (0.5 years)
        // 0.5 year point should expire (become 0.0) -> filtered out?
        // Wait, roll_knots filters if t <= 0.0.
        // 0.5 - 0.5 = 0.0. So it should be filtered out.
        // Resulting curve needs at least 1 point (builder requires it).
        // Actually builder requires "At least one knot point" (line 622).
        // roll_forward returns error if < 2 points?
        // Let's check roll_forward implementation again.
        // "if rolled_points.len() < 2 { return Err(...) }"
        // So we need enough points surviving.

        let hc = HazardCurve::builder("TEST-ROLL")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.5, 0.01), (1.5, 0.02), (2.5, 0.03)])
            .build()
            .expect("Builder works");

        let rolled = hc.roll_forward(183).expect("Roll should succeed"); // > 0.5 years

        // Base date should be shifted
        assert_eq!(rolled.base_date(), base + time::Duration::days(183));

        // Knots should be shifted
        let knots: Vec<f64> = rolled.knot_points().map(|(t, _)| t).collect();
        assert_eq!(knots.len(), 2);
        // 1.5 - (183/365) = 1.5 - 0.50137 = 0.9986
        // 2.5 - (183/365) = 1.9986
        assert!(knots[0] < 1.0 && knots[0] > 0.99);
    }

    #[test]
    fn builder_allows_explicit_zero_time_knot() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let result = HazardCurve::builder("USD-CREDIT")
            .base_date(base)
            .knots([(0.0, 0.01), (5.0, 0.02)])
            .build();

        assert!(result.is_ok(), "t=0 hazard knots should be accepted");
    }

    #[test]
    fn builder_rejects_empty_knots() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let result = HazardCurve::builder("USD-CREDIT").base_date(base).build();
        assert!(
            result.is_err(),
            "builder should enforce the non-empty lambda invariant"
        );
    }

    #[test]
    fn hazard_rate_is_available_for_valid_built_curves() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let hc = HazardCurve::builder("USD-CREDIT")
            .base_date(base)
            .knots([(0.0, 0.01), (5.0, 0.02)])
            .build()
            .expect("HazardCurve builder should succeed with valid test data");

        assert_eq!(hc.hazard_rate(-1.0), 0.01);
        assert_eq!(hc.hazard_rate(0.0), 0.01);
        assert_eq!(hc.hazard_rate(10.0), 0.02);
    }

    #[test]
    fn parallel_bump_rejects_negative_shift_that_crosses_zero_hazard() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let curve = HazardCurve::builder("HY")
            .base_date(base)
            .knots([(1.0, 0.001), (5.0, 0.002)])
            .build()
            .expect("valid hazard curve");

        let err = curve
            .with_parallel_bump(-0.0015)
            .expect_err("negative shifted hazard rate must be rejected");

        assert!(
            err.to_string().contains("negative hazard rate after bump"),
            "unexpected error: {err}"
        );
    }
}

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

/// Seniority level for credit exposures.
///
/// Used to tag a hazard curve with the seniority of the issuer's debt
/// observed by the curve. Drives the recovery-rate prior in default
/// modelling and selects the right LGD prior in
/// [`crate::credit::lgd::seniority`].
///
/// Order is **not** total — `SeniorSecured` is strictly senior to `Senior`,
/// `Subordinated`, and `Junior`, but the relative ordering of `Subordinated`
/// vs. `Junior` is jurisdiction-dependent. Do not rely on `Ord` semantics.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
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

impl crate::parse::NormalizedEnum for Seniority {
    const VARIANTS: &'static [(&'static str, Self)] = &[
        ("senior_secured", Self::SeniorSecured),
        ("senior", Self::Senior),
        ("subordinated", Self::Subordinated),
        ("sub", Self::Subordinated),
        ("junior", Self::Junior),
    ];
}

impl core::str::FromStr for Seniority {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        crate::parse::parse_normalized_enum(s).map_err(|_| crate::error::InputError::Invalid.into())
    }
}

/// Interpolation method for reporting par spreads stored on the curve.
///
/// Applies only to *par-spread* readouts (the spreads quoted at calibration
/// pillars), not to the underlying hazard rates. Hazard interpolation always
/// follows piecewise-constant survival. Use `LogLinear` when spreads span
/// multiple decades (e.g. high-yield issuers) so interpolation stays in
/// log-space; otherwise the default `Linear` is fine.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
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

    fn assert_parses_to(label: &str, expected: Seniority) {
        assert!(matches!(label.parse::<Seniority>(), Ok(value) if value == expected));
    }

    #[test]
    fn test_seniority_fromstr_display_roundtrip() {
        for (input, expected) in [
            ("senior_secured", Seniority::SeniorSecured),
            ("senior", Seniority::Senior),
            ("subordinated", Seniority::Subordinated),
            ("sub", Seniority::Subordinated),
            ("junior", Seniority::Junior),
        ] {
            assert_parses_to(input, expected);
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
        assert!("unknown".parse::<Seniority>().is_err());
    }
}
