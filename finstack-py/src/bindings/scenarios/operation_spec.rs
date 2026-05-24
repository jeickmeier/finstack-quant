//! Typed Python builders for [`finstack_scenarios::OperationSpec`] and supporting enums.
//!
//! This module replaces the raw-JSON authoring path with typed classmethod
//! constructors. Each classmethod constructs the Rust enum variant directly and
//! delegates serialization to serde, so the JSON wire format is guaranteed to
//! match what [`finstack_scenarios::ScenarioSpec`] deserializes.
//!
//! # Round-trip strategy
//!
//! - Python builders take Python-native arguments (strings, lists, floats).
//! - The classmethod converts those into the Rust enum/struct types directly.
//! - `to_json` and `from_json` use `serde_json` on the underlying Rust types,
//!   so the wire contract follows the serde attributes on the Rust types
//!   (notably `#[serde(tag = "kind", rename_all = "snake_case")]` on
//!   `OperationSpec` and `rename = "forward"` / `rename = "par_cds"` on the
//!   `CurveKind` variants).

use std::str::FromStr;

use crate::errors::display_to_py;
use finstack_core::currency::Currency;
use finstack_core::market_data::hierarchy::HierarchyTarget;
use finstack_core::types::CurveId;
use finstack_scenarios::spec::{
    Compounding, CurveKind, OperationSpec, RateBindingSpec, TenorMatchMode, TimeRollMode,
    VolSurfaceKind,
};
use finstack_statements::types::NodeId;
use finstack_valuations::pricer::InstrumentType;
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::PyType;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_currency(code: &str) -> PyResult<Currency> {
    Currency::from_str(code)
        .map_err(|e| crate::errors::value_error(format!("Invalid currency code {code:?}: {e}")))
}

fn parse_instrument_type(name: &str) -> PyResult<InstrumentType> {
    InstrumentType::from_str(name)
        .map_err(|e| crate::errors::value_error(format!("Invalid instrument type {name:?}: {e}")))
}

fn parse_instrument_types(names: Vec<String>) -> PyResult<Vec<InstrumentType>> {
    names.iter().map(|s| parse_instrument_type(s)).collect()
}

fn parse_attrs(pairs: Vec<(String, String)>) -> IndexMap<String, String> {
    let mut map = IndexMap::with_capacity(pairs.len());
    for (k, v) in pairs {
        map.insert(k, v);
    }
    map
}

fn parse_hierarchy_target(json: &str) -> PyResult<HierarchyTarget> {
    serde_json::from_str(json)
        .map_err(|e| crate::errors::value_error(format!("Invalid HierarchyTarget JSON: {e}")))
}

// ---------------------------------------------------------------------------
// CurveKind
// ---------------------------------------------------------------------------

/// Type of market curve targeted by a scenario operation.
///
/// Mirrors [`finstack_scenarios::CurveKind`]. Serde renames `forward` and
/// `par_cds` are preserved on the JSON wire format.
#[pyclass(
    name = "CurveKind",
    module = "finstack.scenarios",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyCurveKind {
    pub(crate) inner: CurveKind,
}

#[pymethods]
impl PyCurveKind {
    /// Discount factor curve.
    #[classmethod]
    fn discount(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: CurveKind::Discount,
        }
    }

    /// Forward rate curve.
    #[classmethod]
    fn forward(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: CurveKind::Forward,
        }
    }

    /// Par CDS spread curve.
    #[classmethod]
    fn par_cds(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: CurveKind::ParCDS,
        }
    }

    /// Inflation index curve.
    #[classmethod]
    fn inflation(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: CurveKind::Inflation,
        }
    }

    /// Commodity forward curve.
    #[classmethod]
    fn commodity(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: CurveKind::Commodity,
        }
    }

    /// Variant name, e.g. ``"Discount"``.
    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }

    /// Serialized wire value, e.g. ``"discount"`` or ``"par_cds"``.
    #[getter]
    fn value(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map(|s| s.trim_matches('"').to_string())
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!("CurveKind.{:?}", self.inner)
    }
}

// ---------------------------------------------------------------------------
// VolSurfaceKind
// ---------------------------------------------------------------------------

/// Category of volatility surface targeted by a scenario operation.
#[pyclass(
    name = "VolSurfaceKind",
    module = "finstack.scenarios",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyVolSurfaceKind {
    pub(crate) inner: VolSurfaceKind,
}

