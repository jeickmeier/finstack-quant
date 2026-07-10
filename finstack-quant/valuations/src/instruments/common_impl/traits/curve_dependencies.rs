// -----------------------------------------------------------------------------
// Curve Dependencies
// -----------------------------------------------------------------------------

use finstack_quant_core::types::CurveId;
use smallvec::SmallVec;

/// Trait for instruments to declare all their curve dependencies.
///
/// This trait enables type-safe discovery of all curves used by an instrument,
/// eliminating the need for runtime downcasting. It's primarily used by risk
/// calculators (e.g., DV01) to identify which curves should be bumped.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::{CurveDependencies, InstrumentCurves};
/// use finstack_quant_core::types::CurveId;
///
/// struct Bond {
///     discount_curve_id: CurveId,
/// }
///
/// impl CurveDependencies for Bond {
///     fn curve_dependencies(&self) -> finstack_quant_core::Result<InstrumentCurves> {
///         InstrumentCurves::builder()
///             .discount(self.discount_curve_id.clone())
///             .build()
///     }
/// }
/// ```
pub trait CurveDependencies: Send + Sync {
    /// Return all curves used by this instrument, categorized by type.
    fn curve_dependencies(&self) -> finstack_quant_core::Result<InstrumentCurves>;
}

/// Collection of curves used by an instrument, categorized by type.
///
/// This struct provides a type-safe way to declare curve dependencies
/// for risk calculations. Uses `SmallVec` internally to avoid heap
/// allocation for the common case (1-2 curves per category).
#[derive(Default, Clone, Debug)]
pub struct InstrumentCurves {
    /// Discount curves used by the instrument (including primary and foreign).
    pub discount_curves: SmallVec<[CurveId; 2]>,
    /// Forward/projection curves used by the instrument.
    pub forward_curves: SmallVec<[CurveId; 2]>,
    /// Credit/hazard curves used by the instrument.
    pub credit_curves: SmallVec<[CurveId; 2]>,
    /// Inflation curves or published inflation indices used by the instrument.
    pub inflation_curves: SmallVec<[CurveId; 2]>,
}

impl InstrumentCurves {
    /// Create a new empty curve collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start building a curve collection.
    #[must_use]
    pub fn builder() -> InstrumentCurvesBuilder {
        InstrumentCurvesBuilder::default()
    }

    /// Iterator over all curves with their kind.
    pub fn all_with_kind(&self) -> impl Iterator<Item = (CurveId, RatesCurveKind)> + '_ {
        self.discount_curves
            .iter()
            .map(|c| (c.clone(), RatesCurveKind::Discount))
            .chain(
                self.forward_curves
                    .iter()
                    .map(|c| (c.clone(), RatesCurveKind::Forward)),
            )
            .chain(
                self.credit_curves
                    .iter()
                    .map(|c| (c.clone(), RatesCurveKind::Credit)),
            )
            .chain(
                self.inflation_curves
                    .iter()
                    .map(|c| (c.clone(), RatesCurveKind::Inflation)),
            )
    }

    /// Check if any curves are defined.
    pub fn is_empty(&self) -> bool {
        self.discount_curves.is_empty()
            && self.forward_curves.is_empty()
            && self.credit_curves.is_empty()
            && self.inflation_curves.is_empty()
    }

    /// Total number of curves.
    pub fn len(&self) -> usize {
        self.discount_curves.len()
            + self.forward_curves.len()
            + self.credit_curves.len()
            + self.inflation_curves.len()
    }
}

/// Builder for [`InstrumentCurves`].
#[derive(Default)]
pub struct InstrumentCurvesBuilder {
    curves: InstrumentCurves,
}

impl InstrumentCurvesBuilder {
    /// Add a discount curve (duplicates are ignored).
    pub fn discount(mut self, curve_id: CurveId) -> Self {
        if !self.curves.discount_curves.contains(&curve_id) {
            self.curves.discount_curves.push(curve_id);
        }
        self
    }

    /// Add a forward curve (duplicates are ignored).
    pub fn forward(mut self, curve_id: CurveId) -> Self {
        if !self.curves.forward_curves.contains(&curve_id) {
            self.curves.forward_curves.push(curve_id);
        }
        self
    }

    /// Add a credit/hazard curve (duplicates are ignored).
    pub fn credit(mut self, curve_id: CurveId) -> Self {
        if !self.curves.credit_curves.contains(&curve_id) {
            self.curves.credit_curves.push(curve_id);
        }
        self
    }

    /// Add an inflation curve or published inflation index (duplicates are ignored).
    pub fn inflation(mut self, curve_id: CurveId) -> Self {
        if !self.curves.inflation_curves.contains(&curve_id) {
            self.curves.inflation_curves.push(curve_id);
        }
        self
    }

    /// Build the final curve collection.
    pub fn build(self) -> finstack_quant_core::Result<InstrumentCurves> {
        Ok(self.curves)
    }
}

/// Identifies the type of rate curve for risk calculations.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum RatesCurveKind {
    /// Discount curve (used for present value discounting).
    Discount,
    /// Forward curve (used for floating rate projection).
    Forward,
    /// Credit/hazard curve (used for credit risk calculations).
    Credit,
    /// Inflation curve or published inflation index.
    Inflation,
}

impl core::fmt::Display for RatesCurveKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Discount => write!(f, "discount"),
            Self::Forward => write!(f, "forward"),
            Self::Credit => write!(f, "credit"),
            Self::Inflation => write!(f, "inflation"),
        }
    }
}

impl core::str::FromStr for RatesCurveKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "discount" => Ok(Self::Discount),
            "forward" => Ok(Self::Forward),
            "credit" => Ok(Self::Credit),
            "inflation" => Ok(Self::Inflation),
            other => Err(format!(
                "Unknown curve kind: '{}'. Valid: discount, forward, credit, inflation",
                other
            )),
        }
    }
}
