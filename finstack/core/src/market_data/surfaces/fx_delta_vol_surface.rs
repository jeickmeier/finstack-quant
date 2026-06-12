//! FX delta-based volatility surface.
//!
//! Stores FX option volatility quotes in delta space (ATM DNS, 25-delta RR/BF,
//! optional 10-delta RR/BF) and provides:
//!
//! - Direct implied-vol lookup by expiry, strike, forward, and interest rates
//! - Delta-to-strike and strike-to-delta conversion using Garman-Kohlhagen
//! - Conversion to a strike-based [`VolSurface`] for compatibility with
//!   existing pricing engines
//!
//! # Delta Convention
//!
//! This module uses **forward delta** (premium-unadjusted) throughout:
//!
//! ```text
//! Delta_call = N(d1)
//! d1 = [ln(F/K) + 0.5 * sigma^2 * T] / (sigma * sqrt(T))
//! ```
//!
//! Inverting gives:
//!
//! ```text
//! K = F * exp(-N_inv(delta) * sigma * sqrt(T) + 0.5 * sigma^2 * T)
//! ```
//!
//! For ATM DNS (delta-neutral straddle): `K_ATM = F * exp(0.5 * sigma^2 * T)`
//!
//! # References
//!
//! - Wystup (2006), *FX Options and Structured Products*, Ch. 1
//! - Clark (2011), *Foreign Exchange Option Pricing*, Ch. 3-4

use crate::{
    error::InputError, math::special_functions::norm_cdf, math::volatility::d1_black76,
    types::CurveId,
};

use super::{
    fx_smile_pillars, interp_linear_clamp, recover_fx_wing_vols, FxDeltaVolSurfaceBuilder,
    VolSurface,
};

// ---------------------------------------------------------------------------
// FxDeltaVolSurface
// ---------------------------------------------------------------------------

/// Delta-quoted FX volatility surface.
///
/// Stores market-standard FX vol quotes (ATM DNS, 25-delta risk-reversal,
/// 25-delta butterfly) across multiple expiries and provides implied-vol
/// lookup by strike via Garman-Kohlhagen delta-to-strike conversion.
///
/// # Examples
///
/// ```rust
/// use finstack_core::market_data::surfaces::FxDeltaVolSurface;
///
/// let surface = FxDeltaVolSurface::new(
///     "EURUSD-DELTA-VOL",
///     vec![0.25, 0.5, 1.0],
///     vec![0.08, 0.085, 0.09],
///     vec![0.01, 0.012, 0.015],
///     vec![0.005, 0.006, 0.007],
/// ).expect("surface should build");
///
/// // Retrieve pillar vols for the first expiry
/// let (atm, put25, call25) = surface.pillar_vols(0);
/// assert!((atm - 0.08).abs() < 1e-12);
/// ```
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawFxDeltaVolSurface")]
pub struct FxDeltaVolSurface {
    id: CurveId,
    /// Expiry times in years (strictly increasing, all positive).
    expiries: Vec<f64>,
    /// ATM delta-neutral straddle vols per expiry.
    atm_vols: Vec<f64>,
    /// 25-delta risk reversal per expiry (call vol - put vol).
    rr_25d: Vec<f64>,
    /// 25-delta butterfly per expiry (wing avg - ATM).
    bf_25d: Vec<f64>,
    /// Optional 10-delta risk reversal per expiry.
    rr_10d: Option<Vec<f64>>,
    /// Optional 10-delta butterfly per expiry.
    bf_10d: Option<Vec<f64>>,
}

/// Raw deserialization state of [`FxDeltaVolSurface`].
///
/// Mirrors the serialized field layout exactly so the wire format is
/// unchanged; conversion runs the same validation as the public
/// constructors and rejects unknown fields.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFxDeltaVolSurface {
    /// Surface identifier.
    id: CurveId,
    /// Expiry times in years.
    expiries: Vec<f64>,
    /// ATM delta-neutral straddle vols per expiry.
    atm_vols: Vec<f64>,
    /// 25-delta risk reversal per expiry.
    rr_25d: Vec<f64>,
    /// 25-delta butterfly per expiry.
    bf_25d: Vec<f64>,
    /// Optional 10-delta risk reversal per expiry.
    rr_10d: Option<Vec<f64>>,
    /// Optional 10-delta butterfly per expiry.
    bf_10d: Option<Vec<f64>>,
}