#[pymethods]
impl PyVolSurfaceKind {
    /// Equity volatility surface.
    #[classmethod]
    fn equity(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: VolSurfaceKind::Equity,
        }
    }

    /// Credit volatility surface.
    #[classmethod]
    fn credit(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: VolSurfaceKind::Credit,
        }
    }

    /// Swaption volatility surface.
    #[classmethod]
    fn swaption(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: VolSurfaceKind::Swaption,
        }
    }

    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }

    #[getter]
    fn value(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map(|s| s.trim_matches('"').to_string())
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!("VolSurfaceKind.{:?}", self.inner)
    }
}

// ---------------------------------------------------------------------------
// TenorMatchMode
// ---------------------------------------------------------------------------

/// Tenor-pillar alignment strategy for curve-node operations.
#[pyclass(
    name = "TenorMatchMode",
    module = "finstack.scenarios",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyTenorMatchMode {
    pub(crate) inner: TenorMatchMode,
}

#[pymethods]
impl PyTenorMatchMode {
    /// Match exact pillar only (errors if missing).
    #[classmethod]
    fn exact(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TenorMatchMode::Exact,
        }
    }

    /// Interpolate the bump across adjacent knots.
    #[classmethod]
    fn interpolate(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TenorMatchMode::Interpolate,
        }
    }

    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }

    #[getter]
    fn value(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map(|s| s.trim_matches('"').to_string())
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!("TenorMatchMode.{:?}", self.inner)
    }
}

// ---------------------------------------------------------------------------
// TimeRollMode
// ---------------------------------------------------------------------------

/// Calendar-vs-business-day semantics for time-roll operations.
#[pyclass(
    name = "TimeRollMode",
    module = "finstack.scenarios",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyTimeRollMode {
    pub(crate) inner: TimeRollMode,
}

#[pymethods]
impl PyTimeRollMode {
    /// Business-day-aware roll (respects calendars when provided).
    #[classmethod]
    fn business_days(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TimeRollMode::BusinessDays,
        }
    }

    /// Pure calendar-day arithmetic.
    #[classmethod]
    fn calendar_days(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TimeRollMode::CalendarDays,
        }
    }

    /// Approximate day-count mode (see Rust docs for non-additivity caveats).
    #[classmethod]
    fn approximate(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TimeRollMode::Approximate,
        }
    }

    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }

    #[getter]
    fn value(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map(|s| s.trim_matches('"').to_string())
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!("TimeRollMode.{:?}", self.inner)
    }
}

// ---------------------------------------------------------------------------
// Compounding
// ---------------------------------------------------------------------------

/// Compounding convention for rate-extraction operations.
#[pyclass(
    name = "Compounding",
    module = "finstack.scenarios",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyCompounding {
    pub(crate) inner: Compounding,
}

#[pymethods]
impl PyCompounding {
    /// Simple interest (no compounding).
    #[classmethod]
    fn simple(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::Simple,
        }
    }

    /// Continuous compounding (default).
    #[classmethod]
    fn continuous(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::Continuous,
        }
    }

    /// Annual compounding.
    #[classmethod]
    fn annual(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::Annual,
        }
    }

    /// Semi-annual compounding.
    #[classmethod]
    fn semi_annual(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::SemiAnnual,
        }
    }

    /// Quarterly compounding.
    #[classmethod]
    fn quarterly(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::Quarterly,
        }
    }

    /// Monthly compounding.
    #[classmethod]
    fn monthly(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::Monthly,
        }
    }

    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }

    #[getter]
    fn value(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map(|s| s.trim_matches('"').to_string())
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!("Compounding.{:?}", self.inner)
    }
}

// ---------------------------------------------------------------------------
// RateBindingSpec
// ---------------------------------------------------------------------------

/// Configuration linking a statement rate node to a market curve.
///
/// Mirrors [`finstack_scenarios::spec::RateBindingSpec`].
#[pyclass(
    name = "RateBindingSpec",
    module = "finstack.scenarios",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyRateBindingSpec {
    pub(crate) inner: RateBindingSpec,
}

