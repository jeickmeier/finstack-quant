//! Serializable state representations for [`MarketContext`](super::MarketContext).
//!
//! This submodule defines the serde-facing snapshot types used to persist and
//! restore market contexts, including typed curve state, FX state, and scalar
//! containers.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::collections::HashSet;
use crate::contract::{
    deserialize_json_value, parse_json_value, ContractDescriptor, ContractError, Diagnostic,
    LoadLimits, LoadPhase, Severity, ValidationReport,
};
use crate::types::CurveId;
use crate::wire::SchemaVersion;

use super::{CurveStorage, MarketContext};

use crate::market_data::{
    dividends::DividendSchedule,
    hierarchy::MarketDataHierarchy,
    scalars::{InflationIndex, MarketScalar, ScalarTimeSeries},
    surfaces::{FxDeltaVolSurface, VolCube, VolSurface},
    term_structures::{
        BaseCorrelationCurve, BasisSpreadCurve, DiscountCurve, ForwardCurve, HazardCurve,
        InflationCurve, ParametricCurve, PriceCurve,
    },
};
use crate::money::fx::{FxMatrix, FxMatrixState, SimpleFxProvider};

// Serde: CurveState and (De)Serialize impls

macro_rules! define_curve_state {
    ($( $variant:ident => {
        accessor: $accessor:ident,
        ty: $ty:ident,
        type_name: $type_name:literal
    } ),* $(,)?) => {
        /// Serializable state representation for any curve type.
        ///
        /// Produced when persisting market data snapshots through serde.
        #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

// Credit Index State (for serialization of CreditIndexData)

/// Serializable state for credit index data.
///
/// Instead of serializing `Arc<Curve>` directly, we store curve IDs that
/// reference curves present in the `MarketContextState`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

// Market Context State (complete snapshot)

/// Current schema version for [`MarketContextState`].
pub const MARKET_CONTEXT_STATE_VERSION: u32 = 1;

/// Persistence contract for [`MarketContextState`].
pub const MARKET_CONTEXT_STATE_CONTRACT: ContractDescriptor =
    ContractDescriptor::new("finstack_quant.market_context_state");

fn deserialize_required_hierarchy<'de, D>(
    deserializer: D,
) -> core::result::Result<Option<MarketDataHierarchy>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <Option<MarketDataHierarchy> as serde::Deserialize>::deserialize(deserializer)
}

/// Complete serializable state of a MarketContext.
///
/// Provides a stable, versioned snapshot of all market data that can be
/// persisted to JSON and reconstructed deterministically.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
// Every key except `fx` is mandatory. `hierarchy` is listed here rather than
// carrying `#[cfg_attr(feature = "json-schema", schemars(required))]` because that attribute also strips the
// field's `null` branch, which would make the schema reject payloads serde
// accepts. `market_context_state_schema_permits_explicit_null_hierarchy` keeps
// this list in step with the field set.
#[cfg_attr(feature = "json-schema", schemars(extend("required" = [
    "schema_version",
    "curves",
    "surfaces",
    "prices",
    "series",
    "inflation_indices",
    "dividends",
    "credit_indices",
    "fx_delta_vol_surfaces",
    "vol_cubes",
    "collateral",
    "hierarchy",
])))]
pub struct MarketContextState {
    /// Required schema version. Only version `1` is accepted.
    pub schema_version: SchemaVersion,
    /// All curves (discount, forward, hazard, inflation, base correlation)
    pub curves: Vec<CurveState>,
    /// FX matrix state (optional)
    pub fx: Option<FxMatrixState>,
    /// Volatility surfaces
    pub surfaces: Vec<VolSurface>,
    /// Market scalars and prices
    pub prices: std::collections::BTreeMap<String, MarketScalar>,
    /// Generic time series
    pub series: Vec<ScalarTimeSeries>,
    /// Inflation indices
    pub inflation_indices: Vec<InflationIndex>,
    /// Dividend schedules
    pub dividends: Vec<DividendSchedule>,
    /// Credit index aggregates (references curves by ID)
    pub credit_indices: Vec<CreditIndexState>,
    /// FX delta-quoted volatility surfaces
    pub fx_delta_vol_surfaces: Vec<FxDeltaVolSurface>,
    /// SABR volatility cubes
    pub vol_cubes: Vec<VolCube>,
    /// Collateral CSA mappings
    pub collateral: std::collections::BTreeMap<String, String>,
    /// Market data hierarchy snapshot, or `null` when no hierarchy is configured.
    ///
    /// The key is mandatory but its value is nullable: `deserialize_with` makes
    /// serde demand the key, and the struct-level `required` extension keeps it
    /// mandatory in the schema without stripping the `null` branch.
    #[serde(deserialize_with = "deserialize_required_hierarchy")]
    pub hierarchy: Option<MarketDataHierarchy>,
}

