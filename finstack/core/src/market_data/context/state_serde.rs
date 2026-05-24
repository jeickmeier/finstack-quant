//! Serializable state representations for [`MarketContext`](super::MarketContext).
//!
//! This submodule defines the serde-facing snapshot types used to persist and
//! restore market contexts, including typed curve state, FX state, and scalar
//! containers.

use std::sync::Arc;

use crate::collections::HashMap;
use crate::types::CurveId;

use super::{CurveStorage, MarketContext};

use crate::market_data::{
    dividends::DividendSchedule,
    hierarchy::MarketDataHierarchy,
    scalars::{InflationIndex, MarketScalar, ScalarTimeSeries},
    surfaces::{FxDeltaVolSurface, VolCube, VolSurface},
    term_structures::{
        BaseCorrelationCurve, BasisSpreadCurve, DiscountCurve, ForwardCurve, HazardCurve,
        InflationCurve, ParametricCurve, PriceCurve, VolatilityIndexCurve,
    },
};
use crate::money::fx::{
    reciprocal_rate_or_err, FxConversionPolicy, FxMatrix, FxMatrixState, FxProvider,
};

// -----------------------------------------------------------------------------
// Serde: CurveState and (De)Serialize impls
// -----------------------------------------------------------------------------

macro_rules! define_curve_state {
    ($( $variant:ident => {
        accessor: $accessor:ident,
        is_accessor: $is_accessor:ident,
        ty: $ty:ident,
        type_name: $type_name:literal
    } ),* $(,)?) => {
        /// Serializable state representation for any curve type.
        ///
        /// Produced when persisting market data snapshots through serde.
        #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        pub enum CurveState {
            $(
                #[doc = concat!($type_name, " curve state")]
                $variant($ty),
            )*
        }

        impl CurveState {
            /// Curve identifier for this state, regardless of variant.
            ///
            /// Useful for cross-cutting code (snapshot diff, dependency
            /// graph emission, etc.) that needs the ID without dispatching
            /// on the curve type.
            pub fn id(&self) -> &CurveId {
                match self {
                    $( CurveState::$variant(curve) => curve.id(), )*
                }
            }
        }

        fn curve_state_id(state: &CurveState) -> &CurveId {
            state.id()
        }

        impl CurveStorage {
            /// Convert to serializable state.
            ///
            /// This conversion is infallible - all curve types can be converted to their state representation.
            pub fn to_state(&self) -> CurveState {
                match self {
                    $( Self::$variant(curve) => CurveState::$variant((**curve).clone()), )*
                }
            }

            /// Reconstruct from serializable state.
            ///
            /// This conversion is infallible - all state variants map directly to storage variants.
            pub fn from_state(state: CurveState) -> Self {
                match state {
                    $( CurveState::$variant(curve) => Self::$variant(Arc::new(curve)), )*
                }
            }
        }
    };
}

super::for_each_context_curve!(define_curve_state);

impl serde::Serialize for CurveStorage {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_state().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for CurveStorage {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let state = CurveState::deserialize(deserializer)?;
        Ok(Self::from_state(state))
    }
}

// -----------------------------------------------------------------------------
// Credit Index State (for serialization of CreditIndexData)
// -----------------------------------------------------------------------------

/// Serializable state for credit index data.
///
/// Instead of serializing `Arc<Curve>` directly, we store curve IDs that
/// reference curves present in the `MarketContextState`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreditIndexState {
    /// Unique identifier for this credit index
    pub id: String,
    /// Number of constituents
    pub num_constituents: u16,
    /// Recovery rate
    pub recovery_rate: f64,
    /// ID of the index hazard curve (must exist in context curves)
    pub index_credit_curve_id: String,
    /// ID of the base correlation curve (must exist in context curves)
    pub base_correlation_curve_id: String,
    /// Optional map of issuer ID → hazard curve ID
    pub issuer_credit_curve_ids: Option<std::collections::BTreeMap<String, String>>,
    /// Optional map of issuer ID → recovery rate
    pub issuer_recovery_rates: Option<std::collections::BTreeMap<String, f64>>,
    /// Optional map of issuer ID → weight
    pub issuer_weights: Option<std::collections::BTreeMap<String, f64>>,
}

// -----------------------------------------------------------------------------
// Market Context State (complete snapshot)
// -----------------------------------------------------------------------------

/// Current schema version for [`MarketContextState`].
pub const MARKET_CONTEXT_STATE_VERSION: u32 = 2;

fn default_market_context_state_version() -> u32 {
    MARKET_CONTEXT_STATE_VERSION
}