#[pymethods]
impl PyRateBindingSpec {
    /// Construct a rate-binding specification.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Statement node identifier to receive the extracted rate.
    /// curve_id : str
    ///     Market curve identifier.
    /// tenor : str
    ///     Tenor at which to sample the curve (e.g. ``"1Y"``).
    /// compounding : Compounding, optional
    ///     Output compounding convention. Defaults to ``Compounding.continuous()``.
    /// day_count : str, optional
    ///     Day-count override (e.g. ``"act/360"``). ``None`` uses the curve's
    ///     native day count.
    #[new]
    #[pyo3(signature = (node_id, curve_id, tenor, compounding=None, day_count=None))]
    fn new(
        node_id: &str,
        curve_id: &str,
        tenor: &str,
        compounding: Option<PyRef<'_, PyCompounding>>,
        day_count: Option<String>,
    ) -> Self {
        let compounding = compounding.map(|c| c.inner).unwrap_or_default();
        Self {
            inner: RateBindingSpec {
                node_id: NodeId::from(node_id),
                curve_id: CurveId::from(curve_id),
                tenor: tenor.to_string(),
                compounding,
                day_count,
            },
        }
    }

    #[getter]
    fn node_id(&self) -> String {
        self.inner.node_id.as_str().to_string()
    }

    #[getter]
    fn curve_id(&self) -> String {
        self.inner.curve_id.as_str().to_string()
    }

    #[getter]
    fn tenor(&self) -> String {
        self.inner.tenor.clone()
    }

    #[getter]
    fn compounding(&self) -> PyCompounding {
        PyCompounding {
            inner: self.inner.compounding,
        }
    }

    #[getter]
    fn day_count(&self) -> Option<String> {
        self.inner.day_count.clone()
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize a `RateBindingSpec` from JSON.
    #[classmethod]
    fn from_json(_cls: &Bound<'_, PyType>, json: &str) -> PyResult<Self> {
        let inner: RateBindingSpec = serde_json::from_str(json).map_err(|e| {
            crate::errors::value_error(format!("Invalid RateBindingSpec JSON: {e}"))
        })?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "RateBindingSpec(node_id='{}', curve_id='{}', tenor='{}')",
            self.inner.node_id.as_str(),
            self.inner.curve_id.as_str(),
            self.inner.tenor,
        )
    }
}

// ---------------------------------------------------------------------------
// OperationSpec
// ---------------------------------------------------------------------------

/// Typed builder for [`finstack_scenarios::OperationSpec`].
///
/// Each classmethod constructor mirrors one Rust enum variant. Serialization
/// goes through serde so the JSON contract stays in lock-step with the Rust
/// type — Python callers should treat ``to_json`` output as the canonical wire
/// representation and pass it straight to ``build_scenario_spec`` or
/// ``ScenarioEngine`` consumers.
#[pyclass(
    name = "OperationSpec",
    module = "finstack.scenarios",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyOperationSpec {
    pub(crate) inner: OperationSpec,
}

#[pymethods]
impl PyOperationSpec {
    /// FX rate percent shift. ``pct = 5.0`` strengthens ``base`` by 5%.
    #[classmethod]
    #[pyo3(signature = (base, quote, pct))]
    fn market_fx_pct(
        _cls: &Bound<'_, PyType>,
        base: &str,
        quote: &str,
        pct: f64,
    ) -> PyResult<Self> {
        let base = parse_currency(base)?;
        let quote = parse_currency(quote)?;
        Ok(Self {
            inner: OperationSpec::MarketFxPct { base, quote, pct },
        })
    }

    /// Equity price percent shock applied to all supplied identifiers.
    #[classmethod]
    #[pyo3(signature = (ids, pct))]
    fn equity_price_pct(_cls: &Bound<'_, PyType>, ids: Vec<String>, pct: f64) -> Self {
        Self {
            inner: OperationSpec::EquityPricePct { ids, pct },
        }
    }

    /// Instrument price shock by exact attribute match.
    ///
    /// ``attrs`` is a list of ``(key, value)`` pairs preserving insertion order.
    #[classmethod]
    #[pyo3(signature = (attrs, pct))]
    fn instrument_price_pct_by_attr(
        _cls: &Bound<'_, PyType>,
        attrs: Vec<(String, String)>,
        pct: f64,
    ) -> Self {
        Self {
            inner: OperationSpec::InstrumentPricePctByAttr {
                attrs: parse_attrs(attrs),
                pct,
            },
        }
    }

