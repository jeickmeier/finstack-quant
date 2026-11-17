//! Serializable implied-volatility surface artifact.
//!
//! Represents market-implied volatility as a function of option maturity and
//! a secondary coordinate (strike or underlying tenor). The artifact stores the
//! observed grid, quote convention, axis meaning, interpolation metadata, and
//! optional expiry clock. Computational evaluation, fitting, and extrapolation
//! live in `finstack-quant-models`.
//!
//! # Financial Context
//!
//! Volatility surfaces capture the volatility smile/skew observed in options
//! markets. The surface shape reflects market views on:
//! - **Secondary dimension**: Strike or underlying tenor
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
//! use finstack_quant_core::market_data::surfaces::{
//!     VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
//! };
//! use finstack_quant_core::types::CurveId;
//!
//! let surface = VolSurface::builder("EQ-FLAT")
//!     .expiries(&[1.0, 2.0])
//!     .secondary_values(&[90.0, 100.0, 110.0])
//!     .secondary_axis(VolSurfaceAxis::Strike)
//!     .quote_type(VolQuoteType::BlackLognormal)
//!     .interpolation_mode(VolInterpolationMode::Vol)
//!     .row(&[0.2, 0.21, 0.22])
//!     .row(&[0.19, 0.2, 0.21])
//!     .build()
//!     .expect("VolSurface builder should succeed");
//! assert_eq!(surface.id(), &CurveId::from("EQ-FLAT"));
//!
//! assert_eq!(surface.grid_shape(), (2, 3));
//! assert_eq!(surface.vols()[1], 0.21);
//! ```

use crate::dates::VolatilityClock;
use crate::market_data::bumps::{BumpMode, BumpType, BumpUnits};
use crate::{
    Error,
    error::InputError,
    market_data::bumps::{BumpSpec, Bumpable},
    types::CurveId,
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
pub enum VolSurfaceAxis {
    /// The secondary axis is literal absolute strike in underlying units.
    Strike,
    /// The secondary axis is strictly positive swap or reference tenor in years.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum VolQuoteType {
    /// Black (lognormal) implied volatility in relative units per square-root source year.
    BlackLognormal,
    /// Displaced Black volatility; one per-expiry displacement accompanies the grid.
    ShiftedBlackLognormal,
    /// Normal (Bachelier) implied volatility in underlying units per square-root source year.
    Normal,
}

impl std::fmt::Display for VolQuoteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlackLognormal => write!(f, "black_lognormal"),
            Self::ShiftedBlackLognormal => write!(f, "shifted_black_lognormal"),
            Self::Normal => write!(f, "normal"),
        }
    }
}

impl std::str::FromStr for VolQuoteType {
    type Err = crate::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "black_lognormal" => Ok(Self::BlackLognormal),
            "shifted_black_lognormal" => Ok(Self::ShiftedBlackLognormal),
            "normal" => Ok(Self::Normal),
            _ => Err(Error::Validation(format!(
                "unknown volatility quote type {value:?}; expected black_lognormal, shifted_black_lognormal or normal"
            ))),
        }
    }
}

/// Interpolation contract for vol surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum VolInterpolationMode {
    /// Interpolate implied volatility directly.
    ///
    /// This is the literal choice when market quotes are given as implied
    /// volatilities on the stored grid and you want local interpolation in
    /// quote space.
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
    /// [`VolSurfaceBuilder::interpolation_mode`].
    Vol,
    /// Interpolate total variance `sigma^2 * t`, then convert back to implied vol.
    ///
    /// Preferred when blending across expiries: total variance behaves more
    /// linearly in time and preserves the no-arbitrage intuition for variance
    /// accumulation, avoiding the calendar-arbitrage trap of linear-in-vol
    /// interpolation (see [`Vol`](Self::Vol)).
    TotalVariance,
}

/// Volatility surface defined on an expiry × secondary-coordinate grid.
///
/// Internally stores annual volatilities in row-major order as a boxed slice.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "VolSurfaceWire", into = "VolSurfaceWire")]
#[cfg_attr(feature = "json-schema", schemars(try_from = "VolSurfaceWire"))]
pub struct VolSurface {
    id: CurveId,
    expiries: Box<[f64]>,
    secondary_values: Box<[f64]>,
    secondary_axis: VolSurfaceAxis,
    quote_type: VolQuoteType,
    interpolation_mode: VolInterpolationMode,
    displacements: Option<Box<[f64]>>,
    expiry_clock: Option<VolatilityClock>,
    /// Row-major storage: vols[expiry_idx * n_secondary + secondary_idx]
    vols: Box<[f64]>,
}