/// Complete serializable state of a MarketContext.
///
/// Provides a stable, versioned snapshot of all market data that can be
/// persisted to JSON and reconstructed deterministically.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarketContextState {
    /// Schema version for format evolution.
    ///
    /// - **1**: initial stable snapshot format.
    /// - **2**: adds optional market data hierarchy snapshots.
    #[serde(default = "default_market_context_state_version")]
    pub version: u32,
    /// All curves (discount, forward, hazard, inflation, base correlation)
    #[schemars(with = "serde_json::Value")]
    pub curves: Vec<CurveState>,
    /// FX matrix state (optional)
    #[schemars(with = "serde_json::Value")]
    pub fx: Option<FxMatrixState>,
    /// Volatility surfaces
    #[schemars(with = "serde_json::Value")]
    pub surfaces: Vec<VolSurface>,
    /// Market scalars and prices
    #[schemars(with = "serde_json::Value")]
    pub prices: std::collections::BTreeMap<String, MarketScalar>,
    /// Generic time series
    #[schemars(with = "serde_json::Value")]
    pub series: Vec<ScalarTimeSeries>,
    /// Inflation indices
    #[schemars(with = "serde_json::Value")]
    pub inflation_indices: Vec<InflationIndex>,
    /// Dividend schedules
    #[schemars(with = "serde_json::Value")]
    pub dividends: Vec<DividendSchedule>,
    /// Credit index aggregates (references curves by ID)
    #[schemars(with = "serde_json::Value")]
    pub credit_indices: Vec<CreditIndexState>,
    /// FX delta-quoted volatility surfaces
    #[serde(default)]
    #[schemars(with = "serde_json::Value")]
    pub fx_delta_vol_surfaces: Vec<FxDeltaVolSurface>,
    /// SABR volatility cubes
    #[serde(default)]
    #[schemars(with = "serde_json::Value")]
    pub vol_cubes: Vec<VolCube>,
    /// Collateral CSA mappings
    pub collateral: std::collections::BTreeMap<String, String>,
    /// Optional market data hierarchy snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "serde_json::Value")]
    pub hierarchy: Option<MarketDataHierarchy>,
}

/// Build a quote-only [`FxMatrix`] from an
/// [`FxConfig`](crate::money::fx::FxConfig) and explicit `(from, to, rate)` quotes.
///
/// This is the same construction path used by [`MarketContext::try_from`] when
/// restoring an FX matrix from a persisted [`FxMatrixState`]. Exposed publicly so
/// that the v3 calibration envelope (which carries FX spots as a flat list of
/// [`crate::market_data::context::MarketContextState::fx`]-style quotes) can
/// materialize the same quote-only snapshot provider without duplicating the
/// internal `SnapshotFxProvider` plumbing.
pub fn build_snapshot_fx_matrix(
    config: crate::money::fx::FxConfig,
    quotes: Vec<(crate::currency::Currency, crate::currency::Currency, f64)>,
) -> crate::Result<Arc<FxMatrix>> {
    let state = FxMatrixState { config, quotes };
    let provider: Arc<dyn FxProvider> = Arc::new(SnapshotFxProvider::from_state(&state));
    let matrix = FxMatrix::try_with_config(provider, state.config)?;
    matrix.load_from_state(&state)?;
    Ok(Arc::new(matrix))
}

/// Quote-only FX provider used when restoring persisted market snapshots.
///
/// Snapshot restore is intentionally limited to the explicit quotes captured in
/// [`FxMatrixState`]. This avoids pretending that an arbitrary live provider can
/// be reconstructed from serialized cache state alone.
#[derive(Default)]
struct SnapshotFxProvider {
    quotes: std::collections::BTreeMap<(crate::currency::Currency, crate::currency::Currency), f64>,
}

impl SnapshotFxProvider {
    fn from_state(state: &FxMatrixState) -> Self {
        let quotes = state
            .quotes
            .iter()
            .map(|(from, to, rate)| ((*from, *to), *rate))
            .collect();
        Self { quotes }
    }
}

impl FxProvider for SnapshotFxProvider {
    fn rate(
        &self,
        from: crate::currency::Currency,
        to: crate::currency::Currency,
        _on: crate::dates::Date,
        _policy: FxConversionPolicy,
    ) -> crate::Result<f64> {
        if from == to {
            return Ok(1.0);
        }
        if let Some(rate) = self.quotes.get(&(from, to)).copied() {
            return Ok(rate);
        }
        if let Some(rate) = self.quotes.get(&(to, from)).copied() {
            return reciprocal_rate_or_err(rate, to, from);
        }
        Err(crate::error::InputError::NotFound {
            id: format!("FX snapshot:{from}->{to}"),
        }
        .into())
    }
}

