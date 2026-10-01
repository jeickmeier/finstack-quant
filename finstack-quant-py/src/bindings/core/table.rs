//! Bindings for `finstack_quant_core::table`: Arrow interchange surface.
//!
//! `ArrowTable` wraps an Arrow `RecordBatch` built from a
//! `TableEnvelope` and implements the Arrow PyCapsule C-stream protocol
//! (`__arrow_c_stream__`), so `pyarrow.table(obj)`, `polars.DataFrame(obj)`,
//! DuckDB, and pandas can consume finstack tabular results zero-copy with no
//! extra Python dependencies.
//!
//! Spec: <https://arrow.apache.org/docs/format/CDataInterface/PyCapsuleInterface.html>

use arrow::ffi_stream::FFI_ArrowArrayStream;
use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::{RecordBatch, RecordBatchIterator};
use finstack_quant_core::table::TableEnvelope;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyCapsule, PyList};

use crate::errors::value_error;

/// Arrow ``RecordBatch`` wrapper exposing the Arrow PyCapsule C-stream protocol.
///
/// Produced by finstack-quant ``to_arrow_*`` methods; there is no public
/// constructor apart from ``from_ipc`` (used by pickle). Consume it with
/// ``pyarrow.table(t)``, ``polars.DataFrame(t)``, DuckDB, or the
/// ``to_pyarrow()`` / ``to_polars()`` / ``to_pandas()`` helpers (each lazily
/// imports its library). pandas recipe without pyarrow's helper:
/// ``pyarrow.table(t).to_pandas()``.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.core.market_data import MarketContext
/// >>> from finstack_quant.portfolio import Portfolio, value_portfolio
/// >>> bundle = {
/// ...     "schema": "finstack_quant.portfolio_materialization/1",
/// ...     "portfolio": {"id": "empty", "base_currency": "USD", "as_of": "2025-01-01", "entities": {}},
/// ...     "instruments": [],
/// ...     "positions": [],
/// ... }
/// >>> portfolio, _ = Portfolio.from_materialization(json.dumps(bundle))
/// >>> table = value_portfolio(portfolio, MarketContext()).to_arrow_positions()
/// >>> (len(table), table.num_columns)
/// (0, 6)
#[pyclass(module = "finstack_quant.core.table", name = "ArrowTable", frozen)]
pub struct PyArrowTable {
    pub(crate) batch: RecordBatch,
}

impl PyArrowTable {
    /// Convert a core table envelope into an Arrow-backed Python table.
    pub(crate) fn from_envelope(table: &TableEnvelope) -> PyResult<Self> {
        let batch =
            finstack_quant_arrow::to_record_batch(table).map_err(crate::errors::core_to_py)?;
        Ok(Self { batch })
    }

    fn to_ipc_bytes(&self) -> PyResult<Vec<u8>> {
        let mut buffer = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut buffer, &self.batch.schema())
                .map_err(|e| value_error(format!("Arrow IPC encode failed: {e}")))?;
            writer
                .write(&self.batch)
                .map_err(|e| value_error(format!("Arrow IPC encode failed: {e}")))?;
            writer
                .finish()
                .map_err(|e| value_error(format!("Arrow IPC encode failed: {e}")))?;
        }
        Ok(buffer)
    }
}

#[pymethods]
impl PyArrowTable {
    /// Number of rows in the table.
    #[getter]
    fn num_rows(&self) -> usize {
        self.batch.num_rows()
    }

    /// Number of columns in the table.
    #[getter]
    fn num_columns(&self) -> usize {
        self.batch.num_columns()
    }

    /// ``len(table)`` — number of rows.
    fn __len__(&self) -> usize {
        self.batch.num_rows()
    }

    /// Column names in declaration order.
    #[pyo3(text_signature = "($self)")]
    fn column_names(&self) -> Vec<String> {
        self.batch
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect()
    }

    /// Schema as ``[(name, arrow_type_string, nullable), ...]`` in column order.
    #[pyo3(text_signature = "($self)")]
    fn schema(&self) -> Vec<(String, String, bool)> {
        self.batch
            .schema()
            .fields()
            .iter()
            .map(|f| (f.name().clone(), f.data_type().to_string(), f.is_nullable()))
            .collect()
    }

