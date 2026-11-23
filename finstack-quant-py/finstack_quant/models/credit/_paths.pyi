"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/credit/paths.rs``."""

from __future__ import annotations

from finstack_quant.core.types import CreditRating
import datetime
import numpy as np
from numpy.typing import NDArray
import pandas
import polars as pl
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import HazardCurve
from finstack_quant.core.table import ArrowTable, LabeledSeries
from typing import Any

class SimulatedPaths:
    """
    Monte Carlo asset path simulation results from a Merton model.

    Examples
    --------
    >>> from finstack_quant.models.credit import MertonModel
    >>> model = MertonModel(100.0, 0.25, 80.0, 0.05)
    >>> paths = model.simulate_paths(4, 10, 1.0, seed=42, antithetic=False)
    >>> (paths.num_paths, paths.num_steps, len(paths.times))
    (4, 10, 11)

    """

    def __repr__(self) -> str:
        """Return a single-line diagnostic representation of the simulation results."""
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` via the canonical JSON round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(SimulatedPaths.from_json, (json,))`` so unpickling rebuilds the paths.

        Notes
        -----
        This accessor does not raise; it serializes the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> SimulatedPaths:
        """
        Deserialize simulated paths from their canonical JSON form.

        Parameters
        ----------
        json : str
            Canonical JSON with ``times``, ``asset_values`` (row-major),
            ``num_paths`` and ``num_steps``.

        Returns
        -------
        SimulatedPaths
            Reconstructed path set.

        Raises
        ------
        ValueError
            If ``json`` is malformed or has an incompatible shape.

        Examples
        --------
        >>> from finstack_quant.models.credit import SimulatedPaths
        >>> p = SimulatedPaths.from_json('{"times":[0.0,1.0],"asset_values":[100.0,105.0],"num_paths":1,"num_steps":1}')
        >>> p.get(0, 1)
        105.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical JSON (``times``, ``asset_values``, ``num_paths``, ``num_steps``).

        Returns
        -------
        str
            Canonical JSON representation of this `SimulatedPaths`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def times(self) -> list[float]:
        """
        Time grid from 0 to the simulation horizon.

        Returns
        -------
        list[float]
            Time points in years.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asset_values(self) -> list[float]:
        """
        Asset values in row-major order.

        Returns
        -------
        list[float]
            Flattened path values.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of simulated paths.

        Returns
        -------
        int
            Exactly the count requested from ``simulate_paths``. Antithetic
            mirrors are included in this total rather than doubling it, so the
            row count of :attr:`asset_values` is always
            ``num_paths * (num_steps + 1)``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_steps(self) -> int:
        """
        Number of time steps between grid points.

        Returns
        -------
        int
            Count of simulation increments, at least ``1``. The grid in
            :attr:`times` holds one more point than this because it includes
            ``t = 0``, and each step spans ``horizon / num_steps`` years.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def values_per_path(self) -> int:
        """
        Number of stored values per path (``num_steps + 1``, including ``t = 0``).

        Returns
        -------
        int
            Number of stored values per path (``num_steps + 1``, including ``t = 0``).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    def to_numpy(self) -> NDArray[np.float64]:
        """Return the simulated states as an independent float64 array.

        Returns
        -------
        numpy.ndarray
            Shape (paths, steps + 1), including the initial asset value.

        Raises
        ------
        ValueError
            If the stored values do not match the result dimensions.
        """
        ...

    def terminal_values(self) -> NDArray[np.float64]:
        """Return the final asset value of each simulated path.

        Returns
        -------
        numpy.ndarray
            Independent float64 array in stored path order.

        Notes
        -----
        This method does not raise for a valid result.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """Export simulated asset paths as a long :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Long table with ``path`` (int), ``time`` (float, years), and ``asset_value`` (float),
            ordered by path then time.

        Raises
        ------
        ValueError
            If the value cannot be represented as an Arrow table.
        """
        ...

    def to_pandas(self) -> pandas.DataFrame:
        """Export simulated asset paths as a long pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Long frame with ``path`` (int), ``time`` (float, years), and ``asset_value`` (float),
            ordered by path then time.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If the value cannot be represented as a pandas DataFrame.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """Export simulated asset paths as a long Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Long frame with ``path`` (int), ``time`` (float, years), and ``asset_value`` (float),
            ordered by path then time.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If the value cannot be represented as a Polars DataFrame.
        """
        ...

    def to_pyarrow(self) -> Any:
        """Export simulated asset paths as a long ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Long table with ``path`` (int), ``time`` (float, years), and ``asset_value`` (float),
            ordered by path then time.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        ValueError
            If the value cannot be represented as a PyArrow table.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def get(self, path_idx: int, time_idx: int) -> float | None:
        """
        Return one asset value by path and time-grid index.

        Parameters
        ----------
        path_idx : int
            Zero-based path index.
        time_idx : int
            Zero-based time-grid index (includes ``t = 0``).

        Returns
        -------
        float or None
            Asset value at the requested coordinate, or ``None`` when indices
            are out of range.

        Notes
        -----
        This method does not raise; a missing result is ``None`` rather than an exception.
        """
        ...

    def path(self, path_idx: int) -> list[float] | None:
        """
        Return the contiguous asset-value row for one path.

        Parameters
        ----------
        path_idx : int
            Zero-based path index.

        Returns
        -------
        list[float] or None
            Asset values along the path, or ``None`` when ``path_idx`` is out
            of range.

        Notes
        -----
        This method does not raise; a missing result is ``None`` rather than an exception.
        """
        ...

    def to_nested(self) -> list[list[float]]:
        """
        Materialize nested path storage as a list of path rows.

        Returns
        -------
        list[list[float]]
            One inner list per simulated path.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...