impl From<&MarketContext> for MarketContextState {
    fn from(ctx: &MarketContext) -> Self {
        // Convert all curves (sort deterministically by id for stable snapshots).
        let mut curves: Vec<CurveState> = ctx
            .curves
            .values()
            .map(|storage| storage.to_state())
            .collect();
        curves.sort_by(|a, b| curve_state_id(a).cmp(curve_state_id(b)));

        // Convert FX (if present)
        let fx = ctx.fx.as_ref().map(|fx| fx.get_serializable_state());

        // Convert all surfaces (sort deterministically by key id).
        let mut surfaces_pairs: Vec<(CurveId, VolSurface)> = ctx
            .surfaces
            .iter()
            .map(|(id, surf)| (id.clone(), (**surf).clone()))
            .collect();
        surfaces_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let surfaces: Vec<_> = surfaces_pairs.into_iter().map(|(_, surf)| surf).collect();

        // Convert prices (CurveId → String)
        let prices: std::collections::BTreeMap<String, _> = ctx
            .prices
            .iter()
            .map(|(id, scalar)| (id.to_string(), scalar.clone()))
            .collect();

        // Convert series (sort deterministically by key id).
        let mut series_pairs: Vec<(CurveId, ScalarTimeSeries)> = ctx
            .series
            .iter()
            .map(|(id, series)| (id.clone(), series.clone()))
            .collect();
        series_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let series: Vec<_> = series_pairs.into_iter().map(|(_, series)| series).collect();

        // Convert inflation indices (sort deterministically by key id).
        let mut inflation_pairs: Vec<(CurveId, InflationIndex)> = ctx
            .inflation_indices
            .iter()
            .map(|(id, idx)| (id.clone(), (**idx).clone()))
            .collect();
        inflation_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let inflation_indices: Vec<_> = inflation_pairs.into_iter().map(|(_, idx)| idx).collect();

        // Convert credit indices (extract IDs from Arc references; sort deterministically by id).
        let mut credit_pairs: Vec<(CurveId, CreditIndexState)> = ctx
            .credit_indices
            .iter()
            .map(|(id, data)| {
                let issuer_ids: Option<std::collections::BTreeMap<String, String>> =
                    data.issuer_credit_curves.as_ref().map(|map| {
                        map.iter()
                            .map(|(issuer, curve)| (issuer.clone(), curve.id().to_string()))
                            .collect()
                    });
                let issuer_recovery_rates: Option<std::collections::BTreeMap<String, f64>> = data
                    .issuer_recovery_rates
                    .as_ref()
                    .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect());
                let issuer_weights: Option<std::collections::BTreeMap<String, f64>> = data
                    .issuer_weights
                    .as_ref()
                    .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect());

                (
                    id.clone(),
                    CreditIndexState {
                        id: id.to_string(),
                        num_constituents: data.num_constituents,
                        recovery_rate: data.recovery_rate,
                        index_credit_curve_id: data.index_credit_curve.id().to_string(),
                        base_correlation_curve_id: data.base_correlation_curve.id().to_string(),
                        issuer_credit_curve_ids: issuer_ids,
                        issuer_recovery_rates,
                        issuer_weights,
                    },
                )
            })
            .collect();
        credit_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let credit_indices: Vec<CreditIndexState> =
            credit_pairs.into_iter().map(|(_, s)| s).collect();

        // Convert dividends (sort deterministically by id).
        let mut dividend_pairs: Vec<(CurveId, DividendSchedule)> = ctx
            .dividends
            .iter()
            .map(|(id, divs)| (id.clone(), (**divs).clone()))
            .collect();
        dividend_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let dividends: Vec<_> = dividend_pairs.into_iter().map(|(_, d)| d).collect();

        // Convert FX delta vol surfaces (sort deterministically by key id).
        let mut fx_delta_pairs: Vec<(CurveId, FxDeltaVolSurface)> = ctx
            .fx_delta_vol_surfaces
            .iter()
            .map(|(id, surf)| (id.clone(), (**surf).clone()))
            .collect();
        fx_delta_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let fx_delta_vol_surfaces: Vec<_> =
            fx_delta_pairs.into_iter().map(|(_, surf)| surf).collect();

        // Convert vol cubes (sort deterministically by key id).
        let mut vol_cube_pairs: Vec<(CurveId, VolCube)> = ctx
            .vol_cubes
            .iter()
            .map(|(id, cube)| (id.clone(), (**cube).clone()))
            .collect();
        vol_cube_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let vol_cubes: Vec<_> = vol_cube_pairs.into_iter().map(|(_, cube)| cube).collect();

        // Convert collateral mappings
        let collateral: std::collections::BTreeMap<String, String> = ctx
            .collateral
            .iter()
            .map(|(csa, curve_id)| (csa.clone(), curve_id.to_string()))
            .collect();

        MarketContextState {
            version: MARKET_CONTEXT_STATE_VERSION,
            curves,
            fx,
            surfaces,
            prices,
            series,
            inflation_indices,
            dividends,
            credit_indices,
            fx_delta_vol_surfaces,
            vol_cubes,
            collateral,
            hierarchy: ctx.hierarchy.clone(),
        }
    }
}

