//! OperationSpec builder wrapper.

use crate::errors::display_to_py;
use finstack_quant_core::types::CurveId;
use finstack_quant_scenarios::spec::OperationSpec;
use finstack_quant_statements::types::NodeId;
use pyo3::prelude::*;
use pyo3::types::PyType;

use super::helpers::{parse_attrs, parse_currency, parse_hierarchy_target, parse_instrument_types};
use super::kinds::{PyCurveKind, PyTenorMatchMode, PyTimeRollMode, PyVolSurfaceKind};
use super::rate_binding::PyRateBindingSpec;

// ---------------------------------------------------------------------------
// OperationSpec
// ---------------------------------------------------------------------------

/// Typed builder for [`finstack_quant_scenarios::OperationSpec`].
///
/// Each classmethod constructor mirrors one Rust enum variant. Serialization
/// goes through serde so the JSON contract stays in lock-step with the Rust
/// type — Python callers should treat ``to_json`` output as the canonical wire
/// representation and pass it straight to ``build_scenario_spec`` or
/// ``ScenarioEngine`` consumers.
#[pyclass(
    name = "OperationSpec",
    module = "finstack_quant.scenarios",
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
