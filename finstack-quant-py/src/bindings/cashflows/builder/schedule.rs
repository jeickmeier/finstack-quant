//! Python bindings for `finstack_quant_cashflows::builder` schedule types.

use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::{
    merge_cashflow_schedules as rust_merge_cashflow_schedules, CashFlowMeta, CashFlowSchedule,
    CashflowRepresentation,
};
use finstack_quant_core::cashflow::CashFlow;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use super::specs::date_money_to_py;
use crate::bindings::cashflows::aggregation::PyPeriodAggregation;
use crate::bindings::core::cashflow::{extract_cf_kind, PyCashFlow};
use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::periods::PyPeriod;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::parse_label;
use crate::bindings::macros::wire_methods;
use crate::bindings::module_utils::spec_from_py;
use crate::errors::core_to_py;

use super::orchestrator::PyCashFlowBuilder;
use super::specs::PyNotional;
use crate::bindings::cashflows::fixings::PyProjectedFixing;

#[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowMeta")]
#[pyclass(
    name = "CashFlowMeta",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyCashFlowMeta {
    /// Inner schedule metadata.
    pub(crate) inner: CashFlowMeta,
}

#[pymethods]
impl PyCashFlowMeta {
    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowMeta.__init__")]
    #[new]
    #[pyo3(
        signature = (representation="contractual", calendar_ids=None, commitment=None, issue_date=None, maturity=None, projected_fixings=None),
        text_signature = "(representation=\"contractual\", calendar_ids=None, commitment=None, issue_date=None, maturity=None, projected_fixings=None)"
    )]
    fn new(
        representation: &str,
        calendar_ids: Option<Vec<String>>,
        commitment: Option<PyMoney>,
        issue_date: Option<&Bound<'_, PyAny>>,
        maturity: Option<&Bound<'_, PyAny>>,
        projected_fixings: Option<Vec<PyProjectedFixing>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CashFlowMeta {
                projected_fixings: projected_fixings
                    .unwrap_or_default()
                    .into_iter()
                    .map(|fixing| fixing.inner)
                    .collect(),
                representation: parse_label::<CashflowRepresentation>(representation)?,
                calendar_ids: calendar_ids.unwrap_or_default(),
                commitment: commitment.map(|m| m.inner),
                issue_date: issue_date.map(py_to_date).transpose()?,
                maturity: maturity.map(py_to_date).transpose()?,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowMeta.representation")]
    #[getter]
    fn representation(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(&self.inner.representation).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowMeta.calendar_ids")]
    #[getter]
    fn calendar_ids(&self) -> Vec<String> {
        self.inner.calendar_ids.clone()
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowMeta.commitment")]
    #[getter]
    fn commitment(&self) -> Option<PyMoney> {
        self.inner.commitment.map(PyMoney::from_inner)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowMeta.issue_date")]
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner.issue_date.map(|d| date_to_py(py, d)).transpose()
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowMeta.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner.maturity.map(|d| date_to_py(py, d)).transpose()
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowMeta.projected_fixings")]
    #[getter]
    fn projected_fixings(&self) -> Vec<PyProjectedFixing> {
        self.inner
            .projected_fixings
            .iter()
            .cloned()
            .map(|inner| PyProjectedFixing { inner })
            .collect()
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CashFlowMeta", &self.inner)
    }
}

wire_methods!(
    PyCashFlowMeta,
    CashFlowMeta,
    "CashFlowMeta",
    module = "finstack_quant.cashflows.builder._schedule"
);

#[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule")]
#[pyclass(
    name = "CashFlowSchedule",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyCashFlowSchedule {
    /// Inner canonical schedule.
    pub(crate) inner: CashFlowSchedule,
}

impl PyCashFlowSchedule {
    /// Build from an existing Rust [`CashFlowSchedule`].
    pub(crate) fn from_inner(inner: CashFlowSchedule) -> Self {
        Self { inner }
    }
}

wire_methods!(
    PyCashFlowSchedule,
    CashFlowSchedule,
    "CashFlowSchedule",
    module = "finstack_quant.cashflows.builder._schedule"
);

/// Extract one string column of a DataFrame-like object as Python objects.
fn frame_column<'py>(
    frame: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
    let columns = frame.getattr("columns")?;
    if !columns.contains(name)? {
        return Ok(None);
    }
    let values = frame.get_item(name)?.call_method0("tolist")?;
    Ok(Some(values.extract()?))
}

fn is_missing(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    if obj.is_none() {
        return Ok(true);
    }
    if let Ok(value) = obj.extract::<f64>() {
        return Ok(value.is_nan());
    }
    // pandas missing-datetime sentinel (``pd.NaT``) is neither ``None`` nor a float.
    if obj.get_type().name()? == "NaTType" {
        return Ok(true);
    }
    Ok(false)
}

fn optional_json_field<T: serde::de::DeserializeOwned + Send>(
    columns: &Option<Vec<Bound<'_, PyAny>>>,
    index: usize,
    label: &str,
) -> PyResult<Option<T>> {
    match columns.as_ref().map(|rows| &rows[index]) {
        Some(value) if !is_missing(value)? => spec_from_py(value.py(), value, label).map(Some),
        _ => Ok(None),
    }
}

/// Extract cashflow rows from ``list[CashFlow]`` or a DataFrame with columns
/// ``date, amount, currency, kind, accrual_factor`` and optional ``reset_date, rate``.
///
/// Rows are only converted here; `CashFlowSchedule::from_flows` validates them.
pub(crate) fn extract_flows(obj: &Bound<'_, PyAny>) -> PyResult<Vec<CashFlow>> {
    if let Ok(flows) = obj.extract::<Vec<PyRef<'_, PyCashFlow>>>() {
        return Ok(flows.iter().map(|f| f.inner.clone()).collect());
    }
    if obj.hasattr("columns")? {
        let missing = |name: &str| {
            crate::errors::value_error(format!(
                "cashflow DataFrame is missing required column '{name}' \
                 (required: date, amount, currency, kind, accrual_factor; optional: reset_date, rate)"
            ))
        };
        let dates = frame_column(obj, "date")?.ok_or_else(|| missing("date"))?;
        let amounts = frame_column(obj, "amount")?.ok_or_else(|| missing("amount"))?;
        let currencies = frame_column(obj, "currency")?.ok_or_else(|| missing("currency"))?;
        let kinds = frame_column(obj, "kind")?.ok_or_else(|| missing("kind"))?;
        let resets = frame_column(obj, "reset_date")?;
        let factors =
            frame_column(obj, "accrual_factor")?.ok_or_else(|| missing("accrual_factor"))?;
        let rates = frame_column(obj, "rate")?;
        let accruals = frame_column(obj, "accrual")?;
        let deltas = frame_column(obj, "principal_delta")?;
        let principal_dates = frame_column(obj, "principal_date")?;
        let rows = dates.len();
        let optional = [
            ("reset_date", resets.as_ref().map(Vec::len)),
            ("rate", rates.as_ref().map(Vec::len)),
            ("accrual", accruals.as_ref().map(Vec::len)),
            ("principal_delta", deltas.as_ref().map(Vec::len)),
            ("principal_date", principal_dates.as_ref().map(Vec::len)),
        ];
        let required = [
            ("amount", amounts.len()),
            ("currency", currencies.len()),
            ("kind", kinds.len()),
            ("accrual_factor", factors.len()),
        ];
        for (name, len) in required.into_iter().chain(
            optional
                .into_iter()
                .filter_map(|(name, len)| len.map(|len| (name, len))),
        ) {
            if len != rows {
                return Err(crate::errors::value_error(format!(
                    "cashflow DataFrame column '{name}' has {len} rows, expected {rows}"
                )));
            }
        }
        let mut flows = Vec::with_capacity(rows);
        for (i, date) in dates.iter().enumerate() {
            let amount: f64 = amounts[i].extract()?;
            let currency = extract_currency(&currencies[i])?;
            let reset = match resets.as_ref().map(|r| &r[i]) {
                Some(r) if !is_missing(r)? => Some(py_to_date(r)?),
                _ => None,
            };
            let accrual_factor: f64 = factors[i].extract()?;
            let rate = match rates.as_ref().map(|r| &r[i]) {
                Some(r) if !is_missing(r)? => Some(r.extract()?),
                _ => None,
            };
            let mut flow = CashFlow::new(
                py_to_date(date)?,
                reset,
                Money::new(amount, currency).map_err(crate::errors::core_to_py)?,
                extract_cf_kind(&kinds[i])?,
                accrual_factor,
                rate,
            );
            flow.accrual = optional_json_field(&accruals, i, "CashFlowAccrual")?;
            flow.principal_delta = optional_json_field(&deltas, i, "principal_delta")?;
            flow.principal_date = match principal_dates.as_ref().map(|dates| &dates[i]) {
                Some(date) if !is_missing(date)? => Some(py_to_date(date)?),
                _ => None,
            };
            flows.push(flow);
        }
        return Ok(flows);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected list[CashFlow] or a pandas DataFrame with columns date, amount, currency, kind",
    ))
}

#[pymethods]
impl PyCashFlowSchedule {
    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.builder")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyCashFlowBuilder {
        PyCashFlowBuilder {
            inner: CashFlowSchedule::builder(),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.from_flows")]
    #[staticmethod]
    #[pyo3(
        signature = (flows, notional, day_count, meta=None),
        text_signature = "(flows, notional, day_count, meta=None)"
    )]
    fn from_flows(
        flows: &Bound<'_, PyAny>,
        notional: PyRef<'_, PyNotional>,
        day_count: PyRef<'_, PyDayCount>,
        meta: Option<PyRef<'_, PyCashFlowMeta>>,
    ) -> PyResult<Self> {
        CashFlowSchedule::from_flows(
            extract_flows(flows)?,
            notional.inner.clone(),
            day_count.inner,
            meta.map_or_else(CashFlowMeta::default, |m| m.inner.clone()),
        )
        .map(Self::from_inner)
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.with_representation")]
    #[pyo3(text_signature = "(self, representation)")]
    fn with_representation(&self, representation: &str) -> PyResult<Self> {
        Ok(Self::from_inner(self.inner.clone().with_representation(
            parse_label::<CashflowRepresentation>(representation)?,
        )))
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.with_notional")]
    #[pyo3(text_signature = "(self, notional)")]
    fn with_notional(&self, notional: PyRef<'_, PyNotional>) -> Self {
        Self::from_inner(self.inner.clone().with_notional(notional.inner.clone()))
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.get_flows")]
    #[pyo3(text_signature = "(self)")]
    fn get_flows(&self) -> Vec<PyCashFlow> {
        self.inner
            .get_flows()
            .iter()
            .map(|f| PyCashFlow::from_inner(f.clone()))
            .collect()
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.coupons")]
    #[pyo3(text_signature = "(self)")]
    fn coupons(&self) -> Vec<PyCashFlow> {
        self.inner
            .coupons()
            .map(|f| PyCashFlow::from_inner(f.clone()))
            .collect()
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.dates")]
    #[pyo3(text_signature = "(self)")]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .dates()
            .into_iter()
            .map(|d| date_to_py(py, d))
            .collect()
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.get_notional")]
    #[pyo3(text_signature = "(self)")]
    fn get_notional(&self) -> PyNotional {
        PyNotional::from_inner(self.inner.get_notional().clone())
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.get_day_count")]
    #[pyo3(text_signature = "(self)")]
    fn get_day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.get_day_count())
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.get_meta")]
    #[pyo3(text_signature = "(self)")]
    fn get_meta(&self) -> PyCashFlowMeta {
        PyCashFlowMeta {
            inner: self.inner.get_meta().clone(),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.validate")]
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.scale_amounts")]
    #[pyo3(text_signature = "(self, scale)")]
    fn scale_amounts(&self, scale: f64) -> PyResult<Self> {
        self.inner
            .clone()
            .scale_amounts(scale)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.wal")]
    #[pyo3(text_signature = "(self, as_of)")]
    fn wal(&self, as_of: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner.wal(py_to_date(as_of)?).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.outstanding_by_date")]
    #[pyo3(text_signature = "(self)")]
    fn outstanding_by_date<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        date_money_to_py(py, &self.inner.outstanding_by_date().map_err(core_to_py)?)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.pv_by_period")]
    #[pyo3(
        signature = (periods, market, disc_curve_id, base, credit_curve_id=None),
        text_signature = "(self, periods, market, disc_curve_id, base, credit_curve_id=None)"
    )]
    fn pv_by_period<'py>(
        &self,
        py: Python<'py>,
        periods: Vec<PyRef<'_, PyPeriod>>,
        market: &Bound<'_, PyAny>,
        disc_curve_id: &str,
        base: &Bound<'_, PyAny>,
        credit_curve_id: Option<&str>,
    ) -> PyResult<PyPeriodAggregation> {
        let periods: Vec<finstack_quant_core::dates::Period> =
            periods.iter().map(|p| p.inner.clone()).collect();
        let market = crate::bindings::extract::extract_market(py, market)?;
        let base = py_to_date(base)?;
        let disc_id = CurveId::from(disc_curve_id);
        let hazard_id = credit_curve_id.map(CurveId::from);
        let schedule = self.inner.clone();
        py.detach(move || {
            schedule.pv_by_period(&periods, &market, &disc_id, hazard_id.as_ref(), base)
        })
        .map(PyPeriodAggregation::from_inner)
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.calendar_year_ladder")]
    #[pyo3(text_signature = "(self, pvs)")]
    fn calendar_year_ladder<'py>(
        &self,
        py: Python<'py>,
        pvs: Vec<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let rows = self.inner.calendar_year_ladder(&pvs).map_err(core_to_py)?;
        let columns = PyDict::new(py);
        columns.set_item("year", rows.iter().map(|r| r.year).collect::<Vec<_>>())?;
        columns.set_item(
            "non_principal",
            rows.iter().map(|r| r.non_principal).collect::<Vec<_>>(),
        )?;
        columns.set_item(
            "principal",
            rows.iter().map(|r| r.principal).collect::<Vec<_>>(),
        )?;
        columns.set_item("pv", rows.iter().map(|r| r.pv).collect::<Vec<_>>())?;
        crate::bindings::pandas_utils::dict_to_dataframe(py, &columns, None)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule.to_dataframe")]
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(signature = (outstanding=false), text_signature = "(self, outstanding=False)")]
    fn to_dataframe<'py>(&self, py: Python<'py>, outstanding: bool) -> PyResult<Bound<'py, PyAny>> {
        let flows = self.inner.get_flows();
        let pd = py.import("pandas")?;
        let dates: Vec<finstack_quant_core::dates::Date> = flows.iter().map(|f| f.date).collect();
        let reset_dates: Vec<Option<Bound<'py, PyAny>>> = flows
            .iter()
            .map(|f| f.reset_date.map(|d| date_to_py(py, d)).transpose())
            .collect::<PyResult<_>>()?;
        let kinds: Vec<String> = flows.iter().map(|f| f.kind.to_string()).collect();
        let amounts: Vec<f64> = flows.iter().map(|f| f.amount.amount()).collect();
        let currencies: Vec<String> = flows
            .iter()
            .map(|f| f.amount.currency().to_string())
            .collect();
        let factors: Vec<f64> = flows.iter().map(|f| f.accrual_factor).collect();
        let rates: Vec<Option<f64>> = flows.iter().map(|f| f.rate).collect();
        let columns = PyDict::new(py);
        columns.set_item(
            "date",
            pd.call_method1(
                "to_datetime",
                (crate::bindings::pandas_utils::dates_to_pylist(py, &dates)?,),
            )?,
        )?;
        columns.set_item(
            "reset_date",
            pd.call_method1("to_datetime", (reset_dates,))?,
        )?;
        columns.set_item("kind", kinds)?;
        columns.set_item("amount", amounts)?;
        columns.set_item("currency", currencies)?;
        columns.set_item("accrual_factor", factors)?;
        columns.set_item("rate", rates)?;
        columns.set_item(
            "accrual",
            crate::bindings::pandas_utils::serde_to_py(
                py,
                &flows.iter().map(|f| &f.accrual).collect::<Vec<_>>(),
            )?,
        )?;
        columns.set_item(
            "principal_delta",
            crate::bindings::pandas_utils::serde_to_py(
                py,
                &flows.iter().map(|f| f.principal_delta).collect::<Vec<_>>(),
            )?,
        )?;
        let principal_dates: Vec<Option<Bound<'py, PyAny>>> = flows
            .iter()
            .map(|flow| {
                flow.principal_date
                    .map(|date| date_to_py(py, date))
                    .transpose()
            })
            .collect::<PyResult<_>>()?;
        columns.set_item(
            "principal_date",
            pd.call_method1("to_datetime", (principal_dates,))?,
        )?;
        if outstanding {
            let balances: Vec<f64> = self
                .inner
                .outstanding_at_dates(&dates)
                .map_err(core_to_py)?
                .into_iter()
                .map(|balance| balance.amount())
                .collect();
            columns.set_item("outstanding", balances)?;
        }
        crate::bindings::pandas_utils::dict_to_dataframe(py, &columns, None)
    }

    /// Python-style summary.
    fn __repr__(&self) -> String {
        let notional = self.inner.get_notional();
        format!(
            "CashFlowSchedule(flows={}, notional={} {}, day_count='{}', representation='{}')",
            self.inner.get_flows().len(),
            notional.initial.amount(),
            notional.initial.currency(),
            self.inner.get_day_count(),
            // Unit-variant tags always serialize; the fallback only keeps repr infallible.
            finstack_quant_core::wire::serde_tag(&self.inner.get_meta().representation)
                .unwrap_or_default(),
        )
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "CashFlowSchedule._repr_html_")]
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py, false).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder._schedule", "merge_cashflow_schedules")]
#[pyfunction]
#[pyo3(text_signature = "(schedules, notional, day_count)")]
pub(crate) fn merge_cashflow_schedules(
    schedules: Vec<PyRef<'_, PyCashFlowSchedule>>,
    notional: PyRef<'_, PyNotional>,
    day_count: PyRef<'_, PyDayCount>,
) -> PyCashFlowSchedule {
    let inner = rust_merge_cashflow_schedules(
        schedules.iter().map(|s| s.inner.clone()),
        notional.inner.clone(),
        day_count.inner,
    );
    PyCashFlowSchedule::from_inner(inner)
}