impl TryFrom<RawFxDeltaVolSurface> for FxDeltaVolSurface {
    type Error = crate::Error;

    fn try_from(raw: RawFxDeltaVolSurface) -> crate::Result<Self> {
        FxDeltaVolSurface::validate(
            &raw.expiries,
            &raw.atm_vols,
            &raw.rr_25d,
            &raw.bf_25d,
            raw.rr_10d.as_deref(),
            raw.bf_10d.as_deref(),
        )?;
        Ok(Self {
            id: raw.id,
            expiries: raw.expiries,
            atm_vols: raw.atm_vols,
            rr_25d: raw.rr_25d,
            bf_25d: raw.bf_25d,
            rr_10d: raw.rr_10d,
            bf_10d: raw.bf_10d,
        })
    }
}

impl FxDeltaVolSurface {
    /// Create a new delta-quoted surface with mandatory 25-delta wings.
    ///
    /// # Arguments
    ///
    /// * `id`       - Unique surface identifier
    /// * `expiries` - Strictly increasing positive expiry times (years)
    /// * `atm_vols` - ATM DNS volatilities per expiry (must be positive)
    /// * `rr_25d`   - 25-delta risk reversal per expiry
    /// * `bf_25d`   - 25-delta butterfly per expiry
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any vector is empty
    /// - Vector lengths are inconsistent
    /// - Expiries are non-positive, non-finite, or not strictly increasing
    /// - ATM vols are non-positive or non-finite
    /// - RR or BF values are non-finite
    pub fn new(
        id: impl Into<CurveId>,
        expiries: Vec<f64>,
        atm_vols: Vec<f64>,
        rr_25d: Vec<f64>,
        bf_25d: Vec<f64>,
    ) -> crate::Result<Self> {
        Self::validate(&expiries, &atm_vols, &rr_25d, &bf_25d, None, None)?;
        Ok(Self {
            id: id.into(),
            expiries,
            atm_vols,
            rr_25d,
            bf_25d,
            rr_10d: None,
            bf_10d: None,
        })
    }

    /// Create a surface with both 25-delta and 10-delta wings.
    pub fn with_10d(
        id: impl Into<CurveId>,
        expiries: Vec<f64>,
        atm_vols: Vec<f64>,
        rr_25d: Vec<f64>,
        bf_25d: Vec<f64>,
        rr_10d: Vec<f64>,
        bf_10d: Vec<f64>,
    ) -> crate::Result<Self> {
        Self::validate(
            &expiries,
            &atm_vols,
            &rr_25d,
            &bf_25d,
            Some(&rr_10d),
            Some(&bf_10d),
        )?;
        Ok(Self {
            id: id.into(),
            expiries,
            atm_vols,
            rr_25d,
            bf_25d,
            rr_10d: Some(rr_10d),
            bf_10d: Some(bf_10d),
        })
    }

    /// Surface identifier.
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Expiry times (years).
    #[inline]
    pub fn expiries(&self) -> &[f64] {
        &self.expiries
    }

    /// Number of expiry pillars.
    #[inline]
    pub fn num_expiries(&self) -> usize {
        self.expiries.len()
    }

    /// Pillar vols at the given expiry index: (ATM, 25D put vol, 25D call vol).
    ///
    /// # Panics
    ///
    /// Panics if `expiry_idx >= self.num_expiries()`.
    pub fn pillar_vols(&self, expiry_idx: usize) -> (f64, f64, f64) {
        let atm = self.atm_vols[expiry_idx];
        let rr = self.rr_25d[expiry_idx];
        let bf = self.bf_25d[expiry_idx];
        let (sigma_put, sigma_call) = recover_fx_wing_vols(atm, rr, bf);
        (atm, sigma_put, sigma_call)
    }

