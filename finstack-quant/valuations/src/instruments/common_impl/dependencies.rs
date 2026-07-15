//! Unified market data dependency representation for instruments.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::{CurveId, PriceId};
use smallvec::SmallVec;

use crate::instruments::json_loader::InstrumentJson;

/// Collection of curves used by an instrument, categorized by market role.
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
    /// Create an empty curve collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Iterate over every curve with its market role.
    pub fn all_with_kind(&self) -> impl Iterator<Item = (CurveId, RatesCurveKind)> + '_ {
        self.discount_curves
            .iter()
            .map(|curve| (curve.clone(), RatesCurveKind::Discount))
            .chain(
                self.forward_curves
                    .iter()
                    .map(|curve| (curve.clone(), RatesCurveKind::Forward)),
            )
            .chain(
                self.credit_curves
                    .iter()
                    .map(|curve| (curve.clone(), RatesCurveKind::Credit)),
            )
            .chain(
                self.inflation_curves
                    .iter()
                    .map(|curve| (curve.clone(), RatesCurveKind::Inflation)),
            )
    }

    /// Return whether no curves are present.
    pub fn is_empty(&self) -> bool {
        self.discount_curves.is_empty()
            && self.forward_curves.is_empty()
            && self.credit_curves.is_empty()
            && self.inflation_curves.is_empty()
    }

    /// Return the total number of curves.
    pub fn len(&self) -> usize {
        self.discount_curves.len()
            + self.forward_curves.len()
            + self.credit_curves.len()
            + self.inflation_curves.len()
    }
}

/// Identifies a rate curve's market role for risk calculations.
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
    /// Discount curve used for present-value discounting.
    Discount,
    /// Forward curve used for floating-rate projection.
    Forward,
    /// Credit or hazard curve.
    Credit,
    /// Inflation curve or published inflation index.
    Inflation,
}

impl core::fmt::Display for RatesCurveKind {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Discount => write!(formatter, "discount"),
            Self::Forward => write!(formatter, "forward"),
            Self::Credit => write!(formatter, "credit"),
            Self::Inflation => write!(formatter, "inflation"),
        }
    }
}

impl core::str::FromStr for RatesCurveKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "discount" => Ok(Self::Discount),
            "forward" => Ok(Self::Forward),
            "credit" => Ok(Self::Credit),
            "inflation" => Ok(Self::Inflation),
            other => Err(format!(
                "Unknown curve kind: '{other}'. Valid: discount, forward, credit, inflation"
            )),
        }
    }
}

/// FX pair identifier using base/quote currency ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FxPair {
    /// Base currency (numerator).
    pub base: Currency,
    /// Quote currency (denominator).
    pub quote: Currency,
}

impl FxPair {
    /// Create a new FX pair identifier.
    pub fn new(base: Currency, quote: Currency) -> Self {
        Self { base, quote }
    }
}

/// A volatility-surface dependency with the context needed for diagnostics.
#[derive(Debug, Clone)]
pub struct VolatilityDependency {
    /// Volatility surface identifier.
    pub surface_id: CurveId,
    /// Optional underlying price identifier paired with the surface.
    pub underlying_id: Option<PriceId>,
    /// Optional contractual strike used by local volatility diagnostics.
    pub reference_strike: Option<f64>,
}

impl VolatilityDependency {
    /// Create a volatility dependency descriptor.
    pub fn new(
        surface_id: impl Into<CurveId>,
        underlying_id: Option<PriceId>,
        reference_strike: Option<f64>,
    ) -> Self {
        Self {
            surface_id: surface_id.into(),
            underlying_id,
            reference_strike,
        }
    }
}

impl PartialEq for VolatilityDependency {
    fn eq(&self, other: &Self) -> bool {
        self.surface_id == other.surface_id
            && self.underlying_id == other.underlying_id
            && self.reference_strike.map(f64::to_bits) == other.reference_strike.map(f64::to_bits)
    }
}

impl Eq for VolatilityDependency {}

/// Unified dependency container for instrument market data requirements.
#[derive(Debug, Clone, Default)]
pub struct MarketDependencies {
    /// Curve dependencies grouped by type.
    pub curves: InstrumentCurves,
    /// Spot identifiers (equity, FX spot IDs, commodity spot IDs).
    pub spot_ids: Vec<String>,
    /// Typed volatility dependencies in deterministic insertion order.
    pub volatility_dependencies: Vec<VolatilityDependency>,
    /// FX pairs required for pricing (spot matrices).
    pub fx_pairs: Vec<FxPair>,
    /// Scalar time series identifiers (e.g., OHLC price series for realized variance).
    pub series_ids: Vec<String>,
}

impl MarketDependencies {
    /// Create an empty dependency set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge curve dependencies into this set.
    pub fn add_curves(&mut self, curves: InstrumentCurves) {
        for id in curves.discount_curves {
            push_unique_curve(&mut self.curves.discount_curves, id);
        }
        for id in curves.forward_curves {
            push_unique_curve(&mut self.curves.forward_curves, id);
        }
        for id in curves.credit_curves {
            push_unique_curve(&mut self.curves.credit_curves, id);
        }
        for id in curves.inflation_curves {
            push_unique_curve(&mut self.curves.inflation_curves, id);
        }
    }

    /// Add a discount-curve dependency.
    pub fn add_discount_curve(&mut self, id: impl Into<CurveId>) {
        push_unique_curve(&mut self.curves.discount_curves, id.into());
    }

    /// Add a forward/projection-curve dependency.
    pub fn add_forward_curve(&mut self, id: impl Into<CurveId>) {
        push_unique_curve(&mut self.curves.forward_curves, id.into());
    }