/// Raw serializable state of a VolSurface.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct VolSurfaceWire {
    /// Surface identifier.
    pub id: String,
    /// Strictly positive option expiry times in years.
    pub expiries: Vec<f64>,
    /// Secondary-axis coordinates (absolute strikes or positive underlying tenors).
    pub secondary_values: Vec<f64>,
    /// Semantic meaning of the secondary axis.
    pub secondary_axis: VolSurfaceAxis,
    /// Quote convention.
    pub quote_type: VolQuoteType,
    /// Interpolation contract.
    pub interpolation_mode: VolInterpolationMode,
    /// Additive displacements in forward/strike units, one per expiry for shifted Black quotes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub displacements: Option<Vec<f64>>,
    /// Optional calibration clock mapping calendar dates to source expiry year fractions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry_clock: Option<VolatilityClock>,
    /// Volatility values in row-major order.
    pub vols_row_major: Vec<f64>,
}

impl From<VolSurface> for VolSurfaceWire {
    fn from(surface: VolSurface) -> Self {
        VolSurfaceWire {
            id: surface.id.to_string(),
            expiries: surface.expiries.to_vec(),
            secondary_values: surface.secondary_values.to_vec(),
            secondary_axis: surface.secondary_axis,
            quote_type: surface.quote_type,
            interpolation_mode: surface.interpolation_mode,
            displacements: surface.displacements.map(Vec::from),
            expiry_clock: surface.expiry_clock,
            vols_row_major: surface.vols.into_vec(),
        }
    }
}

impl TryFrom<VolSurfaceWire> for VolSurface {
    type Error = crate::Error;

    fn try_from(state: VolSurfaceWire) -> crate::Result<Self> {
        let mut surface = Self::from_grid(
            &state.id,
            &state.expiries,
            &state.secondary_values,
            &state.vols_row_major,
            VolGridOpts {
                secondary_axis: state.secondary_axis,
                quote_type: state.quote_type,
                interpolation_mode: state.interpolation_mode,
            },
            state.displacements.as_deref(),
        )?;
        if let Some(clock) = state.expiry_clock {
            surface = surface.with_expiry_clock(clock);
        }
        Ok(surface)
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
            secondary_values: Vec::new(),
            secondary_axis: None,
            quote_type: None,
            interpolation_mode: None,
            displacements: None,
            expiry_clock: None,
            vols: Vec::new(),
        }
    }

    /// Unique identifier of the surface.
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Returns the strictly positive expiry axis in source years.
    pub fn expiries(&self) -> &[f64] {
        &self.expiries
    }

    /// Returns the secondary-axis coordinates (absolute strikes or tenors).
    pub fn secondary_values(&self) -> &[f64] {
        &self.secondary_values
    }

    /// Return the observed volatility grid in expiry-major row order.
    pub fn vols(&self) -> &[f64] {
        &self.vols
    }

    /// Every grid node as `(expiry, secondary_value, vol)` in expiry-major row order.
    pub fn grid_points(&self) -> impl Iterator<Item = (f64, f64, f64)> + '_ {
        let n_secondary = self.secondary_values.len();
        self.vols.iter().enumerate().map(move |(index, &vol)| {
            (
                self.expiries[index / n_secondary],
                self.secondary_values[index % n_secondary],
                vol,
            )
        })
    }

    /// Semantic meaning of the secondary axis.
    pub fn secondary_axis(&self) -> VolSurfaceAxis {
        self.secondary_axis
    }

    /// Quoting convention of the stored volatilities.
    pub fn quote_type(&self) -> VolQuoteType {
        self.quote_type
    }

    /// Per-expiry additive displacements for shifted Black quotes, in the same
    /// units as the forward and strike. Unshifted and normal surfaces return `None`.
    pub fn displacements(&self) -> Option<&[f64]> {
        self.displacements.as_deref()
    }

    /// Optional expiry clock recording the calibration reference date and day count.
    #[must_use]
    pub const fn expiry_clock(&self) -> Option<&VolatilityClock> {
        self.expiry_clock.as_ref()
    }

    /// Attach or replace the optional calibration [`VolatilityClock`] on this surface.
    ///
    /// # Arguments
    ///
    /// * `clock` - Calibration anchor date, day-count convention, and calendar context
    ///   used to map option expiration dates onto the surface expiry axis.
    #[must_use]
    pub fn with_expiry_clock(mut self, clock: VolatilityClock) -> Self {
        self.expiry_clock = Some(clock);
        self
    }

    /// Interpolation contract used when evaluating between grid points.
    pub fn interpolation_mode(&self) -> VolInterpolationMode {
        self.interpolation_mode
    }

    /// Require the semantic axis before a caller uses the surface.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when this surface's secondary axis differs
    /// from `expected`.
    ///
    /// # Arguments
    ///
    /// * `expected` - Expected semantic meaning of the surface secondary axis.
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
    /// * `expected` - Expected quoting convention of the stored volatilities.
    pub fn require_quote_type(&self, expected: VolQuoteType) -> crate::Result<()> {
        if self.quote_type == expected {
            return Ok(());
        }

        Err(Error::Validation(format!(
            "Vol surface '{}' stores '{}' quotes but caller expected '{}'",
            self.id, self.quote_type, expected
        )))
    }

    /// Grid shape as `(n_expiries, n_secondary)`.
    pub fn grid_shape(&self) -> (usize, usize) {
        (self.expiries.len(), self.secondary_values.len())
    }

    /// Add a native additive shift in place to one exact stored grid node.
    ///
    /// Both `expiry` and `secondary_value` must match an existing stored
    /// grid coordinate exactly (no nearest-node mapping or clamping). Returns the
    /// original volatility quote.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Exact stored expiry coordinate in years.
    /// * `secondary_value` - Exact stored secondary-axis coordinate.
    /// * `bump_abs` - Finite additive shift in the surface's native annual
    ///   volatility quote units.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if any input is non-finite, `(expiry, secondary_value)`
    /// does not match a stored grid node, or the shocked volatility is non-finite or negative.
    pub fn bump_point_absolute_in_place(
        &mut self,
        expiry: f64,
        secondary_value: f64,
        bump_abs: f64,
    ) -> crate::Result<f64> {
        if !expiry.is_finite() || !secondary_value.is_finite() || !bump_abs.is_finite() {
            return Err(crate::Error::Validation(format!(
                "volatility point bump inputs must be finite, got expiry={expiry}, secondary={secondary_value}, bump_abs={bump_abs}"
            )));
        }
        let expiry_idx = self
            .expiries
            .iter()
            .position(|v| v.total_cmp(&expiry).is_eq())
            .ok_or_else(|| {
                Error::Validation(format!(
                    "volatility point ({expiry}, {secondary_value}) on '{}' does not match any stored expiry coordinate",
                    self.id
                ))
            })?;
        let secondary_idx = self
            .secondary_values
            .iter()
            .position(|v| v.total_cmp(&secondary_value).is_eq())
            .ok_or_else(|| {
                Error::Validation(format!(
                    "volatility point ({expiry}, {secondary_value}) on '{}' does not match any stored secondary coordinate",
                    self.id
                ))
            })?;
        let idx = expiry_idx * self.secondary_values.len() + secondary_idx;
        let original = self.vols[idx];
        if bump_abs == 0.0 {
            return Ok(original);
        }
        let bumped = original + bump_abs;
        if !bumped.is_finite() || bumped < 0.0 {
            return Err(crate::Error::Validation(format!(
                "bumped volatility must be finite and non-negative, got {bumped}"
            )));
        }
        self.vols[idx] = bumped;
        Ok(original)
    }

    /// Return a new volatility surface scaled uniformly by `scale`.
    ///
    /// Multiplies every volatility by `scale` while preserving ID, axes, quote
    /// convention, displacements, interpolation mode, and expiry clock. Zero `scale`
    /// intentionally zeroes quotes; positive quotes that underflow to zero under a
    /// strictly positive factor are rejected.
    ///
    /// # Arguments
    ///
    /// * `scale` - Finite non-negative multiplier applied to each stored
    ///   annual volatility. `1.01` increases quotes by 1%.
    ///
    /// # Errors
    ///
    /// Rejects a negative or non-finite multiplier, non-finite scaled values, or
    /// positive quotes that underflow to zero under a strictly positive factor.
    pub fn scaled(&self, scale: f64) -> crate::Result<Self> {
        if !scale.is_finite() || scale < 0.0 {
            return Err(crate::Error::Validation(
                "volatility scale must be finite and non-negative".to_string(),
            ));
        }
        self.apply_bucket_bump(None, None, BumpSpec::multiplier(scale))
    }
}