    // -----------------------------------------------------------------------
    // Delta / strike conversions (forward delta, premium-unadjusted)
    // -----------------------------------------------------------------------

    /// Convert a forward delta to a strike using Garman-Kohlhagen.
    ///
    /// Uses **forward delta** (premium-unadjusted) convention, which is standard
    /// for most G10 pairs. The `r_f` parameter is accepted for API compatibility
    /// but is not used — forward delta depends only on the forward price, not
    /// individual interest rates.
    ///
    /// ```text
    /// K = F * exp(-N_inv(delta) * sigma * sqrt(T) + 0.5 * sigma^2 * T)
    /// ```
    ///
    /// `delta` should be in (0, 1) for a call.
    #[inline]
    pub fn delta_to_strike(delta: f64, forward: f64, vol: f64, expiry: f64, _r_f: f64) -> f64 {
        let z_delta = crate::math::special_functions::standard_normal_inv_cdf(delta);
        let sqrt_t = expiry.sqrt();
        forward * (-z_delta * vol * sqrt_t + 0.5 * vol * vol * expiry).exp()
    }

    /// Convert a strike to forward delta using Garman-Kohlhagen.
    ///
    /// Uses **forward delta** (premium-unadjusted) convention. The `r_f`
    /// parameter is accepted for API compatibility but is not used.
    ///
    /// Returns the call delta: `N(d1)` where
    /// `d1 = [ln(F/K) + 0.5 * sigma^2 * T] / (sigma * sqrt(T))`.
    #[inline]
    pub fn strike_to_delta(strike: f64, forward: f64, vol: f64, expiry: f64, _r_f: f64) -> f64 {
        let d1 = d1_black76(forward, strike, vol, expiry);
        norm_cdf(d1)
    }

    // -----------------------------------------------------------------------
    // Implied-vol lookup
    // -----------------------------------------------------------------------

    /// Implied vol at a given expiry time, strike, forward rate, and interest rates.
    ///
    /// Steps:
    /// 1. Interpolate delta-space quotes to the requested `expiry` (linear).
    /// 2. Recover 25D (and optionally 10D) put/call wing vols from ATM, RR, BF.
    /// 3. Convert those deltas to strikes using Garman-Kohlhagen at the
    ///    supplied `forward`, building the smile for the query expiry only.
    /// 4. Interpolate linearly in strike space (flat extrapolation at wings).
    ///
    /// When 10-delta quotes are available, interpolation uses all 5 strikes
    /// (10D put, 25D put, ATM, 25D call, 10D call) for better wing accuracy.
    ///
    /// This is the smile-faithful query path: the smile is rebuilt per query
    /// expiry from interpolated quotes and that expiry's forward, so
    /// short-expiry smiles are never affected by strikes that are only
    /// relevant for long expiries (unlike fixed-strike blending on a
    /// rectangular grid; see [`to_vol_surface`](Self::to_vol_surface)).
    ///
    /// # Errors
    ///
    /// Returns an error if the expiry is non-positive or a recovered wing vol
    /// is non-positive.
    pub fn implied_vol(
        &self,
        expiry: f64,
        strike: f64,
        forward: f64,
        _r_d: f64,
        _r_f: f64,
    ) -> crate::Result<f64> {
        if expiry <= 0.0 || !expiry.is_finite() {
            return Err(InputError::NonPositiveValue.into());
        }

        // Step 1: Interpolate ATM, RR, BF to the requested expiry.
        let atm = interp_linear_clamp(&self.expiries, &self.atm_vols, expiry);
        let rr = interp_linear_clamp(&self.expiries, &self.rr_25d, expiry);
        let bf = interp_linear_clamp(&self.expiries, &self.bf_25d, expiry);

        let wings_10d = match (&self.rr_10d, &self.bf_10d) {
            (Some(rr_10d), Some(bf_10d)) => Some((
                interp_linear_clamp(&self.expiries, rr_10d, expiry),
                interp_linear_clamp(&self.expiries, bf_10d, expiry),
            )),
            _ => None,
        };

        // Steps 2-3: Recover wing vols and convert deltas to strikes for the
        // query expiry's own smile.
        let (known_strikes, known_vols) =
            fx_smile_pillars(forward, expiry, atm, rr, bf, wings_10d)?;

        // Step 4: Linear in strike, flat wings.
        Ok(interp_linear_clamp(&known_strikes, &known_vols, strike))
    }

