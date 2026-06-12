//! Implied volatility surface with bilinear interpolation.
//!
//! Represents market-implied volatility as a function of option maturity and
//! strike. Uses bilinear interpolation on a rectangular grid, providing smooth
//! vega and volga calculations for options Greeks.
//!
//! # Financial Context
//!
//! Volatility surfaces capture the volatility smile/skew observed in options
//! markets. The surface shape reflects market views on:
//! - **Strike dimension**: Implied probability distribution (skew)
//! - **Maturity dimension**: Term structure of volatility
//! - **Surface dynamics**: Sticky strike vs sticky delta behavior
//!
//! # Interpolation
//!
//! Bilinear interpolation provides:
//! - C⁰ continuity (smooth values, discontinuous first derivatives)
//! - Fast evaluation for pricing and Greeks
//! - No spurious oscillations (unlike higher-order methods)
//!
//! # Evaluation Methods
//!
//! Surface queries are often data-driven, so this type provides multiple
//! evaluation methods with different out-of-bounds handling:
//!
//! - [`value_checked`](VolSurface::value_checked) — **Recommended primary API**.
//!   Returns `Result<f64>` with explicit error for out-of-bounds coordinates.
//! - [`value_clamped`](VolSurface::value_clamped) — Flat extrapolation: clamps
//!   coordinates to grid bounds before interpolation. Safe, never panics.
//!
//! Prefer checked or clamped evaluation at API boundaries; there is no unchecked
//! public evaluator.
//!
//! # Examples
//! ```rust
//! use finstack_core::market_data::surfaces::VolSurface;
//! use finstack_core::types::CurveId;
//!
//! let surface = VolSurface::builder("EQ-FLAT")
//!     .expiries(&[1.0, 2.0])
//!     .strikes(&[90.0, 100.0, 110.0])
//!     .row(&[0.2, 0.21, 0.22])
//!     .row(&[0.19, 0.2, 0.21])
//!     .build()
//!     .expect("VolSurface builder should succeed");
//! assert_eq!(surface.id(), &CurveId::from("EQ-FLAT"));
//!
//! // Recommended: use value_checked for explicit error handling
//! let v = surface.value_checked(1.5, 100.0).expect("Value lookup should succeed");
//! assert!(v > 0.2);
//!
//! // Or use value_clamped for flat extrapolation (safe, never panics)
//! let v_clamped = surface.value_clamped(0.5, 80.0); // clamped to grid bounds
//! assert!(v_clamped > 0.0);
//! ```

// Box and Vec are available from the standard prelude; no explicit alloc import needed.

use crate::{
    error::InputError,
    market_data::{
        bumps::{BumpSpec, Bumpable},
        traits::TermStructure,
    },
    math::interp::utils::locate_segment,
    math::volatility::svi,
    types::CurveId,
    Error,
};

/// Semantic meaning of the secondary axis on a [`VolSurface`].
///
/// Most option surfaces are defined on `expiry × strike`, but some calibration
/// workflows materialize ATM matrices on `expiry × tenor`. Keeping the axis type
/// explicit prevents consumers from accidentally interpreting tenor buckets as
/// strikes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum VolSurfaceAxis {
    /// The secondary axis is strike/moneyness.
    #[default]
    Strike,
    /// The secondary axis is swap tenor or another maturity-style bucket.
    Tenor,
}

impl std::fmt::Display for VolSurfaceAxis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Strike => write!(f, "strike"),
            Self::Tenor => write!(f, "tenor"),
        }
    }
}

/// Quoting convention of the volatilities stored on a [`VolSurface`].
///
/// The same `vol_surface_id` channel is read by consumers with very different
/// expectations: rates calibrations typically read normal (Bachelier, absolute)
/// vols on an `expiry × tenor` ATM matrix, while equity/FX/swaption smile
/// consumers read Black (lognormal, relative) vols on `expiry × strike`. The
/// stored numbers are an order of magnitude apart (e.g. 0.008 normal vs 0.20
/// Black), so misreading one as the other silently mis-prices. Tagging the
/// quote type lets consumers enforce their convention via
/// [`VolSurface::require_quote_type`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VolQuoteType {
    /// Black (lognormal) implied volatility, relative units (the default).
    #[default]
    BlackLognormal,
    /// Normal (Bachelier) implied volatility, absolute rate units.
    Normal,
}

impl std::fmt::Display for VolQuoteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlackLognormal => write!(f, "black_lognormal"),
            Self::Normal => write!(f, "normal"),
        }
    }
}

/// Interpolation contract for vol surfaces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolInterpolationMode {
    /// Interpolate implied volatility directly (the default).
    ///
    /// This is the most literal choice when market quotes are already given as
    /// implied volatilities on the stored grid and you want local interpolation
    /// in quote space.
    ///
    /// # ⚠️ Not calendar-arbitrage-safe across expiry
    ///
    /// Linear-in-vol interpolation along the **expiry** axis does not preserve
    /// monotonicity of total variance `σ²·t`, so an off-grid vol read between
    /// two expiry pillars can imply *decreasing* total variance with time —
    /// i.e. calendar arbitrage — even when the pillar grid is itself
    /// arbitrage-free. For arbitrage-sensitive workflows (Dupire local vol,
    /// no-arb checks, term-structure interpolation) select
    /// [`TotalVariance`](Self::TotalVariance) via
    /// [`VolSurfaceBuilder::with_interpolation_mode`]. The default is retained
    /// for backward compatibility and quote-space fidelity at the pillars.
    #[default]
    Vol,
    /// Interpolate total variance `sigma^2 * t`, then convert back to implied vol.
    ///
    /// Preferred when blending across expiries: total variance behaves more
    /// linearly in time and preserves the no-arbitrage intuition for variance
    /// accumulation, avoiding the calendar-arbitrage trap of linear-in-vol
    /// interpolation (see [`Vol`](Self::Vol)).
    TotalVariance,
}

/// Volatility surface defined on expiry × strike grid.
///
/// Internally stores volatilities in row-major order as a boxed slice.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawVolSurface", into = "RawVolSurface")]
pub struct VolSurface {
    id: CurveId,
    expiries: Box<[f64]>,
    strikes: Box<[f64]>,
    secondary_axis: VolSurfaceAxis,
    quote_type: VolQuoteType,
    interpolation_mode: VolInterpolationMode,
    /// Row-major storage: vols[expiry_idx * n_strikes + strike_idx]
    vols: Box<[f64]>,
}

