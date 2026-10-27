"""Stub declarations mirroring ``finstack-quant-py/src/bindings/attribution/return_contribution.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd

# Return Contribution

class ReturnContributionResult:
    """
    Return-contribution attribution result.

    Returned by :func:`attribute_return_contribution`. Decomposes a portfolio
    return into per-instrument, per-group, and per-factor contributions, with
    an optional Brinson-Fachler benchmark-relative block.

    Examples
    --------
    >>> from finstack_quant.attribution import ReturnContributionResult
    >>> try:
    ...     ReturnContributionResult.from_json("{}")
    ... except ValueError as exc:
    ...     "missing field" in str(exc)
    True
    """

    @property
    def portfolio_return(self) -> float:
        """
        Total portfolio return, equal to the summed instrument contributions.

        Returns
        -------
        float
            Period return as a decimal fraction (``0.01`` is 1%). Equal to the
            sum of the ``contribution`` field across
            :attr:`instrument_contribution`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def instrument_contribution(self) -> list[dict[str, Any]]:
        """
        Per-instrument contribution rows.

        Returns
        -------
        list[dict[str, Any]]
            One record per instrument with ``id``, ``weight``, ``return``,
            ``contribution``, and ``active_contribution`` keys.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def group_contribution(self) -> dict[str, Any]:
        """
        Contributions keyed by group dimension.

        Returns
        -------
        dict[str, Any]
            Maps each group dimension name (for example ``"sector"``) to its
            list of bucket records, each with a ``key`` bucket label and the
            summed ``contribution`` of the instruments in that bucket.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def factor_contribution(self) -> list[dict[str, Any]]:
        """
        Factor contribution rows.

        Returns
        -------
        list[dict[str, Any]]

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def specific_return(self) -> float | None:
        """
        Idiosyncratic residual when factor rows were supplied.

        Returns
        -------
        float or None
            ``portfolio_return - sum(factor contributions)``; ``None`` when
            the spec carried no factors.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def benchmark_relative(self) -> dict[str, Any] | None:
        """
        Brinson-Fachler benchmark-relative block.

        Returns
        -------
        dict[str, Any] or None
            ``None`` unless benchmark inputs were supplied on the spec.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def warnings(self) -> list[str]:
        """
        Non-fatal diagnostic warnings from the contribution run.

        Returns
        -------
        list[str]
            For example leveraged weights from a near-flat net-market-value
            book.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-instrument contributions as a pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row per instrument with ``id``, ``weight``, ``return``,
            ``contribution``, and ``active_contribution`` (``NaN`` when no
            benchmark was supplied) columns; the schema columns are present
            even for an empty result.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_group_dataframe(self) -> pd.DataFrame:
        """
        Group-bucket contributions as a long pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Columns ``dimension`` (the ``group:<dimension>`` label name),
            ``key`` (bucket) and ``contribution``; empty with schema columns
            when the spec carried no group labels.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_factor_dataframe(self) -> pd.DataFrame:
        """
        Factor contributions as a pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Columns ``factor``, ``exposure``, ``factor_return``,
            ``contribution``; empty with schema columns when no factor rows
            were supplied.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, rendered from :meth:`to_dataframe`.

        Returns
        -------
        str or None
            pandas HTML markup, or ``None`` when the frame cannot be built
            (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise; failures degrade to ``None``.
        """
        ...

    def to_series(self) -> pd.Series:
        """
        Per-instrument contributions as a pandas Series indexed by instrument id.

        Returns
        -------
        pd.Series
            Named ``contribution``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a compact JSON string.

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
    def from_json(json: str) -> ReturnContributionResult:
        """
        Deserialize from a JSON string.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        ReturnContributionResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the result schema.

        Examples
        --------
        >>> from finstack_quant.attribution import ReturnContributionResult
        >>> try:
        ...     ReturnContributionResult.from_json("{}")
        ... except ValueError as exc:
        ...     "missing field" in str(exc)
        True
        """
        ...
