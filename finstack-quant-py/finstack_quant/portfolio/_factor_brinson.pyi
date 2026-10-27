"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/factor_brinson.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd

class FactorBrinsonResult:
    """
    Jeet-Partani factor-Brinson unified attribution result.

    Returned by :func:`factor_brinson_attribution`.
    ``allocation + selection`` reconstructs :attr:`active_return`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import factor_brinson_attribution
    >>> inputs = {
    ...     "asset_ids": ["A"],
    ...     "asset_returns": [0.02],
    ...     "exposures": [1.0],
    ...     "factor_names": ["Market"],
    ...     "portfolio_weights": [1.0],
    ...     "benchmark_weights": [1.0],
    ... }
    >>> factor_brinson_attribution(json.dumps(inputs), [0.02]).active_return
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

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-factor contributions as a pandas DataFrame.

        Columns: ``factor``, ``active_loading``, ``factor_return``, ``contribution``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.
        The primary frame is the factor axis; the per-asset selection
        breakdown comes from :meth:`to_asset_contributions_dataframe`.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_asset_contributions_dataframe(self) -> pd.DataFrame:
        """
        Per-asset specific contributions as a pandas DataFrame.

        Columns: ``asset``, ``specific_return``, ``active_weight``, ``contribution``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
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
    input_json: str | dict[str, Any] | list[Any] | pd.DataFrame, factor_returns: list[float]
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
    input_json : str | dict | list | pandas.DataFrame
        JSON ``FactorBrinsonInput`` with ``asset_ids``, ``asset_returns``,
        ``exposures`` (row-major ``n_assets x n_factors``), ``factor_names``,
        ``portfolio_weights`` and ``benchmark_weights``. Each weight vector
        must sum to ``1.0`` within ``1e-6``; weights may be negative (short
        positions).
    factor_returns : list[float]
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
        If ``input_json`` is malformed or carries unknown fields.

    Sources
    -------
    See ``docs/REFERENCES.md#jeet-partani-2023``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import factor_brinson_attribution
    >>> inputs = {
    ...     "asset_ids": ["A"],
    ...     "asset_returns": [0.02],
    ...     "exposures": [1.0],
    ...     "factor_names": ["Market"],
    ...     "portfolio_weights": [1.0],
    ...     "benchmark_weights": [1.0],
    ... }
    >>> factor_brinson_attribution(json.dumps(inputs), [0.02]).active_return
    0.0
    """
    ...
