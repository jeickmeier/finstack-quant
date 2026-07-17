//! Python wrapper for the type-state ModelBuilder.
//!
//! Since Python cannot model Rust type-state at the type level, we collapse
//! the two states into a single class and track readiness at runtime.

use super::capital_structure::PyWaterfallSpec;
use super::types::{PyFinancialModelSpec, PyForecastSpec};
use crate::bindings::core::currency::PyCurrency;
use crate::bindings::core::dates::utils::py_to_date;
use crate::bindings::core::money::PyMoney;
use crate::errors::{core_to_py, serde_json_to_py, statements_to_py};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::fx::FxConversionPolicy;
use finstack_quant_statements::builder::{MixedNodeBuilder, ModelBuilder};
use finstack_quant_statements::types::AmountOrScalar;
use pyo3::prelude::*;

/// Validate a formula the same way `ModelBuilder::compute` / `formula` do,
/// without consuming the builder.
///
/// The Rust builder methods consume `self` and return `Err` on an invalid
/// formula, so calling them and mapping the error still leaves the Python
/// wrapper's `inner` as `None`, bricking every later call. Running the identical
/// checks first — reserved-prefix node id, non-empty formula, and
/// `parse_and_compile` — lets a typo fail without destroying accumulated state.
fn validate_compute_args(node_id: &str, formula: &str) -> PyResult<()> {
    finstack_quant_statements::builder::validate_node_id(node_id).map_err(statements_to_py)?;
    if formula.trim().is_empty() {
        return Err(crate::errors::value_error("Formula cannot be empty"));
    }
    finstack_quant_statements::dsl::parse_and_compile(formula).map_err(statements_to_py)?;
    Ok(())
}

/// Validate a period range the same way `ModelBuilder::periods` does, without
/// consuming the builder (see [`validate_compute_args`]).
fn validate_periods_args(range: &str, actuals_until: Option<&str>) -> PyResult<()> {
    finstack_quant_core::dates::build_periods(range, actuals_until).map_err(core_to_py)?;
    Ok(())
}

/// Builder for financial models (type-state collapsed for Python).
///
/// Usage::
///
///     builder = ModelBuilder("Acme Corp")
///     builder.periods("2025Q1..Q4", "2025Q2")
///     builder.value("revenue", [("2025Q1", 10_000_000.0), ("2025Q2", 11_000_000.0)])
///     builder.compute("cogs", "revenue * 0.6")
///     model = builder.build()
#[pyclass(name = "ModelBuilder", module = "finstack_quant.statements")]
pub struct PyModelBuilder {
    inner: Option<BuilderState>,
}

enum BuilderState {
    NeedPeriods(ModelBuilder<finstack_quant_statements::builder::NeedPeriods>),
    Ready(ModelBuilder<finstack_quant_statements::builder::Ready>),
}

/// Metric registry used to add reusable statement metrics to a model.
#[pyclass(
    name = "MetricRegistry",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyMetricRegistry {
    inner: finstack_quant_statements::registry::Registry,
}

#[pymethods]
impl PyMetricRegistry {
    /// Create an empty metric registry.
    #[new]
    fn new() -> Self {
        Self {
            inner: finstack_quant_statements::registry::Registry::new(),
        }
    }

    /// Create a registry preloaded with built-in metrics.
    #[staticmethod]
    fn with_builtins() -> PyResult<Self> {
        let inner = finstack_quant_statements::registry::Registry::with_builtins()
            .map_err(statements_to_py)?;
        Ok(Self { inner })
    }

    /// Load built-in metrics into this registry.
    fn load_builtins(&mut self) -> PyResult<()> {
        self.inner.load_builtins().map_err(statements_to_py)
    }

    /// Load metrics from a JSON string.
    fn load_from_json_str(&mut self, json: &str) -> PyResult<()> {
        self.inner
            .load_from_json_str(json)
            .map(|_| ())
            .map_err(statements_to_py)
    }

    /// Load metrics from a JSON file path.
    fn load_from_json(&mut self, path: &str) -> PyResult<()> {
        self.inner.load_from_json(path).map_err(statements_to_py)
    }

    /// Return whether a fully qualified metric exists.
    fn has(&self, qualified_id: &str) -> bool {
        self.inner.has(qualified_id)
    }

    /// Number of metrics in the registry.
    fn __len__(&self) -> usize {
        self.inner.len()
    }
}

