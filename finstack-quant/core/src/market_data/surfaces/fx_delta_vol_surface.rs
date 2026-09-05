//! FX delta-based volatility surface.
//!
//! Stores FX option volatility quotes in delta space (ATM DNS, 25-delta RR/BF,
//! optional 10-delta RR/BF). Delta-to-strike conversion, volatility
//! evaluation, and strike-grid materialization live in `finstack-quant-models`.
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
//! - Wystup (2006), *FX Options and Structured Products*, Ch. 1 `docs/REFERENCES.md#wystup-fx-options`
//! - Clark (2011), *Foreign Exchange Option Pricing*, Ch. 3-4 `docs/REFERENCES.md#clark-fx-options`

use crate::{error::InputError, types::CurveId};

/// Delta-quoted FX volatility surface.
///
/// Stores market-standard FX vol quotes (ATM DNS, 25-delta risk-reversal,
/// 25-delta butterfly) across multiple expiries. Models-layer functions
/// perform delta conversion and volatility evaluation.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::surfaces::FxDeltaVolSurface;
///
/// let surface = FxDeltaVolSurface::new(
///     "EURUSD-DELTA-VOL",
///     vec![0.25, 0.5, 1.0],
///     vec![0.08, 0.085, 0.09],
///     vec![0.01, 0.012, 0.015],
///     vec![0.005, 0.006, 0.007],
///     None,
/// ).expect("surface should build");
///
/// assert_eq!(surface.num_expiries(), 3);
/// assert!((surface.atm_vols()[0] - 0.08).abs() < 1e-12);
/// ```
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "FxDeltaVolSurfaceWire")]
#[cfg_attr(feature = "json-schema", schemars(try_from = "FxDeltaVolSurfaceWire"))]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct FxDeltaVolSurfaceWire {
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

impl TryFrom<FxDeltaVolSurfaceWire> for FxDeltaVolSurface {
    type Error = crate::Error;

    fn try_from(raw: FxDeltaVolSurfaceWire) -> crate::Result<Self> {
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
    /// Create a delta-quoted surface with mandatory 25-delta wings and
    /// optional 10-delta wings.
    ///
    /// All vectors are indexed by `expiries`. Risk reversals are
    /// call-volatility minus put-volatility; each butterfly is the average
    /// wing volatility minus ATM. Volatilities are decimal annual standard
    /// deviations (for example, `0.12` for 12%), not percentages.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique surface identifier used for lookup in a market context.
    /// * `expiries` - Strictly increasing, positive expiry times in years.
    /// * `atm_vols` - ATM delta-neutral-straddle volatilities per expiry
    ///   (decimal, strictly positive).
    /// * `rr_25d` - 25-delta risk reversal per expiry (decimal vol difference).
    /// * `bf_25d` - 25-delta butterfly per expiry (decimal vol difference).
    /// * `wings_10d` - Optional `(rr_10d, bf_10d)` pair adding 10-delta risk
    ///   reversal and butterfly quotes per expiry; `None` builds a
    ///   three-point (ATM/25d) smile.
    ///
    /// # Errors
    ///
    /// Returns an error if any quote vector is empty or has a length different
    /// from `expiries`, expiries are non-positive, non-finite, or not strictly
    /// increasing, ATM volatilities are non-positive or non-finite, or any
    /// risk-reversal or butterfly quote is non-finite.
    pub fn new(
        id: impl Into<CurveId>,
        expiries: Vec<f64>,
        atm_vols: Vec<f64>,
        rr_25d: Vec<f64>,
        bf_25d: Vec<f64>,
        wings_10d: Option<(Vec<f64>, Vec<f64>)>,
    ) -> crate::Result<Self> {
        let (rr_10d, bf_10d) = match wings_10d {
            Some((rr, bf)) => (Some(rr), Some(bf)),
            None => (None, None),
        };
        Self::validate(
            &expiries,
            &atm_vols,
            &rr_25d,
            &bf_25d,
            rr_10d.as_deref(),
            bf_10d.as_deref(),
        )?;
        Ok(Self {
            id: id.into(),
            expiries,
            atm_vols,
            rr_25d,
            bf_25d,
            rr_10d,
            bf_10d,
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

    /// Return the ATM delta-neutral-straddle volatility nodes.
    pub fn atm_vols(&self) -> &[f64] {
        &self.atm_vols
    }

    /// Return the 25-delta risk-reversal quote nodes.
    pub fn rr_25d(&self) -> &[f64] {
        &self.rr_25d
    }

    /// Return the 25-delta butterfly quote nodes.
    pub fn bf_25d(&self) -> &[f64] {
        &self.bf_25d
    }

    /// Return the optional 10-delta risk-reversal quote nodes.
    pub fn rr_10d(&self) -> Option<&[f64]> {
        self.rr_10d.as_deref()
    }

    /// Return the optional 10-delta butterfly quote nodes.
    pub fn bf_10d(&self) -> Option<&[f64]> {
        self.bf_10d.as_deref()
    }

    fn validate(
        expiries: &[f64],
        atm_vols: &[f64],
        rr_25d: &[f64],
        bf_25d: &[f64],
        rr_10d: Option<&[f64]>,
        bf_10d: Option<&[f64]>,
    ) -> crate::Result<()> {
        if rr_10d.is_some() != bf_10d.is_some() {
            return Err(crate::Error::Validation(
                "rr_10d and bf_10d must both be provided or both omitted".into(),
            ));
        }
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
