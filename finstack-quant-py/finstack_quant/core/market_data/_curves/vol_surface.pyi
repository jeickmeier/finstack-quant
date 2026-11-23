"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/vol_surface.rs``."""

from __future__ import annotations

from typing import Any, Optional, Sequence, Union
import pandas as pd
import polars as pl
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import VolatilityClock
from finstack_quant.core.table import ArrowTable

class VolSurface:
    """
    Two-dimensional implied volatility surface on an expiry x secondary-coordinate grid.

    Volatilities are decimal annualised standard deviations (``0.20`` is 20%)
    stored row-major by expiry. The secondary axis is a strike, tenor or
    moneyness coordinate depending on ``secondary_axis``.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolSurface
    >>> surface = VolSurface(
    ...     "EQ-VOL",
    ...     [1.0, 2.0],
    ...     [90.0, 100.0, 110.0],
    ...     [[0.22, 0.20, 0.21], [0.23, 0.21, 0.22]],
    ...     secondary_axis="strike",
    ...     quote_type="black_lognormal",
    ...     interpolation_mode="vol",
    ... )
    >>> surface.grid_shape
    (2, 3)
    >>> from finstack_quant.models.volatility import get_surface_vol
    >>> round(get_surface_vol(surface, 1.5, 100.0), 4)
    0.205
    >>> surface.vols[0]
    [0.22, 0.2, 0.21]

    """

    def __init__(
        self,
        id: str,
        expiries: Sequence[float],
        secondary_values: Sequence[float],
        vols: Any,
        *,
        secondary_axis: str,
        quote_type: str,
        interpolation_mode: str,
        displacements: list[float] | None = None,
        expiry_clock: VolatilityClock | None = None,
    ) -> None:
        """
        Construct a vol surface from an expiry x secondary-coordinate grid.

        Parameters
        ----------
        id : str
            Unique surface identifier.
        expiries : Sequence[float]
            Strictly increasing expiry times in years.
        secondary_values : Sequence[float]
            Strictly increasing secondary-axis coordinates.
        vols : Sequence[float], Sequence[Sequence[float]], numpy.ndarray, DataFrame, or Arrow table
            Volatilities as decimals, either flat row-major
            (``len(expiries) * len(secondary_values)``) or as ``len(expiries)`` rows of
            ``len(secondary_values)`` values (a 2-D array, pandas/Polars ``DataFrame``, or Arrow table is accepted).
        secondary_axis : str
            ``"strike"`` or ``"tenor"``.
        quote_type : str
            ``"black_lognormal"``, ``"shifted_black_lognormal"``, or ``"normal"``.
        interpolation_mode : str
            ``"vol"`` (bilinear in vol) or ``"total_variance"``.
        displacements : list[float] | None, optional
            One finite rate-unit displacement per expiry for shifted Black;
            absent for other quote conventions.
        expiry_clock : VolatilityClock | None, optional
            Calendar/day-count clock for resolving expiry dates to year fractions.

        Raises
        ------
        ValueError
            If the grid size does not match the axes, an axis is not strictly
            increasing, a vol is non-finite or negative, or a label is unknown.
        TypeError
            If ``vols`` is not a list, nested list or array-like, or a required
            convention keyword argument is missing or unknown.

        Examples
        --------
        >>> from finstack_quant.core.market_data import VolSurface
        >>> VolSurface(
        ...     "EQ-VOL",
        ...     [1.0],
        ...     [90.0, 100.0],
        ...     [0.22, 0.20],
        ...     secondary_axis="strike",
        ...     quote_type="black_lognormal",
        ...     interpolation_mode="vol",
        ... ).secondary_values
        [90.0, 100.0]

        """
        ...

    @staticmethod
    def from_json(json: str) -> VolSurface:
        """
        Deserialize a surface from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        VolSurface
            The constructed volatility surface, ready for interpolated vol queries.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import VolSurface
        >>> surface = VolSurface(
        ...     "EQ-VOL",
        ...     [1.0],
        ...     [90.0, 100.0],
        ...     [0.22, 0.20],
        ...     secondary_axis="strike",
        ...     quote_type="black_lognormal",
        ...     interpolation_mode="vol",
        ... )
        >>> VolSurface.from_json(surface.to_json()) == surface
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the surface cannot be serialized.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export the grid in long form as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Columns ``expiry``, ``secondary_value`` and ``vol``; one row per grid node,
            expiries outer and secondary values inner.

        Raises
        ------
        ValueError
            If the underlying Arrow record batch cannot be built.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export the grid in long form.

        Returns
        -------
        pd.DataFrame
            Columns ``expiry``, ``secondary_value`` and ``vol``; one row per grid node,
            expiries outer and secondary values inner.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export the grid in long form as a ``polars.DataFrame``.

        Returns
        -------
        pl.DataFrame
            Polars DataFrame constructed via the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export the grid in long form as a ``pyarrow.Table``.

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
    @property
    def id(self) -> str:
        """
        Identifier this surface is registered and looked up under.

        Returns
        -------
        str
            The surface id supplied at construction; market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expiries(self) -> list[float]:
        """
        Option expiries the surface is quoted on.

        Returns
        -------
        list[float]
            Ascending expiry year fractions; they index the rows of the stored volatility grid.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def secondary_values(self) -> list[float]:
        """
        Second grid axis of the surface.

        Returns
        -------
        list[float]
            Ascending axis values whose meaning is given by :attr:`secondary_axis`: absolute strikes, swap tenors in years, or moneyness.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def vols(self) -> list[list[float]]:
        """
        Quoted volatilities on the expiry/secondary-axis grid.

        Returns
        -------
        list[list[float]]
            Row-major grid of decimal volatilities (``0.20`` is 20%) in the convention named by :attr:`quote_type`; one row per expiry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def secondary_axis(self) -> str:
        """
        What the second axis of the grid measures.

        Returns
        -------
        str
            Either ``"strike"`` (absolute strike levels) or ``"tenor"`` (underlying swap tenor in years).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def quote_type(self) -> str:
        """
        Volatility convention the grid values are quoted in.

        Returns
        -------
        str
            ``"black_lognormal"`` or ``"shifted_black_lognormal"`` for decimal Black
            vol, or ``"normal"`` for absolute Bachelier vol in rate units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def displacements(self) -> list[float] | None:
        """Return the shifted-Black displacement attached to each expiry.

        Returns
        -------
        list[float] | None
            Rate-unit displacements aligned to ``expiries``, or ``None`` for
            unshifted quotes.

        Notes
        -----
        This accessor does not raise; it returns the stored metadata.
        """
        ...

    @property
    def expiry_clock(self) -> Optional[VolatilityClock]:
        """
        Calendar/day-count volatility clock attached to this surface, if any.

        Returns
        -------
        Optional[VolatilityClock]
            Volatility clock used when resolving expiry dates, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interpolation_mode(self) -> str:
        """
        Quantity interpolated between grid nodes.

        Returns
        -------
        str
            Either ``"vol"`` (interpolate volatilities directly) or ``"total_variance"`` (interpolate ``vol**2 * T``, which is arbitrage-safer in expiry).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def grid_shape(self) -> tuple[int, int]:
        """
        Dimensions of the stored volatility grid.

        Returns
        -------
        tuple[int, int]
            ``(n_expiries, n_secondary_values)``: the row and column counts of the grid, useful for reshaping the flat value list.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...