    // -----------------------------------------------------------------------
    // Conversion to strike-based VolSurface
    // -----------------------------------------------------------------------

    /// Convert this delta-quoted surface to a strike-based [`VolSurface`].
    ///
    /// Uses [`FxDeltaVolSurfaceBuilder`] to produce a standard `VolSurface`
    /// that can be used with the rest of the pricing infrastructure. Each
    /// expiry row samples that expiry's *own* smile on the union strike axis,
    /// so queries at pillar expiries reproduce the per-expiry smile exactly;
    /// off-pillar expiry queries blend rows linearly in vol at fixed strike,
    /// which flattens intermediate smiles when forwards differ across
    /// pillars — prefer [`implied_vol`](Self::implied_vol) for smile-faithful
    /// off-pillar lookups.
    ///
    /// # Arguments
    ///
    /// * `spot`    - FX spot rate
    /// * `r_d`     - Domestic continuously compounded interest rate
    /// * `r_f`     - Foreign continuously compounded interest rate
    pub fn to_vol_surface(&self, spot: f64, r_d: f64, r_f: f64) -> crate::Result<VolSurface> {
        let mut builder = FxDeltaVolSurfaceBuilder::new(self.id.clone())
            .spot(spot)
            .domestic_rate(r_d)
            .foreign_rate(r_f)
            .expiries(&self.expiries)
            .atm_vols(&self.atm_vols)
            .rr_25d(&self.rr_25d)
            .bf_25d(&self.bf_25d);

        if let Some(rr_10d) = &self.rr_10d {
            builder = builder.rr_10d(rr_10d);
        }
        if let Some(bf_10d) = &self.bf_10d {
            builder = builder.bf_10d(bf_10d);
        }

        builder.build()
    }

    // -----------------------------------------------------------------------
    // Validation
    // -----------------------------------------------------------------------

    fn validate(
        expiries: &[f64],
        atm_vols: &[f64],
        rr_25d: &[f64],
        bf_25d: &[f64],
        rr_10d: Option<&[f64]>,
        bf_10d: Option<&[f64]>,
    ) -> crate::Result<()> {
        // Non-empty
        if expiries.is_empty() || atm_vols.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }

        // Consistent lengths
        let n = expiries.len();
        if atm_vols.len() != n || rr_25d.len() != n || bf_25d.len() != n {
            return Err(InputError::DimensionMismatch.into());
        }
        if let Some(rr) = rr_10d {
            if rr.len() != n {
                return Err(InputError::DimensionMismatch.into());
            }
        }
        if let Some(bf) = bf_10d {
            if bf.len() != n {
                return Err(InputError::DimensionMismatch.into());
            }
        }

        // Expiries: positive, finite, strictly increasing
        for &t in expiries {
            if !t.is_finite() || t <= 0.0 {
                return Err(InputError::NonPositiveValue.into());
            }
        }
        for w in expiries.windows(2) {
            if w[1] <= w[0] {
                return Err(InputError::NonMonotonicKnots.into());
            }
        }

        // ATM vols: positive and finite
        for &v in atm_vols {
            if !v.is_finite() || v <= 0.0 {
                return Err(InputError::NonPositiveValue.into());
            }
        }