impl MarketContextState {
    /// Compute the versioned SHA-256 hash of this state's canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the state contains a non-finite number or cannot be
    /// serialized to the core canonical JSON representation.
    pub fn content_hash(&self) -> crate::Result<String> {
        crate::canonical::content_hash(self)
    }
}

fn duplicate_id_diagnostics(
    report: &mut ValidationReport,
    limits: &LoadLimits,
    field: &str,
    ids: impl IntoIterator<Item = String>,
) {
    let mut seen = HashSet::default();
    for (index, id) in ids.into_iter().enumerate() {
        if !seen.insert(id.clone()) {
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "market-context/duplicate-id",
                    LoadPhase::Semantic,
                    Severity::Error,
                    format!("duplicate id '{id}' in MarketContextState.{field}"),
                )
                .with_pointer(format!("/{field}/{index}"))
                .with_contract(MARKET_CONTEXT_STATE_CONTRACT.id),
            );
        }
    }
}

fn validate_restore_references(
    state: &MarketContextState,
    limits: &LoadLimits,
) -> ValidationReport {
    let mut report = ValidationReport::default();
    duplicate_id_diagnostics(
        &mut report,
        limits,
        "curves",
        state.curves.iter().map(|entry| entry.id().to_string()),
    );
    duplicate_id_diagnostics(
        &mut report,
        limits,
        "surfaces",
        state.surfaces.iter().map(|entry| entry.id().to_string()),
    );
    duplicate_id_diagnostics(
        &mut report,
        limits,
        "series",
        state.series.iter().map(|entry| entry.id().to_string()),
    );
    duplicate_id_diagnostics(
        &mut report,
        limits,
        "inflation_indices",
        state.inflation_indices.iter().map(|entry| entry.id.clone()),
    );
    duplicate_id_diagnostics(
        &mut report,
        limits,
        "dividends",
        state
            .dividends
            .iter()
            .map(|entry| entry.get_id().to_string()),
    );
    duplicate_id_diagnostics(
        &mut report,
        limits,
        "credit_indices",
        state.credit_indices.iter().map(|entry| entry.id.clone()),
    );
    duplicate_id_diagnostics(
        &mut report,
        limits,
        "fx_delta_vol_surfaces",
        state
            .fx_delta_vol_surfaces
            .iter()
            .map(|entry| entry.id().to_string()),
    );
    duplicate_id_diagnostics(
        &mut report,
        limits,
        "vol_cubes",
        state.vol_cubes.iter().map(|entry| entry.id().to_string()),
    );

    let discount_ids: HashSet<&str> = state
        .curves
        .iter()
        .filter_map(|entry| match entry {
            CurveState::Discount(curve) => Some(curve.id().as_str()),
            _ => None,
        })
        .collect();
    let hazard_ids: HashSet<&str> = state
        .curves
        .iter()
        .filter_map(|entry| match entry {
            CurveState::Hazard(curve) => Some(curve.id().as_str()),
            _ => None,
        })
        .collect();
    let base_correlation_ids: HashSet<&str> = state
        .curves
        .iter()
        .filter_map(|entry| match entry {
            CurveState::BaseCorrelation(curve) => Some(curve.id().as_str()),
            _ => None,
        })
        .collect();

    for (csa, curve_id) in &state.collateral {
        if !discount_ids.contains(curve_id.as_str()) {
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "market-context/missing-collateral-curve",
                    LoadPhase::Semantic,
                    Severity::Error,
                    format!(
                        "collateral mapping '{csa}' references missing discount curve '{curve_id}'"
                    ),
                )
                .with_pointer(format!("/collateral/{}", escape_json_pointer(csa)))
                .with_contract(MARKET_CONTEXT_STATE_CONTRACT.id),
            );
        }
    }

    for (index, credit_index) in state.credit_indices.iter().enumerate() {
        let mut check_reference = |curve_id: &str, expected: &str, suffix: &str, present: bool| {
            if !present {
                report.push_bounded(
                    limits,
                    Diagnostic::new(
                        "market-context/missing-credit-index-reference",
                        LoadPhase::Semantic,
                        Severity::Error,
                        format!(
                            "credit index '{}' references missing {expected} curve '{curve_id}'",
                            credit_index.id
                        ),
                    )
                    .with_pointer(format!("/credit_indices/{index}/{suffix}"))
                    .with_contract(MARKET_CONTEXT_STATE_CONTRACT.id),
                );
            }
        };
        check_reference(
            &credit_index.index_credit_curve_id,
            "hazard",
            "index_credit_curve_id",
            hazard_ids.contains(credit_index.index_credit_curve_id.as_str()),
        );
        check_reference(
            &credit_index.base_correlation_curve_id,
            "base-correlation",
            "base_correlation_curve_id",
            base_correlation_ids.contains(credit_index.base_correlation_curve_id.as_str()),
        );
        if let Some(issuer_ids) = &credit_index.issuer_credit_curve_ids {
            for (issuer, curve_id) in issuer_ids {
                check_reference(
                    curve_id,
                    "issuer hazard",
                    &format!("issuer_credit_curve_ids/{}", escape_json_pointer(issuer)),
                    hazard_ids.contains(curve_id.as_str()),
                );
            }
        }
    }
    report
}

