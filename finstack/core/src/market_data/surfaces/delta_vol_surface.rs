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
//! - **Merged strike grid limitation**: strikes from all expiries are merged
//!   into one global grid and each expiry row is flat-extrapolated beyond its
//!   own quoted wings. Short-expiry smiles therefore appear flattened at
//!   strikes that are only relevant for long expiries. A per-expiry smile
//!   representation is deferred.
//!
//! # References
//!
//! - Wystup, U. (2006). *FX Options and Structured Products*. Wiley.
//!   Chapter 1 (FX volatility surface conventions).
//! - Clark, I. J. (2011). *Foreign Exchange Option Pricing: A Practitioner's Guide*.
//!   Wiley. Chapters 3-4 (Delta conventions and smile construction).
//! - Castagna, A. (2010). *FX Options and Smile Risk*. Wiley.

use crate::{error::InputError, types::CurveId};

use super::{
    fx_atm_dns_strike, fx_forward, fx_put_call_25d_strikes, fx_put_call_delta_strikes,
    interp_linear_clamp, recover_fx_wing_vols, VolSurface,
};

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
/// use finstack_core::market_data::surfaces::FxDeltaVolSurfaceBuilder;
///
/// let surface = FxDeltaVolSurfaceBuilder::new("EURUSD-VOL")
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
/// // Surface builds and can interpolate vol at expiry/strike
/// assert!(surface.value_clamped(0.5, 1.10) > 0.0);
/// ```
pub struct FxDeltaVolSurfaceBuilder {
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

impl FxDeltaVolSurfaceBuilder {
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
    pub fn spot(mut self, spot: f64) -> Self {
        self.spot = spot;
        self
    }

    /// Set the domestic (numerator currency) continuously compounded interest rate.
    pub fn domestic_rate(mut self, rate: f64) -> Self {
        self.domestic_rate = rate;
        self
    }

    /// Set the foreign (denominator currency) continuously compounded interest rate.
    pub fn foreign_rate(mut self, rate: f64) -> Self {
        self.foreign_rate = rate;
        self
    }

    /// Set the expiry times in years.
    ///
    /// Must be strictly increasing and match the length of `atm_vols`.
    pub fn expiries(mut self, expiries: &[f64]) -> Self {
        self.expiries = expiries.to_vec();
        self
    }

    /// Set the ATM delta-neutral straddle volatilities per expiry.
    ///
    /// Must have the same length as `expiries`.
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
    pub fn bf_25d(mut self, bf: &[f64]) -> Self {
        self.bf_25d = Some(bf.to_vec());
        self
    }

    /// Set the 10-delta risk reversal quotes per expiry.
    pub fn rr_10d(mut self, rr: &[f64]) -> Self {
        self.rr_10d = Some(rr.to_vec());
        self
    }

    /// Set the 10-delta butterfly quotes per expiry.
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
    /// Returns an error if:
    /// - Spot rate is not positive
    /// - Expiries or ATM vols are empty
    /// - Array lengths are inconsistent
    /// - Any volatility is non-positive or non-finite
    /// - Any expiry is non-positive
    pub fn build(self) -> crate::Result<VolSurface> {
        // ── Validate inputs ─────────────────────────────────────────────
        if !self.spot.is_finite() || self.spot <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        if self.expiries.is_empty() || self.atm_vols.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }
        if self.expiries.len() != self.atm_vols.len() {
            return Err(InputError::DimensionMismatch.into());
        }

        // Validate expiries are positive and finite
        for &t in &self.expiries {
            if !t.is_finite() || t <= 0.0 {
                return Err(InputError::NonPositiveValue.into());
            }
        }
        // Validate expiries are strictly increasing
        for w in self.expiries.windows(2) {
            if w[1] <= w[0] {
                return Err(InputError::NonMonotonicKnots.into());
            }
        }

        // Validate ATM vols are positive and finite
        for &v in &self.atm_vols {
            if !v.is_finite() || v <= 0.0 {
                return Err(InputError::NonPositiveValue.into());
            }
        }

        let has_wings = self.rr_25d.is_some() && self.bf_25d.is_some();

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

        // ── Build the surface ───────────────────────────────────────────
        let n_expiries = self.expiries.len();
        let has_10d_wings = self.rr_10d.is_some() && self.bf_10d.is_some();