/// Raw serializable state of a VolSurface
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVolSurface {
    /// Surface identifier
    pub id: String,
    /// Expiry times in years
    pub expiries: Vec<f64>,
    /// Strike prices
    pub strikes: Vec<f64>,
    /// Semantic meaning of the secondary axis. Defaults to strike for older payloads.
    #[serde(default)]
    pub secondary_axis: VolSurfaceAxis,
    /// Quote convention. Defaults to Black/lognormal for older payloads.
    #[serde(default)]
    pub quote_type: VolQuoteType,
    /// Interpolation contract. Defaults to direct vol interpolation for older payloads.
    #[serde(default)]
    pub interpolation_mode: VolInterpolationMode,
    /// Volatility values in row-major order
    pub vols_row_major: Vec<f64>,
}

impl From<VolSurface> for RawVolSurface {
    fn from(surface: VolSurface) -> Self {
        RawVolSurface {
            id: surface.id.to_string(),
            expiries: surface.expiries.to_vec(),
            strikes: surface.strikes.to_vec(),
            secondary_axis: surface.secondary_axis,
            quote_type: surface.quote_type,
            interpolation_mode: surface.interpolation_mode,
            vols_row_major: surface.vols.into_vec(),
        }
    }
}

impl TryFrom<RawVolSurface> for VolSurface {
    type Error = crate::Error;

    fn try_from(state: RawVolSurface) -> crate::Result<Self> {
        Ok(Self::from_grid(
            &state.id,
            &state.expiries,
            &state.strikes,
            &state.vols_row_major,
        )?
        .with_secondary_axis(state.secondary_axis)
        .with_quote_type(state.quote_type)
        .with_interpolation_mode(state.interpolation_mode))
    }
}

impl VolSurface {
    #[inline]
    fn validate_total_variance(&self, total_variance: f64, expiry: f64) -> crate::Result<f64> {
        let variance = total_variance / expiry.max(f64::EPSILON);
        if variance < 0.0 {
            return Err(Error::Validation(format!(
                "Vol surface '{}' produced negative total variance at expiry={} under {:?} interpolation",
                self.id, expiry, self.interpolation_mode
            )));
        }
        Ok(variance.sqrt())
    }

