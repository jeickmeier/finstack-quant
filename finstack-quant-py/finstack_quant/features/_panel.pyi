"""Stub declarations mirroring ``finstack-quant-py/src/bindings/features/panel.rs``."""

from __future__ import annotations

import datetime
from collections.abc import Sequence
from types import ModuleType
from typing import Any
import numpy as np
import numpy.typing as npt
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable
from finstack_quant.features import KeyColumn, ValueColumn

class PanelTransformSpec:
    """
    Specification for a sequential panel transform pipeline.

    Examples
    --------
    >>> from finstack_quant.features import PanelTransformSpec
    >>> spec = PanelTransformSpec(
    ...     [1.0, 3.0], [{"name": "r", "family": "cross_sectional", "op": "rank"}], time_key=["d", "d"]
    ... )
    >>> spec.operation_names
    ['r']
    """

    def __init__(
        self,
        values: ValueColumn,
        operations: Sequence[dict[str, Any]],
        entity: KeyColumn | None = None,
        order: KeyColumn | None = None,
        time_key: KeyColumn | None = None,
    ) -> None:
        """
        Construct a panel spec.

        Parameters
        ----------
        values : ValueColumn
            Input value column (sequence, NumPy array, ``pandas.Series``, or
            ``polars.Series``); ``None`` / NaN is missing.
        operations : sequence of dict
            Ordered operations ``{"name", "family" ("timeseries" |
            "cross_sectional"), "op", "params"?, "input"?}``.
        entity : sequence, optional
            Row-aligned entity keys (required for time-series ops; str or
            date-like; other types raise ``TypeError``).
        order : sequence, optional
            Row-aligned sort keys (required for time-series ops).
        time_key : sequence, optional
            Row-aligned partition keys (required for cross-sectional ops).

        Raises
        ------
        ValueError
            If an operation mapping is malformed (unknown family, op, or key).

        Examples
        --------
        >>> from finstack_quant.features import PanelTransformSpec
        >>> PanelTransformSpec([1.0], [], time_key=["d"]).operation_names
        []
        """
        ...

    @property
    def operation_names(self) -> list[str]:
        """
        Output column names in operation order.

        Returns
        -------
        list[str]
            Operation ``name`` fields.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def values(self) -> list[float | None]:
        """
        Input value column.

        Returns
        -------
        list[float | None]
            Values as supplied.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical ``PanelTransformSpec`` JSON that :meth:`from_json` loads.

        Returns
        -------
        str
            JSON document.

        Raises
        ------
        ValueError
            If a value cannot be represented in JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PanelTransformSpec:
        """
        Deserialize from JSON (strict field names).

        Parameters
        ----------
        json : str
            JSON document.

        Returns
        -------
        PanelTransformSpec
            Reconstructed spec.

        Raises
        ------
        ValueError
            If the JSON is malformed.

        Examples
        --------
        >>> from finstack_quant.features import PanelTransformSpec
        >>> PanelTransformSpec.from_json('{"values": [1.0], "operations": []}').values
        [1.0]
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Pickle support via the JSON wire form.

        Returns
        -------
        tuple
            ``(from_json, (json,))`` reconstructor pair.

        Raises
        ------
        ValueError
            If a value cannot be represented in JSON.
        """
        ...

    def __repr__(self) -> str: ...

class PanelTransformResult:
    """
    Ordered output columns of a panel transform pipeline.

    Examples
    --------
    >>> from finstack_quant.features import transform_panel
    >>> res = transform_panel({
    ...     "values": [1.0, 3.0],
    ...     "time_key": ["d", "d"],
    ...     "operations": [{"name": "r", "family": "cross_sectional", "op": "rank"}],
    ... })
    >>> res.columns
    ['r']
    """

    @property
    def columns(self) -> list[str]:
        """
        Output column names in operation order.

        Returns
        -------
        list[str]
            Column names.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def get_column(self, name: str) -> list[float | None]:
        """
        Values of one output column, aligned to the input rows.

        Parameters
        ----------
        name : str
            Operation output name (case-sensitive).

        Returns
        -------
        list[float | None]
            Column values.

        Raises
        ------
        KeyError
            If no column has that name.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical ``PanelTransformResult`` JSON that :meth:`from_json` loads.

        Returns
        -------
        str
            ``{"columns": [{"name", "values"}, ...]}``.

        Raises
        ------
        ValueError
            If a value cannot be represented in JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PanelTransformResult:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON document.

        Returns
        -------
        PanelTransformResult
            Reconstructed result.

        Raises
        ------
        ValueError
            If the JSON is malformed.

        Examples
        --------
        >>> from finstack_quant.features import PanelTransformResult
        >>> PanelTransformResult.from_json('{"columns": []}').columns
        []
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Pickle support via the JSON wire form.

        Returns
        -------
        tuple
            ``(from_json, (json,))`` reconstructor pair.

        Raises
        ------
        ValueError
            If a value cannot be represented in JSON.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export transform columns as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Arrow table with one ``Float64`` column per transform operation.

        Raises
        ------
        ValueError
            If the underlying Arrow record batch cannot be built.
        """
        ...

    def to_pandas(self, index: Any | None = None) -> pd.DataFrame:
        """
        Columns as a ``pandas.DataFrame`` (one float column per operation).

        Parameters
        ----------
        index : pandas.Index or sequence, optional
            Row index to attach (e.g. the source frame's index).

        Returns
        -------
        pandas.DataFrame
            ``None`` outputs become ``NaN``.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Columns as a ``polars.DataFrame`` (one float column per operation).

        Returns
        -------
        polars.DataFrame
            DataFrame constructed via the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Columns as a ``pyarrow.Table`` (one float column per operation).

        Returns
        -------
        pyarrow.Table
            Zero-copy table built through the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: Any | None = None) -> Any: ...
    def __arrow_c_schema__(self) -> Any: ...
    def _repr_html_(self) -> str | None: ...
    def __repr__(self) -> str: ...

def transform_panel(spec: PanelTransformSpec | dict[str, Any]) -> PanelTransformResult:
    """
    Apply a named panel transform pipeline.

    Operations run sequentially; each reads the previous column unless
    ``input`` selects ``"values"`` or an earlier operation name.

    Parameters
    ----------
    spec : PanelTransformSpec, dict
        Typed spec, an equivalent dict (``values``, ``operations``, optional
        ``entity`` / ``order`` / ``time_key``).

    Returns
    -------
    PanelTransformResult
        Ordered output columns with ``get_column`` and ``to_pandas`` /
        ``to_polars`` / ``to_arrow``.

    Raises
    ------
    ValueError
        If the spec is malformed, an operation name is duplicated or reserved,
        ``input`` is unknown, or an operation fails.

    Examples
    --------
    >>> from finstack_quant.features import transform_panel
    >>> spec = {
    ...     "values": [1.0, 3.0],
    ...     "time_key": ["d", "d"],
    ...     "operations": [{"name": "r", "family": "cross_sectional", "op": "rank"}],
    ... }
    >>> transform_panel(spec).get_column("r")
    [0.0, 1.0]
    """
    ...