    /// Add a credit/hazard-curve dependency.
    pub fn add_credit_curve(&mut self, id: impl Into<CurveId>) {
        push_unique_curve(&mut self.curves.credit_curves, id.into());
    }

    /// Add an inflation-curve or published-index dependency.
    pub fn add_inflation_curve(&mut self, id: impl Into<CurveId>) {
        push_unique_curve(&mut self.curves.inflation_curves, id.into());
    }

    /// Add a spot identifier.
    pub fn add_spot_id(&mut self, id: impl Into<String>) {
        push_unique_string(&mut self.spot_ids, id.into());
    }

    /// Add a typed volatility dependency.
    pub fn add_volatility_dependency(&mut self, dependency: VolatilityDependency) {
        if !self.volatility_dependencies.contains(&dependency) {
            self.volatility_dependencies.push(dependency);
        }
    }

    /// Return unique volatility surface IDs in first-descriptor order.
    pub fn unique_vol_surface_ids(&self) -> Vec<CurveId> {
        let mut ids = Vec::new();
        for dependency in &self.volatility_dependencies {
            if !ids.contains(&dependency.surface_id) {
                ids.push(dependency.surface_id.clone());
            }
        }
        ids
    }

    /// Add a scalar time series identifier.
    pub fn add_series_id(&mut self, id: impl Into<String>) {
        push_unique_string(&mut self.series_ids, id.into());
    }

    /// Add an FX pair dependency.
    pub fn add_fx_pair(&mut self, base: Currency, quote: Currency) {
        push_unique_fx_pair(&mut self.fx_pairs, FxPair::new(base, quote));
    }

    /// Merge another dependency set into this one.
    pub fn merge(&mut self, other: MarketDependencies) {
        self.add_curves(other.curves);
        for id in other.spot_ids {
            self.add_spot_id(id);
        }
        for dependency in other.volatility_dependencies {
            self.add_volatility_dependency(dependency);
        }
        for pair in other.fx_pairs {
            self.add_fx_pair(pair.base, pair.quote);
        }
        for id in other.series_ids {
            self.add_series_id(id);
        }
    }

    /// Build dependencies from a JSON-tagged instrument representation.
    ///
    /// Routes through `InstrumentJson::into_boxed` so JSON-loaded instruments
    /// stay on the same dependency path as trait-object pricing.
    pub fn from_instrument_json(instrument: &InstrumentJson) -> finstack_quant_core::Result<Self> {
        instrument.clone().into_boxed()?.market_dependencies()
    }
}

// Deduplicate while preserving insertion order for deterministic risk reports.

fn push_unique_curve(target: &mut SmallVec<[CurveId; 2]>, id: CurveId) {
    if target.contains(&id) {
        return;
    }
    target.push(id);
}

fn push_unique_string(target: &mut Vec<String>, value: String) {
    if target.contains(&value) {
        return;
    }
    target.push(value);
}

fn push_unique_fx_pair(target: &mut Vec<FxPair>, pair: FxPair) {
    if target.contains(&pair) {
        return;
    }
    target.push(pair);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_volatility_dependency_preserves_underlying_surface_and_strike() {
        let mut dependencies = MarketDependencies::new();
        dependencies.add_spot_id("SPX-SPOT");
        dependencies.add_volatility_dependency(VolatilityDependency::new(
            CurveId::new("SPX-VOL"),
            Some(PriceId::new("SPX-SPOT")),
            Some(4_500.0),
        ));

        assert_eq!(
            dependencies.volatility_dependencies,
            vec![VolatilityDependency::new(
                CurveId::new("SPX-VOL"),
                Some(PriceId::new("SPX-SPOT")),
                Some(4_500.0),
            )]
        );
        assert_eq!(dependencies.spot_ids, vec!["SPX-SPOT"]);
    }

    #[test]
    fn exact_descriptors_deduplicate_in_insertion_order_but_surfaces_can_repeat() {
        let first = VolatilityDependency::new(
            CurveId::new("SPX-VOL"),
            Some(PriceId::new("SPX-SPOT")),
            Some(4_500.0),
        );
        let second = VolatilityDependency::new(
            CurveId::new("SPX-VOL"),
            Some(PriceId::new("SPX-SPOT")),
            Some(4_600.0),
        );
        let mut dependencies = MarketDependencies::new();
        dependencies.add_volatility_dependency(first.clone());
        dependencies.add_volatility_dependency(first.clone());
        dependencies.add_volatility_dependency(second.clone());

        assert_eq!(dependencies.volatility_dependencies, vec![first, second]);
        assert_eq!(
            dependencies.unique_vol_surface_ids(),
            vec![CurveId::new("SPX-VOL")]
        );
    }

    #[test]
    fn merge_preserves_descriptor_order() {
        let mut left = MarketDependencies::new();
        left.add_volatility_dependency(VolatilityDependency::new(
            CurveId::new("LEFT-VOL"),
            None,
            None,
        ));
        let mut right = MarketDependencies::new();
        right.add_volatility_dependency(VolatilityDependency::new(
            CurveId::new("RIGHT-VOL"),
            None,
            None,
        ));
        right.add_volatility_dependency(VolatilityDependency::new(
            CurveId::new("LEFT-VOL"),
            None,
            None,
        ));

        left.merge(right);
        assert_eq!(
            left.unique_vol_surface_ids(),
            vec![CurveId::new("LEFT-VOL"), CurveId::new("RIGHT-VOL")]
        );
    }
}
