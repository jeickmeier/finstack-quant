//! OperationSpec builder wrapper.

use crate::errors::{display_to_py, scenarios_to_py};
use finstack_quant_core::types::CurveId;
use finstack_quant_scenarios::spec::OperationSpec;
use finstack_quant_statements::types::NodeId;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyType};

use super::helpers::{
    extract_attrs, extract_curve_kind, extract_hierarchy_target, extract_tenor_match_mode,
    extract_time_roll_mode, parse_currency, parse_instrument_types,
};
use super::rate_binding::PyRateBindingSpec;

/// One shock, time roll, or binding inside a ``ScenarioSpec``.
///
/// Each classmethod constructor mirrors one Rust ``OperationSpec`` variant.
/// Units follow the constructor name:
///
/// - ``*_pct`` fields are percentage points (``5.0`` = +5%).
/// - ``*_bp`` fields are additive basis points (1 bp = 1e-4) — except on
///   ``CurveKind.commodity()`` curves, where ``bp`` is **percent of the
///   forward** (a commodity price curve has no rate to shift).
/// - Vol-index ``*_pts`` are index points (``1.0`` on 18.5 → 19.5).
/// - Correlation and base-correlation ``*_pts`` are **decimal correlation**
///   (``0.02`` = +0.02, not percentage points).
///
/// Every enum-valued argument accepts the typed wrapper or its snake-case
/// label (``CurveKind.discount()`` or ``"discount"``). Serialization goes
/// through serde so ``to_json`` is the canonical wire representation.
///
/// Examples
/// --------
/// >>> from finstack_quant.scenarios import OperationSpec
/// >>> op = OperationSpec.curve_parallel_bp("discount", "USD-OIS", 25.0)
/// >>> op.kind
/// 'curve_parallel_bp'
/// >>> op == OperationSpec.from_json(op.to_json())
/// True
#[pyclass(
    name = "OperationSpec",
    module = "finstack_quant.scenarios",
    eq,
    frozen,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyOperationSpec {
    pub(crate) inner: OperationSpec,
}

impl PyOperationSpec {
    fn wrap(inner: OperationSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyOperationSpec {
    /// FX spot percent shift. ``pct = 5.0`` strengthens ``base`` against
    /// ``quote`` by 5%; ``pct <= -100`` is rejected at validation.
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
        Ok(Self::wrap(OperationSpec::MarketFxPct { base, quote, pct }))
    }

    /// Equity price percent shock applied to every identifier in ``ids``.
    #[classmethod]
    #[pyo3(signature = (ids, pct))]
    fn equity_price_pct(_cls: &Bound<'_, PyType>, ids: Vec<String>, pct: f64) -> Self {
        Self::wrap(OperationSpec::EquityPricePct { ids, pct })
    }

    /// Instrument price percent shock by exact attribute match.
    ///
    /// ``attrs`` is a ``{key: value}`` mapping or a sequence of
    /// ``(key, value)`` pairs; insertion order is preserved and every pair
    /// must match. Requires instruments in the execution context.
    #[classmethod]
    #[pyo3(signature = (attrs, pct))]
    fn instrument_price_pct_by_attr(
        _cls: &Bound<'_, PyType>,
        attrs: &Bound<'_, PyAny>,
        pct: f64,
    ) -> PyResult<Self> {
        Ok(Self::wrap(OperationSpec::InstrumentPricePctByAttr {
            attrs: extract_attrs(attrs)?,
            pct,
        }))
    }

    /// Parallel basis-point shift on a curve (percent of forward for
    /// ``CurveKind.commodity()``).
    ///
    /// Parameters
    /// ----------
    /// curve_kind : CurveKind | str
    ///     Curve family (``"discount"``, ``"forward"``, ``"par_cds"``,
    ///     ``"inflation"``, ``"commodity"``).
    /// curve_id : str | list[str]
    ///     One curve identifier, or several: a list expands to one operation
    ///     per identifier and the method returns ``list[OperationSpec]``.
    /// bp : float
    ///     Additive shift in basis points (percent of forward for commodity
    ///     curves).
    /// discount_curve_id : str | None
    ///     Discount curve used when re-bootstrapping shocked ParCDS quotes.
    ///
    /// Returns
    /// -------
    /// OperationSpec | list[OperationSpec]
    ///     A single operation for a ``str`` curve id, a list for a list.
    #[classmethod]
    #[pyo3(signature = (curve_kind, curve_id, bp, discount_curve_id=None))]
    fn curve_parallel_bp<'py>(
        _cls: &Bound<'py, PyType>,
        curve_kind: &Bound<'py, PyAny>,
        curve_id: &Bound<'py, PyAny>,
        bp: f64,
        discount_curve_id: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = curve_kind.py();
        let curve_kind = extract_curve_kind(curve_kind)?;
        let discount_curve_id = discount_curve_id.map(CurveId::from);
        if let Ok(single) = curve_id.extract::<String>() {
            let op = OperationSpec::CurveParallelBp {
                curve_kind,
                curve_id: CurveId::from(single.as_str()),
                discount_curve_id,
                bp,
            };
            return Ok(Bound::new(py, Self::wrap(op))?.into_any());
        }
        let ids: Vec<String> = curve_id.extract().map_err(|_| {
            crate::errors::value_error(format!(
                "curve_id must be a str or a list of str; got {}",
                curve_id.get_type()
            ))
        })?;
        let ops = finstack_quant_scenarios::ScenarioSpec::parallel_bp_many(
            curve_kind,
            ids.iter().map(String::as_str),
            bp,
            discount_curve_id,
        );
        let items = ops
            .into_iter()
            .map(|op| Bound::new(py, Self::wrap(op)).map(Bound::into_any))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(PyList::new(py, items)?.into_any())
    }

