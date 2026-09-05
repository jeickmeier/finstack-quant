//! Serializable implied-volatility surface artifact.
//!
//! Represents market-implied volatility as a function of option maturity and
//! strike. The artifact stores the observed grid, quote convention, axis
//! meaning, interpolation metadata, and bump state. Computational evaluation,
//! fitting, and extrapolation live in `finstack-quant-models`.
//!
//! # Financial Context
//!
//! Volatility surfaces capture the volatility smile/skew observed in options
//! markets. The surface shape reflects market views on:
//! - **Strike dimension**: Implied probability distribution (skew)
//! - **Maturity dimension**: Term structure of volatility
//! - **Surface dynamics**: Sticky strike vs sticky delta behavior
//!
//! # Interpolation Metadata
//!
//! [`VolInterpolationMode`] records whether downstream model evaluation should
//! interpolate volatility or total variance. Core validates and preserves the
//! metadata but does not execute the interpolation.
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::market_data::surfaces::VolSurface;
//! use finstack_quant_core::types::CurveId;
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
//! assert_eq!(surface.grid_shape(), (2, 3));
//! assert_eq!(surface.vols()[1], 0.21);
//! ```

// Box and Vec are available from the standard prelude; no explicit alloc import needed.

use crate::{
    error::InputError,
    market_data::bumps::{BumpSpec, Bumpable},
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

impl std::str::FromStr for VolQuoteType {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "black_lognormal" => Ok(Self::BlackLognormal),
            "normal" => Ok(Self::Normal),
            _ => Err(format!(
                "unknown volatility quote type {value:?}; expected black_lognormal or normal"
            )),
        }
    }
}

/// Interpolation contract for vol surfaces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    /// [`VolSurface::with_interpolation_mode`]. Use this default when
    /// quote-space fidelity at the pillars is the governing convention.
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "VolSurfaceWire", into = "VolSurfaceWire")]
#[cfg_attr(feature = "json-schema", schemars(try_from = "VolSurfaceWire"))]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct VolSurfaceWire {
    /// Surface identifier
    pub id: String,
    /// Expiry times in years
    pub expiries: Vec<f64>,
    /// Strike prices
    pub strikes: Vec<f64>,
    /// Semantic meaning of the secondary axis.
    pub secondary_axis: VolSurfaceAxis,
    /// Quote convention.
    pub quote_type: VolQuoteType,
    /// Interpolation contract.
    pub interpolation_mode: VolInterpolationMode,
    /// Volatility values in row-major order
    pub vols_row_major: Vec<f64>,
}

