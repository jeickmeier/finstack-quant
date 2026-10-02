//! Observed volatility by tenor and strike at one fixed option expiry.

use super::{VolQuoteType, VolSurface};
use crate::{types::CurveId, Error, Result};

/// A fixed-expiry slice of a volatility cube with truthful tenor and strike axes.
///
/// Values are annualized decimal volatilities in tenor-major order. Unlike an
/// expiry surface, the first grid axis is the underlying tenor, so interpolation
/// never treats tenor as elapsed option time. Computation lives in models.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "VolCubeExpirySliceWire", into = "VolCubeExpirySliceWire")]
#[cfg_attr(feature = "json-schema", schemars(try_from = "VolCubeExpirySliceWire"))]
pub struct VolCubeExpirySlice {
    id: CurveId,
    expiry: f64,
    tenors: Box<[f64]>,
    strikes: Box<[f64]>,
    vols: Box<[f64]>,
    quote_type: VolQuoteType,
    displacements: Option<Box<[f64]>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct VolCubeExpirySliceWire {
    id: String,
    expiry: f64,
    tenors: Vec<f64>,
    strikes: Vec<f64>,
    vols_row_major: Vec<f64>,
    quote_type: VolQuoteType,
    #[serde(skip_serializing_if = "Option::is_none")]
    displacements: Option<Vec<f64>>,
}

impl From<VolCubeExpirySlice> for VolCubeExpirySliceWire {
    fn from(slice: VolCubeExpirySlice) -> Self {
        Self {
            id: slice.id.to_string(),
            expiry: slice.expiry,
            tenors: slice.tenors.into_vec(),
            strikes: slice.strikes.into_vec(),
            vols_row_major: slice.vols.into_vec(),
            quote_type: slice.quote_type,
            displacements: slice.displacements.map(Vec::from),
        }
    }
}

impl TryFrom<VolCubeExpirySliceWire> for VolCubeExpirySlice {
    type Error = Error;
    fn try_from(raw: VolCubeExpirySliceWire) -> Result<Self> {
        Self::from_grid(
            raw.id,
            raw.expiry,
            &raw.tenors,
            &raw.strikes,
            &raw.vols_row_major,
            raw.quote_type,
            raw.displacements.as_deref(),
        )
    }
}

impl VolCubeExpirySlice {
    /// Construct a fixed-expiry tenor-by-strike volatility artifact.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable market-data identifier retained for auditing and serialization.
    /// * `expiry` - Finite, strictly positive option expiry in years, held fixed for every row.
    /// * `tenors` - Nonempty, strictly increasing positive underlying tenors in years.
    /// * `strikes` - Nonempty, strictly increasing finite strikes in forward-price or rate units.
    /// * `vols_row_major` - Nonnegative finite annualized decimal quotes, flattened in tenor-major order.
    /// * `quote_type` - Black, displaced Black, or normal units of every volatility quote.
    /// * `displacements` - One finite additive shift per tenor for displaced Black quotes; absent for other conventions.
    ///
    /// # Errors
    ///
    /// Returns an input or validation error for invalid axes, dimensions, quote values,
    /// expiry, or displacement metadata inconsistent with the quote convention.
    pub fn from_grid(
        id: impl AsRef<str>,
        expiry: f64,
        tenors: &[f64],
        strikes: &[f64],
        vols_row_major: &[f64],
        quote_type: VolQuoteType,
        displacements: Option<&[f64]>,
    ) -> Result<Self> {
        if !expiry.is_finite() || expiry <= 0.0 || tenors.iter().any(|t| *t <= 0.0) {
            return Err(Error::Validation(
                "slice expiry and tenors must be positive and finite".into(),
            ));
        }
        // Share canonical axis/grid validation; only the resulting slice's
        // truthful tenor/strike fields are retained or serialized.
        let grid = VolSurface::from_grid(id, tenors, strikes, vols_row_major)?;
        super::vol_surface::validate_displacements(quote_type, displacements, tenors.len())?;
        Ok(Self {
            id: grid.id().clone(),
            expiry,
            tenors: tenors.into(),
            strikes: strikes.into(),
            vols: vols_row_major.into(),
            quote_type,
            displacements: displacements.map(Into::into),
        })
    }

    /// Identifier carried by the source cube.
    pub fn get_id(&self) -> &CurveId {
        &self.id
    }
    /// Fixed option expiry in years.
    pub fn get_expiry(&self) -> f64 {
        self.expiry
    }
    /// Underlying tenor axis in years.
    pub fn get_tenors(&self) -> &[f64] {
        &self.tenors
    }
    /// Strike axis in forward-price or decimal-rate units.
    pub fn get_strikes(&self) -> &[f64] {
        &self.strikes
    }
    /// Annualized decimal volatility quotes in tenor-major order.
    pub fn get_vols(&self) -> &[f64] {
        &self.vols
    }
    /// Units and pricing convention of the stored volatility quotes.
    pub fn get_quote_type(&self) -> VolQuoteType {
        self.quote_type
    }
    /// Additive displaced-Black shifts aligned with tenors; absent for other conventions.
    pub fn get_displacements(&self) -> Option<&[f64]> {
        self.displacements.as_deref()
    }
    /// Number of tenor rows and strike columns.
    pub fn get_grid_shape(&self) -> (usize, usize) {
        (self.tenors.len(), self.strikes.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_rejects_missing_displacement_and_invalid_expiry() {
        assert!(VolCubeExpirySlice::from_grid(
            "BAD",
            1.0,
            &[5.0],
            &[0.03],
            &[0.2],
            VolQuoteType::ShiftedBlackLognormal,
            None
        )
        .is_err());
        assert!(VolCubeExpirySlice::from_grid(
            "BAD",
            f64::NAN,
            &[5.0],
            &[0.03],
            &[0.2],
            VolQuoteType::BlackLognormal,
            None
        )
        .is_err());
        let slice = VolCubeExpirySlice::from_grid(
            "SHIFT",
            1.0,
            &[5.0],
            &[0.03],
            &[0.2],
            VolQuoteType::ShiftedBlackLognormal,
            Some(&[0.02]),
        )
        .unwrap();
        let mut raw = serde_json::to_value(slice).unwrap();
        raw["displacements"] = serde_json::Value::Null;
        assert!(serde_json::from_value::<VolCubeExpirySlice>(raw).is_err());
    }
}