    /// Node-level basis-point shifts on a curve.
    ///
    /// ``nodes`` is a list of ``(tenor, bp)`` pairs; ``match_mode``
    /// (``TenorMatchMode | str``) defaults to ``"exact"``.
    #[classmethod]
    #[pyo3(signature = (curve_kind, curve_id, nodes, match_mode=None, discount_curve_id=None))]
    fn curve_node_bp(
        _cls: &Bound<'_, PyType>,
        curve_kind: &Bound<'_, PyAny>,
        curve_id: &str,
        nodes: Vec<(String, f64)>,
        match_mode: Option<&Bound<'_, PyAny>>,
        discount_curve_id: Option<String>,
    ) -> PyResult<Self> {
        let match_mode = match_mode
            .map(extract_tenor_match_mode)
            .transpose()?
            .unwrap_or_default();
        Ok(Self::wrap(OperationSpec::CurveNodeBp {
            curve_kind: extract_curve_kind(curve_kind)?,
            curve_id: CurveId::from(curve_id),
            discount_curve_id: discount_curve_id.map(CurveId::from),
            nodes,
            match_mode,
        }))
    }

    /// Parallel shock to a volatility-index curve in absolute index points.
    #[classmethod]
    #[pyo3(signature = (curve_id, points))]
    fn vol_index_parallel_pts(_cls: &Bound<'_, PyType>, curve_id: &str, points: f64) -> Self {
        Self::wrap(OperationSpec::VolIndexParallelPts {
            curve_id: CurveId::from(curve_id),
            points,
        })
    }

    /// Node-level shocks to a volatility-index curve in absolute index points.
    ///
    /// ``nodes`` is a list of ``(tenor, points)`` pairs; ``match_mode``
    /// (``TenorMatchMode | str``) defaults to ``"exact"``.
    #[classmethod]
    #[pyo3(signature = (curve_id, nodes, match_mode=None))]
    fn vol_index_node_pts(
        _cls: &Bound<'_, PyType>,
        curve_id: &str,
        nodes: Vec<(String, f64)>,
        match_mode: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let match_mode = match_mode
            .map(extract_tenor_match_mode)
            .transpose()?
            .unwrap_or_default();
        Ok(Self::wrap(OperationSpec::VolIndexNodePts {
            curve_id: CurveId::from(curve_id),
            nodes,
            match_mode,
        }))
    }

    /// Parallel shift to a base-correlation surface in decimal correlation
    /// (``0.02`` = +0.02).
    #[classmethod]
    #[pyo3(signature = (surface_id, points))]
    fn base_corr_parallel_pts(_cls: &Bound<'_, PyType>, surface_id: &str, points: f64) -> Self {
        Self::wrap(OperationSpec::BaseCorrParallelPts {
            surface_id: CurveId::from(surface_id),
            points,
        })
    }

    /// Bucketed base-correlation shift in decimal correlation, limited to the
    /// given detachment points (in basis points of the capital structure) or
    /// every bucket when ``detachment_bp`` is ``None``.
    #[classmethod]
    #[pyo3(signature = (surface_id, points, detachment_bp=None))]
    fn base_corr_bucket_pts(
        _cls: &Bound<'_, PyType>,
        surface_id: &str,
        points: f64,
        detachment_bp: Option<Vec<i32>>,
    ) -> Self {
        Self::wrap(OperationSpec::BaseCorrBucketPts {
            surface_id: CurveId::from(surface_id),
            detachment_bp,
            points,
        })
    }

    /// Parallel percent shift to a volatility surface.
    #[classmethod]
    #[pyo3(signature = (vol_surface_id, pct))]
    fn vol_surface_parallel_pct(_cls: &Bound<'_, PyType>, vol_surface_id: &str, pct: f64) -> Self {
        Self::wrap(OperationSpec::VolSurfaceParallelPct {
            vol_surface_id: CurveId::from(vol_surface_id),
            pct,
        })
    }