#[derive(Clone, Copy)]
enum NativeSurfaceShock {
    Additive(f64),
    Multiplicative(f64),
}

fn resolve_native_surface_shock(spec: BumpSpec) -> crate::Result<NativeSurfaceShock> {
    spec.validate()?;
    if !matches!(spec.bump_type, BumpType::Parallel) {
        return Err(InputError::UnsupportedBump {
            reason: "VolSurface only supports Parallel bumps, not key-rate bumps".to_string(),
        }
        .into());
    }
    match (spec.mode, spec.units) {
        (BumpMode::Additive, BumpUnits::Fraction) => Ok(NativeSurfaceShock::Additive(spec.value)),
        (BumpMode::Multiplicative, BumpUnits::Factor) => {
            if !spec.value.is_finite() || spec.value < 0.0 {
                return Err(Error::Validation(format!(
                    "volatility scale factor must be finite and non-negative, got {}",
                    spec.value
                )));
            }
            Ok(NativeSurfaceShock::Multiplicative(spec.value))
        }
        _ => Err(InputError::UnsupportedBump {
            reason: format!(
                "VolSurface only supports Additive/Fraction or Multiplicative/Factor in native quote units, got {:?}/{:?}",
                spec.mode, spec.units
            ),
        }
        .into()),
    }
}

