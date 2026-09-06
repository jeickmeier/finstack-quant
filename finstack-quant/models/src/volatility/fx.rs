//! FX delta-based volatility surface builder.
//!
//! FX options markets quote volatility in delta space rather than strike space.
//! The standard market quotes are:
//!
//! - **ATM DNS** (delta-neutral straddle): The at-the-money volatility where
//!   the delta of a straddle is zero.
//! - **25-delta risk reversal (RR)**: The difference between 25-delta call vol
//!   and 25-delta put vol, capturing the skew.
//! - **25-delta butterfly (BF)**: The average of 25-delta call and put vols
//!   minus the ATM vol, capturing the smile curvature.
//!
//! This module converts those quotes to a standard strike-based [`VolSurface`]
//! using the Garman-Kohlhagen framework for FX options.
//!
//! # Delta Convention: Premium-Unadjusted Forward Delta
//!
//! All delta-to-strike conversions here use the **premium-unadjusted forward
//! delta**:
//!
//! ```text
//! Delta_fwd_call = N(d1)
//! ```
//!
//! where `d1 = [ln(F/K) + 0.5 * sigma^2 * T] / (sigma * sqrt(T))` and
//! `F = S * exp((r_d - r_f) * T)` is the forward rate. The spot delta
//! `exp(-r_f * T) * N(d1)` and premium-adjusted deltas are **not** used.
//! This matches interbank convention for long-dated G10 pairs; markets that
//! quote premium-adjusted spot delta (notably EM pairs such as USD/TRY,
//! USD/BRL, and short-dated G10 spot-delta quotes) will see strike placement
//! biased relative to their convention — convert quotes to forward delta
//! before using this builder.
//!
//! Inverting the forward delta gives the strike:
//!
//! ```text
//! K(Delta) = F * exp(-N_inv(Delta) * sigma * sqrt(T) + 0.5 * sigma^2 * T)
//! ```
//!
//! For the ATM DNS (delta-neutral straddle) strike:
//!
//! ```text
//! K_ATM = F * exp(0.5 * sigma^2 * T)
//! ```
//!
//! # Quote and Interpolation Conventions
//!
//! - **Butterfly quotes are treated as smile (broker) strangles**:
//!   `sigma_wing = ATM + BF ± 0.5 * RR` exactly. No market-strangle
//!   (single-vol strangle) consistency solve is performed; if your BF quotes
//!   are market strangles the wings will be slightly misplaced for skewed
//!   smiles.
//! - **Expiry interpolation is linear in vol** on the resulting
//!   [`VolSurface`] grid, not linear in total variance, so calendar
//!   interpolation between pillars is only approximate.
//! - **Per-expiry smiles**:
//!   Each expiry's smile is built independently from its own
//!   3/5 pillar strikes (via the crate-internal `fx_smile_pillars` helper),
//!   derived from *that expiry's* forward and vol scale.
//!   The rectangular [`VolSurface`] materialization samples each expiry's own
//!   smile on the union of all pillar strikes; because every expiry's knots
//!   are grid points, queries at pillar expiries reproduce the per-expiry
//!   smile exactly (piecewise-linear interpolation is invariant under grid
//!   refinement). Beyond an expiry's own quoted wings the smile is flat.
//!   Models-layer evaluators can instead work directly from the delta-space
//!   artifact when smile-faithful off-pillar evaluation is required.
//!
//! # References
//!
//! - Wystup, U. (2006). *FX Options and Structured Products*. Wiley.
//!   Chapter 1 (FX volatility surface conventions). `docs/REFERENCES.md#wystup-fx-options`
//! - Clark, I. J. (2011). *Foreign Exchange Option Pricing: A Practitioner's Guide*.
//!   Wiley. Chapters 3-4 (Delta conventions and smile construction). `docs/REFERENCES.md#clark-fx-options`
//! - Garman, M. B., & Kohlhagen, S. W. (1983). "Foreign Currency Option Values."
//!   `docs/REFERENCES.md#garman-kohlhagen-1983`

use finstack_quant_core::{error::InputError, types::CurveId};