    /// Start building a new volatility surface with identifier `id`.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_core::market_data::surfaces::VolSurface;
    /// # fn main() -> finstack_core::Result<()> {
    ///
    /// let surface = VolSurface::builder("IR-SWAPTION")
    ///     .expiries(&[1.0, 2.0])
    ///     .strikes(&[0.01, 0.02])
    ///     .row(&[0.25, 0.24])
    ///     .row(&[0.23, 0.22])
    ///     .build()
    ///     ?;
    /// // Use value_checked for safe evaluation with explicit error handling
    /// assert!(surface.value_checked(1.5, 0.015)? > 0.22);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> VolSurfaceBuilder {
        VolSurfaceBuilder {
            id: id.into(),
            expiries: Vec::new(),
            strikes: Vec::new(),
            secondary_axis: VolSurfaceAxis::Strike,
            quote_type: VolQuoteType::BlackLognormal,
            interpolation_mode: VolInterpolationMode::Vol,
            vols: Vec::new(),
        }
    }

    #[inline]
    fn bilinear(q11: f64, q21: f64, q12: f64, q22: f64, t: f64, u: f64) -> f64 {
        (1.0 - t) * (1.0 - u) * q11 + t * (1.0 - u) * q21 + (1.0 - t) * u * q12 + t * u * q22
    }

    /// Safe evaluation: returns `Err` if either coordinate is out of bounds.
    pub fn value_checked(&self, expiry: f64, strike: f64) -> crate::Result<f64> {
        let (ie0, exact_e) = self.value_indices(self.expiries.as_ref(), expiry)?;
        let (is0, exact_s) = self.value_indices(self.strikes.as_ref(), strike)?;
        self.interpolate_from_segments(expiry, strike, ie0, is0, exact_e, exact_s)
    }

    /// Clamped evaluation: clamps to edge values when outside the grid.
    ///
    /// Provides flat extrapolation by clamping coordinates to the grid bounds
    /// before interpolation. This method never panics and is suitable for
    /// pricing scenarios where out-of-bounds coordinates should use edge values.
    pub fn value_clamped(&self, expiry: f64, strike: f64) -> f64 {
        // Get bounds safely using first/last with defensive fallbacks
        let (Some(&exp_min), Some(&exp_max)) = (self.expiries.first(), self.expiries.last()) else {
            return f64::NAN;
        };
        let (Some(&str_min), Some(&str_max)) = (self.strikes.first(), self.strikes.last()) else {
            return f64::NAN;
        };

        let expiry = expiry.clamp(exp_min, exp_max);
        let strike = strike.clamp(str_min, str_max);

        self.value_in_bounds(expiry, strike).unwrap_or(f64::NAN)
    }

    /// Interpolate a vol at coordinates known to be within grid bounds.
    ///
    /// Skips bounds checks and error handling. Coordinates must satisfy
    /// `expiries[0] <= expiry <= expiries[last]` and similarly for strike.
    #[inline]
    fn value_in_bounds(&self, expiry: f64, strike: f64) -> crate::Result<f64> {
        use crate::math::interp::utils::locate_segment_unchecked;

        let ie0 = locate_segment_unchecked(&self.expiries, expiry);
        let is0 = locate_segment_unchecked(&self.strikes, strike);
        #[allow(clippy::float_cmp)]
        let exact_e = self.expiries[ie0] == expiry;
        #[allow(clippy::float_cmp)]
        let exact_s = self.strikes[is0] == strike;

        self.interpolate_from_segments(expiry, strike, ie0, is0, exact_e, exact_s)
    }

    fn interpolate_from_segments(
        &self,
        expiry: f64,
        strike: f64,
        ie0: usize,
        is0: usize,
        exact_e: bool,
        exact_s: bool,
    ) -> crate::Result<f64> {
        let n_strikes = self.strikes.len();
        if exact_e && exact_s {
            return Ok(self.vols[ie0 * n_strikes + is0]);
        }

        let ie1 = if exact_e { ie0 } else { ie0 + 1 };
        let is1 = if exact_s { is0 } else { is0 + 1 };
        let e0 = self.expiries[ie0];
        let e1 = self.expiries[ie1];
        let s0 = self.strikes[is0];
        let s1 = self.strikes[is1];
        let q11 = self.vols[ie0 * n_strikes + is0];
        let q21 = self.vols[ie1 * n_strikes + is0];
        let q12 = self.vols[ie0 * n_strikes + is1];
        let q22 = self.vols[ie1 * n_strikes + is1];
        let t = if exact_e {
            0.0
        } else {
            (expiry - e0) / (e1 - e0)
        };
        let u = if exact_s {
            0.0
        } else {
            (strike - s0) / (s1 - s0)
        };
        match self.interpolation_mode {
            VolInterpolationMode::Vol => Ok(Self::bilinear(q11, q21, q12, q22, t, u)),
            VolInterpolationMode::TotalVariance => {
                let total_variance = Self::bilinear(
                    e0 * q11 * q11,
                    e1 * q21 * q21,
                    e0 * q12 * q12,
                    e1 * q22 * q22,
                    t,
                    u,
                );
                self.validate_total_variance(total_variance, expiry)
            }
        }
    }

    #[inline]
    fn value_indices(&self, xs: &[f64], x: f64) -> Result<(usize, bool), Error> {
        let i = locate_segment(xs, x)?;
        // Exact comparison is intentional: checking for exact grid-point hit.
        #[allow(clippy::float_cmp)]
        let exact = xs[i] == x;
        Ok((i, exact))
    }

    /// Unique identifier of the surface.
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Returns the expiries axis (years).
    pub fn expiries(&self) -> &[f64] {
        &self.expiries
    }

    /// Returns the strikes axis.
    pub fn strikes(&self) -> &[f64] {
        &self.strikes
    }

    /// Semantic meaning of the secondary axis.
    pub fn secondary_axis(&self) -> VolSurfaceAxis {
        self.secondary_axis
    }

    /// Quoting convention of the stored volatilities.
    pub fn quote_type(&self) -> VolQuoteType {
        self.quote_type
    }

    /// Interpolation contract used when evaluating between grid points.
    pub fn interpolation_mode(&self) -> VolInterpolationMode {
        self.interpolation_mode
    }

    /// Return a copy of this surface with an explicit secondary-axis contract.
    #[must_use]
    pub fn with_secondary_axis(mut self, secondary_axis: VolSurfaceAxis) -> Self {
        self.secondary_axis = secondary_axis;
        self
    }

    /// Return a copy of this surface with an explicit quote-type contract.
    #[must_use]
    pub fn with_quote_type(mut self, quote_type: VolQuoteType) -> Self {
        self.quote_type = quote_type;
        self
    }

    /// Return a copy of this surface with an explicit interpolation contract.
    ///
    /// Use [`VolInterpolationMode::TotalVariance`] when the surface should
    /// interpolate linearly in total variance rather than directly in implied
    /// volatility.
    #[must_use]
    pub fn with_interpolation_mode(mut self, interpolation_mode: VolInterpolationMode) -> Self {
        self.interpolation_mode = interpolation_mode;
        self
    }

    /// Require the semantic axis before a caller uses the surface.
    pub fn require_secondary_axis(&self, expected: VolSurfaceAxis) -> crate::Result<()> {
        if self.secondary_axis == expected {
            return Ok(());
        }

        Err(Error::Validation(format!(
            "Vol surface '{}' uses secondary axis '{}' but caller expected '{}'",
            self.id, self.secondary_axis, expected
        )))
    }

    /// Require the quoting convention before a caller uses the surface.
    ///
    /// Consumers that interpret the stored values under a specific convention
    /// (normal/Bachelier vs Black/lognormal) should call this at the read site
    /// so a mis-tagged or mis-wired surface fails loudly instead of silently
    /// mis-pricing by an order of magnitude.
    pub fn require_quote_type(&self, expected: VolQuoteType) -> crate::Result<()> {
        if self.quote_type == expected {
            return Ok(());
        }

        Err(Error::Validation(format!(
            "Vol surface '{}' stores '{}' quotes but caller expected '{}'",
            self.id, self.quote_type, expected
        )))
    }

    /// Grid shape as (n_expiries, n_strikes).
    pub fn grid_shape(&self) -> (usize, usize) {
        (self.expiries.len(), self.strikes.len())
    }

    /// Create a new volatility surface with a single point bumped.
    ///
    /// Bumps the volatility at the specified (expiry, strike) point by a relative amount.
    /// Uses bilinear interpolation to find the grid cell containing the point and bumps
    /// the nearest grid point. This is useful for bucketed Vega calculations.
    ///
    /// # Arguments
    /// * `expiry` - Expiry time in years
    /// * `strike` - Strike price
    /// * `bump_pct` - Relative bump size (e.g., 0.01 for 1% increase)
    ///
    /// # Returns
    /// New VolSurface with bumped volatility at the specified point
    ///
    /// # Errors
    /// Returns error if expiry or strike is out of bounds (even after clamping)
    ///
    /// # Examples
    /// ```rust
    /// use finstack_core::market_data::surfaces::VolSurface;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///
    /// let surface = VolSurface::builder("EQ-VOL")
    ///     .expiries(&[1.0, 2.0])
    ///     .strikes(&[90.0, 100.0, 110.0])
    ///     .row(&[0.2, 0.21, 0.22])
    ///     .row(&[0.19, 0.2, 0.21])
    ///     .build()
    ///     .expect("VolSurface builder should succeed");
    ///
    /// // Bump vol at (1.5 years, 100.0 strike) by 1%
    /// let bumped = surface.bump_point(1.5, 100.0, 0.01)?;
    /// assert!(bumped.value_checked(1.5, 100.0)? > surface.value_checked(1.5, 100.0)?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn bump_point(&self, expiry: f64, strike: f64, bump_pct: f64) -> crate::Result<Self> {
        // Get bounds safely using first/last
        let (Some(&exp_min), Some(&exp_max)) = (self.expiries.first(), self.expiries.last()) else {
            return Err(crate::error::InputError::TooFewPoints.into());
        };
        let (Some(&str_min), Some(&str_max)) = (self.strikes.first(), self.strikes.last()) else {
            return Err(crate::error::InputError::TooFewPoints.into());
        };

        // Clamp to grid bounds
        let clamped_expiry = expiry.clamp(exp_min, exp_max);
        let clamped_strike = strike.clamp(str_min, str_max);

        // Find the closest grid indices
        let expiry_idx = find_closest_grid_index(self.expiries.as_ref(), clamped_expiry);
        let strike_idx = find_closest_grid_index(self.strikes.as_ref(), clamped_strike);

        let n_strikes = self.strikes.len();
        let idx = expiry_idx * n_strikes + strike_idx;

        // Get current vol at that grid point
        let current_vol = self.vols[idx];
        let bumped_vol = current_vol * (1.0 + bump_pct).max(0.0);

        // Clone the vols vec and update the bumped point
        let mut bumped_vols = self.vols.clone();
        bumped_vols[idx] = bumped_vol;

        // Rebuild surface with same ID, grid, and metadata contracts.
        Self::from_grid_opts(
            self.id.as_str(),
            &self.expiries,
            &self.strikes,
            &bumped_vols,
            VolGridOpts {
                secondary_axis: self.secondary_axis,
                quote_type: self.quote_type,
                interpolation_mode: self.interpolation_mode,
            },
        )
    }

    /// Bump a single grid point in place, returning the original vol for reversal.
    ///
    /// Avoids cloning the entire vols vector. Use with
    /// [`unbump_point_in_place`](Self::unbump_point_in_place) to restore.
    pub fn bump_point_in_place(
        &mut self,
        expiry: f64,
        strike: f64,
        bump_pct: f64,
    ) -> crate::Result<f64> {
        let (Some(&exp_min), Some(&exp_max)) = (self.expiries.first(), self.expiries.last()) else {
            return Err(crate::error::InputError::TooFewPoints.into());
        };
        let (Some(&str_min), Some(&str_max)) = (self.strikes.first(), self.strikes.last()) else {
            return Err(crate::error::InputError::TooFewPoints.into());
        };

        let clamped_expiry = expiry.clamp(exp_min, exp_max);
        let clamped_strike = strike.clamp(str_min, str_max);

        let expiry_idx = find_closest_grid_index(self.expiries.as_ref(), clamped_expiry);
        let strike_idx = find_closest_grid_index(self.strikes.as_ref(), clamped_strike);

        let n_strikes = self.strikes.len();
        let idx = expiry_idx * n_strikes + strike_idx;

        let original = self.vols[idx];
        self.vols[idx] = original * (1.0 + bump_pct).max(0.0);
        Ok(original)
    }

    /// Restore a grid point to a previously saved vol value.
    pub fn unbump_point_in_place(&mut self, expiry: f64, strike: f64, original_vol: f64) {
        let clamped_expiry = match (self.expiries.first(), self.expiries.last()) {
            (Some(&min), Some(&max)) => expiry.clamp(min, max),
            _ => return,
        };
        let clamped_strike = match (self.strikes.first(), self.strikes.last()) {
            (Some(&min), Some(&max)) => strike.clamp(min, max),
            _ => return,
        };

        let expiry_idx = find_closest_grid_index(self.expiries.as_ref(), clamped_expiry);
        let strike_idx = find_closest_grid_index(self.strikes.as_ref(), clamped_strike);

        let n_strikes = self.strikes.len();
        self.vols[expiry_idx * n_strikes + strike_idx] = original_vol;
    }

    /// Return a new volatility surface scaled uniformly by `scale`.
    ///
    /// This creates a copy of the surface with the same identifier and grid,
    /// multiplying every volatility by `scale`. It avoids the overhead of
    /// serializing to a row-major buffer and rebuilding via `from_grid`.
    ///
    /// For greek bumps that apply a uniform percentage change to the entire
    /// surface, prefer this method over `to_state()`/`from_grid()`.
    pub fn scaled(&self, scale: f64) -> Self {
        // Fast path: return an identical copy when scale == 1.0
        if (scale - 1.0).abs() < f64::EPSILON {
            return Self {
                id: self.id.clone(),
                expiries: self.expiries.clone(),
                strikes: self.strikes.clone(),
                secondary_axis: self.secondary_axis,
                quote_type: self.quote_type,
                interpolation_mode: self.interpolation_mode,
                vols: self.vols.clone(),
            };
        }

        // Scale all vols
        let scaled_vols = self.vols.iter().map(|&v| v * scale).collect::<Box<[f64]>>();

        Self {
            id: self.id.clone(),
            expiries: self.expiries.clone(),
            strikes: self.strikes.clone(),
            secondary_axis: self.secondary_axis,
            quote_type: self.quote_type,
            interpolation_mode: self.interpolation_mode,
            vols: scaled_vols,
        }
    }
}