fn resolve_exact_axis_mask(
    axis: &[f64],
    filter: Option<&[f64]>,
    axis_label: &str,
    surface_id: &CurveId,
) -> crate::Result<Vec<bool>> {
    let Some(targets) = filter else {
        return Ok(vec![true; axis.len()]);
    };
    let mut selected = vec![false; axis.len()];
    for &target in targets {
        if !target.is_finite() {
            return Err(Error::Validation(format!(
                "bucket bump {axis_label} filter on '{surface_id}' must contain finite values, got {target}"
            )));
        }
        let idx = axis
            .iter()
            .position(|v| v.total_cmp(&target).is_eq())
            .ok_or_else(|| {
                Error::Validation(format!(
                    "bucket bump {axis_label} coordinate {target} on '{surface_id}' matches no stored grid coordinate"
                ))
            })?;
        if selected[idx] {
            return Err(Error::Validation(format!(
                "bucket bump {axis_label} filter on '{surface_id}' contains duplicate coordinate {target}"
            )));
        }
        selected[idx] = true;
    }
    Ok(selected)
}

impl Bumpable for VolSurface {
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        self.apply_bucket_bump(None, None, spec)
    }
}

impl VolSurface {
    /// Apply a checked native quote shock across the Cartesian product of exact
    /// expiry and secondary-coordinate sets.
    ///
    /// `None` selects the entire corresponding axis; `Some(&[])` is an intentional
    /// empty selection (validates the request and returns an exact clone). Every
    /// coordinate in a `Some(...)` slice must be finite, unique, and equal to an
    /// exact stored grid coordinate without tolerances or clamping.
    ///
    /// # Arguments
    ///
    /// * `expiries_filter` - Optional exact expiry coordinates in years; `None` selects all expiries.
    /// * `secondary_filter` - Optional exact secondary-axis coordinates; `None` selects all columns.
    /// * `spec` - Parallel native quote shock (`Additive`/`Fraction` or `Multiplicative`/`Factor` with factor `>= 0.0`).
    ///
    /// # Errors
    ///
    /// Returns an error when `spec` uses key-rate bumps, `RateBp`/`Percent` units,
    /// negative scale factors, non-finite inputs, duplicate or unmatched filter
    /// coordinates, negative shocked quotes, non-finite results, or positive-quote
    /// underflow to zero under a strictly positive multiplier.
    pub fn apply_bucket_bump(
        &self,
        expiries_filter: Option<&[f64]>,
        secondary_filter: Option<&[f64]>,
        spec: BumpSpec,
    ) -> crate::Result<Self> {
        let shock = resolve_native_surface_shock(spec)?;
        let exp_mask =
            resolve_exact_axis_mask(&self.expiries, expiries_filter, "expiry", &self.id)?;
        let sec_mask = resolve_exact_axis_mask(
            &self.secondary_values,
            secondary_filter,
            "secondary",
            &self.id,
        )?;

        if exp_mask.iter().all(|&m| !m) || sec_mask.iter().all(|&m| !m) {
            return Ok(self.clone());
        }
        match shock {
            NativeSurfaceShock::Additive(0.0) => return Ok(self.clone()),
            NativeSurfaceShock::Multiplicative(factor) if factor.total_cmp(&1.0).is_eq() => {
                return Ok(self.clone());
            }
            _ => {}
        }

        let (n_expiries, n_secondary) = self.grid_shape();
        let mut next_vols = self.vols.to_vec();
        for (ei, _) in self.expiries.iter().enumerate().take(n_expiries) {
            if !exp_mask[ei] {
                continue;
            }
            for (si, _) in self.secondary_values.iter().enumerate().take(n_secondary) {
                if !sec_mask[si] {
                    continue;
                }
                let index = ei * n_secondary + si;
                let prev = self.vols[index];
                let next = match shock {
                    NativeSurfaceShock::Additive(delta) => prev + delta,
                    NativeSurfaceShock::Multiplicative(factor) => {
                        let scaled = prev * factor;
                        if prev > 0.0 && factor > 0.0 && scaled == 0.0 {
                            return Err(Error::Validation(format!(
                                "multiplicative volatility shock by {factor} on '{}' underflowed positive quote {prev} to zero",
                                self.id
                            )));
                        }
                        scaled
                    }
                };
                if !next.is_finite() || next < 0.0 {
                    return Err(Error::Validation(format!(
                        "bumped volatility must be finite and non-negative, got {next}"
                    )));
                }
                next_vols[index] = next;
            }
        }

        let mut bumped = self.clone();
        bumped.vols = next_vols.into_boxed_slice();
        Ok(bumped)
    }
}

/// Fluent builder for [`VolSurface`].
///
/// Callers must specify `secondary_axis`, `quote_type`, and `interpolation_mode`
/// explicitly before calling [`VolSurfaceBuilder::build`].
pub struct VolSurfaceBuilder {
    id: CurveId,
    expiries: Vec<f64>,
    secondary_values: Vec<f64>,
    secondary_axis: Option<VolSurfaceAxis>,
    quote_type: Option<VolQuoteType>,
    interpolation_mode: Option<VolInterpolationMode>,
    displacements: Option<Vec<f64>>,
    expiry_clock: Option<VolatilityClock>,
    vols: Vec<Vec<f64>>, // row-major expiries
}