use finstack_quant_core::market_data::surfaces::{FxDeltaVolSurface, VolSurface};
use finstack_quant_core::math::interp::{
    ExtrapolationPolicy, InterpFn, Interpolator, LinearStrategy, ValidationPolicy,
};
use finstack_quant_core::Result;

/// Builder that converts FX delta-quoted vols to a standard strike-based [`VolSurface`].
///
/// FX markets quote volatility in delta space (ATM DNS, 25-delta RR, 25-delta BF),
/// not strike space. This builder converts those quotes to strikes and
/// builds a standard [`VolSurface`] for the pricing engine.
///
/// # Quote Conventions
///
/// From the market quotes, individual wing volatilities are recovered as:
///
/// ```text
/// sigma_25d_call = ATM + BF + 0.5 * RR
/// sigma_25d_put  = ATM + BF - 0.5 * RR
/// ```
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::volatility::FxVolSurfaceBuilder;
///
/// let surface = FxVolSurfaceBuilder::new("EURUSD-VOL")
///     .spot(1.10)
///     .domestic_rate(0.05)
///     .foreign_rate(0.04)
///     .expiries(&[0.25, 0.5, 1.0])
///     .atm_vols(&[0.08, 0.085, 0.09])
///     .rr_25d(&[0.01, 0.012, 0.015])
///     .bf_25d(&[0.005, 0.006, 0.007])
///     .build()
///     .expect("FX delta vol surface should build");
///
/// assert_eq!(surface.grid_shape().0, 3);
/// assert!(surface.vols().iter().all(|vol| *vol > 0.0));
/// ```
pub struct FxVolSurfaceBuilder {
    id: CurveId,
    spot: f64,
    domestic_rate: f64,
    foreign_rate: f64,
    expiries: Vec<f64>,
    atm_vols: Vec<f64>,
    rr_25d: Option<Vec<f64>>,
    bf_25d: Option<Vec<f64>>,
    rr_10d: Option<Vec<f64>>,
    bf_10d: Option<Vec<f64>>,
}

impl FxVolSurfaceBuilder {
    /// Create a new builder with the given surface identifier.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the resulting [`VolSurface`]
    pub fn new(id: impl Into<CurveId>) -> Self {
        Self {
            id: id.into(),
            spot: 0.0,
            domestic_rate: 0.0,
            foreign_rate: 0.0,
            expiries: Vec::new(),
            atm_vols: Vec::new(),
            rr_25d: None,
            bf_25d: None,
            rr_10d: None,
            bf_10d: None,
        }
    }

    /// Set the FX spot rate (e.g., 1.10 for EUR/USD).
    ///
    /// # Arguments
    ///
    /// * `spot` - Positive finite FX spot in domestic-currency units per one foreign unit; copied into the builder and validated by build.
    pub fn spot(mut self, spot: f64) -> Self {
        self.spot = spot;
        self
    }

    /// Set the domestic (numerator currency) continuously compounded interest rate.
    ///
    /// # Arguments
    ///
    /// * `rate` - Finite continuously compounded annual domestic interest rate as a decimal, for example 0.05 for 5 percent; negative rates are allowed.
    pub fn domestic_rate(mut self, rate: f64) -> Self {
        self.domestic_rate = rate;
        self
    }

    /// Set the foreign (denominator currency) continuously compounded interest rate.
    ///
    /// # Arguments
    ///
    /// * `rate` - Finite continuously compounded annual foreign interest rate as a decimal; used with the domestic rate to compute forwards.
    pub fn foreign_rate(mut self, rate: f64) -> Self {
        self.foreign_rate = rate;
        self
    }

    /// Set the expiry times in years.
    ///
    /// Must be strictly increasing and match the length of `atm_vols`.
    ///
    /// # Arguments
    ///
    /// * `expiries` - Nonempty, strictly increasing positive finite year fractions; copied into the builder and matched one-for-one to every quote array.
    pub fn expiries(mut self, expiries: &[f64]) -> Self {
        self.expiries = expiries.to_vec();
        self
    }