impl Bumpable for VolSurface {
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        use crate::error::InputError;

        // Only parallel bumps are supported for now
        if !matches!(
            spec.bump_type,
            crate::market_data::bumps::BumpType::Parallel
        ) {
            return Err(InputError::UnsupportedBump {
                reason: "VolSurface only supports Parallel bumps, not key-rate bumps".to_string(),
            }
            .into());
        }

        let (raw_val, is_multiplicative) = spec.resolve_standard_values().ok_or_else(|| {
            InputError::UnsupportedBump {
                reason: format!(
                    "VolSurface only supports Additive/{{RateBp,Percent,Fraction}} or Multiplicative/Factor, got {:?}/{:?}",
                    spec.mode, spec.units
                ),
            }
        })?;

        let bumped_vols = if is_multiplicative {
            // Factor bump: new_vol = vol * factor
            self.vols.iter().map(|&v| (v * raw_val).max(0.0)).collect()
        } else {
            // Additive bump: new_vol = vol + delta
            self.vols.iter().map(|&v| (v + raw_val).max(0.0)).collect()
        };

        Ok(Self {
            id: self.id.clone(),
            expiries: self.expiries.clone(),
            strikes: self.strikes.clone(),
            secondary_axis: self.secondary_axis,
            quote_type: self.quote_type,
            interpolation_mode: self.interpolation_mode,
            vols: bumped_vols,
        })
    }
}

/// Relative tolerance used by [`VolSurface::apply_bucket_bump`] when matching
/// filter values against the surface's own expiry/strike grid.
///
/// Filter values are expected to be the surface's exact grid values; the
/// tolerance only absorbs floating-point round-off (mirroring
/// `BASE_CORR_DETACHMENT_MATCH_TOLERANCE` in `base_correlation.rs`). It scales
/// as `tol * max(1, |grid_value|)` so it works for both rate strikes (~0.01)
/// and price strikes (~100.0).
pub const BUCKET_BUMP_MATCH_TOLERANCE: f64 = 1.0e-9;

/// True when `filter_value` matches `grid_value` within
/// [`BUCKET_BUMP_MATCH_TOLERANCE`] relative tolerance.
#[inline]
fn bucket_filter_matches(filter_value: f64, grid_value: f64) -> bool {
    (filter_value - grid_value).abs() <= BUCKET_BUMP_MATCH_TOLERANCE * grid_value.abs().max(1.0)
}