impl VolSurfaceBuilder {
    /// Attach one additive displaced-Black shift to each expiry row.
    ///
    /// Does not alter `quote_type`; callers using shifted quotes must still set
    /// [`VolQuoteType::ShiftedBlackLognormal`] explicitly.
    ///
    /// # Arguments
    ///
    /// * `displacements` - Finite shifts in forward/strike units, one per expiry.
    pub fn displacements(mut self, displacements: &[f64]) -> Self {
        self.displacements = Some(displacements.to_vec());
        self
    }

    /// Append strictly positive option expiry pillars in years.
    ///
    /// # Arguments
    ///
    /// * `exps` - Strictly increasing finite positive expiry coordinates in years.
    pub fn expiries(mut self, exps: &[f64]) -> Self {
        self.expiries.extend_from_slice(exps);
        self
    }

    /// Append secondary-axis coordinates (absolute strikes or underlying tenors).
    ///
    /// # Arguments
    ///
    /// * `values` - Strictly increasing finite secondary coordinates matching `secondary_axis`.
    pub fn secondary_values(mut self, values: &[f64]) -> Self {
        self.secondary_values.extend_from_slice(values);
        self
    }

    /// Set the semantic meaning of the secondary axis.
    ///
    /// # Arguments
    ///
    /// * `axis` - Whether secondary coordinates represent absolute strikes or positive tenors.
    pub fn secondary_axis(mut self, axis: VolSurfaceAxis) -> Self {
        self.secondary_axis = Some(axis);
        self
    }

    /// Set the quoting convention of the stored volatilities.
    ///
    /// # Arguments
    ///
    /// * `quote_type` - Stored volatility convention (`BlackLognormal`, `ShiftedBlackLognormal`, or `Normal`).
    pub fn quote_type(mut self, quote_type: VolQuoteType) -> Self {
        self.quote_type = Some(quote_type);
        self
    }

    /// Set the interpolation contract used for off-grid evaluation.
    ///
    /// # Arguments
    ///
    /// * `mode` - Interpolation contract between grid nodes: direct volatility or total variance.
    pub fn interpolation_mode(mut self, mode: VolInterpolationMode) -> Self {
        self.interpolation_mode = Some(mode);
        self
    }

    /// Attach an optional calibration clock mapping expiry dates to source year fractions.
    ///
    /// # Arguments
    ///
    /// * `clock` - Calibration anchor date, day-count convention, and calendar context.
    pub fn expiry_clock(mut self, clock: VolatilityClock) -> Self {
        self.expiry_clock = Some(clock);
        self
    }

    /// Append a row of annual volatilities matching the configured secondary axis.
    ///
    /// # Arguments
    ///
    /// * `row` - Non-negative finite volatility quotes for the next expiry row.
    pub fn row(mut self, row: &[f64]) -> Self {
        self.vols.push(row.to_vec());
        self
    }

    /// Append multiple rows of annual volatilities in expiry order.
    ///
    /// # Arguments
    ///
    /// * `rows` - Volatility rows matching the expiry axis length and secondary axis width.
    pub fn rows(mut self, rows: &[Vec<f64>]) -> Self {
        self.vols.extend_from_slice(rows);
        self
    }

    /// Finalise the surface and return an immutable [`VolSurface`] instance.
    ///
    /// # Errors
    ///
    /// Returns a validation error when required metadata (`secondary_axis`,
    /// `quote_type`, or `interpolation_mode`) was omitted, when row/column
    /// counts mismatch, or when [`VolSurface::from_grid`] validation fails.
    pub fn build(self) -> crate::Result<VolSurface> {
        let secondary_axis = self.secondary_axis.ok_or_else(|| {
            Error::Validation("VolSurface builder requires explicit secondary_axis".to_string())
        })?;
        let quote_type = self.quote_type.ok_or_else(|| {
            Error::Validation("VolSurface builder requires explicit quote_type".to_string())
        })?;
        let interpolation_mode = self.interpolation_mode.ok_or_else(|| {
            Error::Validation("VolSurface builder requires explicit interpolation_mode".to_string())
        })?;
        let mut surface = VolSurface::from_rows_opts(
            self.id.as_str(),
            &self.expiries,
            &self.secondary_values,
            &self.vols,
            VolGridOpts {
                secondary_axis,
                quote_type,
                interpolation_mode,
            },
            self.displacements.as_deref(),
        )?;
        if let Some(clock) = self.expiry_clock {
            surface = surface.with_expiry_clock(clock);
        }
        Ok(surface)
    }
}

/// Options bundle for [`VolSurface::from_grid`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolGridOpts {
    /// Semantic meaning of the secondary axis (strike vs tenor).
    pub secondary_axis: VolSurfaceAxis,
    /// Quoting convention of the stored volatilities.
    pub quote_type: VolQuoteType,
    /// Interpolation contract (direct vol vs total-variance).
    pub interpolation_mode: VolInterpolationMode,
}