    /// Set the ATM delta-neutral straddle volatilities per expiry.
    ///
    /// Must have the same length as `expiries`.
    ///
    /// # Arguments
    ///
    /// * `vols` - Positive finite annualized Black volatilities as decimals, one per expiry, under the premium-unadjusted forward-delta ATM DNS convention.
    pub fn atm_vols(mut self, vols: &[f64]) -> Self {
        self.atm_vols = vols.to_vec();
        self
    }

    /// Set the 25-delta risk reversal quotes per expiry.
    ///
    /// `RR = sigma_25d_call - sigma_25d_put`
    ///
    /// Must have the same length as `expiries`. If not provided, only ATM
    /// strikes are generated (single-strike surface).
    ///
    /// # Arguments
    ///
    /// * `rr` - Finite call-minus-put 25-delta volatility differences as decimals, one per expiry; supply bf_25d together with these quotes.
    pub fn rr_25d(mut self, rr: &[f64]) -> Self {
        self.rr_25d = Some(rr.to_vec());
        self
    }

    /// Set the 25-delta butterfly quotes per expiry.
    ///
    /// `BF = 0.5 * (sigma_25d_call + sigma_25d_put) - sigma_ATM`
    ///
    /// Must have the same length as `expiries`. If not provided, only ATM
    /// strikes are generated (single-strike surface).
    ///
    /// # Arguments
    ///
    /// * `bf` - Finite 25-delta smile-strangle butterfly quotes as decimal volatility, one per expiry; paired with rr_25d. These are not market-strangle quotes.
    pub fn bf_25d(mut self, bf: &[f64]) -> Self {
        self.bf_25d = Some(bf.to_vec());
        self
    }

    /// Set the 10-delta risk reversal quotes per expiry.
    ///
    /// # Arguments
    ///
    /// * `rr` - Finite call-minus-put 10-delta volatility differences as decimals, one per expiry; paired with bf_10d to add outer wings to the 25-delta smile.
    pub fn rr_10d(mut self, rr: &[f64]) -> Self {
        self.rr_10d = Some(rr.to_vec());
        self
    }

    /// Set the 10-delta butterfly quotes per expiry.
    ///
    /// # Arguments
    ///
    /// * `bf` - Finite 10-delta smile-strangle butterfly quotes as decimal volatility, one per expiry; paired with rr_10d to add outer wings.
    pub fn bf_10d(mut self, bf: &[f64]) -> Self {
        self.bf_10d = Some(bf.to_vec());
        self
    }

    /// Build the strike-based [`VolSurface`] from the delta-space quotes.
    ///
    /// # Conversion Steps
    ///
    /// 1. Recover individual wing vols from RR/BF quotes
    /// 2. Compute the forward rate `F = S * exp((r_d - r_f) * T)` per expiry
    /// 3. Convert delta to strike using Garman-Kohlhagen inversion
    /// 4. Assemble the strike-vol grid into a standard [`VolSurface`]
    ///
    /// # Errors
    ///
    /// Returns an error if the spot is non-positive or non-finite; expiries or
    /// ATM vols are empty; array lengths disagree; any expiry is non-positive
    /// or non-finite; expiries are not strictly increasing; any ATM vol is
    /// non-positive or non-finite; RR/BF quotes are unpaired or non-finite; or
    /// a recovered wing volatility is non-positive.
    pub fn build(self) -> Result<VolSurface> {
        if !self.spot.is_finite() || self.spot <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        if self.expiries.is_empty() || self.atm_vols.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }
        if self.expiries.len() != self.atm_vols.len() {
            return Err(InputError::DimensionMismatch.into());
        }

        for &t in &self.expiries {
            if !t.is_finite() || t <= 0.0 {
                return Err(InputError::NonPositiveValue.into());
            }
        }
        for w in self.expiries.windows(2) {
            if w[1] <= w[0] {
                return Err(InputError::NonMonotonicKnots.into());
            }
        }

        for &v in &self.atm_vols {
            if !v.is_finite() || v <= 0.0 {
                return Err(InputError::NonPositiveValue.into());
            }
        }

