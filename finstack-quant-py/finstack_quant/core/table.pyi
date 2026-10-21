"""
Type stubs for ``finstack_quant.core.table``.

Arrow interchange surface for finstack-quant tabular results, from the
``finstack-quant-core``/``finstack-quant-arrow`` Rust crates. Exposes
:class:`ArrowTable`, a zero-copy Arrow ``RecordBatch`` wrapper implementing
the Arrow PyCapsule C-stream protocol so ``pyarrow``, ``polars``, ``duckdb``,
and ``pandas`` can consume finstack tabular results directly.

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
>>> table = value_portfolio(portfolio, MarketContext()).to_arrow_positions()
>>> (table.num_rows, table.num_columns, table.column_names())
(0, 8, ['position_id', 'entity_id', 'value_native', 'value_base', 'currency_native', 'currency_base', 'fx_rate', 'risk_error'])

"""

from __future__ import annotations

from typing import Any

__all__ = ["ArrowTable"]

class ArrowTable:
    """
    Arrow ``RecordBatch`` wrapper exposing the Arrow PyCapsule C-stream protocol.

    Produced internally from a core ``TableEnvelope`` by finstack-quant
    ``to_arrow_*`` producer methods. Has no Python-facing constructor apart
    from :meth:`from_ipc` (the pickle path); consume it via the standard
    Arrow PyCapsule interface, for example ``pyarrow.table(arrow_table)`` or
    ``polars.DataFrame(arrow_table)``, or the lazy helpers
    :meth:`to_pyarrow`, :meth:`to_polars` and :meth:`to_pandas`.

    pandas recipe (requires ``pyarrow`` and ``pandas``)::

        df = pyarrow.table(arrow_table).to_pandas()  # or arrow_table.to_pandas()

    Supports ``len(table)`` (row count), structural ``==`` and pickling
    (via Arrow IPC bytes).

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
    >>> table = value_portfolio(portfolio, MarketContext()).to_arrow_positions()
    >>> (table.num_rows, table.num_columns, table.column_names())
    (0, 8, ['position_id', 'entity_id', 'value_native', 'value_base', 'currency_native', 'currency_base', 'fx_rate', 'risk_error'])
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

    @classmethod
    def from_ipc(cls, data: bytes) -> ArrowTable:
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
        >>> table = value_portfolio(portfolio, MarketContext()).to_arrow_positions()
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

    def to_polars(self) -> Any:
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

    def to_pandas(self) -> Any:
        """
        ``pandas.DataFrame`` view via ``pyarrow.table(self).to_pandas()``.

        Returns
        -------
        pandas.DataFrame
            DataFrame with one column per Arrow field.

        Raises
        ------
        ImportError
            If ``pyarrow`` or ``pandas`` is not installed.
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

    def __repr__(self) -> str: ...