impl VolSurface {
    /// Canonical grid constructor requiring explicit grid conventions.
    ///
    /// `vols_row_major[expiry_idx * secondary_values.len() + secondary_idx]`
    /// supplies each quote. Expiries must be strictly positive, finite, and
    /// strictly increasing.
    ///
    /// # Errors
    ///
    /// Returns `InputError::TooFewPoints` for an empty axis,
    /// `InputError::DimensionMismatch` when the flat grid has the wrong size,
    /// an input-validation error for non-positive expiries or non-monotone/invalid
    /// secondary coordinates, and an error for non-finite or negative quotes.
    ///
    /// # Arguments
    ///
    /// * `id` - Market-data identifier stored on the surface and used for context lookup.
    /// * `expiries` - Strictly positive, strictly increasing option expiries in years.
    /// * `secondary_values` - Strictly increasing secondary coordinates matching `opts.secondary_axis`.
    /// * `vols_row_major` - Annualized volatilities flattened in expiry-major row order.
    /// * `opts` - Explicit secondary-axis meaning, quote type, and interpolation contract.
    /// * `displacements` - One finite additive shift per expiry for shifted Black quotes; must be absent for normal or unshifted Black quotes.
    pub fn from_grid(
        id: impl AsRef<str>,
        expiries: &[f64],
        secondary_values: &[f64],
        vols_row_major: &[f64],
        opts: VolGridOpts,
        displacements: Option<&[f64]>,
    ) -> crate::Result<Self> {
        validate_displacements(opts.quote_type, displacements, expiries.len())?;
        validate_grid(expiries, secondary_values, vols_row_major)?;
        validate_secondary_domain(
            opts.secondary_axis,
            opts.quote_type,
            secondary_values,
            displacements,
        )?;
        Ok(Self {
            id: CurveId::new(id.as_ref()),
            expiries: expiries.to_vec().into_boxed_slice(),
            secondary_values: secondary_values.to_vec().into_boxed_slice(),
            secondary_axis: opts.secondary_axis,
            quote_type: opts.quote_type,
            interpolation_mode: opts.interpolation_mode,
            displacements: displacements.map(Into::into),
            expiry_clock: None,
            vols: vols_row_major.to_vec().into_boxed_slice(),
        })
    }

    fn from_rows_opts(
        id: impl AsRef<str>,
        expiries: &[f64],
        secondary_values: &[f64],
        vol_rows: &[Vec<f64>],
        opts: VolGridOpts,
        displacements: Option<&[f64]>,
    ) -> crate::Result<Self> {
        if vol_rows.len() != expiries.len() {
            return Err(Error::Validation(format!(
                "vol_rows has {} rows but expiries has {} entries",
                vol_rows.len(),
                expiries.len()
            )));
        }

        for (i, row) in vol_rows.iter().enumerate() {
            if row.len() != secondary_values.len() {
                return Err(Error::Validation(format!(
                    "vol_rows[{i}] has {} entries but secondary_values has {}",
                    row.len(),
                    secondary_values.len()
                )));
            }
        }
        let size = expiries
            .len()
            .checked_mul(secondary_values.len())
            .ok_or(InputError::DimensionMismatch)?;
        let mut flat = Vec::with_capacity(size);
        for row in vol_rows {
            flat.extend_from_slice(row);
        }
        Self::from_grid(id, expiries, secondary_values, &flat, opts, displacements)
    }
}

pub(super) fn validate_displacements(
    quote_type: VolQuoteType,
    displacements: Option<&[f64]>,
    rows: usize,
) -> crate::Result<()> {
    match (quote_type, displacements) {
        (VolQuoteType::ShiftedBlackLognormal, Some(shifts))
            if shifts.len() == rows && shifts.iter().all(|s| s.is_finite()) =>
        {
            Ok(())
        }
        (VolQuoteType::ShiftedBlackLognormal, _) | (_, Some(_)) => Err(Error::Validation(
            "shifted Black quotes require one finite displacement per row; other conventions cannot carry shifts".into(),
        )),
        (_, None) => Ok(()),
    }
}

use super::validate_axis;

/// Validate axes, row-major dimensions and annualized volatility quotes.
pub(super) fn validate_grid(
    expiries: &[f64],
    secondary_values: &[f64],
    vols_row_major: &[f64],
) -> crate::Result<()> {
    if expiries.is_empty() || secondary_values.is_empty() {
        return Err(InputError::TooFewPoints.into());
    }
    validate_axis(expiries)?;
    validate_axis(secondary_values)?;
    if expiries.iter().any(|expiry| *expiry <= 0.0) {
        return Err(Error::Validation(
            "volatility expiries must be strictly positive".into(),
        ));
    }
    let n = expiries
        .len()
        .checked_mul(secondary_values.len())
        .ok_or(InputError::DimensionMismatch)?;
    if vols_row_major.len() != n {
        return Err(InputError::DimensionMismatch.into());
    }
    for &v in vols_row_major {
        if !v.is_finite() {
            return Err(crate::Error::Validation(
                "surface volatilities must be finite".to_string(),
            ));
        }
        if v < 0.0 {
            return Err(InputError::NegativeValue.into());
        }
    }
    Ok(())
}

