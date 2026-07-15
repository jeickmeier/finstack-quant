//! Unified market data dependency representation for instruments.

use crate::instruments::common_impl::traits::{
    CurveDependencies, EquityDependencies, EquityInstrumentDeps, InstrumentCurves,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::{CurveId, PriceId};
use smallvec::SmallVec;

use crate::instruments::json_loader::InstrumentJson;

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
    /// Volatility surface identifiers.
    ///
    /// Temporary B09-B13 compatibility projection. New code should use
    /// [`MarketDependencies::volatility_dependencies`].
    pub vol_surface_ids: Vec<String>,
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

    /// Return the curve dependencies view for this market dependency set.
    pub fn curve_dependencies(&self) -> &InstrumentCurves {
        &self.curves
    }

    /// Return the primary equity dependencies view for this market dependency set.
    ///
    /// This returns the first spot/vol IDs when multiple are present (e.g., baskets).
    pub fn equity_dependencies(&self) -> EquityInstrumentDeps {
        let volatility = self.volatility_dependencies.first();
        EquityInstrumentDeps {
            spot_id: volatility
                .and_then(|dependency| dependency.underlying_id.as_ref())
                .map(|id| id.as_str().to_string())
                .or_else(|| self.spot_ids.first().cloned()),
            vol_surface_id: volatility
                .map(|dependency| dependency.surface_id.as_str().to_string())
                .or_else(|| self.vol_surface_ids.first().cloned()),
            reference_strike: volatility.and_then(|dependency| dependency.reference_strike),
        }
    }

    /// Build dependencies from an instrument implementing [`CurveDependencies`].
    pub fn from_curve_dependencies<T: CurveDependencies>(
        instrument: &T,
    ) -> finstack_quant_core::Result<Self> {
        let mut deps = Self::new();
        deps.add_curves(instrument.curve_dependencies()?);
        Ok(deps)
    }

    /// Build dependencies from an instrument implementing both curve and equity traits.
    pub fn from_curves_and_equity<T: CurveDependencies + EquityDependencies>(
        instrument: &T,
    ) -> finstack_quant_core::Result<Self> {
        let mut deps = Self::new();
        deps.add_curves(instrument.curve_dependencies()?);
        deps.add_equity_dependencies(instrument.equity_dependencies()?);
        Ok(deps)
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

    /// Merge equity dependencies into this set.
    pub fn add_equity_dependencies(&mut self, deps: EquityInstrumentDeps) {
        let EquityInstrumentDeps {
            spot_id,
            vol_surface_id,
            reference_strike,
        } = deps;
        let underlying_id = spot_id.as_deref().map(PriceId::new);
        if let Some(spot_id) = spot_id {
            self.add_spot_id(spot_id);
        }
        if let Some(vol_surface_id) = vol_surface_id {
            self.add_volatility_dependency(VolatilityDependency::new(
                CurveId::new(vol_surface_id),
                underlying_id,
                reference_strike,
            ));
        }
    }

    /// Add a spot identifier.
    pub fn add_spot_id(&mut self, id: impl Into<String>) {
        push_unique_string(&mut self.spot_ids, id.into());
    }

    /// Add a volatility surface identifier.
    pub fn add_vol_surface_id(&mut self, id: impl Into<String>) {
        self.add_volatility_dependency(VolatilityDependency::new(
            CurveId::new(id.into()),
            None,
            None,
        ));
    }

    /// Add a typed volatility dependency and synchronize the legacy surface projection.
    pub fn add_volatility_dependency(&mut self, dependency: VolatilityDependency) {
        push_unique_string(
            &mut self.vol_surface_ids,
            dependency.surface_id.as_str().to_string(),
        );
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
        for id in &self.vol_surface_ids {
            let id = CurveId::new(id);
            if !ids.contains(&id) {
                ids.push(id);
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
        for id in other.vol_surface_ids {
            if !self.vol_surface_ids.contains(&id) {
                self.add_vol_surface_id(id);
            }
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
    fn legacy_equity_adapter_preserves_underlying_surface_and_strike() {
        let mut dependencies = MarketDependencies::new();
        dependencies.add_equity_dependencies(EquityInstrumentDeps {
            spot_id: Some("SPX-SPOT".to_string()),
            vol_surface_id: Some("SPX-VOL".to_string()),
            reference_strike: Some(4_500.0),
        });

        assert_eq!(
            dependencies.volatility_dependencies,
            vec![VolatilityDependency::new(
                CurveId::new("SPX-VOL"),
                Some(PriceId::new("SPX-SPOT")),
                Some(4_500.0),
            )]
        );
        assert_eq!(dependencies.vol_surface_ids, vec!["SPX-VOL"]);

        let roundtrip = dependencies.equity_dependencies();
        assert_eq!(roundtrip.spot_id.as_deref(), Some("SPX-SPOT"));
        assert_eq!(roundtrip.vol_surface_id.as_deref(), Some("SPX-VOL"));
        assert_eq!(roundtrip.reference_strike, Some(4_500.0));
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
        assert_eq!(dependencies.vol_surface_ids, vec!["SPX-VOL"]);
        assert_eq!(
            dependencies.unique_vol_surface_ids(),
            vec![CurveId::new("SPX-VOL")]
        );
    }

    #[test]
    fn merge_preserves_descriptor_order_and_projection() {
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
        assert_eq!(left.vol_surface_ids, vec!["LEFT-VOL", "RIGHT-VOL"]);
        assert_eq!(
            left.unique_vol_surface_ids(),
            vec![CurveId::new("LEFT-VOL"), CurveId::new("RIGHT-VOL")]
        );
    }
}