    /// Bucketed volatility-surface percent shock restricted to the given
    /// ``tenors`` and/or ``strikes`` (``None`` = all).
    #[classmethod]
    #[pyo3(signature = (vol_surface_id, pct, tenors=None, strikes=None))]
    fn vol_surface_bucket_pct(
        _cls: &Bound<'_, PyType>,
        vol_surface_id: &str,
        pct: f64,
        tenors: Option<Vec<String>>,
        strikes: Option<Vec<f64>>,
    ) -> Self {
        Self::wrap(OperationSpec::VolSurfaceBucketPct {
            vol_surface_id: CurveId::from(vol_surface_id),
            tenors,
            strikes,
            pct,
        })
    }

    /// Statement forecast percent change on ``node_id``.
    #[classmethod]
    #[pyo3(signature = (node_id, pct))]
    fn stmt_forecast_percent(_cls: &Bound<'_, PyType>, node_id: &str, pct: f64) -> Self {
        Self::wrap(OperationSpec::StmtForecastPercent {
            node_id: NodeId::from(node_id),
            pct,
        })
    }

    /// Statement forecast value assignment on ``node_id``.
    #[classmethod]
    #[pyo3(signature = (node_id, value))]
    fn stmt_forecast_assign(_cls: &Bound<'_, PyType>, node_id: &str, value: f64) -> Self {
        Self::wrap(OperationSpec::StmtForecastAssign {
            node_id: NodeId::from(node_id),
            value,
        })
    }

    /// Bind a statement rate node to a curve for the lifetime of the scenario.
    #[classmethod]
    #[pyo3(signature = (binding))]
    fn rate_binding(_cls: &Bound<'_, PyType>, binding: PyRef<'_, PyRateBindingSpec>) -> Self {
        Self::wrap(OperationSpec::RateBinding {
            binding: binding.inner.clone(),
        })
    }

    /// Instrument spread shock (additive basis points) by exact attribute
    /// match. ``attrs`` is a mapping or a sequence of ``(key, value)`` pairs.
    #[classmethod]
    #[pyo3(signature = (attrs, bp))]
    fn instrument_spread_bp_by_attr(
        _cls: &Bound<'_, PyType>,
        attrs: &Bound<'_, PyAny>,
        bp: f64,
    ) -> PyResult<Self> {
        Ok(Self::wrap(OperationSpec::InstrumentSpreadBpByAttr {
            attrs: extract_attrs(attrs)?,
            bp,
        }))
    }

    /// Instrument price percent shock by instrument type. ``instrument_types``
    /// accepts snake_case identifiers (e.g. ``"bond"``, ``"cds_index"``).
    #[classmethod]
    #[pyo3(signature = (instrument_types, pct))]
    fn instrument_price_pct_by_type(
        _cls: &Bound<'_, PyType>,
        instrument_types: Vec<String>,
        pct: f64,
    ) -> PyResult<Self> {
        let instrument_types = parse_instrument_types(instrument_types)?;
        Ok(Self::wrap(OperationSpec::InstrumentPricePctByType {
            instrument_types,
            pct,
        }))
    }

    /// Instrument spread shock (additive basis points) by instrument type.
    #[classmethod]
    #[pyo3(signature = (instrument_types, bp))]
    fn instrument_spread_bp_by_type(
        _cls: &Bound<'_, PyType>,
        instrument_types: Vec<String>,
        bp: f64,
    ) -> PyResult<Self> {
        let instrument_types = parse_instrument_types(instrument_types)?;
        Ok(Self::wrap(OperationSpec::InstrumentSpreadBpByType {
            instrument_types,
            bp,
        }))
    }

    /// Structured-credit asset-correlation shock in decimal correlation
    /// (``delta_pts = 0.05`` adds 0.05 to the correlation).
    #[classmethod]
    #[pyo3(signature = (delta_pts))]
    fn asset_correlation_pts(_cls: &Bound<'_, PyType>, delta_pts: f64) -> Self {
        Self::wrap(OperationSpec::AssetCorrelationPts { delta_pts })
    }

    /// Structured-credit prepay/default correlation shock in decimal
    /// correlation.
    #[classmethod]
    #[pyo3(signature = (delta_pts))]
    fn prepay_default_correlation_pts(_cls: &Bound<'_, PyType>, delta_pts: f64) -> Self {
        Self::wrap(OperationSpec::PrepayDefaultCorrelationPts { delta_pts })
    }

