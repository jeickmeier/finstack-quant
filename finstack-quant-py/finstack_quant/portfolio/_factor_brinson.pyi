"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/factor_brinson.rs``."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any
import numpy as np
import numpy.typing as npt
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable

class FactorBrinsonResult:
    """
    Jeet-Partani factor-Brinson unified attribution result.

    Returned by :func:`factor_brinson_attribution`.
    ``allocation + selection`` reconstructs :attr:`active_return`.

    Examples
    --------
    >>> from finstack_quant.portfolio import factor_brinson_attribution
    >>> inputs = {
    ...     "asset_ids": ["A"],
    ...     "asset_returns": [0.02],
    ...     "exposures": [1.0],
    ...     "factor_names": ["Market"],
    ...     "portfolio_weights": [1.0],
    ...     "benchmark_weights": [1.0],
    ... }
    >>> factor_brinson_attribution(inputs, [0.02]).active_return
    0.0
    """

    @property
    def portfolio_return(self) -> float:
        """
        Portfolio total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_return(self) -> float:
        """
        Benchmark total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def active_return(self) -> float:
        """
        Active return; portfolio minus benchmark total return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def allocation(self) -> float:
        """
        Factor (allocation) contribution to the active return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def selection(self) -> float:
        """
        Specific (selection) contribution to the active return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def factor_contributions(self) -> list[dict[str, object]]:
        """
        Per-factor breakdown of ``allocation`` as records.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def asset_contributions(self) -> list[dict[str, object]]:
        """
        Per-asset breakdown of ``selection`` as records.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Per-factor contributions as an :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``factor``, ``active_loading``, ``factor_return``, ``contribution``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.
        The primary table is the factor axis; the per-asset selection
        breakdown comes from :meth:`to_asset_contributions_table`.

        Returns
        -------
        ArrowTable
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into an Arrow table.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Per-factor contributions as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Per-factor contributions as a Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Per-factor contributions as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def to_asset_contributions_table(self) -> ArrowTable:
        """
        Per-asset specific contributions as an :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``asset``, ``specific_return``, ``active_weight``, ``contribution``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        ArrowTable
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into an Arrow table.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FactorBrinsonResult:
        """
        Deserialize a ``FactorBrinsonResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        FactorBrinsonResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``FactorBrinsonResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import FactorBrinsonResult
        >>> doc = '{"portfolio_return": 0.0, "benchmark_return": 0.0, "active_return": 0.0, "allocation": 0.0, "selection": 0.0, "factor_contributions": [], "asset_contributions": []}'
        >>> FactorBrinsonResult.from_json(doc).allocation
        0.0
        """
        ...

def factor_brinson_attribution(
    input: dict[str, Any] | list[Any] | pd.DataFrame | pl.DataFrame | Any,
    factor_returns: Sequence[float] | npt.NDArray[np.float64] | pd.Series | pl.Series | Any,
) -> FactorBrinsonResult:
    """
    Compute Jeet-Partani (2023) factor-Brinson unified attribution.

    Binds Rust ``finstack_quant_portfolio::factor_brinson_attribution``:
    generalizes classical Brinson-Fachler allocation/selection to continuous
    factor exposures by replacing the sector partition with a
    factor-exposure matrix and a caller-supplied benchmark factor-return
    vector.

    Parameters
    ----------
    input : dict | list | pandas.DataFrame | polars.DataFrame | Any
        ``FactorBrinsonInput`` with ``asset_ids``, ``asset_returns``,
        ``exposures`` (row-major ``n_assets x n_factors``), ``factor_names``,
        ``portfolio_weights`` and ``benchmark_weights``. Each weight vector
        must sum to ``1.0`` within ``1e-6``; weights may be negative (short
        positions).
    factor_returns : Sequence[float] | numpy.ndarray | pandas.Series | polars.Series | Any
        Caller-supplied benchmark factor returns ``f_b``, length
        ``input.factor_names``. Typically fit with
        :func:`finstack_quant.analytics.constrained_least_squares` (using
        benchmark weights) so the completeness condition below holds.

    Returns
    -------
    FactorBrinsonResult
        Typed result with ``allocation``, ``selection``, and their
        per-factor / per-asset breakdowns; use
        serialize with ``to_json()`` for the wire string.

    Raises
    ------
    PortfolioError
        If any array has the wrong length relative to ``n_assets`` /
        ``n_factors``, either ``n_assets`` or ``n_factors`` is zero, any
        input value is non-finite, portfolio or benchmark weights don't sum
        to ``1.0`` within tolerance, or the Jeet-Partani completeness
        residual ``|h_b'eps_b|`` exceeds tolerance — meaning the supplied
        ``factor_returns`` do not fully explain the benchmark return. The
        error directs callers to
        :func:`finstack_quant.analytics.constrained_least_squares`.
    ValueError
        If ``input`` is malformed or carries unknown fields.

    Sources
    -------
    See ``docs/REFERENCES.md#jeet-partani-2023``.

    Examples
    --------
    >>> from finstack_quant.portfolio import factor_brinson_attribution
    >>> inputs = {
    ...     "asset_ids": ["A"],
    ...     "asset_returns": [0.02],
    ...     "exposures": [1.0],
    ...     "factor_names": ["Market"],
    ...     "portfolio_weights": [1.0],
    ...     "benchmark_weights": [1.0],
    ... }
    >>> factor_brinson_attribution(inputs, [0.02]).active_return
    0.0
    """
    ...