/// Fluent builder for a mixed statement node.
#[pyclass(name = "MixedNodeBuilder", module = "finstack_quant.statements")]
pub struct PyMixedNodeBuilder {
    inner: Option<MixedNodeBuilder>,
}

#[pymethods]
impl PyMixedNodeBuilder {
    /// Set scalar explicit values for the mixed node.
    fn values(&mut self, values: Vec<(String, f64)>) -> PyResult<()> {
        let builder = self.take()?;
        let parsed = parse_scalar_values(values)?;
        self.inner = Some(builder.values(&parsed));
        Ok(())
    }

    /// Set monetary explicit values for the mixed node.
    fn values_money(&mut self, values: Vec<(String, PyMoney)>) -> PyResult<()> {
        let builder = self.take()?;
        let parsed: Vec<(PeriodId, AmountOrScalar)> = values
            .into_iter()
            .map(|(p, money)| {
                let pid: PeriodId = p.parse().map_err(core_to_py)?;
                Ok((pid, AmountOrScalar::Amount(money.inner)))
            })
            .collect::<PyResult<Vec<_>>>()?;
        self.inner = Some(builder.values(&parsed));
        Ok(())
    }

    /// Set the forecast specification.
    fn forecast(&mut self, forecast_spec: PyRef<'_, PyForecastSpec>) -> PyResult<()> {
        let builder = self.take()?;
        self.inner = Some(builder.forecast(forecast_spec.inner.clone()));
        Ok(())
    }

    /// Set the fallback formula.
    fn formula(&mut self, formula: &str) -> PyResult<()> {
        // Validate before `take()` so a bad formula does not consume the
        // mixed-node builder and brick the chain. `MixedNodeBuilder::formula`
        // validates via the same `parse_and_compile`.
        if formula.trim().is_empty() {
            return Err(crate::errors::value_error("Formula cannot be empty"));
        }
        finstack_quant_statements::dsl::parse_and_compile(formula).map_err(statements_to_py)?;
        let builder = self.take()?;
        self.inner = Some(builder.formula(formula).map_err(statements_to_py)?);
        Ok(())
    }

    /// Set the human-readable name.
    fn name(&mut self, name: &str) -> PyResult<()> {
        let builder = self.take()?;
        self.inner = Some(builder.name(name));
        Ok(())
    }

    /// Build the mixed node and return a ready model builder.
    fn build(&mut self) -> PyResult<PyModelBuilder> {
        let builder = self.take()?;
        let ready = builder.build().map_err(statements_to_py)?;
        Ok(PyModelBuilder {
            inner: Some(BuilderState::Ready(ready)),
        })
    }
}

impl PyMixedNodeBuilder {
    fn take(&mut self) -> PyResult<MixedNodeBuilder> {
        self.inner
            .take()
            .ok_or_else(|| crate::errors::value_error("MixedNodeBuilder has already been consumed"))
    }
}

#[pymethods]
impl PyModelBuilder {
    /// Create a new model builder.
    #[new]
    #[pyo3(text_signature = "(id)")]
    fn new(id: &str) -> Self {
        Self {
            inner: Some(BuilderState::NeedPeriods(ModelBuilder::new(id))),
        }
    }

    /// Define periods using a range expression (e.g. ``"2025Q1..Q4"``).
    ///
    /// Parameters
    /// ----------
    /// range : str
    ///     Period range expression.
    /// actuals_until : str | None
    ///     Optional cutoff for actual values.
    #[pyo3(signature = (range, actuals_until=None), text_signature = "($self, range, actuals_until=None)")]
    fn periods(&mut self, range: &str, actuals_until: Option<&str>) -> PyResult<()> {
        // Validate before `take_any()` so a bad range does not brick the builder
        // (see `validate_periods_args`).
        validate_periods_args(range, actuals_until)?;
        let state = self.take_any()?;
        match state {
            BuilderState::NeedPeriods(b) => {
                let ready = b.periods(range, actuals_until).map_err(statements_to_py)?;
                self.inner = Some(BuilderState::Ready(ready));
                Ok(())
            }
            BuilderState::Ready(b) => {
                self.inner = Some(BuilderState::Ready(b));
                Err(crate::errors::value_error("Periods already set"))
            }
        }
    }