impl VolSurface {
    /// Apply a filtered bucket bump (percentage) to matching expiry/strike cells.
    ///
    /// Filter values must equal the surface's own grid values (a tiny relative
    /// tolerance of [`BUCKET_BUMP_MATCH_TOLERANCE`] absorbs floating-point
    /// round-off only); values that fall between grid points match nothing.
    pub fn apply_bucket_bump(
        &self,
        expiries_filter: Option<&[f64]>,
        strikes_filter: Option<&[f64]>,
        pct: f64,
    ) -> Option<Self> {
        let factor = 1.0 + pct / 100.0;
        let (n_expiries, n_strikes) = self.grid_shape();
        let mut builder = VolSurface::builder(self.id.clone())
            .expiries(self.expiries())
            .strikes(self.strikes())
            .secondary_axis(self.secondary_axis)
            .quote_type(self.quote_type)
            .interpolation_mode(self.interpolation_mode);

        for (ei, &expiry) in self.expiries.iter().enumerate().take(n_expiries) {
            let mut row = Vec::with_capacity(n_strikes);
            for (si, &strike) in self.strikes.iter().enumerate().take(n_strikes) {
                let val = self.vols[ei * n_strikes + si];
                let expiry_match = expiries_filter
                    .map(|flt| flt.iter().any(|&e| bucket_filter_matches(e, expiry)))
                    .unwrap_or(true);
                let strike_match = strikes_filter
                    .map(|flt| flt.iter().any(|&s| bucket_filter_matches(s, strike)))
                    .unwrap_or(true);

                if expiry_match && strike_match {
                    row.push((val * factor).max(0.0));
                } else {
                    row.push(val);
                }
            }
            builder = builder.row(&row);
        }

        builder.build().ok()
    }
}

impl VolSurface {
    /// Evaluate implied vol with SVI-based wing extrapolation for out-of-bounds strikes.
    ///
    /// For strikes within the grid, this method uses the standard bilinear interpolation.
    /// For strikes outside the grid bounds, it fits an SVI parameterization to the
    /// nearest expiry slice and extrapolates the wings. For expiries outside the grid,
    /// the nearest expiry slice is used (flat in the expiry dimension).
    ///
    /// This provides a theoretically consistent wing extrapolation that:
    /// - Produces the correct asymptotic slope (linear in log-moneyness)
    /// - Avoids flat extrapolation artifacts at extreme strikes
    /// - Matches the smile shape at the boundary
    ///
    /// # Arguments
    ///
    /// * `expiry` — option expiry in years
    /// * `strike` — option strike
    /// * `forward` — forward price at this expiry (needed for log-moneyness)
    ///
    /// # Returns
    ///
    /// Implied volatility. Returns the SVI-extrapolated value for out-of-bounds strikes,
    /// or bilinear interpolated value for in-bounds coordinates. Falls back to
    /// `value_clamped` if SVI calibration fails (e.g., too few strikes).
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_core::market_data::surfaces::VolSurface;
    ///
    /// let surface = VolSurface::builder("EQ-SMILE")
    ///     .expiries(&[0.5, 1.0, 2.0])
    ///     .strikes(&[80.0, 90.0, 95.0, 100.0, 105.0, 110.0, 120.0])
    ///     .row(&[0.30, 0.25, 0.22, 0.20, 0.21, 0.23, 0.28])
    ///     .row(&[0.28, 0.24, 0.21, 0.19, 0.20, 0.22, 0.26])
    ///     .row(&[0.26, 0.22, 0.20, 0.18, 0.19, 0.21, 0.24])
    ///     .build()
    ///     .expect("surface should build");
    ///
    /// // In-bounds: uses bilinear interpolation
    /// let v_in = surface.value_extrapolated(1.0, 100.0, 100.0);
    /// assert!(v_in > 0.0);
    ///
    /// // Out-of-bounds strike: SVI wing extrapolation
    /// let v_deep_otm = surface.value_extrapolated(1.0, 60.0, 100.0);
    /// assert!(v_deep_otm > 0.0);
    /// ```
    pub fn value_extrapolated(&self, expiry: f64, strike: f64, forward: f64) -> f64 {
        if !forward.is_finite() || forward <= 0.0 {
            return f64::NAN;
        }

        // Try bilinear interpolation first
        if let Ok(v) = self.value_checked(expiry, strike) {
            return v;
        }

        // Determine the expiry slice to use
        let Some(&exp_min) = self.expiries.first() else {
            return f64::NAN;
        };
        let Some(&exp_max) = self.expiries.last() else {
            return f64::NAN;
        };

        // Clamp expiry to grid range
        let clamped_expiry = expiry.clamp(exp_min, exp_max);

        // If the strike is in bounds, the issue is only expiry — clamp works fine
        let Some(&str_min) = self.strikes.first() else {
            return f64::NAN;
        };
        let Some(&str_max) = self.strikes.last() else {
            return f64::NAN;
        };

        if strike >= str_min && strike <= str_max {
            // Strike is in range, expiry was out of range — flat extrapolate in expiry
            return self.value_clamped(clamped_expiry, strike);
        }

        // Strike is out of bounds — use SVI wing extrapolation
        // Find the closest expiry index for the slice
        let expiry_idx = find_closest_grid_index(&self.expiries, clamped_expiry);
        let n_strikes = self.strikes.len();

        // Need at least 5 strikes for SVI calibration
        if n_strikes < 5 {
            return self.value_clamped(clamped_expiry, strike);
        }

        // Extract the vol slice at this expiry
        let slice_vols: Vec<f64> = (0..n_strikes)
            .map(|si| self.vols[expiry_idx * n_strikes + si])
            .collect();

        let slice_expiry = self.expiries[expiry_idx];

        // Calibrate SVI to this slice
        match svi::calibrate_svi(&self.strikes, &slice_vols, forward, slice_expiry) {
            Ok(params) => {
                let k = (strike / forward).ln();
                let vol = params.implied_vol(k, slice_expiry);
                if vol.is_finite() && vol > 0.0 {
                    vol
                } else {
                    self.value_clamped(clamped_expiry, strike)
                }
            }
            Err(_) => {
                // SVI calibration failed — fall back to flat extrapolation.
                self.value_clamped(clamped_expiry, strike)
            }
        }
    }
}

/// Helper to find the closest grid index for a target value.
fn find_closest_grid_index(arr: &[f64], target: f64) -> usize {
    if target <= arr[0] {
        return 0;
    }
    if target >= arr[arr.len() - 1] {
        return arr.len() - 1;
    }

    arr.windows(2)
        .enumerate()
        .find_map(|(i, w)| {
            if target >= w[0] && target <= w[1] {
                // Return the closer of the two
                Some(if (target - w[0]).abs() < (target - w[1]).abs() {
                    i
                } else {
                    i + 1
                })
            } else {
                None
            }
        })
        .unwrap_or(arr.len() - 1)
}

// Minimal trait implementation for polymorphism where needed

impl TermStructure for VolSurface {
    #[inline]
    fn id(&self) -> &CurveId {
        &self.id
    }
}

