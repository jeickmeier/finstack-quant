"""
Type stubs for ``finstack_quant.core.table``.

Arrow interchange surface for finstack-quant tabular and 1-D labeled vector
results, from the ``finstack-quant-core`` and ``finstack-quant-arrow`` Rust
crates. Exposes :class:`ArrowTable` and :class:`LabeledSeries`, both
implementing the Arrow PyCapsule C-stream protocol so ``pyarrow``, ``polars``,
``duckdb``, and ``pandas`` can consume finstack results directly.

Examples
--------
>>> from finstack_quant.core.table import LabeledSeries
>>> s = LabeledSeries(["FUND", "BENCH"], [1.4, 0.9], name="sharpe", index_name="ticker")
>>> (len(s), s["FUND"], s.to_dict()["BENCH"])
(2, 1.4, 0.9)

"""

from __future__ import annotations

from collections.abc import Iterator, Sequence
from typing import Any

import numpy as np
import numpy.typing as npt
import pandas as pd
import polars as pl

__all__ = ["ArrowTable", "LabeledSeries"]

class ArrowTable:
    """
    Arrow ``RecordBatch`` wrapper exposing the Arrow PyCapsule C-stream protocol.

    Produced from a core ``TableEnvelope`` or typed Rust result table by
    finstack-quant ``to_arrow`` / ``to_*_table`` methods. Has no public Python
    constructor apart from :meth:`from_ipc` (the pickle path); consume it via
    the standard Arrow PyCapsule interface (``pyarrow.table(arrow_table)``,
    ``polars.DataFrame(arrow_table)``, ``duckdb``) or via :meth:`to_pandas`,
    :meth:`to_polars`, :meth:`to_pyarrow`, :meth:`to_dict`, and
    :meth:`to_records`.

    Supports ``len(table)`` (row count), structural ``==``, HTML rendering in
    notebooks via ``_repr_html_``, and pickling (via Arrow IPC bytes).

    Examples
    --------
    >>> import json
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, value_portfolio
    >>> bundle = {
    ...     "schema": "finstack_quant.portfolio_materialization/1",
    ...     "portfolio": {"id": "empty", "base_currency": "USD", "as_of": "2025-01-01", "entities": {}},
    ...     "instruments": [],
    ...     "positions": [],
    ... }
    >>> portfolio, _ = Portfolio.from_materialization(json.dumps(bundle))
    >>> table = value_portfolio(portfolio, MarketContext()).to_arrow()
    >>> (len(table), table.shape, table.column_names())
    (0, (0, 10), ['position_id', 'entity_id', 'value_native', 'value_base', 'currency_native', 'currency_base', 'fx_rate', 'fx_triangulated', 'risk_metrics_complete', 'risk_error'])
    >>> import pickle
    >>> pickle.loads(pickle.dumps(table)) == table
    True

    """

    @property
    def num_rows(self) -> int:
        """
        Number of rows in the table.

        Returns
        -------
        int
            The row count of the underlying Arrow ``RecordBatch``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_columns(self) -> int:
        """
        Number of columns in the table.

        Returns
        -------
        int
            The column count of the underlying Arrow ``RecordBatch``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def shape(self) -> tuple[int, int]:
        """
        Dimensions of the table as ``(num_rows, num_columns)``.

        Returns
        -------
        tuple[int, int]
            Two-element tuple giving row count followed by column count.

        Notes
        -----
        This accessor does not raise; it returns the stored dimensions.
        """
        ...

    def column_names(self) -> list[str]:
        """
        Column names in declaration order.

        Returns
        -------
        list[str]
            Field names of the underlying Arrow schema, in column order.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def schema(self) -> list[tuple[str, str, bool]]:
        """
        Schema as ``(name, arrow_type, nullable)`` triples in column order.

        Returns
        -------
        list[tuple[str, str, bool]]
            Field name, Arrow data-type string (e.g. ``"Float64"``,
            ``"Utf8"``) and nullability for every column.

        Notes
        -----
        This method does not raise.
        """
        ...

    def to_ipc(self) -> bytes:
        """
        Serialize to Arrow IPC stream bytes (the pickle wire format).

        Returns
        -------
        bytes
            A single-batch Arrow IPC stream.

        Raises
        ------
        ValueError
            If IPC encoding fails.
        """
        ...

    @staticmethod
    def from_ipc(data: bytes) -> ArrowTable:
        """
        Rebuild a table from Arrow IPC stream bytes produced by :meth:`to_ipc`.

        Parameters
        ----------
        data : bytes
            Arrow IPC stream; multiple batches are concatenated.

        Returns
        -------
        ArrowTable
            The decoded table.

        Raises
        ------
        ValueError
            If *data* is not a valid Arrow IPC stream.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.core.table import ArrowTable
        >>> from finstack_quant.portfolio import Portfolio, value_portfolio
        >>> bundle = {
        ...     "schema": "finstack_quant.portfolio_materialization/1",
        ...     "portfolio": {"id": "empty", "base_currency": "USD", "as_of": "2025-01-01", "entities": {}},
        ...     "instruments": [],
        ...     "positions": [],
        ... }
        >>> portfolio, _ = Portfolio.from_materialization(json.dumps(bundle))
        >>> table = value_portfolio(portfolio, MarketContext()).to_arrow()
        >>> ArrowTable.from_ipc(table.to_ipc()) == table
        True
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        ``pyarrow.Table`` view of this table.

        Returns
        -------
        pyarrow.Table
            Zero-copy table built through the PyCapsule protocol.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        ``polars.DataFrame`` view of this table.

        Returns
        -------
        polars.DataFrame
            DataFrame built through the PyCapsule protocol.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        ``pandas.DataFrame`` view of this table (lazily imports ``pandas``).

        Uses ``pyarrow.table(self).to_pandas()`` when ``pyarrow`` is available,
        and falls back to direct Arrow buffer conversion so ``pandas`` alone
        is sufficient when ``pyarrow`` is not installed.

        Returns
        -------
        pandas.DataFrame
            DataFrame with one column per Arrow field and matching dtypes.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_dict(self) -> dict[str, list[Any]]:
        """
        Convert columns to a plain ``dict[str, list[Any]]`` mapping.

        Returns
        -------
        dict[str, list[Any]]
            Column names mapped to Python lists of scalar values and ``None``.

        Raises
        ------
        ValueError
            If a column array cannot be decoded into Python values.
        """
        ...

    def to_records(self) -> list[dict[str, Any]]:
        """
        Convert rows to a list of per-row ``dict[str, Any]`` records.

        Returns
        -------
        list[dict[str, Any]]
            One dictionary per row keyed by column name in schema order.

        Raises
        ------
        ValueError
            If a column array cannot be decoded into Python values.
        """
        ...

    def __len__(self) -> int:
        """Return the number of rows.

        Returns
        -------
        int
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Return whether two tables have the same schema and column contents.

        Returns
        -------
        bool
        """
        ...

    def __reduce__(self) -> tuple[object, tuple[bytes]]: ...
    def __arrow_c_stream__(self, requested_schema: Any | None = None) -> Any:
        """
        Export the table via the Arrow PyCapsule C-stream protocol.

        Implements the Arrow PyCapsule Interface
        (https://arrow.apache.org/docs/format/CDataInterface/PyCapsuleInterface.html):
        returns a ``PyCapsule`` named ``"arrow_array_stream"`` wrapping an
        ``ArrowArrayStream`` with a single record batch. Consumers such as
        ``pyarrow.table(obj)``, ``polars.DataFrame(obj)``, and DuckDB call
        this automatically; it is rarely invoked directly.

        Parameters
        ----------
        requested_schema : object or None
            Optional PyCapsule-protocol schema-negotiation argument. Accepted
            for interface compliance and ignored: the native schema is
            always exported.

        Returns
        -------
        object
            A ``PyCapsule`` named ``"arrow_array_stream"``.

        Raises
        ------
        RuntimeError
            If the underlying Arrow stream cannot be constructed.
        """
        ...

    def __arrow_c_schema__(self) -> Any:
        """
        Export the Arrow schema via the Arrow PyCapsule Interface.

        Returns a ``PyCapsule`` named ``"arrow_schema"`` so consumers such as
        ``pyarrow.schema(obj)`` can inspect the schema without allocating data
        stream buffers.

        Returns
        -------
        object
            A ``PyCapsule`` named ``"arrow_schema"``.

        Raises
        ------
        ValueError
            If the Arrow C schema export fails.
        """
        ...

    def _repr_html_(self) -> str: ...
    def __repr__(self) -> str: ...