    /// Add a value node with explicit period values.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier.
    /// values : list[tuple[str, float]]
    ///     List of (period_string, value) tuples.
    #[pyo3(text_signature = "($self, node_id, values)")]
    fn value(&mut self, node_id: &str, values: Vec<(String, f64)>) -> PyResult<()> {
        // Parse arguments BEFORE `take_ready()` so a bad period string does not
        // permanently consume the in-progress builder (leaving a misleading
        // "consumed by build()" error on the next call).
        let parsed = parse_scalar_values(values)?;
        let state = self.take_ready()?;

        let ready = state.value(node_id, &parsed);
        self.inner = Some(BuilderState::Ready(ready));
        Ok(())
    }

    /// Add a scalar value node with explicit period values.
    #[pyo3(text_signature = "($self, node_id, values)")]
    fn value_scalar(&mut self, node_id: &str, values: Vec<(String, f64)>) -> PyResult<()> {
        // Parse before `take_ready()` (see `value`).
        let parsed: Vec<(PeriodId, f64)> = values
            .into_iter()
            .map(|(p, v)| {
                let pid: PeriodId = p.parse().map_err(core_to_py)?;
                Ok((pid, v))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let state = self.take_ready()?;

        let ready = state.value_scalar(node_id, &parsed);
        self.inner = Some(BuilderState::Ready(ready));
        Ok(())
    }

    /// Add a monetary value node with explicit period values.
    #[pyo3(text_signature = "($self, node_id, values)")]
    fn value_money(&mut self, node_id: &str, values: Vec<(String, PyMoney)>) -> PyResult<()> {
        // Parse before `take_ready()` (see `value`).
        let parsed: Vec<(PeriodId, finstack_quant_core::money::Money)> = values
            .into_iter()
            .map(|(p, money)| {
                let pid: PeriodId = p.parse().map_err(core_to_py)?;
                Ok((pid, money.inner))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let state = self.take_ready()?;

        let ready = state.value_money(node_id, &parsed);
        self.inner = Some(BuilderState::Ready(ready));
        Ok(())
    }

    /// Add a computed node with a formula.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier.
    /// formula : str
    ///     DSL formula expression (e.g. ``"revenue - cogs"``).
    #[pyo3(text_signature = "($self, node_id, formula)")]
    fn compute(&mut self, node_id: &str, formula: &str) -> PyResult<()> {
        // Validate BEFORE `take_ready()` so a bad formula does not permanently
        // consume the in-progress builder. `ModelBuilder::compute` consumes
        // `self` and returns `Err` on an invalid formula, which would otherwise
        // leave `inner = None` and brick every subsequent call with a
        // misleading "consumed by build()" error. These are the exact checks
        // `compute` runs internally, so behaviour is unchanged on success.
        validate_compute_args(node_id, formula)?;
        let state = self.take_ready()?;
        let ready = state.compute(node_id, formula).map_err(statements_to_py)?;
        self.inner = Some(BuilderState::Ready(ready));
        Ok(())
    }

    /// Start configuring a mixed node.
    #[pyo3(text_signature = "($self, node_id)")]
    fn mixed(&mut self, node_id: &str) -> PyResult<PyMixedNodeBuilder> {
        let state = self.take_ready()?;
        Ok(PyMixedNodeBuilder {
            inner: Some(state.mixed(node_id)),
        })
    }

    /// Attach a forecast specification to an existing or new node.
    #[pyo3(text_signature = "($self, node_id, forecast_spec)")]
    fn forecast(
        &mut self,
        node_id: &str,
        forecast_spec: PyRef<'_, PyForecastSpec>,
    ) -> PyResult<()> {
        let state = self.take_ready()?;
        let ready = state.forecast(node_id, forecast_spec.inner.clone());
        self.inner = Some(BuilderState::Ready(ready));
        Ok(())
    }

    /// Attach a where clause to the last added node.
    #[pyo3(text_signature = "($self, where_clause)")]
    fn where_clause(&mut self, where_clause: &str) -> PyResult<()> {
        let state = self.take_ready()?;
        let ready = state.where_clause(where_clause);
        self.inner = Some(BuilderState::Ready(ready));
        Ok(())
    }

    /// Add model-level metadata from a JSON payload.
    #[pyo3(text_signature = "($self, key, value_json)")]
    fn with_meta(&mut self, key: &str, value_json: &str) -> PyResult<()> {
        let value: serde_json::Value = serde_json::from_str(value_json)
            .map_err(|e| serde_json_to_py(e, "invalid meta value JSON"))?;
        let state = self.take_ready()?;
        let ready = state.with_meta(key, value);
        self.inner = Some(BuilderState::Ready(ready));
        Ok(())
    }

    /// Enable standard accounting name normalization.
    #[pyo3(text_signature = "($self)")]
    fn with_name_normalization(&mut self) -> PyResult<()> {
        let state = self.take_ready()?;
        let ready = state.with_name_normalization();
        self.inner = Some(BuilderState::Ready(ready));
        Ok(())
    }

    /// Add all built-in statement metrics.
    #[pyo3(text_signature = "($self)")]
    fn with_builtin_metrics(&mut self) -> PyResult<()> {
        let state = self.take_ready()?;
        let ready = state.with_builtin_metrics().map_err(statements_to_py)?;
        self.inner = Some(BuilderState::Ready(ready));
        Ok(())
    }

    /// Add one metric and its dependencies from a registry.
    #[pyo3(text_signature = "($self, qualified_id, registry)")]
    fn add_metric_from_registry(
        &mut self,
        qualified_id: &str,
        registry: PyRef<'_, PyMetricRegistry>,
    ) -> PyResult<()> {
        let state = self.take_ready()?;
        let ready = state
            .add_metric_from_registry(qualified_id, &registry.inner)
            .map_err(statements_to_py)?;
        self.inner = Some(BuilderState::Ready(ready));
        Ok(())
    }

    /// Add a fixed-rate bond to the capital structure (US conventions: 30/360, semi-annual).
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// notional : Money
    ///     Principal amount (must be in a valid Currency).
    /// coupon_rate : float
    ///     Annual coupon rate (e.g. 0.05 for 5%).
    /// issue_date, maturity_date : datetime.date
    ///     Bond issue and maturity dates.
    /// discount_curve_id : str
    ///     Discount curve identifier used for pricing.
    #[pyo3(
        text_signature = "($self, id, notional, coupon_rate, issue_date, maturity_date, discount_curve_id)"
    )]
    fn add_bond(
        &mut self,
        id: &str,
        notional: PyRef<'_, PyMoney>,
        coupon_rate: f64,
        issue_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
    ) -> PyResult<()> {
        let notional = notional.inner;
        let issue = py_to_date(issue_date)?;
        let maturity = py_to_date(maturity_date)?;
        let state = self.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(
                b.add_bond(
                    id,
                    notional,
                    coupon_rate,
                    issue,
                    maturity,
                    discount_curve_id,
                )
                .map_err(statements_to_py)?,
            ),
            BuilderState::Ready(b) => BuilderState::Ready(
                b.add_bond(
                    id,
                    notional,
                    coupon_rate,
                    issue,
                    maturity,
                    discount_curve_id,
                )
                .map_err(statements_to_py)?,
            ),
        };
        self.inner = Some(next);
        Ok(())
    }

    /// Add an interest rate swap to the capital structure (US conventions).
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// notional : Money
    ///     Swap notional.
    /// fixed_rate : float
    ///     Fixed leg rate (e.g. 0.04 for 4%).
    /// start_date, maturity_date : datetime.date
    /// discount_curve_id, forward_curve_id : str
    ///     Discount curve and floating-leg forward curve identifiers.
    #[pyo3(
        text_signature = "($self, id, notional, fixed_rate, start_date, maturity_date, discount_curve_id, forward_curve_id)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn add_swap(
        &mut self,
        id: &str,
        notional: PyRef<'_, PyMoney>,
        fixed_rate: f64,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        forward_curve_id: &str,
    ) -> PyResult<()> {
        let notional = notional.inner;
        let start = py_to_date(start_date)?;
        let maturity = py_to_date(maturity_date)?;
        let state = self.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(
                b.add_swap(
                    id,
                    notional,
                    fixed_rate,
                    start,
                    maturity,
                    discount_curve_id,
                    forward_curve_id,
                )
                .map_err(statements_to_py)?,
            ),
            BuilderState::Ready(b) => BuilderState::Ready(
                b.add_swap(
                    id,
                    notional,
                    fixed_rate,
                    start,
                    maturity,
                    discount_curve_id,
                    forward_curve_id,
                )
                .map_err(statements_to_py)?,
            ),
        };
        self.inner = Some(next);
        Ok(())
    }

    /// Add a generic debt instrument via an opaque JSON specification.
    ///
    /// Use this for term loans, RCFs, or any instrument not covered by the
    /// convenience constructors. The ``spec`` is passed straight through to
    /// the capital-structure engine and must match the Rust deserialization
    /// contract for the intended instrument type.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// spec_json : str
    ///     JSON string matching the target instrument's serde shape.
    #[pyo3(text_signature = "($self, id, spec_json)")]
    fn add_custom_debt(&mut self, id: &str, spec_json: &str) -> PyResult<()> {
        let spec: serde_json::Value = serde_json::from_str(spec_json)
            .map_err(|e| serde_json_to_py(e, "invalid debt instrument spec JSON"))?;
        let state = self.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(b.add_custom_debt(id, spec)),
            BuilderState::Ready(b) => BuilderState::Ready(b.add_custom_debt(id, spec)),
        };
        self.inner = Some(next);
        Ok(())
    }

    /// Set the reporting currency used for capital-structure totals.
    #[pyo3(text_signature = "($self, currency)")]
    fn reporting_currency(&mut self, currency: PyRef<'_, PyCurrency>) -> PyResult<()> {
        let ccy = currency.inner;
        let state = self.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(b.reporting_currency(ccy)),
            BuilderState::Ready(b) => BuilderState::Ready(b.reporting_currency(ccy)),
        };
        self.inner = Some(next);
        Ok(())
    }

    /// Set the FX conversion policy for capital-structure cashflows.
    ///
    /// Parameters
    /// ----------
    /// policy : str
    ///     One of ``"cashflow_date"``, ``"period_end"``, ``"period_average"``, ``"custom"``.
    #[pyo3(text_signature = "($self, policy)")]
    fn fx_policy(&mut self, policy: &str) -> PyResult<()> {
        let policy_value = serde_json::Value::String(policy.to_string());
        let parsed: FxConversionPolicy =
            serde_json::from_value(policy_value).map_err(|e| {
                crate::errors::value_error(format!(
                    "invalid fx_policy {policy:?}: {e}; expected one of cashflow_date, period_end, period_average, custom"
                ))
            })?;
        let state = self.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(b.fx_policy(parsed)),
            BuilderState::Ready(b) => BuilderState::Ready(b.fx_policy(parsed)),
        };
        self.inner = Some(next);
        Ok(())
    }

    /// Attach a waterfall specification (PIK toggle + ECF sweep + priority-of-payments).
    #[pyo3(text_signature = "($self, waterfall_spec)")]
    fn waterfall(&mut self, waterfall_spec: PyRef<'_, PyWaterfallSpec>) -> PyResult<()> {
        let spec = waterfall_spec.inner.clone();
        let state = self.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(b.waterfall(spec)),
            BuilderState::Ready(b) => BuilderState::Ready(b.waterfall(spec)),
        };
        self.inner = Some(next);
        Ok(())
    }

    /// Build the model specification.
    ///
    /// Returns
    /// -------
    /// FinancialModelSpec
    ///     The completed model specification.
    #[pyo3(text_signature = "($self)")]
    fn build(&mut self) -> PyResult<PyFinancialModelSpec> {
        let state = self.take_ready()?;
        let spec = state.build().map_err(statements_to_py)?;
        Ok(PyFinancialModelSpec { inner: spec })
    }
}