        // RR and BF: finite
        for &v in rr_25d.iter().chain(bf_25d.iter()) {
            if !v.is_finite() {
                return Err(InputError::Invalid.into());
            }
        }
        if let Some(rr) = rr_10d {
            for &v in rr {
                if !v.is_finite() {
                    return Err(InputError::Invalid.into());
                }
            }
        }
        if let Some(bf) = bf_10d {
            for &v in bf {
                if !v.is_finite() {
                    return Err(InputError::Invalid.into());
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a typical 3-expiry surface.
    fn sample_surface() -> FxDeltaVolSurface {
        FxDeltaVolSurface::new(
            "EURUSD-DELTA-VOL",
            vec![0.25, 0.5, 1.0],
            vec![0.08, 0.085, 0.09],
            vec![0.01, 0.012, 0.015],
            vec![0.005, 0.006, 0.007],
        )
        .expect("sample surface should build")
    }

    // -- delta_to_strike / strike_to_delta roundtrip -----------------------

    #[test]
    fn delta_strike_roundtrip_25d_call() {
        let forward = 1.10;
        let vol = 0.09;
        let expiry = 1.0;
        let r_f = 0.04;
        let delta = 0.25;

        let strike = FxDeltaVolSurface::delta_to_strike(delta, forward, vol, expiry, r_f);
        let recovered_delta = FxDeltaVolSurface::strike_to_delta(strike, forward, vol, expiry, r_f);

        assert!(
            (recovered_delta - delta).abs() < 1e-10,
            "roundtrip failed: original delta={delta}, recovered={recovered_delta}"
        );
    }

    #[test]
    fn delta_strike_roundtrip_75d_call() {
        let forward = 1.10;
        let vol = 0.09;
        let expiry = 0.5;
        let r_f = 0.04;
        let delta = 0.75;

        let strike = FxDeltaVolSurface::delta_to_strike(delta, forward, vol, expiry, r_f);
        let recovered_delta = FxDeltaVolSurface::strike_to_delta(strike, forward, vol, expiry, r_f);

        assert!(
            (recovered_delta - delta).abs() < 1e-10,
            "roundtrip failed: original delta={delta}, recovered={recovered_delta}"
        );
    }

    // -- ATM DNS strike ~ forward for flat smile ---------------------------

    #[test]
    fn atm_dns_strike_approx_forward_for_flat_smile() {
        let forward = 1.10;
        let vol = 0.001; // nearly zero vol -> K_ATM -> F
        let expiry = 1.0;
        let r_f = 0.0;

        // ATM DNS: delta = 0.5, strike = F * exp(0.5 * sigma^2 * T)
        let k_atm = FxDeltaVolSurface::delta_to_strike(0.5, forward, vol, expiry, r_f);

        // With very small vol, K_ATM should be very close to F
        assert!(
            (k_atm - forward).abs() < 1e-4,
            "ATM strike={k_atm} should be close to forward={forward}"
        );
    }

    // -- Non-monotonic expiries rejected -----------------------------------

    #[test]
    fn non_monotonic_expiries_rejected() {
        let result = FxDeltaVolSurface::new(
            "BAD",
            vec![0.5, 0.25, 1.0], // not sorted
            vec![0.08, 0.085, 0.09],
            vec![0.01, 0.012, 0.015],
            vec![0.005, 0.006, 0.007],
        );
        assert!(result.is_err(), "non-monotonic expiries should be rejected");
    }

    #[test]
    fn duplicate_expiries_rejected() {
        let result = FxDeltaVolSurface::new(
            "BAD",
            vec![0.25, 0.25, 1.0], // duplicate
            vec![0.08, 0.085, 0.09],
            vec![0.01, 0.012, 0.015],
            vec![0.005, 0.006, 0.007],
        );
        assert!(result.is_err(), "duplicate expiries should be rejected");
    }

    // -- Mismatched vector lengths rejected --------------------------------

    #[test]
    fn mismatched_lengths_rejected() {
        // ATM vols too short
        let result = FxDeltaVolSurface::new(
            "BAD",
            vec![0.25, 0.5, 1.0],
            vec![0.08, 0.085], // only 2
            vec![0.01, 0.012, 0.015],
            vec![0.005, 0.006, 0.007],
        );
        assert!(result.is_err(), "mismatched atm_vols should be rejected");

        // RR too long
        let result2 = FxDeltaVolSurface::new(
            "BAD",
            vec![0.25, 0.5, 1.0],
            vec![0.08, 0.085, 0.09],
            vec![0.01, 0.012, 0.015, 0.02], // 4 vs 3
            vec![0.005, 0.006, 0.007],
        );
        assert!(result2.is_err(), "mismatched rr_25d should be rejected");
    }

    // -- Basic implied_vol returns sensible result -------------------------

    #[test]
    fn implied_vol_returns_positive_finite() {
        let surface = sample_surface();
        let forward = 1.10;
        let r_d = 0.05;
        let r_f = 0.04;

        // ATM-ish strike
        let k_atm = forward * (0.5 * 0.09_f64.powi(2) * 1.0).exp();
        let vol = surface
            .implied_vol(1.0, k_atm, forward, r_d, r_f)
            .expect("implied_vol should succeed");

        assert!(vol > 0.0, "vol should be positive, got {vol}");
        assert!(vol.is_finite(), "vol should be finite, got {vol}");
        // Should be close to ATM (9%)
        assert!(
            (vol - 0.09).abs() < 0.02,
            "ATM vol should be ~0.09, got {vol}"
        );
    }

    #[test]
    fn implied_vol_at_wing_strikes() {
        let surface = sample_surface();
        let forward = 1.10;
        let r_d = 0.05;
        let r_f = 0.04;

        // Low strike (put wing)
        let vol_low = surface
            .implied_vol(1.0, 0.95, forward, r_d, r_f)
            .expect("low strike implied_vol should succeed");
        assert!(vol_low > 0.0 && vol_low.is_finite());

        // High strike (call wing)
        let vol_high = surface
            .implied_vol(1.0, 1.30, forward, r_d, r_f)
            .expect("high strike implied_vol should succeed");
        assert!(vol_high > 0.0 && vol_high.is_finite());
    }

    #[test]
    fn implied_vol_interpolated_expiry() {
        let surface = sample_surface();
        let forward = 1.10;
        let r_d = 0.05;
        let r_f = 0.04;
        let k_atm = forward * (0.5 * 0.0825_f64.powi(2) * 0.375).exp();

        let vol = surface
            .implied_vol(0.375, k_atm, forward, r_d, r_f)
            .expect("interpolated expiry should succeed");

        assert!(vol > 0.0 && vol.is_finite());
        // Between 0.25Y (8%) and 0.5Y (8.5%) ATM
        assert!(
            vol > 0.07 && vol < 0.10,
            "interpolated vol should be reasonable, got {vol}"
        );
    }

    // -- Pillar vols accessor ----------------------------------------------

    #[test]
    fn pillar_vols_correct() {
        let surface = sample_surface();
        let (atm, sigma_put, sigma_call) = surface.pillar_vols(0);

        assert!((atm - 0.08).abs() < 1e-12);
        // sigma_call = ATM + BF + 0.5*RR = 0.08 + 0.005 + 0.005 = 0.09
        assert!((sigma_call - 0.09).abs() < 1e-12, "sigma_call={sigma_call}");
        // sigma_put  = ATM + BF - 0.5*RR = 0.08 + 0.005 - 0.005 = 0.08
        assert!((sigma_put - 0.08).abs() < 1e-12, "sigma_put={sigma_put}");
    }

    // -- to_vol_surface produces valid VolSurface --------------------------

    #[test]
    fn to_vol_surface_builds() {
        let surface = sample_surface();
        let vol_surface = surface
            .to_vol_surface(1.10, 0.05, 0.04)
            .expect("to_vol_surface should succeed");

        assert_eq!(vol_surface.id().as_str(), "EURUSD-DELTA-VOL");
        let (n_exp, n_str) = vol_surface.grid_shape();
        assert_eq!(n_exp, 3, "should have 3 expiries");
        assert!(n_str >= 3, "should have at least 3 strikes");
    }

    // -- Non-positive expiry rejected in implied_vol -----------------------

    #[test]
    fn implied_vol_rejects_non_positive_expiry() {
        let surface = sample_surface();
        assert!(surface.implied_vol(0.0, 1.10, 1.10, 0.05, 0.04).is_err());
        assert!(surface.implied_vol(-0.5, 1.10, 1.10, 0.05, 0.04).is_err());
    }
}