    /// Parallel basis-point shift on a rate-style curve.
    #[classmethod]
    #[pyo3(signature = (curve_kind, curve_id, bp, discount_curve_id=None))]
    fn curve_parallel_bp(
        _cls: &Bound<'_, PyType>,
        curve_kind: PyRef<'_, PyCurveKind>,
        curve_id: &str,
        bp: f64,
        discount_curve_id: Option<String>,
    ) -> Self {
        Self {
            inner: OperationSpec::CurveParallelBp {
                curve_kind: curve_kind.inner,
                curve_id: CurveId::from(curve_id),
                discount_curve_id: discount_curve_id.map(CurveId::from),
                bp,
            },
        }
    }

    /// Node-level basis-point shifts on a rate-style curve.
    ///
    /// ``nodes`` is a list of ``(tenor, bp)`` pairs.
    #[classmethod]
    #[pyo3(signature = (curve_kind, curve_id, nodes, match_mode=None, discount_curve_id=None))]
    fn curve_node_bp(
        _cls: &Bound<'_, PyType>,
        curve_kind: PyRef<'_, PyCurveKind>,
        curve_id: &str,
        nodes: Vec<(String, f64)>,
        match_mode: Option<PyRef<'_, PyTenorMatchMode>>,
        discount_curve_id: Option<String>,
    ) -> Self {
        let match_mode = match_mode.map(|m| m.inner).unwrap_or_default();
        Self {
            inner: OperationSpec::CurveNodeBp {
                curve_kind: curve_kind.inner,
                curve_id: CurveId::from(curve_id),
                discount_curve_id: discount_curve_id.map(CurveId::from),
                nodes,
                match_mode,
            },
        }
    }

    /// Parallel shock to a volatility-index curve in absolute index points.
    #[classmethod]
    #[pyo3(signature = (curve_id, points))]
    fn vol_index_parallel_pts(_cls: &Bound<'_, PyType>, curve_id: &str, points: f64) -> Self {
        Self {
            inner: OperationSpec::VolIndexParallelPts {
                curve_id: CurveId::from(curve_id),
                points,
            },
        }
    }

    /// Node-level shocks to a volatility-index curve in absolute index points.
    #[classmethod]
    #[pyo3(signature = (curve_id, nodes, match_mode=None))]
    fn vol_index_node_pts(
        _cls: &Bound<'_, PyType>,
        curve_id: &str,
        nodes: Vec<(String, f64)>,
        match_mode: Option<PyRef<'_, PyTenorMatchMode>>,
    ) -> Self {
        let match_mode = match_mode.map(|m| m.inner).unwrap_or_default();
        Self {
            inner: OperationSpec::VolIndexNodePts {
                curve_id: CurveId::from(curve_id),
                nodes,
                match_mode,
            },
        }
    }

    /// Parallel shift to a base-correlation surface (absolute correlation points).
    #[classmethod]
    #[pyo3(signature = (surface_id, points))]
    fn base_corr_parallel_pts(_cls: &Bound<'_, PyType>, surface_id: &str, points: f64) -> Self {
        Self {
            inner: OperationSpec::BaseCorrParallelPts {
                surface_id: CurveId::from(surface_id),
                points,
            },
        }
    }

    /// Bucketed base-correlation shock by detachment and (reserved) maturity.
    #[classmethod]
    #[pyo3(signature = (surface_id, points, detachment_bps=None, maturities=None))]
    fn base_corr_bucket_pts(
        _cls: &Bound<'_, PyType>,
        surface_id: &str,
        points: f64,
        detachment_bps: Option<Vec<i32>>,
        maturities: Option<Vec<String>>,
    ) -> Self {
        Self {
            inner: OperationSpec::BaseCorrBucketPts {
                surface_id: CurveId::from(surface_id),
                detachment_bps,
                maturities,
                points,
            },
        }
    }

    /// Parallel percent shift to a volatility surface.
    #[classmethod]
    #[pyo3(signature = (surface_kind, surface_id, pct))]
    fn vol_surface_parallel_pct(
        _cls: &Bound<'_, PyType>,
        surface_kind: PyRef<'_, PyVolSurfaceKind>,
        surface_id: &str,
        pct: f64,
    ) -> Self {
        Self {
            inner: OperationSpec::VolSurfaceParallelPct {
                surface_kind: surface_kind.inner,
                surface_id: CurveId::from(surface_id),
                pct,
            },
        }
    }