        if self.rr_25d.is_some() != self.bf_25d.is_some()
            || self.rr_10d.is_some() != self.bf_10d.is_some()
        {
            return Err(InputError::Invalid.into());
        }
        let has_wings = self.rr_25d.is_some();

        if let Some(ref rr) = self.rr_25d {
            if rr.len() != self.expiries.len() {
                return Err(InputError::DimensionMismatch.into());
            }
            for &v in rr {
                if !v.is_finite() {
                    return Err(InputError::Invalid.into());
                }
            }
        }
        if let Some(ref bf) = self.bf_25d {
            if bf.len() != self.expiries.len() {
                return Err(InputError::DimensionMismatch.into());
            }
            for &v in bf {
                if !v.is_finite() {
                    return Err(InputError::Invalid.into());
                }
            }
        }
        if let Some(ref rr) = self.rr_10d {
            if rr.len() != self.expiries.len() {
                return Err(InputError::DimensionMismatch.into());
            }
            for &v in rr {
                if !v.is_finite() {
                    return Err(InputError::Invalid.into());
                }
            }
        }
        if let Some(ref bf) = self.bf_10d {
            if bf.len() != self.expiries.len() {
                return Err(InputError::DimensionMismatch.into());
            }
            for &v in bf {
                if !v.is_finite() {
                    return Err(InputError::Invalid.into());
                }
            }
        }

        let n_expiries = self.expiries.len();
        let has_10d_wings = self.rr_10d.is_some() && self.bf_10d.is_some();

        if has_wings {
            let (Some(rr), Some(bf)) = (self.rr_25d.as_ref(), self.bf_25d.as_ref()) else {
                return Err(InputError::Invalid.into());
            };
            let wings_10d = match (self.rr_10d.as_ref(), self.bf_10d.as_ref()) {
                (Some(rr10), Some(bf10)) => Some((rr10, bf10)),
                _ => None,
            };

            let mut all_strikes: Vec<f64> =
                Vec::with_capacity(if has_10d_wings { 5 } else { 3 } * n_expiries);
            let mut per_expiry_smiles: Vec<(Vec<f64>, Vec<f64>)> = Vec::with_capacity(n_expiries);

            for i in 0..n_expiries {
                let t = self.expiries[i];
                let fwd = fx_forward(self.spot, self.domestic_rate, self.foreign_rate, t);
                let (known_strikes, known_vols) = fx_smile_pillars(
                    fwd,
                    t,
                    self.atm_vols[i],
                    rr[i],
                    bf[i],
                    wings_10d.map(|(rr10, bf10)| (rr10[i], bf10[i])),
                )?;
                all_strikes.extend(known_strikes.iter().copied());
                per_expiry_smiles.push((known_strikes, known_vols));
            }

            // Strike axis: union of all expiries' pillar strikes (sorted,
            // deduplicated). Every expiry's own knots are grid points, so
            // sampling the per-expiry smiles on this axis is lossless at the
            // pillar expiries: piecewise-linear interpolation is invariant
            // under grid refinement.
            all_strikes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
            all_strikes.dedup_by(|a, b| (*a - *b).abs() < 1e-10);

            let strikes = &all_strikes;

            let mut builder = VolSurface::builder(self.id)
                .expiries(&self.expiries)
                .strikes(strikes);

            // Each row samples that expiry's OWN smile only: linear within
            // its own 3/5 pillars, flat beyond its own quoted wings. Strikes
            // contributed by other expiries never alter the smile shape.
            for (known_strikes, known_vols) in per_expiry_smiles {
                let smile = Interpolator::<LinearStrategy>::new(
                    known_strikes.into_boxed_slice(),
                    known_vols.into_boxed_slice(),
                    ExtrapolationPolicy::FlatZero,
                    ValidationPolicy::AllowNegative,
                )?;
                let row: Vec<f64> = strikes.iter().map(|&k| smile.interp(k)).collect();
                builder = builder.row(&row);
            }

            builder.build()
        } else {
            let mut all_strikes: Vec<f64> = Vec::with_capacity(n_expiries);

            for i in 0..n_expiries {
                let t = self.expiries[i];
                let atm = self.atm_vols[i];
                let fwd = fx_forward(self.spot, self.domestic_rate, self.foreign_rate, t);
                let k_atm = fx_atm_dns_strike(fwd, atm, t);
                all_strikes.push(k_atm);
            }

            all_strikes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
            all_strikes.dedup_by(|a, b| (*a - *b).abs() < 1e-10);

            let n_strikes = all_strikes.len();

            let mut builder = VolSurface::builder(self.id)
                .expiries(&self.expiries)
                .strikes(&all_strikes);

            for &atm in &self.atm_vols {
                builder = builder.row(&vec![atm; n_strikes]);
            }

            builder.build()
        }
    }
}