impl PyModelBuilder {
    fn take_any(&mut self) -> PyResult<BuilderState> {
        self.inner.take().ok_or_else(|| {
            crate::errors::value_error(
                "Builder is no longer usable: it was consumed by build()/mixed(), or a prior \
                 fallible call failed after taking ownership. Construct a new ModelBuilder.",
            )
        })
    }

    fn take_ready(&mut self) -> PyResult<ModelBuilder<finstack_quant_statements::builder::Ready>> {
        let state = self.take_any()?;
        match state {
            BuilderState::Ready(b) => Ok(b),
            BuilderState::NeedPeriods(b) => {
                self.inner = Some(BuilderState::NeedPeriods(b));
                Err(crate::errors::value_error(
                    "Must call periods() before adding nodes",
                ))
            }
        }
    }
}

/// Register builder classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyModelBuilder>()?;
    m.add_class::<PyMixedNodeBuilder>()?;
    m.add_class::<PyMetricRegistry>()?;
    Ok(())
}

fn parse_scalar_values(values: Vec<(String, f64)>) -> PyResult<Vec<(PeriodId, AmountOrScalar)>> {
    values
        .into_iter()
        .map(|(p, v)| {
            let pid: PeriodId = p.parse().map_err(core_to_py)?;
            Ok((pid, AmountOrScalar::scalar(v)))
        })
        .collect()
}
