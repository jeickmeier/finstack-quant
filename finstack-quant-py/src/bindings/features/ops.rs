//! Python enum wrappers for feature transform operation selectors.

use crate::errors::core_to_py;
use finstack_quant_features::{CrossSectionalOp, PairwiseOp, TimeSeriesOp};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyType};

/// Generate a string-accepting operation enum wrapper with ``__members__``
/// and ``values()``.
#[rustfmt::skip]
macro_rules! op_enum {
    ($py_type:ident, $rust_type:ident, $name:literal, $doc:literal) => {
        #[doc = $doc]
        #[pyclass(
            name = $name,
            module = "finstack_quant.features",
            frozen,
            eq,
            skip_from_py_object
        )]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $py_type {
            /// Inner operation.
            pub(crate) inner: $rust_type,
        }

        #[pymethods]
        impl $py_type {
            /// Parse an operation from its snake_case name (e.g. ``"rolling_mean"``).
            ///
            /// Parameters
            /// ----------
            /// name : str
            ///     Canonical snake_case operation name; see ``values()``.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If ``name`` is not an accepted operation; the message lists
            ///     every accepted name.
            #[new]
            #[pyo3(text_signature = "(name)")]
            fn new(name: &str) -> PyResult<Self> {
                name.parse::<$rust_type>()
                    .map(|inner| Self { inner })
                    .map_err(core_to_py)
            }

            /// Canonical snake_case operation name.
            #[getter]
            fn name(&self) -> String {
                self.inner.name()
            }

            /// JSON ``params`` keys this operation reads; any other key is rejected.
            #[getter]
            fn param_keys(&self) -> Vec<&'static str> {
                self.inner.param_keys().to_vec()
            }

            /// Every accepted operation name, in declaration order.
            #[staticmethod]
            #[pyo3(text_signature = "()")]
            fn values() -> Vec<String> {
                $rust_type::names()
            }

            /// Mapping ``{UPPER_SNAKE_NAME: op}`` of every operation (enum-style).
            #[classattr]
            #[allow(non_snake_case)]
            fn __members__(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
                let members = PyDict::new(py);
                for op in $rust_type::ALL {
                    members.set_item(op.name().to_ascii_uppercase(), Self { inner: *op })?;
                }
                Ok(members)
            }

            /// Look up an operation by ``UPPER_SNAKE`` member name (``Op["ROLLING_MEAN"]``).
            #[classmethod]
            fn __class_getitem__(cls: &Bound<'_, PyType>, key: &str) -> PyResult<Self> {
                let members = cls.getattr("__members__")?;
                Ok(*members.get_item(key)?.extract::<PyRef<'_, Self>>()?)
            }

            /// Snake_case name.
            fn __str__(&self) -> String {
                self.inner.name()
            }

            /// Python-style representation (``TimeSeriesOp.ROLLING_MEAN``).
            fn __repr__(&self) -> String {
                format!("{}.{}", $name, self.inner.name().to_ascii_uppercase())
            }
        }
    };
}

op_enum!(
    PyTimeSeriesOp,
    TimeSeriesOp,
    "TimeSeriesOp",
    "Time-series (per-entity, backward-looking) operation selector.\n\n\
     Accepts the snake_case name (``TimeSeriesOp(\"returns\")``) or an\n\
     ``UPPER_SNAKE`` member (``TimeSeriesOp[\"RETURNS\"]``). ``values()`` lists\n\
     every accepted name; ``param_keys`` lists the JSON ``params`` keys an\n\
     operation reads.\n\n\
     Examples\n\
     --------\n\
     >>> from finstack_quant.features import TimeSeriesOp\n\
     >>> TimeSeriesOp(\"returns\").param_keys\n\
     ['periods']"
);
op_enum!(
    PyCrossSectionalOp,
    CrossSectionalOp,
    "CrossSectionalOp",
    "Cross-sectional (per-timestamp) operation selector.\n\n\
     Accepts the snake_case name (``CrossSectionalOp(\"zscore\")``) or an\n\
     ``UPPER_SNAKE`` member. ``values()`` lists every accepted name;\n\
     ``param_keys`` lists the JSON ``params`` keys an operation reads.\n\n\
     Examples\n\
     --------\n\
     >>> from finstack_quant.features import CrossSectionalOp\n\
     >>> CrossSectionalOp(\"winsorize\").param_keys\n\
     ['lower', 'upper']"
);
op_enum!(
    PyPairwiseOp,
    PairwiseOp,
    "PairwiseOp",
    "Pairwise rolling operation selector (``rolling_cov``, ``rolling_corr``,\n\
     ``rolling_beta``). Accepts the snake_case name or an ``UPPER_SNAKE`` member.\n\n\
     Examples\n\
     --------\n\
     >>> from finstack_quant.features import PairwiseOp\n\
     >>> PairwiseOp.values()\n\
     ['rolling_cov', 'rolling_corr', 'rolling_beta']"
);
