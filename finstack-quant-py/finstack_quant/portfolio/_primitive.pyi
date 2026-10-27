"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/primitive.rs``."""

from __future__ import annotations

import pandas as pd
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.portfolio._container import Portfolio

class PortfolioPrimitiveExposureReport:
    """
    Portfolio primitive decomposition with path and net/gross concentration views.

    Produced by :func:`primitive_exposures`. Every value and additive risk
    amount is in :attr:`base_currency`; per-metric ``measures`` /
    ``net_measures`` / ``gross_measures`` are carried on the :meth:`to_json`
    wire form.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioPrimitiveExposureReport
    >>> report = PortfolioPrimitiveExposureReport.from_json('{"base_currency":"USD","paths":[],"aggregates":[]}')
    >>> (str(report.base_currency), report.path_count, report.aggregate_count)
    ('USD', 0, 0)
    """

    @classmethod
    def from_json(cls, json_str: str) -> PortfolioPrimitiveExposureReport:
        """
        Parse a report from its canonical JSON.

        Parameters
        ----------
        json_str : str
            Strict JSON produced by :meth:`to_json`.

        Returns
        -------
        PortfolioPrimitiveExposureReport
            Parsed report.

        Raises
        ------
        ValueError
            If the JSON is malformed or does not match the report schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioPrimitiveExposureReport
        >>> report = PortfolioPrimitiveExposureReport.from_json('{"base_currency":"EUR","paths":[],"aggregates":[]}')
        >>> str(report.base_currency)
        'EUR'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize paths and aggregates, including per-metric measures, to JSON.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def base_currency(self) -> Currency:
        """
        Portfolio reporting currency of every value and risk amount.

        Returns
        -------
        Currency
            The portfolio base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def path_count(self) -> int:
        """
        Number of position-aware primitive paths before overlap netting.

        Returns
        -------
        int
            Count of position-aware primitive paths, one per direct position
            and one per composite leg reached.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def aggregate_count(self) -> int:
        """
        Number of net/gross aggregates (one per primitive instrument id).

        Returns
        -------
        int
            Count of distinct primitive instrument ids across all positions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the net/gross aggregates as a pandas DataFrame.

        Columns: ``instrument_id``, ``instrument_type``, ``net_quantity``,
        ``gross_quantity``, ``net_value``, ``gross_value``, ``currency``.

        Returns
        -------
        pd.DataFrame
            One row per primitive instrument id, ordered by id.

        Raises
        ------
        ImportError
            If pandas is not installed.
        """
        ...

    def to_paths_dataframe(self) -> pd.DataFrame:
        """
        Export the position-aware primitive paths as a pandas DataFrame.

        Columns: ``position_id``, ``path`` (composite and leg ids joined with
        ``/``), ``instrument_id``, ``instrument_type``, ``quantity``,
        ``value``, ``currency``.

        Returns
        -------
        pd.DataFrame
            One row per primitive path.

        Raises
        ------
        ImportError
            If pandas is not installed.
        """
        ...

def primitive_exposures(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    metrics: list[str],
) -> PortfolioPrimitiveExposureReport:
    """
    Decompose every portfolio position into primitive economic exposures.

    Direct instruments yield one primitive path; composite positions recurse
    through their frozen resolved leg quantities. The position quantity scales
    every primitive quantity, value and additive risk amount, and net and
    gross exposure is aggregated per primitive instrument id across positions.

    Parameters
    ----------
    portfolio : Portfolio | str
        Built portfolio or ``PortfolioSpec`` JSON; ``base_currency`` is the
        reporting currency.
    market : MarketContext | str
        Complete market for primitive valuation and FX conversion.
    metrics : list[str]
        Additive metric ids to report per primitive (e.g. ``"dv01"``); pass
        ``[]`` for quantity and value only.

    Returns
    -------
    PortfolioPrimitiveExposureReport
        Path-level and net/gross primitive exposures in the base currency.

    Raises
    ------
    ValueError
        If a metric id is not canonical.
    PortfolioError
        If a metric is non-additive, primitive instruments sharing an id have
        conflicting definitions, or a composite is invalid.
    KeyError
        If market data or FX needed for valuation is missing.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, primitive_exposures
    >>> equity = {
    ...     "type": "equity",
    ...     "spec": {
    ...         "id": "ACME",
    ...         "ticker": "ACME",
    ...         "currency": "USD",
    ...         "quantity": 1.0,
    ...         "quoted_spot": 100.0,
    ...         "spot_id": None,
    ...         "div_yield_id": None,
    ...         "discrete_dividends": [],
    ...         "discount_curve_id": "USD",
    ...         "attributes": {},
    ...     },
    ... }
    >>> book = Portfolio.from_spec(
    ...     json.dumps({
    ...         "id": "BOOK",
    ...         "as_of": "2025-01-02",
    ...         "base_currency": "USD",
    ...         "entities": {"FUND": {"id": "FUND"}},
    ...         "positions": [
    ...             {
    ...                 "position_id": "P1",
    ...                 "entity_id": "FUND",
    ...                 "instrument_id": "ACME",
    ...                 "instrument_spec": equity,
    ...                 "quantity": -2.0,
    ...                 "unit": "units",
    ...             }
    ...         ],
    ...     })
    ... )
    >>> report = primitive_exposures(book, MarketContext(), [])
    >>> report.to_dataframe()[["instrument_id", "net_quantity", "gross_value"]].values.tolist()
    [['ACME', -2.0, 200.0]]
    """
    ...
