"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/scenarios.rs``."""

from __future__ import annotations

import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.scenarios import ApplicationReport, ScenarioSpec
from finstack_quant.portfolio._container import Portfolio
from finstack_quant.portfolio._valuation import PortfolioValuation

class ScenarioPnl:
    """
    Scenario-attributable profit and loss, in the portfolio base currency.

    Held as ``pnl`` by the :class:`ScenarioPnlBatchItem` that
    :func:`scenario_pnl` and :func:`scenario_pnl_batch` return, beside the
    scenario :class:`~finstack_quant.scenarios.ApplicationReport`. Positions added or
    removed by the scenario are zero-filled against the missing side, so
    :attr:`by_position` always sums to :attr:`total`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import ScenarioPnl
    >>> doc = {"total": {"amount": "0", "currency": "USD"}, "by_position": {}}
    >>> pnl = ScenarioPnl.from_json(json.dumps(doc))
    >>> (pnl.total, pnl.currency, pnl.by_position)
    (0.0, 'USD', {})
    """

    @property
    def total(self) -> float:
        """
        Total scenario P&L in the portfolio base currency.

        Returns
        -------
        float
            Sum of :attr:`by_position`, quoted in :attr:`currency`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        Base currency the P&L is denominated in.

        Returns
        -------
        str
            ISO 4217 code of the portfolio base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_position(self) -> dict[str, float]:
        """
        Per-position scenario P&L, keyed by position id.

        Returns
        -------
        dict[str, float]
            Base-currency amounts in the canonical Rust ``IndexMap``
            iteration order (stressed positions first, then base-only
            positions), matching :meth:`to_dataframe`, :meth:`to_series`,
            and :meth:`to_json`. Sums to :attr:`total`.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-position P&L ladder as a pandas DataFrame.

        Columns: ``position_id``, ``pnl``. The schema is pinned, so an empty
        ladder keeps the same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per position, with float ``pnl`` in :attr:`currency`.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_series(self) -> pd.Series:
        """
        Per-position P&L as a pandas Series indexed by position id.

        Returns
        -------
        pd.Series
            Float base-currency amounts named ``pnl``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this P&L ladder to canonical JSON.

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
    def from_json(json: str) -> ScenarioPnl:
        """
        Deserialize a ``ScenarioPnl`` from JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json`, or the ``pnl``
            member of a :func:`scenario_pnl_batch` entry.

        Returns
        -------
        ScenarioPnl
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``ScenarioPnl`` schema.

        Examples
        --------
        >>> import json as _json
        >>> from finstack_quant.portfolio import ScenarioPnl
        >>> doc = {"total": {"amount": "0", "currency": "USD"}, "by_position": {}}
        >>> ScenarioPnl.from_json(_json.dumps(doc)).total
        0.0
        """
        ...

class ScenarioPnlBatchItem:
    """
    One ordered result from :func:`scenario_pnl_batch`.

    Carries the scenario identifier, its typed :class:`ScenarioPnl` ladder,
    and the scenario :class:`~finstack_quant.scenarios.ApplicationReport`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import ScenarioPnlBatchItem
    >>> doc = {
    ...     "scenario_id": "up_10bp",
    ...     "pnl": {"total": {"amount": "0", "currency": "USD"}, "by_position": {}},
    ...     "report": {
    ...         "operations_applied": 0,
    ...         "user_operations": 0,
    ...         "expanded_operations": 0,
    ...         "changes": {
    ...             "market_targets": [],
    ...             "changed_instrument_indices": [],
    ...             "as_of_changed": False,
    ...             "portfolio_shape_changed": False,
    ...             "all_dirty": False,
    ...         },
    ...         "warnings": [],
    ...     },
    ... }
    >>> ScenarioPnlBatchItem.from_json(json.dumps(doc)).scenario_id
    'up_10bp'
    """

    @property
    def scenario_id(self) -> str:
        """
        Identifier copied from the input scenario.

        Returns
        -------
        str
            The stored label from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def pnl(self) -> ScenarioPnl:
        """
        Scenario-attributable portfolio P&L ladder.

        Returns
        -------
        ScenarioPnl
            Typed P&L ladder in the portfolio base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def report(self) -> ApplicationReport:
        """
        Application provenance and warnings for this scenario.

        Returns
        -------
        ApplicationReport
            Scenario application report with counts and warnings.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-position P&L ladder as a pandas DataFrame.

        Columns: ``scenario_id``, ``position_id``, ``pnl``. The schema is pinned, so an empty result keeps the
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
    def from_json(json: str) -> ScenarioPnlBatchItem:
        """
        Deserialize a ``ScenarioPnlBatchItem`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        ScenarioPnlBatchItem
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``ScenarioPnlBatchItem`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import ScenarioPnlBatchItem
        >>> doc = (
        ...     '{"scenario_id": "up", "pnl": {"total": {"amount": "0",'
        ...     ' "currency": "USD"}, "by_position": {}}, "report":'
        ...     ' {"operations_applied": 0, "user_operations": 0,'
        ...     ' "expanded_operations": 0, "changes": {"market_targets": [],'
        ...     ' "changed_instrument_indices": [], "as_of_changed": false,'
        ...     ' "portfolio_shape_changed": false, "all_dirty": false},'
        ...     ' "warnings": []}}'
        ... )
        >>> ScenarioPnlBatchItem.from_json(doc).scenario_id
        'up'
        """
        ...

def apply_scenario_and_revalue(
    portfolio: Portfolio | str,
    scenario: ScenarioSpec | str,
    market: MarketContext | str,
) -> tuple[PortfolioValuation, ApplicationReport]:
    """
    Apply a scenario and revalue the portfolio.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON to revalue.
    scenario : ScenarioSpec | str
        Typed :class:`~finstack_quant.scenarios.ScenarioSpec` or its canonical
        JSON string describing market-data shocks.
    market : MarketContext or str
        Base market context object or JSON to shock before revaluation.

    Returns
    -------
    tuple[PortfolioValuation, ApplicationReport]
        The revalued portfolio and the scenario application report. Call
        ``.to_json()`` on either for its wire form.

    Raises
    ------
    TypeError
        If ``portfolio`` or ``market`` is neither its typed wrapper nor a JSON string.
    ValueError
        If portfolio, scenario, or market JSON is malformed or schema-incompatible.
    PortfolioError
        If portfolio construction, scenario application, market lookup, FX
        conversion, or valuation fails.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, apply_scenario_and_revalue
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> scenario = '{"id":"s","name":"S","operations":[]}'
    >>> valuation, report = apply_scenario_and_revalue(Portfolio.from_spec(spec), scenario, MarketContext())
    >>> (valuation.total_value, report.operations_applied)
    (0.0, 0)
    """
    ...

def scenario_pnl(
    portfolio: Portfolio | str,
    scenario: ScenarioSpec | str,
    market: MarketContext | str,
) -> ScenarioPnlBatchItem:
    """
    Compute the profit and loss attributable to a scenario.

    Values the portfolio against the unshocked market and against the
    scenario-shocked market, then reports the base-currency difference per
    position and in total. Positions added or removed by the scenario are
    zero-filled against the missing side, so ``by_position`` always sums to
    ``total``.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON valued on both the
        unshocked and shocked legs.
    scenario : ScenarioSpec | str
        Typed :class:`~finstack_quant.scenarios.ScenarioSpec` or its canonical
        JSON string describing the market-data shocks whose profit-and-loss
        impact is measured.
    market : MarketContext or str
        Unshocked market context object or JSON used as the base leg and as the
        source the scenario operations are applied to.

    Returns
    -------
    ScenarioPnlBatchItem
        The scenario's ``scenario_id``, its P&L ladder ``pnl``
        (:class:`ScenarioPnl`, base-currency ``total`` and ``by_position``
        amounts) and the scenario application ``report`` carrying which
        operations were applied; the same item type
        :func:`scenario_pnl_batch` returns per scenario. ``to_json()`` gives
        its wire form.

    Raises
    ------
    TypeError
        If ``portfolio`` or ``market`` is neither its typed wrapper nor a JSON string.
    ValueError
        If a JSON input is malformed or does not match its serialized schema.
    PortfolioError
        If scenario application or either valuation fails, including for
        missing market data or mismatched base and shocked currencies.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, scenario_pnl
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> item = scenario_pnl(Portfolio.from_spec(spec), '{"id":"s","name":"S","operations":[]}', MarketContext())
    >>> (item.scenario_id, item.pnl.total, item.pnl.currency, item.pnl.by_position)
    ('s', 0.0, 'USD', {})
    """
    ...

def scenario_pnl_batch(
    portfolio: Portfolio | str,
    scenarios: list[ScenarioSpec | str] | str,
    market: MarketContext | str,
) -> list[ScenarioPnlBatchItem]:
    """
    Compute ordered scenario P&L results while reusing one base valuation.

    The Rust portfolio engine values the unshocked portfolio once, applies
    each scenario independently, and returns results in the same order as the
    input JSON array. It is the batch counterpart to :func:`scenario_pnl`;
    use it when more than one scenario is evaluated against one portfolio and
    market snapshot.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON. The typed form
        avoids rebuilding the portfolio for the batch.
    scenarios : list[ScenarioSpec | str] | str
        Ordered scenarios as a sequence of typed ``ScenarioSpec`` objects or
        canonical JSON strings, or one canonical JSON array string. Order is
        preserved exactly; an empty batch returns an empty list without
        valuation.
    market : MarketContext or str
        Unshocked market context or canonical market JSON used for the shared
        base valuation and each scenario application.

    Returns
    -------
    list[ScenarioPnlBatchItem]
        One ordered item per input scenario, each carrying ``scenario_id``,
        a typed ``pnl`` (:class:`ScenarioPnl`) and ``report``
        (:class:`~finstack_quant.scenarios.ApplicationReport`); use
        serialize with ``to_json()`` for the wire string.

    Raises
    ------
    ValueError
        If ``scenarios`` is malformed or cannot deserialize to an ordered
        array of valid ``ScenarioSpec`` values.
    PortfolioError
        If scenario application, valuation, or base-currency P&L differencing
        fails. The reported error is for the earliest failing input scenario.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, scenario_pnl_batch
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> scenario_pnl_batch(Portfolio.from_spec(spec), "[]", MarketContext())
    []
    """
    ...