impl crate::market_data::traits::VolProvider for VolSurface {
    fn vol(&self, expiry: f64, tenor: f64, strike: f64) -> crate::Result<f64> {
        let secondary = match self.secondary_axis {
            VolSurfaceAxis::Strike => strike,
            VolSurfaceAxis::Tenor => tenor,
        };
        self.value_checked(expiry, secondary)
    }
    fn vol_clamped(&self, expiry: f64, tenor: f64, strike: f64) -> f64 {
        let secondary = match self.secondary_axis {
            VolSurfaceAxis::Strike => strike,
            VolSurfaceAxis::Tenor => tenor,
        };
        self.value_clamped(expiry, secondary)
    }
    fn vol_id(&self) -> &crate::types::CurveId {
        self.id()
    }
}

/// Fluent builder for [`VolSurface`].
pub struct VolSurfaceBuilder {
    id: CurveId,
    expiries: Vec<f64>,
    strikes: Vec<f64>,
    secondary_axis: VolSurfaceAxis,
    quote_type: VolQuoteType,
    interpolation_mode: VolInterpolationMode,
    vols: Vec<Vec<f64>>, // row-major expiries
}

impl VolSurfaceBuilder {
    /// Set the vector of option **expiries** (years).
    pub fn expiries(mut self, exps: &[f64]) -> Self {
        self.expiries.extend_from_slice(exps);
        self
    }
    /// Set the vector of option **strikes**.
    pub fn strikes(mut self, ks: &[f64]) -> Self {
        self.strikes.extend_from_slice(ks);
        self
    }

    /// Set the semantic meaning of the secondary axis.
    pub fn secondary_axis(mut self, axis: VolSurfaceAxis) -> Self {
        self.secondary_axis = axis;
        self
    }

    /// Set the quoting convention of the stored volatilities.
    pub fn quote_type(mut self, quote_type: VolQuoteType) -> Self {
        self.quote_type = quote_type;
        self
    }

    /// Set the interpolation contract used for off-grid evaluation.
    pub fn interpolation_mode(mut self, mode: VolInterpolationMode) -> Self {
        self.interpolation_mode = mode;
        self
    }

    /// Append a row of implied volatilities corresponding to the previously
    /// set strikes. Rows must be added in the **same order** as expiries.
    pub fn row(mut self, row: &[f64]) -> Self {
        self.vols.push(row.to_vec());
        self
    }

    /// Finalise the surface and return an immutable [`VolSurface`] instance.
    /// Performs consistency checks on grid dimensions.
    pub fn build(self) -> crate::Result<VolSurface> {
        if self.expiries.is_empty() || self.strikes.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }
        validate_axis(&self.expiries[..])?;
        validate_axis(&self.strikes[..])?;
        if self.vols.len() != self.expiries.len() {
            return Err(InputError::DimensionMismatch.into());
        }
        for row in &self.vols {
            if row.len() != self.strikes.len() {
                return Err(InputError::DimensionMismatch.into());
            }
            // Validate numeric properties: volatilities must be finite and non-negative
            for &v in row {
                if !v.is_finite() {
                    return Err(InputError::Invalid.into());
                }
                if v < 0.0 {
                    return Err(InputError::NegativeValue.into());
                }
            }
        }
        let flat: Vec<f64> = self.vols.into_iter().flatten().collect();
        Ok(VolSurface {
            id: self.id,
            expiries: self.expiries.into_boxed_slice(),
            strikes: self.strikes.into_boxed_slice(),
            secondary_axis: self.secondary_axis,
            quote_type: self.quote_type,
            interpolation_mode: self.interpolation_mode,
            vols: flat.into_boxed_slice(),
        })
    }
}

/// Options bundle for [`VolSurface::from_grid_opts`].
///
/// Use this when grid construction needs a non-default secondary-axis or
/// interpolation contract.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VolGridOpts {
    /// Semantic meaning of the secondary axis (strike vs tenor).
    pub secondary_axis: VolSurfaceAxis,
    /// Quoting convention of the stored volatilities.
    pub quote_type: VolQuoteType,
    /// Interpolation contract (direct vol vs total-variance).
    pub interpolation_mode: VolInterpolationMode,
}

impl VolGridOpts {
    /// Shorthand constructor (quote type defaults to Black/lognormal).
    pub fn new(secondary_axis: VolSurfaceAxis, interpolation_mode: VolInterpolationMode) -> Self {
        Self {
            secondary_axis,
            quote_type: VolQuoteType::default(),
            interpolation_mode,
        }
    }

    /// Set the quoting convention.
    #[must_use]
    pub fn with_quote_type(mut self, quote_type: VolQuoteType) -> Self {
        self.quote_type = quote_type;
        self
    }
}

impl VolSurface {
    /// Canonical grid constructor for callers that need explicit construction
    /// options.
    pub fn from_grid_opts(
        id: impl AsRef<str>,
        expiries: &[f64],
        strikes: &[f64],
        vols_row_major: &[f64],
        opts: VolGridOpts,
    ) -> crate::Result<Self> {
        if expiries.is_empty() || strikes.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }
        validate_axis(expiries)?;
        validate_axis(strikes)?;
        let n = expiries.len() * strikes.len();
        if vols_row_major.len() != n {
            return Err(InputError::DimensionMismatch.into());
        }
        for &v in vols_row_major {
            if !v.is_finite() {
                return Err(InputError::Invalid.into());
            }
            if v < 0.0 {
                return Err(InputError::NegativeValue.into());
            }
        }
        Ok(Self {
            id: CurveId::new(id.as_ref()),
            expiries: expiries.to_vec().into_boxed_slice(),
            strikes: strikes.to_vec().into_boxed_slice(),
            secondary_axis: opts.secondary_axis,
            quote_type: opts.quote_type,
            interpolation_mode: opts.interpolation_mode,
            vols: vols_row_major.to_vec().into_boxed_slice(),
        })
    }

    /// Construct directly from axes and a row-major flat values array.
    ///
    /// Equivalent to [`from_grid_opts`](Self::from_grid_opts) with
    /// [`VolGridOpts::default()`].
    pub fn from_grid(
        id: impl AsRef<str>,
        expiries: &[f64],
        strikes: &[f64],
        vols_row_major: &[f64],
    ) -> crate::Result<Self> {
        Self::from_grid_opts(
            id,
            expiries,
            strikes,
            vols_row_major,
            VolGridOpts::default(),
        )
    }

    /// Construct directly from axes and row-major volatility rows.
    ///
    /// `vol_rows[expiry_idx][strike_idx]` is flattened and validated through
    /// [`Self::from_grid_opts`]. This is the canonical entry point for callers
    /// whose input is naturally represented as expiry rows.
    ///
    /// # Errors
    /// - Row count does not match `expiries.len()`
    /// - Any row length does not match `strikes.len()`
    /// - Any invariant enforced by [`Self::from_grid_opts`] fails
    pub fn from_rows(
        id: impl AsRef<str>,
        expiries: &[f64],
        strikes: &[f64],
        vol_rows: &[Vec<f64>],
    ) -> crate::Result<Self> {
        if vol_rows.len() != expiries.len() {
            return Err(Error::Validation(format!(
                "vol_rows has {} rows but expiries has {} entries",
                vol_rows.len(),
                expiries.len()
            )));
        }

        let mut flat = Vec::with_capacity(expiries.len() * strikes.len());
        for (i, row) in vol_rows.iter().enumerate() {
            if row.len() != strikes.len() {
                return Err(Error::Validation(format!(
                    "vol_rows[{i}] has {} entries but strikes has {}",
                    row.len(),
                    strikes.len()
                )));
            }
            flat.extend_from_slice(row);
        }

        Self::from_grid(id, expiries, strikes, &flat)
    }
}