    /// Bucketed volatility surface percent shock.
    #[classmethod]
    #[pyo3(signature = (surface_kind, surface_id, pct, tenors=None, strikes=None))]
    fn vol_surface_bucket_pct(
        _cls: &Bound<'_, PyType>,
        surface_kind: PyRef<'_, PyVolSurfaceKind>,
        surface_id: &str,
        pct: f64,
        tenors: Option<Vec<String>>,
        strikes: Option<Vec<f64>>,
    ) -> Self {
        Self {
            inner: OperationSpec::VolSurfaceBucketPct {
                surface_kind: surface_kind.inner,
                surface_id: CurveId::from(surface_id),
                tenors,
                strikes,
                pct,
            },
        }
    }

    /// Statement forecast percent change.
    #[classmethod]
    #[pyo3(signature = (node_id, pct))]
    fn stmt_forecast_percent(_cls: &Bound<'_, PyType>, node_id: &str, pct: f64) -> Self {
        Self {
            inner: OperationSpec::StmtForecastPercent {
                node_id: NodeId::from(node_id),
                pct,
            },
        }
    }

    /// Statement forecast value assignment.
    #[classmethod]
    #[pyo3(signature = (node_id, value))]
    fn stmt_forecast_assign(_cls: &Bound<'_, PyType>, node_id: &str, value: f64) -> Self {
        Self {
            inner: OperationSpec::StmtForecastAssign {
                node_id: NodeId::from(node_id),
                value,
            },
        }
    }

    /// Bind a statement rate node to a curve for the lifetime of the scenario.
    #[classmethod]
    #[pyo3(signature = (binding))]
    fn rate_binding(_cls: &Bound<'_, PyType>, binding: PyRef<'_, PyRateBindingSpec>) -> Self {
        Self {
            inner: OperationSpec::RateBinding {
                binding: binding.inner.clone(),
            },
        }
    }

    /// Instrument spread shock (basis points) by exact attribute match.
    #[classmethod]
    #[pyo3(signature = (attrs, bp))]
    fn instrument_spread_bp_by_attr(
        _cls: &Bound<'_, PyType>,
        attrs: Vec<(String, String)>,
        bp: f64,
    ) -> Self {
        Self {
            inner: OperationSpec::InstrumentSpreadBpByAttr {
                attrs: parse_attrs(attrs),
                bp,
            },
        }
    }

    /// Instrument price shock by `InstrumentType`. ``instrument_types`` accepts
    /// snake_case identifiers (e.g. ``"bond"``, ``"cds_index"``).
    #[classmethod]
    #[pyo3(signature = (instrument_types, pct))]
    fn instrument_price_pct_by_type(
        _cls: &Bound<'_, PyType>,
        instrument_types: Vec<String>,
        pct: f64,
    ) -> PyResult<Self> {
        let instrument_types = parse_instrument_types(instrument_types)?;
        Ok(Self {
            inner: OperationSpec::InstrumentPricePctByType {
                instrument_types,
                pct,
            },
        })
    }

    /// Instrument spread shock by `InstrumentType`.
    #[classmethod]
    #[pyo3(signature = (instrument_types, bp))]
    fn instrument_spread_bp_by_type(
        _cls: &Bound<'_, PyType>,
        instrument_types: Vec<String>,
        bp: f64,
    ) -> PyResult<Self> {
        let instrument_types = parse_instrument_types(instrument_types)?;
        Ok(Self {
            inner: OperationSpec::InstrumentSpreadBpByType {
                instrument_types,
                bp,
            },
        })
    }

    /// Asset-correlation shock for structured credit (additive correlation points).
    #[classmethod]
    #[pyo3(signature = (delta_pts))]
    fn asset_correlation_pts(_cls: &Bound<'_, PyType>, delta_pts: f64) -> Self {
        Self {
            inner: OperationSpec::AssetCorrelationPts { delta_pts },
        }
    }

    /// Prepay-default correlation shock for structured credit.
    #[classmethod]
    #[pyo3(signature = (delta_pts))]
    fn prepay_default_correlation_pts(_cls: &Bound<'_, PyType>, delta_pts: f64) -> Self {
        Self {
            inner: OperationSpec::PrepayDefaultCorrelationPts { delta_pts },
        }
    }

