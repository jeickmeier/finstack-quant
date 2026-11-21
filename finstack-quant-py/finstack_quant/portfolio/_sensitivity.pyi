"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/sensitivity.rs``."""

from __future__ import annotations

from finstack_quant.models.factor import FactorDefinition

import datetime
from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.table import ArrowTable
from finstack_quant.portfolio._container import Portfolio

# Factor Sensitivity

class SensitivityMatrix:
    """
    Positions-by-factors sensitivity matrix.

    Each element ``(i, j)`` is the first-order sensitivity of position *i* to
    factor *j*, denominated in the factor's bump units (e.g. PV change per 1 bp
    for a rates factor).

    Construct via :func:`compute_factor_sensitivities`.

    JSON interchange matches WASM: ``base_currency``, ordered ``position_ids``
    and ``factor_ids``, and nested ``data[position][factor]`` rows. Row and
    column counts must match the axes, and every entry must be finite.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, compute_factor_sensitivities
    >>> book = Portfolio.builder("book", "USD", "2025-01-15").build()
    >>> matrix = compute_factor_sensitivities(book, "[]", MarketContext(), "2025-01-15")
    >>> (matrix.n_positions, matrix.n_factors)
    (0, 0)
    """

    @property
    def base_currency(self) -> str:
        """
        ISO reporting currency of monetary entries.

        Returns
        -------
        str
            Three-letter currency code used for the calculation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> SensitivityMatrix:
        """
        Parse the canonical sensitivity-matrix wire JSON.

        The wire object is ``{base_currency, position_ids, factor_ids, data}``
        with ``data`` as one row per position (``data[position][factor]``):
        the JSON :meth:`to_json` emits, the WASM
        ``computeFactorSensitivities`` returns and ``models.factor.risk.decomposeFactors``
        accepts.

        Parameters
        ----------
        json : str
            Canonical object with required ``base_currency`` (ISO reporting
            currency), ordered ``position_ids``/``factor_ids``, and nested
            ``data[position][factor]`` rows. Empty factor axes require one
            empty row per position. Unknown fields are rejected.

        Returns
        -------
        SensitivityMatrix
            Reconstructed matrix with its reporting currency.

        Raises
        ------
        ValueError
            If JSON is malformed, the currency is unknown or missing, row or
            column counts disagree with the axes, an entry is non-finite, or
            an unknown field (including obsolete ``n_factors``) is supplied.

        Examples
        --------
        >>> from finstack_quant.portfolio import SensitivityMatrix
        >>> wire = '{"base_currency":"USD","position_ids":["A"],"factor_ids":["F1","F2"],"data":[[1.0,2.0]]}'
        >>> SensitivityMatrix.from_json(wire).delta(0, 1)
        2.0
        >>> try:
        ...     SensitivityMatrix.from_json(wire.replace("[[1.0,2.0]]", "[[1.0]]"))
        ... except ValueError as exc:
        ...     print("row 0 has 1 element(s)" in str(exc))
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical sensitivity-matrix wire JSON.

        Returns
        -------
        str
            Canonical monetary matrix JSON with nested rows, accepted by
            :meth:`from_json` and WASM ``models.factor.risk.decomposeFactors``; also backs
            ``pickle``. The derived ``n_factors`` count is omitted.

        Notes
        -----
        This method does not raise for a well-formed value.
        """
        ...

    @property
    def position_ids(self) -> list[str]:
        """
        Ordered position identifiers (row axis).

        Returns
        -------
        list[str]
            Ordered position identifiers (row axis).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factor_ids(self) -> list[str]:
        """
        Ordered factor identifiers (column axis).

        Returns
        -------
        list[str]
            Ordered factor identifiers (column axis).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_positions(self) -> int:
        """
        Number of positions (rows).

        Returns
        -------
        int
            Number of positions (rows).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_factors(self) -> int:
        """
        Number of factors (columns).

        Returns
        -------
        int
            Number of factors (columns).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def delta(self, position_idx: int | str, factor_idx: int | str) -> float:
        """Read the sensitivity of a position to a factor.

        Parameters
        ----------
        position_idx : int | str
            Zero-based row index or exact position identifier from ``position_ids``.
        factor_idx : int | str
            Zero-based column index or exact factor identifier from ``factor_ids``.

        Returns
        -------
        float
            Sensitivity in reporting currency per unit of the factor's bump.

        Raises
        ------
        KeyError
            If a supplied position or factor identifier is unknown.
        ValueError
            If an unsigned integer index is outside the matrix bounds.
        OverflowError
            If an integer index is negative or too large for a platform index.
        TypeError
            If a coordinate is neither an integer nor a string.
        """
        ...

    def position_deltas(self, position_idx: int) -> list[float]:
        """
        Sensitivity row for a single position across all factors.

        Parameters
        ----------
        position_idx : int
            Zero-based row index into ``position_ids`` for the requested position.

        Returns
        -------
        list[float]
            List of delta values, one per factor.

        Raises
        ------
        ValueError
            If ``position_idx`` is outside the matrix bounds.
        """
        ...

    def factor_deltas(self, factor_idx: int) -> list[float]:
        """
        Sensitivity column for a single factor across all positions.

        Parameters
        ----------
        factor_idx : int
            Column index.

        Returns
        -------
        list[float]
            List of delta values, one per position.

        Raises
        ------
        ValueError
            If ``factor_idx`` is outside the matrix bounds.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export as an :class:`~finstack_quant.core.table.ArrowTable` with ``position_id``
        and factor sensitivity columns.

        Returns
        -------
        ArrowTable
            One row per position; columns ``position_id`` plus factor IDs.

        Raises
        ------
        ValueError
            If the result cannot be serialized into an Arrow table.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export as a pandas ``DataFrame`` indexed by ``position_id`` with factors as columns.

        Returns
        -------
        pd.DataFrame
            DataFrame indexed by ``position_id`` with factor IDs as column names.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export as a Polars ``DataFrame`` with ``position_id`` and factor columns.

        Returns
        -------
        pl.DataFrame
            DataFrame with ``position_id`` and factor ID columns.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export as a ``pyarrow.Table`` with ``position_id`` and factor columns.

        Returns
        -------
        pyarrow.Table
            Table with ``position_id`` and factor ID columns.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class FactorPnlProfile:
    """
    P&L profile for one factor across a scenario grid.

    Each profile captures the hypothetical P&L for every position at each
    scenario shift, enabling non-linear (gamma, convexity) analysis.

    Construct via :func:`compute_pnl_profiles`.

    Examples
    --------
    >>> from finstack_quant.portfolio import FactorPnlProfile
    >>> profile = FactorPnlProfile.from_json(
    ...     '{"base_currency":"USD","factor_id":"USD_10Y","position_ids":["P1"],"shifts":[-1.0,0.0,1.0],"position_pnls":[[-1.0],[0.0],[1.0]]}'
    ... )
    >>> profile.to_pandas()["P1"].tolist()
    [-1.0, 0.0, 1.0]
    """

    @property
    def base_currency(self) -> str:
        """
        ISO reporting currency of monetary entries.

        Returns
        -------
        str
            Three-letter currency code used for the calculation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FactorPnlProfile:
        """
        Parse from canonical JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Canonical payload.

        Returns
        -------
        FactorPnlProfile
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.portfolio import FactorPnlProfile
        >>> try:
        ...     FactorPnlProfile.from_json("{}")
        ... except ValueError:
        ...     print("missing fields")
        missing fields
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json`; also backs ``pickle``.

        Notes
        -----
        This method does not raise for a well-formed value.
        """
        ...

    @property
    def factor_id(self) -> str:
        """
        Identifier of the contributing risk factor in the model.

        Returns
        -------
        str
            Identifier of the contributing risk factor in the model.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_ids(self) -> list[str]:
        """
        Ordered position identifiers indexing the inner ``position_pnls`` axis.

        Returns
        -------
        list[str]
            Column labels used by :meth:`to_arrow`, :meth:`to_pandas`, :meth:`to_polars`, and :meth:`to_pyarrow`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def shifts(self) -> list[float]:
        """
        Scenario shift coordinates (bump-size multiples).

        Returns
        -------
        list[float]
            Scenario shift coordinates (bump-size multiples).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_pnls(self) -> list[list[float]]:
        """
        Per-shift P&L vectors indexed as ``[shift_idx][position_idx]``.

        Returns
        -------
        list[list[float]]
            Per-shift P&L vectors indexed as ``[shift_idx][position_idx]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export as an :class:`~finstack_quant.core.table.ArrowTable` with ``shift``
        and position columns.

        Returns
        -------
        ArrowTable
            Columns ``shift`` and one column per entry of ``position_ids``.

        Raises
        ------
        ValueError
            If the table cannot be built.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export as a pandas ``DataFrame`` indexed by ``shift`` with positions as columns.

        Returns
        -------
        pd.DataFrame
            Indexed by ``shift`` value; one column per entry of ``position_ids``.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export as a Polars ``DataFrame`` with ``shift`` and position columns.

        Returns
        -------
        pl.DataFrame
            DataFrame with ``shift`` and one column per entry of ``position_ids``.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export as a ``pyarrow.Table`` with ``shift`` and position columns.

        Returns
        -------
        pyarrow.Table
            Table with ``shift`` and one column per entry of ``position_ids``.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def compute_factor_sensitivities(
    portfolio: Portfolio | str,
    factors: list[FactorDefinition] | str | list[dict[str, Any]] | pd.DataFrame,
    market: MarketContext | str,
    as_of: datetime.date | str,
    bump_config: str | dict[str, Any] | None = None,
) -> SensitivityMatrix:
    """
    Compute the position-by-factor sensitivity matrix of a portfolio.

    Each cell is the central difference of the position's base-currency PV
    under an up and a down bump of the factor, weighted by the position's
    scale factor (``quantity``, or ``quantity / 100`` for percentage
    positions). Rows follow portfolio position order, columns follow
    ``factors``, and every value is in ``portfolio.base_currency``, converted
    with spot FX on each bumped market.

    Parameters
    ----------
    portfolio : Portfolio | str
        Built ``Portfolio`` or ``PortfolioSpec`` JSON. Its positions are the
        rows and its ``base_currency`` is the reporting currency.
    factors : str | list | pandas.DataFrame
        ``FactorDefinition`` array as JSON text, a list of dicts or a records
        ``DataFrame``. Every factor must map to market data; credit hierarchy
        factors (``Credit`` with an empty ``curve_ids`` list) are rejected, use
        :meth:`FactorModel.compute_sensitivities` for them.
    market : MarketContext | str
        Market to bump, including the FX needed for cross-currency positions.
    as_of : datetime.date | str
        Valuation date for pricing and spot FX.
    bump_config : str | dict, optional
        ``BumpSizeConfig`` as JSON text or a dict. Defaults to 1 bp / 1 % per
        factor type.

    Returns
    -------
    SensitivityMatrix
        Positions-by-factors delta matrix in ``portfolio.base_currency``.

    Raises
    ------
    TypeError
        If ``portfolio`` or ``market`` is neither a handle nor a JSON string.
    ValueError
        If the portfolio spec, factors, bump configuration or date is malformed,
        a factor is a credit hierarchy factor, or a factor cannot be bumped or
        priced.
    KeyError
        If repricing requires market data or FX that is absent from ``market``.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, compute_factor_sensitivities
    >>> book = Portfolio.builder("book", "USD", "2025-01-15").build()
    >>> matrix = compute_factor_sensitivities(book, "[]", MarketContext(), "2025-01-15")
    >>> matrix.to_pandas().shape
    (0, 0)
    """
    ...

def compute_pnl_profiles(
    portfolio: Portfolio | str,
    factors: list[FactorDefinition] | str | list[dict[str, Any]] | pd.DataFrame,
    market: MarketContext | str,
    as_of: datetime.date | str,
    bump_config: str | dict[str, Any] | None = None,
    n_points: int | None = None,
) -> list[FactorPnlProfile]:
    """
    Compute full-repricing scenario P&L profiles of a portfolio, one per factor.

    Each factor is shifted across a symmetric grid of multiples of its bump
    size. A profile row holds every position's scale-factor-weighted P&L in
    ``portfolio.base_currency``.

    Parameters
    ----------
    portfolio : Portfolio | str
        Built ``Portfolio`` or ``PortfolioSpec`` JSON. Its positions are the
        profile columns and its ``base_currency`` is the reporting currency.
    factors : str | list | pandas.DataFrame
        ``FactorDefinition`` array, in returned profile order (same accepted
        forms and credit-hierarchy restriction as
        :func:`compute_factor_sensitivities`).
    market : MarketContext | str
        Market reused by every scenario.
    as_of : datetime.date | str
        Valuation date for pricing and spot FX.
    bump_config : str | dict, optional
        ``BumpSizeConfig`` as JSON text or a dict.
    n_points : int, optional
        Odd number of scenario grid points in ``3..=1001``; ``None`` selects 5
        (shifts ``[-2, -1, 0, 1, 2]``).

    Returns
    -------
    list[FactorPnlProfile]
        One profile per factor, each containing scenario P&L for every position.

    Raises
    ------
    TypeError
        If ``portfolio`` or ``market`` is neither a handle nor a JSON string.
    ValueError
        If the portfolio spec, factors, bump configuration or date is malformed;
        a factor is a credit hierarchy factor; ``n_points`` is less than ``3``,
        greater than ``1001`` or even; or a factor cannot be bumped or priced.
    KeyError
        If repricing requires market data or FX that is absent from ``market``.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, compute_pnl_profiles
    >>> book = Portfolio.builder("book", "USD", "2025-01-15").build()
    >>> compute_pnl_profiles(book, "[]", MarketContext(), "2025-01-15")
    []
    """
    ...