impl VolSurface {
    /// Construct a volatility surface from SABR parameters evaluated on a grid.
    ///
    /// This is a convenience constructor that evaluates the Hagan (2002) SABR
    /// approximation at each (expiry, strike) point to create a grid surface.
    ///
    /// # Arguments
    ///
    /// * `id` - Surface identifier
    /// * `forward` - Forward rate (assumed constant across expiries for simplicity)
    /// * `params` - SABR parameters (alpha, beta, rho, nu)
    /// * `expiries` - Expiry times in years
    /// * `strikes` - Strike rates
    ///
    /// # Returns
    ///
    /// A new `VolSurface` with implied vols from SABR at each grid point.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_core::market_data::surfaces::VolSurface;
    /// use finstack_core::math::volatility::sabr::SabrParams;
    ///
    /// let params = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    /// let surface = VolSurface::from_sabr(
    ///     "SABR-USD-5Y",
    ///     0.05,
    ///     &params,
    ///     &[0.5, 1.0, 2.0, 5.0],
    ///     &[0.03, 0.04, 0.05, 0.06, 0.07],
    /// ).expect("SABR surface should build");
    /// ```
    pub fn from_sabr(
        id: impl Into<CurveId>,
        forward: f64,
        params: &crate::math::volatility::sabr::SabrParams,
        expiries: &[f64],
        strikes: &[f64],
    ) -> crate::Result<Self> {
        let id: CurveId = id.into();
        let mut builder = VolSurface::builder(id.clone())
            .expiries(expiries)
            .strikes(strikes);

        let mut floored = 0usize;
        for &t in expiries {
            let row: Vec<f64> = strikes
                .iter()
                .map(|&k| {
                    let vol = params.implied_vol_lognormal(forward, k, t);
                    // Floor for invalid params: use a small positive vol instead of
                    // NaN/negative which would fail builder validation.
                    super::floor_sabr_vol(vol, &mut floored)
                })
                .collect();
            builder = builder.row(&row);
        }
        super::warn_sabr_vol_floored("VolSurface::from_sabr", &id, floored);

        builder.build()
    }
}