    /// Hierarchy-targeted parallel curve shift (basis points).
    ///
    /// ``target_json`` is a JSON-serialized ``HierarchyTarget``
    /// (``{"path": [...], "tag_filter": {...}}``). The full structure round-trips
    /// through serde so any field added on the Rust side is forwarded verbatim.
    #[classmethod]
    #[pyo3(signature = (curve_kind, target_json, bp, discount_curve_id=None))]
    fn hierarchy_curve_parallel_bp(
        _cls: &Bound<'_, PyType>,
        curve_kind: PyRef<'_, PyCurveKind>,
        target_json: &str,
        bp: f64,
        discount_curve_id: Option<String>,
    ) -> PyResult<Self> {
        let target = parse_hierarchy_target(target_json)?;
        Ok(Self {
            inner: OperationSpec::HierarchyCurveParallelBp {
                curve_kind: curve_kind.inner,
                target,
                bp,
                discount_curve_id: discount_curve_id.map(CurveId::from),
            },
        })
    }

    /// Hierarchy-targeted vol-surface percent shift.
    #[classmethod]
    #[pyo3(signature = (surface_kind, target_json, pct))]
    fn hierarchy_vol_surface_parallel_pct(
        _cls: &Bound<'_, PyType>,
        surface_kind: PyRef<'_, PyVolSurfaceKind>,
        target_json: &str,
        pct: f64,
    ) -> PyResult<Self> {
        let target = parse_hierarchy_target(target_json)?;
        Ok(Self {
            inner: OperationSpec::HierarchyVolSurfaceParallelPct {
                surface_kind: surface_kind.inner,
                target,
                pct,
            },
        })
    }

    /// Hierarchy-targeted equity price shift.
    #[classmethod]
    #[pyo3(signature = (target_json, pct))]
    fn hierarchy_equity_price_pct(
        _cls: &Bound<'_, PyType>,
        target_json: &str,
        pct: f64,
    ) -> PyResult<Self> {
        let target = parse_hierarchy_target(target_json)?;
        Ok(Self {
            inner: OperationSpec::HierarchyEquityPricePct { target, pct },
        })
    }

    /// Hierarchy-targeted base-correlation parallel shift.
    #[classmethod]
    #[pyo3(signature = (target_json, points))]
    fn hierarchy_base_corr_parallel_pts(
        _cls: &Bound<'_, PyType>,
        target_json: &str,
        points: f64,
    ) -> PyResult<Self> {
        let target = parse_hierarchy_target(target_json)?;
        Ok(Self {
            inner: OperationSpec::HierarchyBaseCorrParallelPts { target, points },
        })
    }

    /// Time-roll the valuation horizon forward.
    ///
    /// ``apply_shocks`` defaults to ``True`` to match the Rust
    /// ``#[serde(default = "default_true")]`` attribute. ``roll_mode`` defaults
    /// to ``TimeRollMode.business_days()``.
    #[classmethod]
    #[pyo3(signature = (period, apply_shocks=true, roll_mode=None))]
    fn time_roll_forward(
        _cls: &Bound<'_, PyType>,
        period: &str,
        apply_shocks: bool,
        roll_mode: Option<PyRef<'_, PyTimeRollMode>>,
    ) -> Self {
        let roll_mode = roll_mode.map(|m| m.inner).unwrap_or_default();
        Self {
            inner: OperationSpec::TimeRollForward {
                period: period.to_string(),
                apply_shocks,
                roll_mode,
            },
        }
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize an `OperationSpec` from JSON.
    #[classmethod]
    fn from_json(_cls: &Bound<'_, PyType>, json: &str) -> PyResult<Self> {
        let inner: OperationSpec = serde_json::from_str(json)
            .map_err(|e| crate::errors::value_error(format!("Invalid OperationSpec JSON: {e}")))?;
        Ok(Self { inner })
    }

    /// Return the variant discriminator (the serde ``kind`` tag value).
    #[getter]
    fn kind(&self) -> PyResult<String> {
        let value = serde_json::to_value(&self.inner).map_err(display_to_py)?;
        value
            .get("kind")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| crate::errors::value_error("OperationSpec JSON missing 'kind' tag"))
    }

    fn __repr__(&self) -> String {
        match self.kind() {
            Ok(k) => format!("OperationSpec(kind='{}')", k),
            Err(_) => "OperationSpec(<unknown>)".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register `OperationSpec` and supporting enums on the scenarios submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCurveKind>()?;
    m.add_class::<PyVolSurfaceKind>()?;
    m.add_class::<PyTenorMatchMode>()?;
    m.add_class::<PyTimeRollMode>()?;
    m.add_class::<PyCompounding>()?;
    m.add_class::<PyRateBindingSpec>()?;
    m.add_class::<PyOperationSpec>()?;
    Ok(())
}
