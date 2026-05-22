//! Unified market data dependency representation for instruments.

use crate::instruments::common_impl::traits::{
    CurveDependencies, CurveIdVec, EquityDependencies, EquityInstrumentDeps, InstrumentCurves,
};
use finstack_core::currency::Currency;
use finstack_core::types::CurveId;

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

/// Unified dependency container for instrument market data requirements.
#[derive(Debug, Clone, Default)]
pub struct MarketDependencies {
    /// Curve dependencies grouped by type.
    pub curves: InstrumentCurves,
    /// Spot identifiers (equity, FX spot IDs, commodity spot IDs).
    pub spot_ids: Vec<String>,
    /// Volatility surface identifiers.
    pub vol_surface_ids: Vec<String>,
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
        EquityInstrumentDeps {
            spot_id: self.spot_ids.first().cloned(),
            vol_surface_id: self.vol_surface_ids.first().cloned(),
        }
    }

    /// Build dependencies from an instrument implementing [`CurveDependencies`].
    pub fn from_curve_dependencies<T: CurveDependencies>(
        instrument: &T,
    ) -> finstack_core::Result<Self> {
        let mut deps = Self::new();
        deps.add_curves(instrument.curve_dependencies()?);
        Ok(deps)
    }

    /// Build dependencies from an instrument implementing both curve and equity traits.
    pub fn from_curves_and_equity<T: CurveDependencies + EquityDependencies>(
        instrument: &T,
    ) -> finstack_core::Result<Self> {
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
    }

    /// Merge equity dependencies into this set.
    pub fn add_equity_dependencies(&mut self, deps: EquityInstrumentDeps) {
        if let Some(spot_id) = deps.spot_id {
            self.add_spot_id(spot_id);
        }
        if let Some(vol_surface_id) = deps.vol_surface_id {
            self.add_vol_surface_id(vol_surface_id);
        }
    }

    /// Add a spot identifier.
    pub fn add_spot_id(&mut self, id: impl Into<String>) {
        push_unique_string(&mut self.spot_ids, id.into());
    }

    /// Add a volatility surface identifier.
    pub fn add_vol_surface_id(&mut self, id: impl Into<String>) {
        push_unique_string(&mut self.vol_surface_ids, id.into());
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
        for id in other.vol_surface_ids {
            self.add_vol_surface_id(id);
        }
        for pair in other.fx_pairs {
            self.add_fx_pair(pair.base, pair.quote);
        }
        for id in other.series_ids {
            self.add_series_id(id);
        }
    }

    /// Build dependencies from a JSON-tagged instrument representation.
    pub fn from_instrument_json(instrument: &InstrumentJson) -> finstack_core::Result<Self> {
        instrument.market_dependencies()
    }
}

// Deduplicate while preserving insertion order for deterministic risk reports.

fn push_unique_curve(target: &mut CurveIdVec, id: CurveId) {
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