    /// Structural equality: same schema and identical column contents.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, PyArrowTable>>()
            .map(|rhs| self.batch == rhs.batch)
            .unwrap_or(false)
    }

    /// Export the table via the Arrow PyCapsule C-stream protocol.
    ///
    /// Returns a ``PyCapsule`` named ``arrow_array_stream`` containing an
    /// ``ArrowArrayStream`` with a single record batch. Each call produces a
    /// fresh stream (the underlying buffers are shared, not copied), so the
    /// table can be consumed by multiple consumers. ``requested_schema`` is
    /// accepted per the protocol and ignored: the native schema is always
    /// exported. See the PyCapsule Interface spec for the full protocol:
    /// <https://arrow.apache.org/docs/format/CDataInterface/PyCapsuleInterface.html>
    #[pyo3(signature = (requested_schema=None))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let _ = requested_schema; // schema negotiation unsupported; export native schema
        let schema = self.batch.schema();
        let reader = RecordBatchIterator::new([Ok(self.batch.clone())], schema);
        let stream = FFI_ArrowArrayStream::new(Box::new(reader));
        PyCapsule::new_with_value(py, stream, c"arrow_array_stream")
    }

    /// Serialize to Arrow IPC stream bytes (the pickle wire format).
    #[pyo3(text_signature = "($self)")]
    fn to_ipc<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.to_ipc_bytes()?))
    }

    /// Rebuild a table from Arrow IPC stream bytes produced by ``to_ipc``.
    ///
    /// Raises ``ValueError`` if the bytes are not a single-batch IPC stream.
    #[staticmethod]
    #[pyo3(text_signature = "(data)")]
    fn from_ipc(data: &[u8]) -> PyResult<Self> {
        let reader = StreamReader::try_new(std::io::Cursor::new(data), None)
            .map_err(|e| value_error(format!("Arrow IPC decode failed: {e}")))?;
        let schema = reader.schema();
        let batches: Vec<RecordBatch> = reader
            .collect::<Result<_, _>>()
            .map_err(|e| value_error(format!("Arrow IPC decode failed: {e}")))?;
        let batch = arrow::compute::concat_batches(&schema, &batches)
            .map_err(|e| value_error(format!("Arrow IPC decode failed: {e}")))?;
        Ok(Self { batch })
    }

    /// Support ``pickle`` via the Arrow IPC stream format.
    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (Bound<'py, PyBytes>,))> {
        let from_ipc = py.get_type::<Self>().getattr("from_ipc")?;
        Ok((from_ipc, (self.to_ipc(py)?,)))
    }

    /// ``pyarrow.Table`` view (lazily imports ``pyarrow``).
    #[pyo3(text_signature = "($self)")]
    fn to_pyarrow<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        py.import("pyarrow")?.call_method1("table", (slf,))
    }

    /// ``polars.DataFrame`` view (lazily imports ``polars``).
    #[pyo3(text_signature = "($self)")]
    fn to_polars<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        py.import("polars")?.call_method1("DataFrame", (slf,))
    }

    /// ``pandas.DataFrame`` view via ``pyarrow.table(self).to_pandas()``
    /// (lazily imports ``pyarrow``, which itself requires ``pandas``).
    #[pyo3(text_signature = "($self)")]
    fn to_pandas<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Self::to_pyarrow(slf, py)?.call_method0("to_pandas")
    }

    /// Return a debug-style representation.
    fn __repr__(&self) -> String {
        format!(
            "ArrowTable(rows={}, columns={})",
            self.batch.num_rows(),
            self.batch.num_columns()
        )
    }
}

/// Build the `finstack_quant.core.table` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "table")?;
    m.setattr(
        "__doc__",
        "Arrow interchange surface for finstack-quant tabular results.\n\n\
         Exposes ArrowTable, a RecordBatch wrapper implementing the Arrow \
         PyCapsule C-stream protocol so pyarrow, polars, duckdb, and pandas \
         can consume finstack tabular outputs. Backed by the supporting \
         finstack-quant-arrow Rust crate (TableEnvelope <-> RecordBatch).",
    )?;
    m.add_class::<PyArrowTable>()?;
    let all = PyList::new(py, ["ArrowTable"])?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;
    Ok(())
}