impl TryFrom<MarketContextState> for MarketContext {
    type Error = crate::Error;

    fn try_from(state: MarketContextState) -> crate::Result<Self> {
        if !(1..=MARKET_CONTEXT_STATE_VERSION).contains(&state.version) {
            return Err(crate::Error::Validation(format!(
                "Unsupported MarketContextState version: {} (expected 1..={})",
                state.version, MARKET_CONTEXT_STATE_VERSION
            )));
        }
        let mut ctx = MarketContext::new();

        // Reconstruct all curves
        for curve_state in state.curves {
            let storage = CurveStorage::from_state(curve_state);
            ctx.curves.insert(storage.id().clone(), storage);
        }

        // Reconstruct all surfaces
        for surface in state.surfaces {
            ctx.surfaces.insert(surface.id().clone(), Arc::new(surface));
        }

        // Reconstruct FX matrix as a quote-only snapshot. Persisted state does not
        // encode the original live provider, only the captured explicit quotes.
        if let Some(fx_state) = state.fx {
            tracing::info!(
                explicit_quote_count = fx_state.quotes.len(),
                "restoring MarketContext FX as quote-only snapshot"
            );
            let provider: Arc<dyn FxProvider> = Arc::new(SnapshotFxProvider::from_state(&fx_state));
            let matrix = FxMatrix::try_with_config(Arc::clone(&provider), fx_state.config)?;
            matrix.load_from_state(&fx_state)?;
            ctx.fx = Some(Arc::new(matrix));
        }

        // Reconstruct prices
        for (id_str, scalar) in state.prices {
            ctx.prices.insert(CurveId::from(id_str), scalar);
        }

        // Reconstruct series
        for series in state.series {
            ctx.series.insert(series.id().clone(), series);
        }

        // Reconstruct inflation indices
        for idx in state.inflation_indices {
            let id = MarketContext::inflation_index_key_for_insert(idx.id.clone(), &idx);
            ctx.inflation_indices.insert(id, Arc::new(idx));
        }

        // Reconstruct dividends
        for schedule in state.dividends {
            let id = schedule.id.clone();
            ctx.dividends.insert(id, Arc::new(schedule));
        }

        // Reconstruct credit indices (resolve curve references)
        for credit_state in state.credit_indices {
            // Resolve hazard curve
            let index_curve = ctx.get_hazard(&credit_state.index_credit_curve_id)?;

            // Resolve base correlation curve
            let base_corr = ctx.get_base_correlation(&credit_state.base_correlation_curve_id)?;

            // Resolve issuer curves if present
            let issuer_curves = if let Some(issuer_ids) = credit_state.issuer_credit_curve_ids {
                let mut map = HashMap::default();
                for (issuer, curve_id) in issuer_ids {
                    let curve = ctx.get_hazard(&curve_id)?;
                    map.insert(issuer, curve);
                }
                Some(map)
            } else {
                None
            };

            let data = crate::market_data::term_structures::CreditIndexData {
                num_constituents: credit_state.num_constituents,
                recovery_rate: credit_state.recovery_rate,
                index_credit_curve: index_curve,
                base_correlation_curve: base_corr,
                issuer_credit_curves: issuer_curves,
                issuer_recovery_rates: credit_state
                    .issuer_recovery_rates
                    .map(|m| m.into_iter().collect::<HashMap<_, _>>()),
                issuer_weights: credit_state
                    .issuer_weights
                    .map(|m| m.into_iter().collect::<HashMap<_, _>>()),
            };

            ctx.credit_indices
                .insert(CurveId::from(credit_state.id), Arc::new(data));
        }
        let _invalidated = ctx.rebind_all_credit_indices();

        // Reconstruct FX delta vol surfaces
        for surface in state.fx_delta_vol_surfaces {
            let id = surface.id().to_owned();
            ctx.fx_delta_vol_surfaces.insert(id, Arc::new(surface));
        }

        // Reconstruct vol cubes
        for cube in state.vol_cubes {
            let id = cube.id().to_owned();
            ctx.vol_cubes.insert(id, Arc::new(cube));
        }

        // Reconstruct collateral mappings
        for (csa, curve_id_str) in state.collateral {
            ctx.collateral.insert(csa, CurveId::from(curve_id_str));
        }

        ctx.hierarchy = state.hierarchy;

        Ok(ctx)
    }
}

// -----------------------------------------------------------------------------
// Optional Serialize/Deserialize for MarketContext (via State)
// -----------------------------------------------------------------------------

impl serde::Serialize for MarketContext {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let state: MarketContextState = self.into();
        state.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for MarketContext {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let state = MarketContextState::deserialize(deserializer)?;
        Self::try_from(state).map_err(serde::de::Error::custom)
    }
}
