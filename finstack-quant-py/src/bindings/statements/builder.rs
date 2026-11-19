//! Python wrapper for `ModelBuilder`.
//!
//! Every fallible configuration step goes through the crate's non-consuming
//! mutable setters, so a typo in a formula, period range, or metric id raises
//! without discarding the model accumulated so far.

use super::capital_structure::PyWaterfallSpec;
use super::registry::PyRegistry;
use super::types::{PyFinancialModelSpec, PyForecastSpec, PyNodeSpec};
use super::{PERIOD_GRAMMAR, extract_money_series, extract_scalar_series, extract_value_series};
use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::dates::periods::PyPeriodPlan;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, statements_to_py, value_error};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::{MixedNodeBuilder, ModelBuilder};
use finstack_quant_statements::types::{
    AmountOrScalar, FinancialStatementInstrument, NodeId, RollupMethod,
};
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder")]
#[pyclass(name = "ModelBuilder", module = "finstack_quant.statements")]
pub struct PyModelBuilder {
    inner: Option<ModelBuilder>,
}

#[doc = pydoc!("finstack_quant.statements._builder", "MixedNodeBuilder")]
#[pyclass(name = "MixedNodeBuilder", module = "finstack_quant.statements")]
pub struct PyMixedNodeBuilder {
    inner: Option<MixedNodeBuilder>,
}

#[pymethods]
impl PyMixedNodeBuilder {
    #[doc = pydoc!("finstack_quant.statements._builder", "MixedNodeBuilder.values")]
    #[pyo3(text_signature = "($self, values)")]
    fn values<'py>(
        mut slf: PyRefMut<'py, Self>,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = extract_value_series(values)?;
        slf.builder_mut()?.values(&parsed);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "MixedNodeBuilder.values_money")]
    #[pyo3(text_signature = "($self, values)")]
    fn values_money<'py>(
        mut slf: PyRefMut<'py, Self>,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed: Vec<(PeriodId, AmountOrScalar)> = extract_money_series(values)?
            .into_iter()
            .map(|(pid, money)| (pid, AmountOrScalar::Amount(money)))
            .collect();
        slf.builder_mut()?.values(&parsed);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "MixedNodeBuilder.forecast")]
    fn forecast<'py>(
        mut slf: PyRefMut<'py, Self>,
        forecast_spec: PyRef<'_, PyForecastSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?.forecast(forecast_spec.inner.clone());
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "MixedNodeBuilder.formula")]
    fn formula<'py>(mut slf: PyRefMut<'py, Self>, formula: &str) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?
            .formula(formula)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "MixedNodeBuilder.name")]
    fn name<'py>(mut slf: PyRefMut<'py, Self>, name: &str) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?.name(name);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "MixedNodeBuilder.rollup")]
    fn rollup<'py>(mut slf: PyRefMut<'py, Self>, rollup: &str) -> PyResult<PyRefMut<'py, Self>> {
        let method: RollupMethod =
            finstack_quant_core::wire::serde_parse(rollup).map_err(core_to_py)?;
        slf.builder_mut()?.rollup(method);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "MixedNodeBuilder.build")]
    fn build(&mut self) -> PyResult<PyModelBuilder> {
        let builder = self.take()?;
        let ready = builder.build().map_err(statements_to_py)?;
        Ok(PyModelBuilder { inner: Some(ready) })
    }

    /// Return ``MixedNodeBuilder(node_id='revenue')`` (or ``consumed``).
    fn __repr__(&self) -> String {
        match &self.inner {
            Some(builder) => format!("MixedNodeBuilder(node_id={:?})", builder.node_id().as_str()),
            None => "MixedNodeBuilder(state='consumed')".to_string(),
        }
    }
}

impl PyMixedNodeBuilder {
    fn take(&mut self) -> PyResult<MixedNodeBuilder> {
        self.inner
            .take()
            .ok_or_else(|| value_error("MixedNodeBuilder has already been consumed"))
    }

    fn builder_mut(&mut self) -> PyResult<&mut MixedNodeBuilder> {
        self.inner
            .as_mut()
            .ok_or_else(|| value_error("MixedNodeBuilder has already been consumed"))
    }
}