pub(super) fn validate_secondary_domain(
    secondary_axis: VolSurfaceAxis,
    quote_type: VolQuoteType,
    secondary_values: &[f64],
    displacements: Option<&[f64]>,
) -> crate::Result<()> {
    if secondary_axis == VolSurfaceAxis::Tenor {
        if let Some(&bad) = secondary_values.iter().find(|&&tenor| tenor <= 0.0) {
            return Err(Error::Validation(format!(
                "tenor volatility surface requires strictly positive tenors, got {bad}"
            )));
        }
        return Ok(());
    }
    match quote_type {
        VolQuoteType::BlackLognormal => {
            if let Some(&bad) = secondary_values.iter().find(|&&strike| strike <= 0.0) {
                return Err(Error::Validation(format!(
                    "Black lognormal volatility surface requires strictly positive strikes, got {bad}"
                )));
            }
        }
        VolQuoteType::ShiftedBlackLognormal => {
            for &shift in displacements.unwrap_or(&[]) {
                for &strike in secondary_values {
                    let shifted = strike + shift;
                    if !shifted.is_finite() || shifted <= 0.0 {
                        return Err(Error::Validation(format!(
                            "shifted Black volatility surface requires strike + displacement > 0 at every row, got strike={strike}, displacement={shift}"
                        )));
                    }
                }
            }
        }
        VolQuoteType::Normal => {}
    }
    Ok(())
}

#[cfg(test)]
mod displacement_tests {
    use super::*;
    use crate::dates::{DayCount, DayCountContextState, VolatilityClock};
    use time::macros::date;

    fn black_vol_opts() -> VolGridOpts {
        VolGridOpts {
            secondary_axis: VolSurfaceAxis::Strike,
            quote_type: VolQuoteType::BlackLognormal,
            interpolation_mode: VolInterpolationMode::Vol,
        }
    }

    #[test]
    fn shifted_surface_construction_and_serde_require_aligned_displacements() {
        let opts = VolGridOpts {
            secondary_axis: VolSurfaceAxis::Strike,
            quote_type: VolQuoteType::ShiftedBlackLognormal,
            interpolation_mode: VolInterpolationMode::Vol,
        };
        assert!(
            VolSurface::from_grid("BAD", &[1.0, 2.0], &[0.03], &[0.2, 0.2], opts, None).is_err()
        );
        assert!(
            VolSurface::from_grid(
                "BAD",
                &[1.0, 2.0],
                &[0.03],
                &[0.2, 0.2],
                opts,
                Some(&[0.02])
            )
            .is_err()
        );

        // Displacements on builder do not silently retag quote_type.
        let unshifted_err = VolSurface::builder("NO-RETAG")
            .expiries(&[1.0])
            .secondary_values(&[0.03])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .displacements(&[0.02])
            .row(&[0.2])
            .build();
        assert!(unshifted_err.is_err());

        // Missing explicit conventions fail closed on builder.
        assert!(
            VolSurface::builder("NO-META")
                .expiries(&[1.0])
                .secondary_values(&[100.0])
                .row(&[0.2])
                .build()
                .is_err()
        );

        let surface = VolSurface::from_grid(
            "SHIFT",
            &[1.0, 2.0],
            &[0.03],
            &[0.2, 0.2],
            opts,
            Some(&[0.02, 0.03]),
        )
        .unwrap();
        assert_eq!(surface.displacements(), Some(&[0.02, 0.03][..]));
        let mut raw = serde_json::to_value(surface).unwrap();
        raw["displacements"] = serde_json::Value::Null;
        assert!(serde_json::from_value::<VolSurface>(raw).is_err());
    }

    #[test]
    fn surface_expiries_and_tenors_must_be_strictly_positive() {
        for bad_expiry in [0.0, -0.25] {
            assert!(
                VolSurface::from_grid(
                    "BAD",
                    &[bad_expiry],
                    &[0.03],
                    &[0.2],
                    black_vol_opts(),
                    None
                )
                .is_err()
            );
        }

        let tenor_opts = VolGridOpts {
            secondary_axis: VolSurfaceAxis::Tenor,
            quote_type: VolQuoteType::Normal,
            interpolation_mode: VolInterpolationMode::Vol,
        };
        for bad_tenor in [0.0, -1.0] {
            assert!(
                VolSurface::from_grid(
                    "BAD-TENOR",
                    &[1.0],
                    &[bad_tenor],
                    &[0.005],
                    tenor_opts,
                    None
                )
                .is_err()
            );
        }

        let surface =
            VolSurface::from_grid("OK", &[1.0], &[0.03], &[0.2], black_vol_opts(), None).unwrap();
        let mut raw = serde_json::to_value(surface).unwrap();
        raw["expiries"][0] = serde_json::json!(0.0);
        assert!(serde_json::from_value::<VolSurface>(raw).is_err());
    }