/// Recover 25d/10d wing vols from ATM/RR/BF quotes, treating BF as a
/// **smile (broker) strangle**: `sigma_wing = ATM + BF ± RR/2` exactly.
/// No market-strangle consistency solve is performed.
#[inline]
fn recover_fx_wing_vols(atm: f64, rr: f64, bf: f64) -> (f64, f64) {
    let sigma_call = atm + bf + 0.5 * rr;
    let sigma_put = atm + bf - 0.5 * rr;
    (sigma_put, sigma_call)
}

/// Garman-Kohlhagen FX forward `F = S * exp((r_d - r_f) * T)` with
/// continuously compounded rates.
#[inline]
fn fx_forward(spot: f64, domestic_rate: f64, foreign_rate: f64, expiry: f64) -> f64 {
    spot * ((domestic_rate - foreign_rate) * expiry).exp()
}

/// Delta-neutral-straddle ATM strike `K = F * exp(sigma^2 T / 2)` under the
/// premium-unadjusted **forward delta** convention.
#[inline]
fn fx_atm_dns_strike(forward: f64, vol: f64, expiry: f64) -> f64 {
    forward * (0.5 * vol * vol * expiry).exp()
}

/// Strikes for put/call at absolute delta `delta_abs` using the
/// premium-unadjusted **forward delta** convention (`Delta_call = N(d1)`),
/// i.e. `K = F * exp(∓ N⁻¹(Δ) σ √T + σ² T / 2)`. Spot-delta and
/// premium-adjusted conventions are intentionally not supported here.
#[inline]
fn fx_put_call_delta_strikes(
    forward: f64,
    sigma_put: f64,
    sigma_call: f64,
    expiry: f64,
    delta_abs: f64,
) -> (f64, f64) {
    (
        delta_to_strike(1.0 - delta_abs, forward, sigma_put, expiry),
        delta_to_strike(delta_abs, forward, sigma_call, expiry),
    )
}

#[inline]
fn fx_put_call_25d_strikes(
    forward: f64,
    sigma_put: f64,
    sigma_call: f64,
    expiry: f64,
) -> (f64, f64) {
    fx_put_call_delta_strikes(forward, sigma_put, sigma_call, expiry, 0.25)
}