fn validate_axis(axis: &[f64]) -> crate::Result<()> {
    if axis.is_empty() {
        return Err(InputError::TooFewPoints.into());
    }
    if axis.iter().any(|x| !x.is_finite()) {
        return Err(InputError::Invalid.into());
    }
    // Allow singleton axes (e.g., a 1xN “surface”) for clamped evaluation.
    if axis.len() > 1 {
        crate::math::interp::utils::validate_knots(axis)?;
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn flat_surface() -> VolSurface {
        VolSurface::builder("EQ-FLAT")
            .expiries(&[1.0, 2.0])
            .strikes(&[90.0, 100.0, 110.0])
            .row(&[0.2, 0.2, 0.2])
            .row(&[0.2, 0.2, 0.2])
            .build()
            .expect("VolSurface builder should succeed in test")
    }

    #[test]
    fn flat_returns_constant() {
        let vs = flat_surface();
        // Use value_checked (recommended primary API)
        assert!(
            (vs.value_checked(1.5, 95.0)
                .expect("Value lookup should succeed in test")
                - 0.2)
                .abs()
                < 1e-12
        );
        // clamped path (below min strike/expiry uses flat extrapolation)
        assert!((vs.value_clamped(0.5, 80.0) - 0.2).abs() < 1e-12);
    }

    #[test]
    fn oob_checked_errors() {
        let vs = flat_surface();
        assert!(vs.value_checked(0.5, 95.0).is_err());
        assert!(vs.value_checked(1.5, 50.0).is_err());
    }

    #[test]
    fn from_sabr_constructs_valid_surface() {
        use crate::math::volatility::sabr::SabrParams;

        let params = SabrParams::new(0.035, 0.5, -0.2, 0.4).expect("valid SABR params");
        let expiries = [0.5, 1.0, 2.0, 5.0];
        let strikes = [0.03, 0.04, 0.05, 0.06, 0.07];

        let surface = VolSurface::from_sabr("SABR-TEST", 0.05, &params, &expiries, &strikes)
            .expect("from_sabr should build a valid surface");

        // Grid shape should match inputs
        assert_eq!(surface.grid_shape(), (4, 5));

        // All vols should be positive and finite
        for &t in &expiries {
            for &k in &strikes {
                let vol = surface
                    .value_checked(t, k)
                    .expect("grid point should be in bounds");
                assert!(vol > 0.0, "vol({t}, {k}) = {vol} should be positive");
                assert!(vol.is_finite(), "vol({t}, {k}) = {vol} should be finite");
            }
        }

        // ATM vol should be reasonable (order of magnitude check)
        let atm_vol = surface
            .value_checked(1.0, 0.05)
            .expect("ATM lookup should succeed");
        assert!(
            atm_vol > 0.01 && atm_vol < 2.0,
            "ATM vol {atm_vol} should be in reasonable range"
        );

        // With negative rho, expect left skew: low-strike vol > ATM vol
        let low_strike_vol = surface
            .value_checked(1.0, 0.03)
            .expect("low strike lookup should succeed");
        assert!(
            low_strike_vol > atm_vol,
            "Expected left skew: vol(K=3%) = {low_strike_vol:.4} should be > vol(ATM) = {atm_vol:.4}"
        );
    }

    fn smile_surface() -> VolSurface {
        VolSurface::builder("EQ-SMILE")
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[80.0, 85.0, 90.0, 95.0, 100.0, 105.0, 110.0, 115.0, 120.0])
            .row(&[0.32, 0.29, 0.26, 0.23, 0.20, 0.21, 0.23, 0.26, 0.30])
            .row(&[0.30, 0.27, 0.24, 0.21, 0.19, 0.20, 0.22, 0.25, 0.28])
            .row(&[0.28, 0.25, 0.22, 0.20, 0.18, 0.19, 0.21, 0.23, 0.26])
            .build()
            .expect("VolSurface builder should succeed in test")
    }

    #[test]
    fn extrapolated_in_bounds_matches_checked() {
        let vs = smile_surface();
        let forward = 100.0;

        // In-bounds query should match value_checked
        let checked = vs
            .value_checked(1.0, 100.0)
            .expect("in-bounds should succeed");
        let extrap = vs.value_extrapolated(1.0, 100.0, forward);
        assert!(
            (checked - extrap).abs() < 1e-12,
            "In-bounds: checked={checked}, extrapolated={extrap}"
        );
    }

    #[test]
    fn extrapolated_deep_otm_returns_positive_vol() {
        let vs = smile_surface();
        let forward = 100.0;

        // Deep OTM put (strike far below grid)
        let vol_low = vs.value_extrapolated(1.0, 50.0, forward);
        assert!(
            vol_low > 0.0 && vol_low.is_finite(),
            "Deep OTM low strike vol should be positive: {vol_low}"
        );

        // Deep OTM call (strike far above grid)
        let vol_high = vs.value_extrapolated(1.0, 160.0, forward);
        assert!(
            vol_high > 0.0 && vol_high.is_finite(),
            "Deep OTM high strike vol should be positive: {vol_high}"
        );
    }

    #[test]
    fn extrapolated_wings_higher_than_atm() {
        let vs = smile_surface();
        let forward = 100.0;

        let atm_vol = vs.value_extrapolated(1.0, 100.0, forward);
        let wing_low = vs.value_extrapolated(1.0, 50.0, forward);
        let wing_high = vs.value_extrapolated(1.0, 160.0, forward);

        // Wings should be higher than ATM for a typical smile
        assert!(
            wing_low > atm_vol,
            "Low wing vol {wing_low:.4} should exceed ATM {atm_vol:.4}"
        );
        assert!(
            wing_high > atm_vol,
            "High wing vol {wing_high:.4} should exceed ATM {atm_vol:.4}"
        );
    }

    #[test]
    fn extrapolated_expiry_out_of_bounds() {
        let vs = smile_surface();
        let forward = 100.0;

        // Expiry below grid (0.5 is min), strike in bounds
        let vol = vs.value_extrapolated(0.1, 100.0, forward);
        assert!(
            vol > 0.0 && vol.is_finite(),
            "Extrapolated for short expiry should be valid: {vol}"
        );
    }

    #[test]
    fn extrapolated_invalid_forward_surfaces_svi_failure() {
        let vs = smile_surface();
        let vol = vs.value_extrapolated(1.0, 50.0, 0.0);
        assert!(
            vol.is_nan(),
            "invalid forward should not silently clamp a failed SVI fit"
        );
    }

    #[test]
    fn builder_validation_errors() {
        // Unsorted expiries
        let bad = VolSurface::builder("BAD")
            .expiries(&[2.0, 1.0])
            .strikes(&[90.0, 100.0])
            .row(&[0.2, 0.2])
            .row(&[0.2, 0.2])
            .build();
        assert!(bad.is_err());

        // Mismatched row length
        let bad2 = VolSurface::builder("BAD2")
            .expiries(&[1.0, 2.0])
            .strikes(&[90.0, 100.0])
            .row(&[0.2])
            .row(&[0.2, 0.2])
            .build();
        assert!(bad2.is_err());
    }

    #[test]
    fn quote_type_serde_round_trips_and_defaults_to_black() {
        // Round trip: an explicit Normal tag must survive serde.
        let normal = flat_surface().with_quote_type(VolQuoteType::Normal);
        let json = serde_json::to_string(&normal).expect("serialize");
        let back: VolSurface = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.quote_type(), VolQuoteType::Normal);

        // Back-compat: older payloads without the field default to Black/lognormal.
        let legacy = r#"{
            "id": "EQ-LEGACY",
            "expiries": [1.0, 2.0],
            "strikes": [90.0, 100.0],
            "vols_row_major": [0.2, 0.2, 0.2, 0.2]
        }"#;
        let surface: VolSurface = serde_json::from_str(legacy).expect("legacy deserialize");
        assert_eq!(surface.quote_type(), VolQuoteType::BlackLognormal);
    }

    #[test]
    fn require_quote_type_enforces_convention() {
        let vs = flat_surface();
        assert!(vs.require_quote_type(VolQuoteType::BlackLognormal).is_ok());
        let err = vs
            .require_quote_type(VolQuoteType::Normal)
            .expect_err("mismatched quote type must error");
        assert!(format!("{err}").contains("quote"), "got: {err}");
    }

    #[test]
    fn metadata_contracts_survive_bumps_and_scaling() {
        let vs = flat_surface()
            .with_quote_type(VolQuoteType::Normal)
            .with_secondary_axis(VolSurfaceAxis::Tenor)
            .with_interpolation_mode(VolInterpolationMode::TotalVariance);

        let bumped = vs.bump_point(1.5, 100.0, 0.01).expect("bump");
        assert_eq!(bumped.quote_type(), VolQuoteType::Normal);
        assert_eq!(bumped.secondary_axis(), VolSurfaceAxis::Tenor);
        assert_eq!(
            bumped.interpolation_mode(),
            VolInterpolationMode::TotalVariance
        );

        let scaled = vs.scaled(1.1);
        assert_eq!(scaled.quote_type(), VolQuoteType::Normal);

        let bucket = vs
            .apply_bucket_bump(None, None, 1.0)
            .expect("bucket bump should succeed");
        assert_eq!(bucket.quote_type(), VolQuoteType::Normal);
        assert_eq!(bucket.secondary_axis(), VolSurfaceAxis::Tenor);
    }

    #[test]
    fn total_variance_interpolation_differs_from_direct_vol_interpolation() {
        let surface = VolSurface::builder("EQ-TV")
            .expiries(&[1.0, 2.0])
            .strikes(&[100.0, 110.0])
            .interpolation_mode(VolInterpolationMode::TotalVariance)
            .row(&[0.20, 0.20])
            .row(&[0.30, 0.30])
            .build()
            .expect("surface should build");

        let interpolated = surface
            .value_checked(1.5, 100.0)
            .expect("interpolated lookup should succeed");
        let expected = ((0.5 * (1.0 * 0.20_f64.powi(2) + 2.0 * 0.30_f64.powi(2))) / 1.5).sqrt();

        assert!(
            (interpolated - expected).abs() < 1e-12,
            "total-variance interpolation should match expected value: expected {}, got {}",
            expected,
            interpolated
        );
        assert!(
            (interpolated - 0.25).abs() > 1e-6,
            "total-variance interpolation should not collapse to direct vol interpolation"
        );
    }
}
