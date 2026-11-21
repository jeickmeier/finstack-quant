//! Factor-model covariance matrix and configuration wrappers.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::dataframe_utils::{
    arrow_to_pyarrow_table, extract_2d_f64, extract_arrow_float64_column, extract_f64_vec,
    has_arrow_c_stream, is_polars_dataframe, table_column_names, table_column_to_pylist,
    transpose_cols_to_row_major,
};
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::{core_to_py, value_error};
use finstack_quant_factor::{FactorCovarianceMatrix, FactorId, FactorModelConfig};
use numpy::{PyArray2, PyArrayMethods};
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.models.factor._config", "FactorCovarianceMatrix")]
#[pyclass(
    name = "FactorCovarianceMatrix",
    module = "finstack_quant.models.factor",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyFactorCovarianceMatrix {
    pub(crate) inner: FactorCovarianceMatrix,
}

impl PyFactorCovarianceMatrix {
    pub(crate) fn from_inner(inner: FactorCovarianceMatrix) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFactorCovarianceMatrix {
    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorCovarianceMatrix.__init__")]
    #[new]
    #[pyo3(signature = (factor_ids, data))]
    fn new(factor_ids: Vec<String>, data: &Bound<'_, PyAny>) -> PyResult<Self> {
        let n = factor_ids.len();
        let flat =
            crate::bindings::matrix_input::extract_square_matrix(data.py(), data, n, "data")?;
        let ids: Vec<FactorId> = factor_ids.into_iter().map(FactorId::new).collect();
        let inner = FactorCovarianceMatrix::new(ids, flat).map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorCovarianceMatrix.from_dataframe")]
    #[staticmethod]
    fn from_dataframe(df: &Bound<'_, PyAny>) -> PyResult<Self> {
        if is_polars_dataframe(df)? || has_arrow_c_stream(df)? {
            let cols = table_column_names(df)?;
            if cols.is_empty() {
                return Self::new(Vec::new(), df);
            }
            let py = df.py();
            let is_polars = is_polars_dataframe(df)?;
            let nrows: usize = if is_polars {
                df.getattr("height")?.extract()?
            } else {
                arrow_to_pyarrow_table(py, df)?
                    .getattr("num_rows")?
                    .extract()?
            };
            if nrows == cols.len() {
                let (flat, _, _) = extract_2d_f64(df, "data")?
                    .ok_or_else(|| value_error("expected a Polars DataFrame or Arrow table"))?;
                let ids: Vec<FactorId> = cols.into_iter().map(FactorId::new).collect();
                let inner = FactorCovarianceMatrix::new(ids, flat).map_err(core_to_py)?;
                return Ok(Self { inner });
            }
            if nrows + 1 == cols.len() {
                let row_labels: Vec<String> = table_column_to_pylist(df, &cols[0])?
                    .unwrap_or_default()
                    .into_iter()
                    .map(|item| item.str()?.extract::<String>())
                    .collect::<PyResult<_>>()?;
                let names = cols[1..].to_vec();
                if row_labels != names {
                    return Err(value_error(
                        "DataFrame row and column factor identifiers must match in order",
                    ));
                }
                let mut col_data = Vec::with_capacity(names.len());
                if is_polars {
                    for col in &names {
                        col_data.push(extract_f64_vec(&df.get_item(col)?, col)?);
                    }
                } else {
                    let tbl = arrow_to_pyarrow_table(py, df)?;
                    let pa = py.import("pyarrow")?;
                    let pa_types = pa.getattr("types")?;
                    let float64_ty = pa.getattr("float64")?.call0()?;
                    for col in &names {
                        col_data.push(extract_arrow_float64_column(
                            &pa_types,
                            &float64_ty,
                            &tbl.call_method1("column", (col,))?,
                            col,
                        )?);
                    }
                }
                let (flat, _, _) = transpose_cols_to_row_major(&col_data);
                let ids: Vec<FactorId> = names.into_iter().map(FactorId::new).collect();
                let inner = FactorCovarianceMatrix::new(ids, flat).map_err(core_to_py)?;
                return Ok(Self { inner });
            }
            return Err(value_error(format!(
                "expected square (n x n) or labeled (n x (n + 1)) DataFrame, got ({nrows} x {})",
                cols.len()
            )));
        }
        let pandas = df.py().import("pandas")?;
        if !df.is_instance(&pandas.getattr("DataFrame")?)? {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "df must be a pandas DataFrame, Polars DataFrame, or Arrow table",
            ));
        }
        let names: Vec<String> = df.getattr("columns")?.call_method0("tolist")?.extract()?;
        let rows: Vec<String> = df.getattr("index")?.call_method0("tolist")?.extract()?;
        if rows != names {
            return Err(value_error(
                "DataFrame row and column factor identifiers must match in order",
            ));
        }
        Self::new(names, &df.call_method1("to_numpy", ("float64",))?)
    }

    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorCovarianceMatrix.variance")]
    fn variance(&self, factor_id: &str) -> f64 {
        self.inner.variance(&FactorId::new(factor_id))
    }

    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorCovarianceMatrix.covariance")]
    fn covariance(&self, lhs: &str, rhs: &str) -> f64 {
        self.inner
            .covariance(&FactorId::new(lhs), &FactorId::new(rhs))
    }

    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorCovarianceMatrix.correlation")]
    fn correlation(&self, lhs: &str, rhs: &str) -> f64 {
        self.inner
            .correlation(&FactorId::new(lhs), &FactorId::new(rhs))
    }

    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorCovarianceMatrix.to_numpy")]
    #[pyo3(text_signature = "($self)")]
    fn to_numpy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f64>>> {
        let n = self.inner.n_factors();
        let flat = PyArray2::<f64>::zeros(py, [n, n], false);
        {
            let mut view = flat.readwrite();
            let mut target = view.as_array_mut();
            for (i, row) in target.rows_mut().into_iter().enumerate() {
                for (j, cell) in row.into_iter().enumerate() {
                    *cell = self.inner.covariance_at(i, j);
                }
            }
        }
        Ok(flat)
    }

    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorCovarianceMatrix.to_pandas")]
    #[pyo3(text_signature = "($self)")]
    fn to_pandas<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let bound = Bound::new(py, self.to_arrow_table()?)?;
        let frame = PyArrowTable::to_pandas(&bound, py)?;
        frame
            .call_method1("set_index", ("factor_id",))?
            .call_method1("rename_axis", (py.None(),))
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorCovarianceMatrix(factor_ids={:?}, n_factors={})",
            self.factor_ids(),
            self.inner.n_factors()
        )
    }
}