#[pymethods]
impl PyModelBuilder {
    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.__init__")]
    #[new]
    #[pyo3(text_signature = "(id)")]
    fn new(id: &str) -> Self {
        Self {
            inner: Some(ModelBuilder::new(id)),
        }
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.from_spec")]
    #[staticmethod]
    fn from_spec(spec: &PyFinancialModelSpec) -> PyResult<Self> {
        let builder = ModelBuilder::from_spec(spec.inner.clone()).map_err(statements_to_py)?;
        Ok(Self {
            inner: Some(builder),
        })
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.periods")]
    #[pyo3(signature = (range, actuals_until=None), text_signature = "($self, range, actuals_until=None)")]
    fn periods<'py>(
        mut slf: PyRefMut<'py, Self>,
        range: &str,
        actuals_until: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?
            .periods(range, actuals_until)
            .map_err(|error| {
                value_error(format!(
                    "invalid period range {range:?}: {error}; {PERIOD_GRAMMAR}"
                ))
            })?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.periods_explicit")]
    #[pyo3(text_signature = "($self, plan)")]
    fn periods_explicit<'py>(
        mut slf: PyRefMut<'py, Self>,
        plan: PyRef<'_, PyPeriodPlan>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?
            .periods_explicit(plan.inner.clone())
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.constant")]
    fn constant<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::extract_amount_or_scalar(value)?;
        slf.builder_mut()?
            .constant(node_id, value)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.value")]
    #[pyo3(text_signature = "($self, node_id, values)")]
    fn value<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = extract_value_series(values)?;
        slf.builder_mut()?.value(node_id, &parsed);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.value_scalar")]
    #[pyo3(text_signature = "($self, node_id, values)")]
    fn value_scalar<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = extract_scalar_series(values)?;
        slf.builder_mut()?.value_scalar(node_id, &parsed);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.value_money")]
    #[pyo3(text_signature = "($self, node_id, values)")]
    fn value_money<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = extract_money_series(values)?;
        slf.builder_mut()?.value_money(node_id, &parsed);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.from_dataframe")]
    #[pyo3(signature = (df, node_id_column=None), text_signature = "($self, df, node_id_column=None)")]
    fn from_dataframe<'py>(
        mut slf: PyRefMut<'py, Self>,
        df: &Bound<'py, PyAny>,
        node_id_column: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let frame = match node_id_column {
            Some(column) => df.call_method1("set_index", (column,))?,
            None => df.clone(),
        };
        let columns: Vec<String> = frame
            .getattr("columns")?
            .try_iter()?
            .map(|column| Ok(column?.str()?.to_string()))
            .collect::<PyResult<Vec<_>>>()?;

        let mut rows: Vec<(NodeId, Vec<Option<AmountOrScalar>>)> = Vec::new();
        for row in frame.call_method0("iterrows")?.try_iter()? {
            let (index, series): (Bound<'py, PyAny>, Bound<'py, PyAny>) = row?.extract()?;
            let node_id = index.str()?.to_string();
            let mut cells = Vec::with_capacity(columns.len());
            for item in series.call_method0("items")?.try_iter()? {
                let (column, cell): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
                if cell.is_none() {
                    cells.push(None);
                    continue;
                }
                let value: f64 = cell.extract().map_err(|_| {
                    value_error(format!(
                        "from_dataframe cell ({node_id:?}, {column}) is not numeric"
                    ))
                })?;
                // pandas stores a missing numeric cell as NaN.
                cells.push((!value.is_nan()).then(|| AmountOrScalar::scalar(value)));
            }
            rows.push((NodeId::new(node_id), cells));
        }

        let labels: Vec<&str> = columns.iter().map(String::as_str).collect();
        slf.builder_mut()?
            .import_observations(&labels, &rows)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.availability_dates")]
    #[pyo3(text_signature = "($self, node_id, availability_dates)")]
    fn availability_dates<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        availability_dates: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = super::extract_period_pairs(availability_dates)?
            .into_iter()
            .map(|(period, date)| Ok((super::parse_period_id(&period)?, py_to_date(&date)?)))
            .collect::<PyResult<Vec<_>>>()?;
        slf.builder_mut()?
            .availability_dates(node_id, &parsed)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.rollup")]
    #[pyo3(text_signature = "($self, node_id, rollup)")]
    fn rollup<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        rollup: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let method: RollupMethod =
            finstack_quant_core::wire::serde_parse(rollup).map_err(core_to_py)?;
        slf.builder_mut()?
            .rollup(node_id, method)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.compute")]
    #[pyo3(text_signature = "($self, node_id, formula)")]
    fn compute<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        formula: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?
            .compute(node_id, formula)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.insert_node")]
    #[pyo3(text_signature = "($self, node)")]
    fn insert_node<'py>(
        mut slf: PyRefMut<'py, Self>,
        node: PyRef<'_, PyNodeSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec = node.inner.clone();
        slf.builder_mut()?.insert_node(spec);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.replace_node")]
    #[pyo3(text_signature = "($self, node)")]
    fn replace_node<'py>(
        mut slf: PyRefMut<'py, Self>,
        node: PyRef<'_, PyNodeSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec = node.inner.clone();
        slf.builder_mut()?
            .replace_node(spec)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.remove_node")]
    #[pyo3(text_signature = "($self, node_id)")]
    fn remove_node<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?
            .remove_node(node_id)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.source_snapshot")]
    #[pyo3(text_signature = "($self, snapshot_id)")]
    fn source_snapshot<'py>(
        mut slf: PyRefMut<'py, Self>,
        snapshot_id: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?.source_snapshot(snapshot_id);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.mixed")]
    #[pyo3(text_signature = "($self, node_id)")]
    fn mixed(&mut self, node_id: &str) -> PyResult<PyMixedNodeBuilder> {
        let state = self.take()?;
        Ok(PyMixedNodeBuilder {
            inner: Some(state.mixed(node_id)),
        })
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.forecast")]
    #[pyo3(text_signature = "($self, node_id, forecast_spec)")]
    fn forecast<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        forecast_spec: PyRef<'_, PyForecastSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?
            .forecast(node_id, forecast_spec.inner.clone());
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.where_clause")]
    #[pyo3(text_signature = "($self, node_id, where_clause)")]
    fn where_clause<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        where_clause: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?
            .where_clause(node_id, where_clause)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.currency")]
    #[pyo3(text_signature = "($self, currency)")]
    fn currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        currency: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let currency = extract_currency(currency)?;
        slf.builder_mut()?.currency(currency);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.with_meta")]
    #[pyo3(text_signature = "($self, key, value)")]
    fn with_meta<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        key: &str,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::module_utils::py_to_serde(py, value, "meta value")?;
        slf.builder_mut()?.with_meta(key, value);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.with_builtin_metrics")]
    #[pyo3(text_signature = "($self)")]
    fn with_builtin_metrics(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        slf.builder_mut()?
            .with_builtin_metrics()
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.add_metric_from_registry")]
    #[pyo3(text_signature = "($self, qualified_id, registry)")]
    fn add_metric_from_registry<'py>(
        mut slf: PyRefMut<'py, Self>,
        qualified_id: &str,
        registry: PyRef<'_, PyRegistry>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?
            .add_metric_from_registry(qualified_id, &registry.inner)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.add_debt")]
    #[pyo3(text_signature = "($self, id, instrument)")]
    fn add_debt<'py>(
        mut slf: PyRefMut<'py, Self>,
        id: &str,
        instrument: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec = crate::bindings::extract::extract_instrument(instrument)?
            .into_instrument_json()
            .map_err(core_to_py)?;
        let spec = FinancialStatementInstrument::try_from(spec).map_err(statements_to_py)?;
        slf.builder_mut()?.add_debt(id, spec);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.reporting_currency")]
    #[pyo3(text_signature = "($self, currency)")]
    fn reporting_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        currency: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = extract_currency(currency)?;
        slf.builder_mut()?.reporting_currency(ccy);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.fx_policy")]
    #[pyo3(text_signature = "($self, policy)")]
    fn fx_policy<'py>(mut slf: PyRefMut<'py, Self>, policy: &str) -> PyResult<PyRefMut<'py, Self>> {
        let parsed: finstack_quant_statements::types::CapitalFxPolicy = policy
            .parse()
            .map_err(|e| value_error(format!("invalid fx_policy {policy:?}: {e}")))?;
        slf.builder_mut()?.fx_policy(parsed);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.waterfall")]
    #[pyo3(text_signature = "($self, waterfall_spec)")]
    fn waterfall<'py>(
        mut slf: PyRefMut<'py, Self>,
        waterfall_spec: PyRef<'_, PyWaterfallSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec = waterfall_spec.inner.clone();
        slf.builder_mut()?.waterfall(spec);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.statements._builder", "ModelBuilder.build")]
    #[pyo3(text_signature = "($self)")]
    fn build(&mut self) -> PyResult<PyFinancialModelSpec> {
        let state = self.take()?;
        let spec = state.build().map_err(statements_to_py)?;
        Ok(PyFinancialModelSpec { inner: spec })
    }

    /// Return ``ModelBuilder(id='acme', state='ready', periods=4, nodes=3)``.
    ///
    /// ``state`` is ``'need_periods'`` before ``periods()``, ``'ready'``
    /// afterwards, and ``'consumed'`` once ``build()`` / ``mixed()`` ran.
    fn __repr__(&self) -> String {
        match &self.inner {
            Some(b) => format!(
                "ModelBuilder(id={:?}, state='{}', periods={}, nodes={})",
                b.id(),
                if b.periods_slice().is_empty() {
                    "need_periods"
                } else {
                    "ready"
                },
                b.periods_slice().len(),
                b.nodes().len()
            ),
            None => "ModelBuilder(state='consumed')".to_string(),
        }
    }
}

impl PyModelBuilder {
    fn consumed_error() -> PyErr {
        value_error(
            "Builder is no longer usable: it was consumed by build()/mixed(). Construct a new \
             ModelBuilder.",
        )
    }

    fn take(&mut self) -> PyResult<ModelBuilder> {
        self.inner.take().ok_or_else(Self::consumed_error)
    }

    /// Mutable access to the builder without taking ownership, so a
    /// fallible Rust call cannot leave the wrapper empty.
    pub(crate) fn builder_mut(&mut self) -> PyResult<&mut ModelBuilder> {
        self.inner.as_mut().ok_or_else(Self::consumed_error)
    }
}

/// Register builder classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyModelBuilder>()?;
    m.add_class::<PyMixedNodeBuilder>()?;
    Ok(())
}