fn escape_json_pointer(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

/// Build a quote-only [`FxMatrix`] from an
/// [`FxConfig`](crate::money::fx::FxConfig) and explicit `(from, to, rate)` quotes.
///
/// This is the same construction path used by [`MarketContext::try_from`] when
/// restoring an FX matrix from a persisted [`FxMatrixState`]. Exposed publicly so
/// that the v3 calibration envelope (which carries FX spots as a flat list of
/// [`crate::market_data::context::MarketContextState::fx`]-style quotes) can
/// materialize an explicit quote matrix without duplicating FX storage.
///
/// Each quote is units of `to` per one unit of `from`. The matrix contains
/// only the supplied explicit quotes and their reciprocal lookup; it carries
/// no live-feed, historical, or policy-date behavior. The resulting matrix uses
/// `config` for cache, policy, and triangulation settings, while its initial
/// global quotes come from `quotes`.
///
/// # Errors
///
/// Returns an error if the FX configuration is invalid or any quote cannot be
/// loaded into the matrix (for example, it is non-finite or not strictly
/// positive). No matrix is returned on failure.
///
/// # Arguments
///
/// * `config` - FX policy, caching, and triangulation configuration used by the
///   reconstructed matrix.
/// * `quotes` - Owned direct FX quotes as `(from, to, rate)`, with `rate` in
///   units of `to` per one unit of `from`.
pub fn build_snapshot_fx_matrix(
    config: crate::money::fx::FxConfig,
    quotes: Vec<(crate::currency::Currency, crate::currency::Currency, f64)>,
) -> crate::Result<Arc<FxMatrix>> {
    let matrix = FxMatrix::try_with_config(Arc::new(SimpleFxProvider::new()), config)?;
    matrix.set_quotes(&quotes)?;
    Ok(Arc::new(matrix))
}

impl From<&MarketContext> for MarketContextState {
    fn from(ctx: &MarketContext) -> Self {
        let mut curves: Vec<CurveState> = ctx
            .curves
            .values()
            .map(|storage| storage.to_state())
            .collect();
        curves.sort_by(|a, b| a.id().cmp(b.id()));

        let fx = ctx.fx.as_ref().map(|fx| fx.get_serializable_state());

        let mut surfaces_pairs: Vec<(CurveId, VolSurface)> = ctx
            .surfaces
            .iter()
            .map(|(id, surf)| (id.clone(), (**surf).clone()))
            .collect();
        surfaces_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let surfaces: Vec<_> = surfaces_pairs.into_iter().map(|(_, surf)| surf).collect();

        let prices: std::collections::BTreeMap<String, _> = ctx
            .prices
            .iter()
            .map(|(id, scalar)| (id.to_string(), scalar.clone()))
            .collect();

        let mut series_pairs: Vec<(CurveId, ScalarTimeSeries)> = ctx
            .series
            .iter()
            .map(|(id, series)| (id.clone(), series.clone()))
            .collect();
        series_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let series: Vec<_> = series_pairs.into_iter().map(|(_, series)| series).collect();

        let mut inflation_pairs: Vec<(CurveId, InflationIndex)> = ctx
            .inflation_indices
            .iter()
            .map(|(id, idx)| (id.clone(), (**idx).clone()))
            .collect();
        inflation_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let inflation_indices: Vec<_> = inflation_pairs.into_iter().map(|(_, idx)| idx).collect();

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

        let mut dividend_pairs: Vec<(CurveId, DividendSchedule)> = ctx
            .dividends
            .iter()
            .map(|(id, divs)| (id.clone(), (**divs).clone()))
            .collect();
        dividend_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let dividends: Vec<_> = dividend_pairs.into_iter().map(|(_, d)| d).collect();

        let mut fx_delta_pairs: Vec<(CurveId, FxDeltaVolSurface)> = ctx
            .fx_delta_vol_surfaces
            .iter()
            .map(|(id, surf)| (id.clone(), (**surf).clone()))
            .collect();
        fx_delta_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let fx_delta_vol_surfaces: Vec<_> =
            fx_delta_pairs.into_iter().map(|(_, surf)| surf).collect();

        let mut vol_cube_pairs: Vec<(CurveId, VolCube)> = ctx
            .vol_cubes
            .iter()
            .map(|(id, cube)| (id.clone(), (**cube).clone()))
            .collect();
        vol_cube_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let vol_cubes: Vec<_> = vol_cube_pairs.into_iter().map(|(_, cube)| cube).collect();

        let collateral: std::collections::BTreeMap<String, String> = ctx
            .collateral
            .iter()
            .map(|(csa, curve_id)| (csa.clone(), curve_id.to_string()))
            .collect();

        MarketContextState {
            schema_version: SchemaVersion::CURRENT,
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

fn restore_market_context(
    state: MarketContextState,
    limits: &LoadLimits,
) -> Result<(MarketContext, ValidationReport), ContractError> {
    let mut report = validate_restore_references(&state, limits);
    if report.has_errors() {
        return Err(ContractError::Report(Box::new(report)));
    }
    let mut ctx = MarketContext::new();

    for curve_state in state.curves {
        let storage = CurveStorage::from_state(curve_state);
        Arc::make_mut(&mut ctx.curves).insert(storage.id().clone(), storage);
    }

    for surface in state.surfaces {
        Arc::make_mut(&mut ctx.surfaces).insert(surface.id().clone(), Arc::new(surface));
    }

    // Persisted state does not encode the original live FX provider, only the
    // captured global, pinned, and provider quotes.
    if let Some(fx_state) = state.fx {
        tracing::info!(
            explicit_quote_count = fx_state.quotes.len(),
            "restoring MarketContext FX as quote-only snapshot"
        );
        let provider = SimpleFxProvider::new();
        provider.set_quotes(&fx_state.provider_quotes)?;
        let matrix = FxMatrix::try_with_config(Arc::new(provider), fx_state.config)?;
        matrix.load_from_state(&fx_state)?;
        ctx.fx = Some(Arc::new(matrix));
    }

    for (id_str, scalar) in state.prices {
        Arc::make_mut(&mut ctx.prices).insert(CurveId::from(id_str), scalar);
    }

    for series in state.series {
        Arc::make_mut(&mut ctx.series).insert(series.id().clone(), series);
    }

    for idx in state.inflation_indices {
        let id = MarketContext::inflation_index_key_for_insert(idx.id.clone(), &idx);
        Arc::make_mut(&mut ctx.inflation_indices).insert(id, Arc::new(idx));
    }

    for schedule in state.dividends {
        let id = schedule.get_id().clone();
        Arc::make_mut(&mut ctx.dividends).insert(id, Arc::new(schedule));
    }

    for credit_state in state.credit_indices {
        let index_curve = ctx.get_hazard(&credit_state.index_credit_curve_id)?;
        let base_corr = ctx.get_base_correlation(&credit_state.base_correlation_curve_id)?;
        let issuer_curves = if let Some(issuer_ids) = credit_state.issuer_credit_curve_ids {
            let mut map = BTreeMap::new();
            for (issuer, curve_id) in issuer_ids {
                let curve = ctx.get_hazard(&curve_id)?;
                map.insert(issuer, curve);
            }
            Some(map)
        } else {
            None
        };

        let mut builder = crate::market_data::term_structures::CreditIndexData::builder()
            .num_constituents(credit_state.num_constituents)
            .recovery_rate(credit_state.recovery_rate)
            .index_credit_curve(index_curve)
            .base_correlation_curve(base_corr);
        if let Some(issuer_curves) = issuer_curves {
            builder = builder.issuer_curves(issuer_curves);
        }
        if let Some(recovery_rates) = credit_state.issuer_recovery_rates {
            builder = builder.issuer_recovery_rates(recovery_rates);
        }
        if let Some(weights) = credit_state.issuer_weights {
            builder = builder.issuer_weights(weights);
        }
        let data = builder.build()?;

        Arc::make_mut(&mut ctx.credit_indices)
            .insert(CurveId::from(credit_state.id), Arc::new(data));
    }
    let invalidated = ctx.rebind_all_credit_indices();
    for id in invalidated {
        report.push_bounded(
                limits,
                Diagnostic::new(
                    "market-context/credit-index-rebind-failed",
                    LoadPhase::Build,
                    Severity::Error,
                    format!(
                        "credit index '{}' was dropped after its persisted curve references failed to rebind",
                        id.as_str()
                    ),
                )
                .with_pointer("/credit_indices")
                .with_contract(MARKET_CONTEXT_STATE_CONTRACT.id),
            );
    }

    for surface in state.fx_delta_vol_surfaces {
        let id = surface.id().to_owned();
        Arc::make_mut(&mut ctx.fx_delta_vol_surfaces).insert(id, Arc::new(surface));
    }

    for cube in state.vol_cubes {
        let id = cube.id().to_owned();
        Arc::make_mut(&mut ctx.vol_cubes).insert(id, Arc::new(cube));
    }

    for (csa, curve_id_str) in state.collateral {
        Arc::make_mut(&mut ctx.collateral).insert(csa, CurveId::from(curve_id_str));
    }

    ctx.hierarchy = state.hierarchy;

    if report.has_errors() {
        Err(ContractError::Report(Box::new(report)))
    } else {
        Ok((ctx, report))
    }
}

impl TryFrom<MarketContextState> for MarketContext {
    type Error = crate::Error;

    fn try_from(state: MarketContextState) -> crate::Result<Self> {
        match restore_market_context(state, &LoadLimits::default()) {
            Ok((context, _report)) => Ok(context),
            Err(ContractError::Core(error)) => Err(error),
            Err(ContractError::Report(report)) => Err(crate::Error::Validation(format!(
                "MarketContextState restore failed with {report} error(s)"
            ))),
            Err(error) => Err(crate::Error::Validation(error.to_string())),
        }
    }
}

impl MarketContext {
    /// Load a persisted market-context state under strict contract policy.
    ///
    /// Unlike ordinary serde deserialization, this entry point requires an
    /// explicit `schema_version` key. It enforces encoded byte and JSON-depth limits,
    /// validates the version, and then restores the complete
    /// market context.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete UTF-8 JSON encoding of a
    ///   [`MarketContextState`].
    /// * `limits` - Resource policy bounding input size, JSON depth, and
    ///   retained diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] for malformed JSON, resource-limit failures,
    /// missing or unsupported versions, invalid state shape, duplicate IDs,
    /// unresolved credit-index or collateral references, or failed market
    /// context reconstruction. Semantic findings are retained up to
    /// [`LoadLimits::max_diagnostics`].
    pub fn from_state_slice(
        bytes: &[u8],
        limits: &LoadLimits,
    ) -> Result<(Self, ValidationReport), ContractError> {
        let value = parse_json_value(bytes, limits)?;
        let version = match value.get("schema_version") {
            Some(version) => Some(
                deserialize_json_value::<u32>(version.clone(), limits).map_err(
                    |error| match error {
                        ContractError::Report(mut report) => {
                            for diagnostic in &mut report.diagnostics {
                                if diagnostic.pointer.is_none() {
                                    diagnostic.pointer = Some("/schema_version".to_string());
                                }
                            }
                            ContractError::Report(report)
                        }
                        other => other,
                    },
                )?,
            ),
            None => None,
        };
        MARKET_CONTEXT_STATE_CONTRACT.resolve_strict(version, "/schema_version", limits)?;
        let state: MarketContextState = deserialize_json_value(value, limits)?;
        restore_market_context(state, limits)
    }
}

// Optional Serialize/Deserialize for MarketContext (via State)

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