        if has_wings {
            // 3 or 5 strikes per expiry depending on whether 10d wings are available.
            let (rr, bf) = match (self.rr_25d.as_ref(), self.bf_25d.as_ref()) {
                (Some(r), Some(b)) => (r, b),
                _ => return Err(InputError::Invalid.into()),
            };
            let rr_10d = self.rr_10d.as_ref();
            let bf_10d = self.bf_10d.as_ref();

            // Compute a common strike grid across all expiries.
            // Collect all strikes, then sort and deduplicate.
            let mut all_strikes: Vec<f64> =
                Vec::with_capacity(if has_10d_wings { 5 } else { 3 } * n_expiries);
            let mut per_expiry_data: Vec<(Vec<f64>, Vec<f64>)> = Vec::with_capacity(n_expiries);

            for i in 0..n_expiries {
                let t = self.expiries[i];
                let atm = self.atm_vols[i];

                // Recover wing vols from RR/BF
                let (sigma_put, sigma_call) = recover_fx_wing_vols(atm, rr[i], bf[i]);

                // Validate wing vols
                if sigma_call <= 0.0 || sigma_put <= 0.0 {
                    return Err(InputError::NegativeValue.into());
                }

                // Forward rate
                let fwd = fx_forward(self.spot, self.domestic_rate, self.foreign_rate, t);
                let k_atm = fx_atm_dns_strike(fwd, atm, t);
                let (k_put, k_call) = fx_put_call_25d_strikes(fwd, sigma_put, sigma_call, t);
                let mut known_strikes = vec![k_put, k_atm, k_call];
                let mut known_vols = vec![sigma_put, atm, sigma_call];

                if has_10d_wings {
                    let (rr10, bf10) = match (rr_10d, bf_10d) {
                        (Some(rr10), Some(bf10)) => (rr10, bf10),
                        _ => return Err(InputError::Invalid.into()),
                    };
                    let (sigma_put_10d, sigma_call_10d) =
                        recover_fx_wing_vols(atm, rr10[i], bf10[i]);
                    if sigma_call_10d <= 0.0 || sigma_put_10d <= 0.0 {
                        return Err(InputError::NegativeValue.into());
                    }
                    let (k_put_10d, k_call_10d) =
                        fx_put_call_delta_strikes(fwd, sigma_put_10d, sigma_call_10d, t, 0.10);
                    known_strikes = vec![k_put_10d, k_put, k_atm, k_call, k_call_10d];
                    known_vols = vec![sigma_put_10d, sigma_put, atm, sigma_call, sigma_call_10d];
                }

                all_strikes.extend(known_strikes.iter().copied());
                per_expiry_data.push((known_strikes, known_vols));
            }

            // Build a sorted, deduplicated strike grid
            all_strikes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
            all_strikes.dedup_by(|a, b| (*a - *b).abs() < 1e-10);

            let strikes = &all_strikes;
            let n_strikes = strikes.len();

            // For each expiry, interpolate vols onto the common strike grid.
            // We use simple linear interpolation on the 3 known points.
            let mut builder = VolSurface::builder(self.id)
                .expiries(&self.expiries)
                .strikes(strikes);

            for (known_strikes, known_vols) in &per_expiry_data {
                let mut row = Vec::with_capacity(n_strikes);
                for &k in strikes {
                    let vol = interp_linear_clamp(known_strikes, known_vols, k);
                    row.push(vol);
                }
                builder = builder.row(&row);
            }

            builder.build()
        } else {
            // ATM-only surface: single strike per expiry
            // Compute ATM strike per expiry and build a single-strike surface
            let mut all_strikes: Vec<f64> = Vec::with_capacity(n_expiries);

            for i in 0..n_expiries {
                let t = self.expiries[i];
                let atm = self.atm_vols[i];
                let fwd = fx_forward(self.spot, self.domestic_rate, self.foreign_rate, t);
                let k_atm = fx_atm_dns_strike(fwd, atm, t);
                all_strikes.push(k_atm);
            }

            // Sort and deduplicate
            all_strikes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
            all_strikes.dedup_by(|a, b| (*a - *b).abs() < 1e-10);

            let strikes = &all_strikes;
            let n_strikes = strikes.len();

            let mut builder = VolSurface::builder(self.id)
                .expiries(&self.expiries)
                .strikes(strikes);

            for i in 0..n_expiries {
                let t = self.expiries[i];
                let atm = self.atm_vols[i];
                let fwd = fx_forward(self.spot, self.domestic_rate, self.foreign_rate, t);
                // k_atm computed for reference; flat surface uses atm vol for all strikes
                let _k_atm = fx_atm_dns_strike(fwd, atm, t);

                let mut row = Vec::with_capacity(n_strikes);
                for _ in strikes {
                    // Flat extrapolation from ATM: all strikes use the single known vol
                    row.push(atm);
                }
                builder = builder.row(&row);
            }

            builder.build()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_uses_five_point_smile_when_10d_quotes_are_available() {
        let surface = FxDeltaVolSurfaceBuilder::new("EURUSD-VOL")
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
}