/// Per-expiry FX smile pillars: the (strikes, vols) of one expiry's own
/// 3-point (25Δ put, ATM DNS, 25Δ call) or 5-point (plus 10Δ wings) smile.
///
/// Both direct FX smile evaluation and [`FxVolSurfaceBuilder`] use this
/// canonical per-expiry representation when evaluating or materializing a
/// rectangular data artifact. Each expiry's strikes are derived from *that
/// expiry's* forward and vol scale; no strikes from other expiries are involved.
///
/// `wings_10d` carries `(rr_10d, bf_10d)` when 10-delta quotes are available.
///
/// # Errors
///
/// Returns [`InputError::NegativeValue`](finstack_quant_core::error::InputError) if any
/// recovered wing vol is non-positive.
fn fx_smile_pillars(
    forward: f64,
    expiry: f64,
    atm: f64,
    rr_25d: f64,
    bf_25d: f64,
    wings_10d: Option<(f64, f64)>,
) -> Result<(Vec<f64>, Vec<f64>)> {
    let (sigma_put, sigma_call) = recover_fx_wing_vols(atm, rr_25d, bf_25d);
    if sigma_call <= 0.0 || sigma_put <= 0.0 {
        return Err(finstack_quant_core::error::InputError::NegativeValue.into());
    }

    let k_atm = fx_atm_dns_strike(forward, atm, expiry);
    let (k_put, k_call) = fx_put_call_25d_strikes(forward, sigma_put, sigma_call, expiry);

    if let Some((rr_10d, bf_10d)) = wings_10d {
        let (sigma_put_10d, sigma_call_10d) = recover_fx_wing_vols(atm, rr_10d, bf_10d);
        if sigma_call_10d <= 0.0 || sigma_put_10d <= 0.0 {
            return Err(finstack_quant_core::error::InputError::NegativeValue.into());
        }
        let (k_put_10d, k_call_10d) =
            fx_put_call_delta_strikes(forward, sigma_put_10d, sigma_call_10d, expiry, 0.10);
        Ok((
            vec![k_put_10d, k_put, k_atm, k_call, k_call_10d],
            vec![sigma_put_10d, sigma_put, atm, sigma_call, sigma_call_10d],
        ))
    } else {
        Ok((vec![k_put, k_atm, k_call], vec![sigma_put, atm, sigma_call]))
    }
}

/// Convert premium-unadjusted forward delta to strike.
///
/// # Arguments
///
/// * `delta` - Absolute call delta on the open unit interval.
/// * `forward` - Positive FX forward in domestic-currency units per foreign unit.
/// * `vol` - Annualized Black volatility as a decimal.
/// * `expiry` - Time to expiry in years.
pub fn delta_to_strike(delta: f64, forward: f64, vol: f64, expiry: f64) -> f64 {
    let z = finstack_quant_core::math::standard_normal_inv_cdf(delta);
    forward * (-z * vol * expiry.sqrt() + 0.5 * vol * vol * expiry).exp()
}

/// Convert strike to premium-unadjusted forward call delta.
///
/// # Arguments
///
/// * `strike` - Positive strike in domestic-currency units per foreign unit.
/// * `forward` - Positive FX forward in the same units as `strike`.
/// * `vol` - Annualized Black volatility as a decimal.
/// * `expiry` - Time to expiry in years.
pub fn strike_to_delta(strike: f64, forward: f64, vol: f64, expiry: f64) -> f64 {
    finstack_quant_core::math::norm_cdf(super::black::d1_black76(forward, strike, vol, expiry))
}

/// Evaluate an FX delta-quoted volatility artifact.
///
/// # Arguments
///
/// * `surface` - Structurally validated ATM, risk-reversal, and butterfly quotes.
/// * `expiry` - Positive option expiry in years; wings are flat outside the axis.
/// * `strike` - Positive strike in domestic-currency units per foreign unit.
/// * `forward` - Positive FX forward in the same units as `strike`.
///
/// # Errors
///
/// Returns an input error for non-positive or non-finite coordinates, or for
/// quotes that imply a non-positive wing volatility.
pub fn get_fx_delta_vol(
    surface: &FxDeltaVolSurface,
    expiry: f64,
    strike: f64,
    forward: f64,
) -> Result<f64> {
    if expiry <= 0.0
        || !expiry.is_finite()
        || strike <= 0.0
        || !strike.is_finite()
        || forward <= 0.0
        || !forward.is_finite()
    {
        return Err(InputError::NonPositiveValue.into());
    }
    let atm = linear_clamped(surface.expiries(), surface.atm_vols(), expiry);
    let rr25 = linear_clamped(surface.expiries(), surface.rr_25d(), expiry);
    let bf25 = linear_clamped(surface.expiries(), surface.bf_25d(), expiry);
    let wings_10d = surface.rr_10d().zip(surface.bf_10d()).map(|(rr, bf)| {
        (
            linear_clamped(surface.expiries(), rr, expiry),
            linear_clamped(surface.expiries(), bf, expiry),
        )
    });
    let (strikes, vols) = fx_smile_pillars(forward, expiry, atm, rr25, bf25, wings_10d)?;
    Ok(linear_clamped(&strikes, &vols, strike))
}