    #[test]
    fn negative_vol_shocks_and_non_positive_lognormal_strikes_fail_closed() {
        use crate::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable};

        let mut surface = VolSurface::from_grid(
            "VOL",
            &[1.0, 2.0],
            &[90.0, 100.0],
            &[0.20, 0.22, 0.21, 0.23],
            black_vol_opts(),
            None,
        )
        .expect("surface");

        let additive_down = BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value: -0.25,
            bump_type: BumpType::Parallel,
        };
        assert!(surface.apply_bump(additive_down).is_err());
        assert!(
            surface
                .bump_point_absolute_in_place(1.0, 90.0, -0.25)
                .is_err()
        );
        // Point shock requires exact stored coordinates (no nearest-node mapping).
        assert!(
            surface
                .bump_point_absolute_in_place(1.0 + 1e-12, 90.0, 0.01)
                .is_err()
        );
        assert!((surface.vols()[0] - 0.20).abs() < 1e-12);
        assert!(
            surface
                .apply_bucket_bump(None, None, BumpSpec::multiplier(-0.05))
                .is_err()
        );

        // RateBp and Percent are rejected on VolSurface.
        assert!(surface.apply_bump(BumpSpec::parallel_bp(10.0)).is_err());
        assert!(
            surface
                .apply_bump(BumpSpec::inflation_shift_pct(1.0))
                .is_err()
        );

        // Intentional zero scale (k=0) is admitted; positive underflow is rejected; exact empty bucket is no-op.
        let zeroed = surface
            .apply_bucket_bump(Some(&[1.0]), Some(&[90.0]), BumpSpec::multiplier(0.0))
            .expect("k=0 should be admitted");
        assert_eq!(zeroed.vols()[0], 0.0);
        assert_eq!(
            surface
                .apply_bucket_bump(Some(&[]), None, BumpSpec::multiplier(2.0))
                .expect("empty filter should be valid no-op")
                .vols(),
            surface.vols()
        );
        let subnormal = VolSurface::from_grid(
            "SUB",
            &[1.0],
            &[100.0],
            &[f64::from_bits(1)],
            black_vol_opts(),
            None,
        )
        .unwrap();
        assert!(subnormal.scaled(0.5).is_err());
        assert!(
            surface
                .apply_bucket_bump(Some(&[1.0, 1.0]), None, BumpSpec::multiplier(1.1))
                .is_err()
        );
        assert!(
            surface
                .apply_bucket_bump(Some(&[1.0 + 1e-11]), None, BumpSpec::multiplier(1.1))
                .is_err()
        );

        for bad_strike in [-5.0, 0.0] {
            assert!(
                VolSurface::from_grid(
                    "BAD-BLACK",
                    &[1.0],
                    &[bad_strike],
                    &[0.2],
                    black_vol_opts(),
                    None,
                )
                .is_err()
            );
        }
        let mut raw_black = serde_json::to_value(&surface).expect("serialize black surface");
        raw_black["secondary_values"][0] = serde_json::json!(0.0);
        assert!(serde_json::from_value::<VolSurface>(raw_black).is_err());

        let shifted_opts = VolGridOpts {
            secondary_axis: VolSurfaceAxis::Strike,
            quote_type: VolQuoteType::ShiftedBlackLognormal,
            interpolation_mode: VolInterpolationMode::Vol,
        };
        assert!(
            VolSurface::from_grid(
                "SHIFT-OK",
                &[1.0],
                &[-0.01, 0.02],
                &[0.2, 0.2],
                shifted_opts,
                Some(&[0.02]),
            )
            .is_ok()
        );
        assert!(
            VolSurface::from_grid(
                "SHIFT-BAD",
                &[1.0],
                &[-0.02, 0.02],
                &[0.2, 0.2],
                shifted_opts,
                Some(&[0.02]),
            )
            .is_err()
        );
    }

    #[test]
    fn expiry_clock_round_trips_and_survives_shocks() {
        let clock = VolatilityClock::try_new(
            date!(2025 - 01 - 15),
            DayCount::Act365F,
            DayCountContextState::default(),
        )
        .unwrap();
        let surface = VolSurface::builder("CLK")
            .expiries(&[1.0])
            .secondary_values(&[100.0])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .expiry_clock(clock.clone())
            .row(&[0.2])
            .build()
            .unwrap();
        assert_eq!(surface.expiry_clock(), Some(&clock));
        let bumped = surface
            .apply_bucket_bump(None, None, BumpSpec::multiplier(1.1))
            .unwrap();
        assert_eq!(bumped.expiry_clock(), Some(&clock));
        let json = serde_json::to_value(&surface).unwrap();
        let roundtrip: VolSurface = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.expiry_clock(), Some(&clock));
    }
}