class LabeledSeries:
    """
    Immutable 1-D labeled vector with mapping access and Arrow/pandas/Polars export.

    Returned by finstack-quant methods that compute one metric value per key
    (such as per-ticker analytics metrics on :class:`finstack_quant.analytics.Performance`,
    1-D cross-sectional feature vectors, and per-node default probabilities).
    Supports dict-like lookups (``s["FUND"]``, ``"FUND" in s``, ``s.get(...)``,
    ``len(s)``, ``keys()``, ``values()``, ``items()``), iteration over labels,
    NumPy conversion via :meth:`to_numpy`, host DataFrame/Series conversion via
    :meth:`to_pandas` / :meth:`to_polars` / :meth:`to_arrow`, and the Arrow
    PyCapsule protocol.

    Parameters
    ----------
    index : Sequence[str]
        Ordered string labels naming each entry; must match the length of
        ``values``.
    values : Sequence[float | int | None] | numpy.ndarray | pandas.Series | polars.Series
        One-dimensional numeric values matching ``index`` in length; ``None``
        represents missing entries and booleans are rejected with ``TypeError``.
    name : str, default="value"
        Name of the metric column or resulting Series.
    index_name : str, default="label"
        Name of the label column when exported to two-column tables via
        :meth:`to_arrow`, :meth:`to_polars`, or :meth:`to_pyarrow`.
    dtype : str or None, default=None
        Optional explicit storage dtype (``"float64"`` or ``"int64"``). When
        ``None``, integer inputs without floats use nullable ``"int64"`` and
        all other non-empty inputs use ``"float64"`` (nullable when ``None``
        is present).

    Raises
    ------
    ValueError
        If ``index`` and ``values`` differ in length or ``dtype`` is not one
        of ``None``, ``"float64"``, or ``"int64"``.
    TypeError
        If ``values`` contains booleans, non-numeric values, or floats when
        ``dtype="int64"``.

    Examples
    --------
    >>> from finstack_quant.core.table import LabeledSeries
    >>> s = LabeledSeries(["A", "B"], [1.5, 2.5], name="sharpe", index_name="ticker")
    >>> (s["A"], "B" in s, list(s.keys()))
    (1.5, True, ['A', 'B'])
    >>> df = s.to_polars()
    >>> df.columns
    ['ticker', 'sharpe']
    """

    def __init__(
        self,
        index: Sequence[str],
        values: Sequence[float | int | None] | npt.NDArray[Any] | pd.Series[Any] | pl.Series,
        *,
        name: str = "value",
        index_name: str = "label",
        dtype: str | None = None,
    ) -> None: ...
    @property
    def name(self) -> str:
        """
        Name of the value series or Arrow measure column.

        Returns
        -------
        str
            Metric name stored on the series.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def index_name(self) -> str:
        """
        Name of the label column in two-column table exports.

        Returns
        -------
        str
            Label column name such as ``"label"`` or ``"ticker"``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def index(self) -> list[str]:
        """
        Ordered index labels matching each entry in the series.

        Returns
        -------
        list[str]
            Copy of the string labels in declaration order.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def get(self, label: str, default: Any = None) -> float | int | Any:
        """
        Return the value for *label*, or *default* if *label* is absent.

        Parameters
        ----------
        label : str
            Label key to look up in the series index.
        default : object, default=None
            Fallback value returned when ``label`` is not present in
            :attr:`index`. Stored ``None`` entries still return ``None``.

        Returns
        -------
        float | int | object
            Associated value (or ``None`` if stored as null) when present,
            otherwise ``default``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def keys(self) -> list[str]:
        """
        Return the ordered index labels (alias for :attr:`index`).

        Returns
        -------
        list[str]
            Index labels in declaration order.

        Notes
        -----
        This method does not raise.
        """
        ...

    def values(self) -> list[float | int | None]:
        """
        Return the series values as a Python ``list``.

        Returns
        -------
        list[float | int | None]
            Numeric values (and ``None`` for missing entries) in index order.

        Notes
        -----
        This method does not raise.
        """
        ...

    def items(self) -> list[tuple[str, float | int | None]]:
        """
        Return ``(label, value)`` pairs in declaration order.

        Returns
        -------
        list[tuple[str, float | int | None]]
            Label/value pairs aligned by index position.

        Notes
        -----
        This method does not raise.
        """
        ...

    def to_dict(self) -> dict[str, float | int | None]:
        """
        Convert entries to an ordered ``dict`` mapping labels to values.

        Returns
        -------
        dict[str, float | int | None]
            Mapping from index label to stored scalar or ``None``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def to_list(self) -> list[float | int | None]:
        """
        Convert values to a plain Python ``list`` (same as :meth:`values`).

        Returns
        -------
        list[float | int | None]
            Values in declaration order.

        Notes
        -----
        This method does not raise.
        """
        ...

    def to_numpy(self) -> npt.NDArray[Any]:
        """
        Convert values to a 1-D NumPy ``ndarray`` (lazily imports ``numpy``).

        Nullable ``float64`` series represent ``None`` as ``NaN`` in a
        ``float64`` array; non-null ``int64`` series produce an ``int64`` array;
        ``int64`` series containing ``None`` produce an ``object`` array.

        Returns
        -------
        numpy.ndarray
            One-dimensional NumPy array containing the series values.

        Raises
        ------
        ImportError
            If ``numpy`` is not installed.
        """
        ...

    def to_pandas(self) -> pd.Series[Any]:
        """
        Convert to a labeled ``pandas.Series`` (lazily imports ``pandas``).

        Returns
        -------
        pandas.Series
            Series indexed by :attr:`index` and named :attr:`name`, using
            ``float64`` or nullable ``"Int64"`` dtype.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export as a two-column :class:`ArrowTable` (``[index_name, name]``).

        Returns
        -------
        ArrowTable
            Two-column Arrow table holding the string index column and the
            typed value column.

        Raises
        ------
        ValueError
            If the underlying Arrow record batch cannot be built.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export as a two-column ``polars.DataFrame`` (``[index_name, name]``).

        Returns
        -------
        polars.DataFrame
            DataFrame built via the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export as a two-column ``pyarrow.Table`` (``[index_name, name]``).

        Returns
        -------
        pyarrow.Table
            Zero-copy PyArrow table built via the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def to_ipc(self) -> bytes:
        """
        Serialize this series to two-column Arrow IPC stream bytes.

        Returns
        -------
        bytes
            Single-batch Arrow IPC stream preserving index name, series name,
            dtype, and nullability.

        Raises
        ------
        ValueError
            If IPC encoding fails.
        """
        ...

    @staticmethod
    def from_ipc(data: bytes) -> LabeledSeries:
        """
        Rebuild a :class:`LabeledSeries` from Arrow IPC bytes from :meth:`to_ipc`.

        Parameters
        ----------
        data : bytes
            Single-batch Arrow IPC stream containing a ``Utf8`` label column
            followed by a ``Float64`` or ``Int64`` value column.

        Returns
        -------
        LabeledSeries
            Reconstructed series preserving labels, names, and dtype.

        Raises
        ------
        ValueError
            If *data* is malformed, lacks two columns, contains null labels, or
            uses an unsupported Arrow value type.

        Examples
        --------
        >>> from finstack_quant.core.table import LabeledSeries
        >>> s = LabeledSeries(["FUND", "BENCH"], [1.2, 0.8], name="sharpe", index_name="ticker")
        >>> LabeledSeries.from_ipc(s.to_ipc()) == s
        True
        """
        ...

    def __len__(self) -> int: ...
    def __contains__(self, label: object) -> bool: ...
    def __getitem__(self, label: str) -> float | int | None: ...
    def __iter__(self) -> Iterator[str]: ...
    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[object, tuple[bytes]]: ...
    def __arrow_c_stream__(self, requested_schema: Any | None = None) -> Any: ...
    def __arrow_c_schema__(self) -> Any: ...
    def _repr_html_(self) -> str | None: ...
    def __repr__(self) -> str: ...