/// Return ATM, 25-delta put, and 25-delta call volatilities at a stored expiry.
///
/// # Arguments
///
/// * `surface` - Structurally validated FX delta-volatility quotes.
/// * `expiry_index` - Zero-based index into the stored expiry axis.
///
/// # Errors
///
/// Returns an input error when `expiry_index` is outside the stored axis.
pub fn get_fx_delta_pillar_vols(
    surface: &FxDeltaVolSurface,
    expiry_index: usize,
) -> Result<(f64, f64, f64)> {
    let atm = *surface
        .atm_vols()
        .get(expiry_index)
        .ok_or(InputError::Invalid)?;
    let rr = surface.rr_25d()[expiry_index];
    let bf = surface.bf_25d()[expiry_index];
    let (put, call) = recover_fx_wing_vols(atm, rr, bf);
    Ok((atm, put, call))
}

/// Materialize an FX delta-quoted artifact on a rectangular strike grid.
///
/// # Arguments
///
/// * `surface` - Structurally validated FX delta-volatility quotes.
/// * `spot` - Positive FX spot in domestic-currency units per foreign unit.
/// * `domestic_rate` - Continuously compounded domestic rate as an annual decimal.
/// * `foreign_rate` - Continuously compounded foreign rate as an annual decimal.
///
/// # Errors
///
/// Returns an input or structural-validation error when the market inputs or
/// recovered volatility grid are invalid.
pub fn materialize_fx_delta_surface(
    surface: &FxDeltaVolSurface,
    spot: f64,
    domestic_rate: f64,
    foreign_rate: f64,
) -> Result<VolSurface> {
    let mut builder = FxVolSurfaceBuilder::new(surface.id().clone())
        .spot(spot)
        .domestic_rate(domestic_rate)
        .foreign_rate(foreign_rate)
        .expiries(surface.expiries())
        .atm_vols(surface.atm_vols())
        .rr_25d(surface.rr_25d())
        .bf_25d(surface.bf_25d());
    if let Some(rr) = surface.rr_10d() {
        builder = builder.rr_10d(rr);
    }
    if let Some(bf) = surface.bf_10d() {
        builder = builder.bf_10d(bf);
    }
    builder.build()
}

#[inline]
fn linear_clamped(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[xs.len() - 1] {
        return ys[ys.len() - 1];
    }
    let upper = xs.partition_point(|node| *node < x);
    let weight = (x - xs[upper - 1]) / (xs[upper] - xs[upper - 1]);
    ys[upper - 1] + weight * (ys[upper] - ys[upper - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_uses_five_point_smile_when_10d_quotes_are_available() {
        let surface = FxVolSurfaceBuilder::new("EURUSD-VOL")
            .spot(1.10)
            .domestic_rate(0.05)
            .foreign_rate(0.04)
            .expiries(&[0.5, 1.0])
            .atm_vols(&[0.08, 0.09])
            .rr_25d(&[0.01, 0.012])
            .bf_25d(&[0.005, 0.006])
            .rr_10d(&[0.015, 0.018])
            .bf_10d(&[0.008, 0.009])
            .build()
            .expect("surface should build with 10d and 25d wings");

        assert!(
            surface.strikes().len() >= 5,
            "10d wings should add extra smile points"
        );
    }

    #[test]
    fn builder_rejects_unpaired_rr_and_bf_quotes() {
        let base = || {
            FxVolSurfaceBuilder::new("EURUSD-VOL")
                .spot(1.10)
                .expiries(&[1.0])
                .atm_vols(&[0.09])
        };
        assert!(base().rr_25d(&[0.01]).build().is_err());
        assert!(base().bf_25d(&[0.005]).build().is_err());
        assert!(base()
            .rr_25d(&[0.01])
            .bf_25d(&[0.005])
            .rr_10d(&[0.02])
            .build()
            .is_err());
        assert!(base()
            .rr_25d(&[0.01])
            .bf_25d(&[0.005])
            .bf_10d(&[0.01])
            .build()
            .is_err());
    }
}