    /// Hierarchy-targeted parallel curve shift (basis points; percent of
    /// forward for commodity curves).
    ///
    /// ``target`` is a ``HierarchyTarget`` or its JSON string
    /// (``{"path": [...], "tag_filter": {...}}``).
    #[classmethod]
    #[pyo3(signature = (curve_kind, target, bp, discount_curve_id=None))]
    fn hierarchy_curve_parallel_bp(
        _cls: &Bound<'_, PyType>,
        curve_kind: &Bound<'_, PyAny>,
        target: &Bound<'_, PyAny>,
        bp: f64,
        discount_curve_id: Option<String>,
    ) -> PyResult<Self> {
        Ok(Self::wrap(OperationSpec::HierarchyCurveParallelBp {
            curve_kind: extract_curve_kind(curve_kind)?,
            target: extract_hierarchy_target(target)?,
            bp,
            discount_curve_id: discount_curve_id.map(CurveId::from),
        }))
    }

    /// Hierarchy-targeted vol-surface percent shift. ``target`` is a
    /// ``HierarchyTarget`` or its JSON string.
    #[classmethod]
    #[pyo3(signature = (target, pct))]
    fn hierarchy_vol_surface_parallel_pct(
        _cls: &Bound<'_, PyType>,
        target: &Bound<'_, PyAny>,
        pct: f64,
    ) -> PyResult<Self> {
        Ok(Self::wrap(OperationSpec::HierarchyVolSurfaceParallelPct {
            target: extract_hierarchy_target(target)?,
            pct,
        }))
    }

    /// Hierarchy-targeted equity price percent shift. ``target`` is a
    /// ``HierarchyTarget`` or its JSON string.
    #[classmethod]
    #[pyo3(signature = (target, pct))]
    fn hierarchy_equity_price_pct(
        _cls: &Bound<'_, PyType>,
        target: &Bound<'_, PyAny>,
        pct: f64,
    ) -> PyResult<Self> {
        Ok(Self::wrap(OperationSpec::HierarchyEquityPricePct {
            target: extract_hierarchy_target(target)?,
            pct,
        }))
    }

    /// Hierarchy-targeted base-correlation parallel shift in decimal
    /// correlation. ``target`` is a ``HierarchyTarget`` or its JSON string.
    #[classmethod]
    #[pyo3(signature = (target, points))]
    fn hierarchy_base_corr_parallel_pts(
        _cls: &Bound<'_, PyType>,
        target: &Bound<'_, PyAny>,
        points: f64,
    ) -> PyResult<Self> {
        Ok(Self::wrap(OperationSpec::HierarchyBaseCorrParallelPts {
            target: extract_hierarchy_target(target)?,
            points,
        }))
    }

    /// Time-roll the valuation horizon forward by ``period`` (a tenor such as
    /// ``"3M"``; validated eagerly).
    ///
    /// ``apply_shocks`` defaults to ``True``; ``roll_mode``
    /// (``TimeRollMode | str``) defaults to ``"business_days"``.
    #[classmethod]
    #[pyo3(signature = (period, apply_shocks=true, roll_mode=None))]
    fn time_roll_forward(
        _cls: &Bound<'_, PyType>,
        period: &str,
        apply_shocks: bool,
        roll_mode: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let roll_mode = roll_mode.map(extract_time_roll_mode).transpose()?;
        Ok(Self::wrap(OperationSpec::time_roll_forward(
            period,
            Some(apply_shocks),
            roll_mode,
        )))
    }

    /// Validate this operation with the canonical Rust rules (identifiers,
    /// finite numbers, variant-specific floors, tenor parsing).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the operation is invalid.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(scenarios_to_py)
    }

    /// Whether this operation needs instruments in the execution context
    /// (instrument-scoped shocks and time rolls).
    fn requires_instruments(&self) -> bool {
        self.inner.requires_instruments()
    }

    /// Whether this operation can replace or mutate instruments (price,
    /// spread, and structured-credit correlation shocks).
    fn mutates_instruments(&self) -> bool {
        self.inner.mutates_instruments()
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize an ``OperationSpec`` from its canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON does not match any ``OperationSpec`` variant (unknown
    ///     fields are rejected).
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: OperationSpec = serde_json::from_str(json)
            .map_err(|e| crate::errors::value_error(format!("Invalid OperationSpec JSON: {e}")))?;
        Ok(Self { inner })
    }

    /// The variant discriminator (the serde ``kind`` tag value), e.g.
    /// ``"curve_parallel_bp"``.
    #[getter]
    fn kind(&self) -> PyResult<String> {
        let value = serde_json::to_value(&self.inner).map_err(display_to_py)?;
        value
            .get("kind")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| crate::errors::value_error("OperationSpec JSON missing 'kind' tag"))
    }

    /// Python-style repr rendered from the wire fields, e.g.
    /// ``OperationSpec(kind='curve_parallel_bp', curve_kind='discount',
    /// curve_id='USD-OIS', bp=25.0)``.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("OperationSpec", &self.inner)
    }
}