py_getters!(PyFactorCovarianceMatrix |s| {
    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorCovarianceMatrix.n_factors")]
    n_factors -> usize = s.inner.n_factors();
    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorCovarianceMatrix.factor_ids")]
    factor_ids -> Vec<String> = s
        .inner
        .factor_ids()
        .iter()
        .map(ToString::to_string)
        .collect();
    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorCovarianceMatrix.data")]
    data -> Vec<f64> = s.inner.as_slice().to_vec();
});

impl PyFactorCovarianceMatrix {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let ids = self.factor_ids();
        let n = ids.len();
        let mut builder = TableBuilder::new().utf8("factor_id", ids.clone())?;
        for (j, id) in ids.iter().enumerate() {
            let column: Vec<f64> = (0..n).map(|i| self.inner.covariance_at(i, j)).collect();
            builder = builder.f64(id, column)?;
        }
        builder.build()
    }
}

tabular_methods!(
    PyFactorCovarianceMatrix,
    "FactorCovarianceMatrix",
    module = "finstack_quant.models.factor._config",
    custom_pandas
);

wire_methods!(
    PyFactorCovarianceMatrix,
    FactorCovarianceMatrix,
    "FactorCovarianceMatrix",
    module = "finstack_quant.models.factor._config"
);

#[doc = pydoc!("finstack_quant.models.factor._config", "FactorModelConfig")]
#[pyclass(
    name = "FactorModelConfig",
    module = "finstack_quant.models.factor",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFactorModelConfig {
    pub(crate) inner: FactorModelConfig,
}

impl PyFactorModelConfig {
    pub(crate) fn from_inner(inner: FactorModelConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFactorModelConfig {
    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorModelConfig.factors")]
    #[getter]
    fn factors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.factors)
    }

    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorModelConfig.matching")]
    #[getter]
    fn matching<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.matching)
    }

    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorModelConfig.risk_measure")]
    #[getter]
    fn risk_measure<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.risk_measure)
    }

    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorModelConfig.bump_config")]
    #[getter]
    fn bump_config<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .bump_config
            .as_ref()
            .map(|value| serde_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorModelConfig.validate")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!("FactorModelConfig(n_factors={})", self.inner.factors.len())
    }
}

py_getters!(PyFactorModelConfig |s| {
    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorModelConfig.n_factors")]
    n_factors -> usize = s.inner.factors.len();
    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorModelConfig.factor_ids")]
    factor_ids -> Vec<String> = s
        .inner
        .factors
        .iter()
        .map(|factor| factor.id.to_string())
        .collect();
    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorModelConfig.covariance")]
    covariance -> PyFactorCovarianceMatrix =
        PyFactorCovarianceMatrix::from_inner(s.inner.covariance.clone());
    #[doc = pydoc!("finstack_quant.models.factor._config", "FactorModelConfig.unmatched_policy")]
    unmatched_policy -> Option<String> =
        s.inner.unmatched_policy.map(|policy| policy.to_string());
});

wire_methods!(
    PyFactorModelConfig,
    FactorModelConfig,
    "FactorModelConfig",
    module = "finstack_quant.models.factor._config",
    validate
);