impl From<VolSurface> for VolSurfaceWire {
    fn from(surface: VolSurface) -> Self {
        VolSurfaceWire {
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

impl TryFrom<VolSurfaceWire> for VolSurface {
    type Error = crate::Error;

    fn try_from(state: VolSurfaceWire) -> crate::Result<Self> {
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
    /// Start building a new observed volatility surface.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable identifier used for market-context lookup and
    ///   serialization.
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

    /// Return the observed volatility grid in expiry-major row order.
    pub fn vols(&self) -> &[f64] {
        &self.vols
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
    ///
    /// # Arguments
    ///
    /// * `interpolation_mode` - Interpolation policy used between volatility grid nodes.
    #[must_use]
    pub fn with_interpolation_mode(mut self, interpolation_mode: VolInterpolationMode) -> Self {
        self.interpolation_mode = interpolation_mode;
        self
    }

    /// Require the semantic axis before a caller uses the surface.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when this surface's secondary axis differs
    /// from `expected`.
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
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when this surface's quote convention differs
    /// from `expected`.
    ///
    /// # Arguments
    ///
    /// * `expected` - Expected dimension or reference value used for validation.
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
    /// use finstack_quant_core::market_data::surfaces::VolSurface;
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
    /// assert!(bumped.vols()[1] > surface.vols()[1]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn bump_point(&self, expiry: f64, strike: f64, bump_pct: f64) -> crate::Result<Self> {
        if !expiry.is_finite() || !strike.is_finite() || !bump_pct.is_finite() {
            return Err(InputError::Invalid.into());
        }
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
    /// Add an absolute bump to one grid point in place.
    ///
    /// Coordinates map to the nearest clamped grid node. The updated
    /// volatility is floored at zero, and the original value is returned for
    /// reversal with [`Self::unbump_point_in_place`].
    ///
    /// # Arguments
    ///
    /// * `expiry` - Expiry in years used to select the nearest grid row.
    /// * `strike` - Strike coordinate used to select the nearest grid column.
    /// * `bump_abs` - Absolute volatility change in decimal units; `0.01` is
    ///   one volatility point.
    pub fn bump_point_absolute_in_place(
        &mut self,
        expiry: f64,
        strike: f64,
        bump_abs: f64,
    ) -> crate::Result<f64> {
        if !expiry.is_finite() || !strike.is_finite() || !bump_abs.is_finite() {
            return Err(crate::Error::Validation(format!(
                "absolute volatility point bump must be finite, got {bump_abs}"
            )));
        }
        let (Some(&exp_min), Some(&exp_max)) = (self.expiries.first(), self.expiries.last()) else {
            return Err(crate::error::InputError::TooFewPoints.into());
        };
        let (Some(&str_min), Some(&str_max)) = (self.strikes.first(), self.strikes.last()) else {
            return Err(crate::error::InputError::TooFewPoints.into());
        };
        let expiry_idx =
            find_closest_grid_index(self.expiries.as_ref(), expiry.clamp(exp_min, exp_max));
        let strike_idx =
            find_closest_grid_index(self.strikes.as_ref(), strike.clamp(str_min, str_max));
        let idx = expiry_idx * self.strikes.len() + strike_idx;
        let original = self.vols[idx];
        let bumped = original + bump_abs;
        if !bumped.is_finite() {
            return Err(InputError::Invalid.into());
        }
        self.vols[idx] = bumped.max(0.0);
        Ok(original)
    }

    /// Restore a grid point to a previously saved vol value.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Option expiry date or year-fraction used to locate the volatility point
    /// * `strike` - Option strike in the surface's quote units (absolute or relative)
    /// * `original_vol` - Original annualized volatility restored after the temporary bump.
    ///
    /// # Errors
    ///
    /// Rejects non-finite coordinates or a non-finite/negative restored value
    /// without modifying the surface.
    pub fn unbump_point_in_place(
        &mut self,
        expiry: f64,
        strike: f64,
        original_vol: f64,
    ) -> crate::Result<()> {
        if !expiry.is_finite()
            || !strike.is_finite()
            || !original_vol.is_finite()
            || original_vol < 0.0
        {
            return Err(InputError::Invalid.into());
        }
        let clamped_expiry = match (self.expiries.first(), self.expiries.last()) {
            (Some(&min), Some(&max)) => expiry.clamp(min, max),
            _ => return Err(InputError::TooFewPoints.into()),
        };
        let clamped_strike = match (self.strikes.first(), self.strikes.last()) {
            (Some(&min), Some(&max)) => strike.clamp(min, max),
            _ => return Err(InputError::TooFewPoints.into()),
        };

        let expiry_idx = find_closest_grid_index(self.expiries.as_ref(), clamped_expiry);
        let strike_idx = find_closest_grid_index(self.strikes.as_ref(), clamped_strike);

        let n_strikes = self.strikes.len();
        self.vols[expiry_idx * n_strikes + strike_idx] = original_vol;
        Ok(())
    }

    /// Return a new volatility surface scaled uniformly by `scale`.
    ///
    /// This creates a copy of the surface with the same identifier and grid,
    /// multiplying every volatility by `scale`. It avoids the overhead of
    /// serializing to a row-major buffer and rebuilding via `from_grid`.
    ///
    /// For greek bumps that apply a uniform percentage change to the entire
    /// surface, prefer this method over `to_state()`/`from_grid()`.
    ///
    /// # Arguments
    ///
    /// * `scale` - Finite nonnegative multiplier applied to each annualized
    ///   volatility. A value of 1.01 increases all volatilities by 1%.
    ///
    /// # Errors
    ///
    /// Rejects a negative/non-finite multiplier or non-finite scaled values.
    pub fn scaled(&self, scale: f64) -> crate::Result<Self> {
        if !scale.is_finite() || scale < 0.0 {
            return Err(InputError::Invalid.into());
        }
        // Fast path: return an identical copy when scale == 1.0
        if scale.total_cmp(&1.0).is_eq() {
            return Ok(Self {
                id: self.id.clone(),
                expiries: self.expiries.clone(),
                strikes: self.strikes.clone(),
                secondary_axis: self.secondary_axis,
                quote_type: self.quote_type,
                interpolation_mode: self.interpolation_mode,
                vols: self.vols.clone(),
            });
        }

        // Scale all vols
        let scaled_vols = self.vols.iter().map(|&v| v * scale).collect::<Box<[f64]>>();
        if scaled_vols.iter().any(|v| !v.is_finite()) {
            return Err(InputError::Invalid.into());
        }

        Ok(Self {
            id: self.id.clone(),
            expiries: self.expiries.clone(),
            strikes: self.strikes.clone(),
            secondary_axis: self.secondary_axis,
            quote_type: self.quote_type,
            interpolation_mode: self.interpolation_mode,
            vols: scaled_vols,
        })
    }
}

impl Bumpable for VolSurface {
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        use crate::error::InputError;

        spec.validate_finite()?;
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

        if is_multiplicative {
            return self.scaled(raw_val);
        }
        let bumped_vols: Box<[f64]> = self.vols.iter().map(|&v| (v + raw_val).max(0.0)).collect();
        if bumped_vols.iter().any(|v| !v.is_finite()) {
            return Err(InputError::Invalid.into());
        }

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
    /// tolerance of `BUCKET_BUMP_MATCH_TOLERANCE` absorbs floating-point
    /// round-off only); values that fall between grid points match nothing.
    pub fn apply_bucket_bump(
        &self,
        expiries_filter: Option<&[f64]>,
        strikes_filter: Option<&[f64]>,
        pct: f64,
    ) -> Option<Self> {
        if !pct.is_finite()
            || expiries_filter.is_some_and(|values| values.iter().any(|v| !v.is_finite()))
            || strikes_filter.is_some_and(|values| values.iter().any(|v| !v.is_finite()))
        {
            return None;
        }
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

fn find_closest_grid_index(values: &[f64], target: f64) -> usize {
    values
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| (*left - target).abs().total_cmp(&(*right - target).abs()))
        .map_or(0, |(index, _)| index)
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
    ///
    /// # Errors
    ///
    /// Returns `InputError::DimensionMismatch` when the number of rows does
    /// not match expiries or a row does not match strikes. It also propagates
    /// axis and volatility validation failures from [`VolSurface::from_grid_opts`].
    pub fn build(self) -> crate::Result<VolSurface> {
        if self.vols.len() != self.expiries.len() {
            return Err(InputError::DimensionMismatch.into());
        }
        for row in &self.vols {
            if row.len() != self.strikes.len() {
                return Err(InputError::DimensionMismatch.into());
            }
        }
        let flat: Vec<f64> = self.vols.into_iter().flatten().collect();
        VolSurface::from_grid_opts(
            self.id.as_str(),
            &self.expiries,
            &self.strikes,
            &flat,
            VolGridOpts {
                secondary_axis: self.secondary_axis,
                quote_type: self.quote_type,
                interpolation_mode: self.interpolation_mode,
            },
        )
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
    ///
    /// `vols_row_major[expiry_idx * strikes.len() + secondary_idx]` supplies
    /// the value for each grid point. Expiries and secondary-axis coordinates
    /// must be strictly increasing and finite.
    ///
    /// # Errors
    ///
    /// Returns `InputError::TooFewPoints` for an empty axis,
    /// `InputError::DimensionMismatch` when the flat grid has the wrong size,
    /// an input-validation error for non-monotone or non-finite axes, and an
    /// error for non-finite or negative volatility values.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable string identifier used for lookup and serialization of this object
    /// * `expiries` - Strictly increasing option expiries in year-fraction units.
    /// * `strikes` - Strictly ordered strike coordinates in underlying price units.
    /// * `vols_row_major` - Annualized decimal volatilities flattened in expiry-major row order.
    /// * `opts` - Options controlling validation, interpolation, or execution behavior.
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
    ///
    /// # Errors
    ///
    /// Propagates the grid-shape, axis, and volatility validation errors from
    /// [`Self::from_grid_opts`].
    ///
    /// # Arguments
    ///
    /// * `id` - Stable string identifier used for lookup and serialization of this object
    /// * `expiries` - Strictly increasing option expiries in year-fraction units.
    /// * `strikes` - Strictly ordered strike coordinates in underlying price units.
    /// * `vols_row_major` - Annualized decimal volatilities flattened in expiry-major row order.
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
