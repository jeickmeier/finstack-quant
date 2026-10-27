"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/var.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations.instruments._pricing import MarketHistory

def calculate_var(
    instruments: list[Any],
    market: MarketContext | str,
    history: MarketHistory | dict[str, Any] | str,
    as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    config: dict[str, Any] | str | None = None,
    model: str = "default",
) -> VarResult:
    """
    Historical VaR and expected shortfall of a list of instruments.

    Mirrors Rust ``metrics::risk::calculate_var``: every
    instrument is repriced under every ``history`` scenario, the per-scenario
    P&Ls are summed across instruments, and VaR / ES are read off that single
    portfolio distribution (R type-7 linear-interpolated quantile). Unlike
    summing the per-instrument ``hvar`` metric, offsetting positions
    diversify. Quote-recalibrated shocks (credit spreads) use the same
    recalibration provider as :func:`price_instrument`.

    Parameters
    ----------
    instruments : list[Bond | InterestRateSwap | ... | str]
        Typed instruments or ``finstack_quant.instrument/1`` envelopes. An
        empty list returns zero VaR and ES.
    market : MarketContext | str
        Unshocked base market every scenario perturbs.
    history : MarketHistory | dict[str, Any] | str
        Historical risk-factor shifts; a non-empty portfolio needs at least
        one scenario.
    as_of : datetime.date | datetime.datetime | pd.Timestamp | str
        Valuation date for the base and every scenario revaluation.
    config : dict[str, Any] | str | None, default None
        Rust ``VarConfig``: ``confidence_level`` (decimal in ``(0, 1)``),
        ``method`` (``"full_revaluation"`` or ``"taylor_approximation"``) and
        ``reporting_currency`` (required for mixed-currency portfolios).
        ``None`` uses Rust ``VarConfig::default()``: 95%, full revaluation,
        natural currency.
    model : str, default "default"
        ``"default"`` (each instrument's canonical pricing path) or one model
        key from :func:`list_models` applied to every instrument.

    Returns
    -------
    VarResult
        VaR, expected shortfall and the sorted P&L distribution; losses are
        negative.

    Raises
    ------
    TypeError
        If an ``instruments`` entry is not a typed instrument, an envelope dict or a JSON string.
    ValueError
        If an instrument, ``config``, ``history`` or ``model`` is invalid, the
        confidence level is outside ``(0, 1)``, a non-empty portfolio has no
        scenarios, or a mixed-currency portfolio has no ``reporting_currency``.
    KeyError
        If a curve, surface or price an instrument needs is missing.
    RuntimeError
        If a scenario revaluation fails numerically.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import StubKind
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.core.types import Rate
    >>> from finstack_quant.valuations.instruments import Bond, MarketHistory, calculate_var
    >>> as_of = datetime.date(2024, 1, 2)
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
    >>> def bond(id):
    ...     return Bond.fixed(
    ...         id, Money(1e6, Currency("USD")), Rate(0.05), as_of, datetime.date(2029, 1, 2), StubKind.NONE, "USD-OIS"
    ...     )
    >>> shift = lambda s: [{"factor": {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0}, "shift": s}]
    >>> history = MarketHistory(
    ...     as_of,
    ...     2,
    ...     [{"date": "2023-12-29", "shifts": shift(0.0010)}, {"date": "2023-12-28", "shifts": shift(-0.0005)}],
    ... )
    >>> one = calculate_var([bond("A")], market, history, as_of)
    >>> two = calculate_var([bond("A"), bond("B")], market, history, as_of)
    >>> (two.num_scenarios, one.var < 0, abs(two.var - 2 * one.var) < 1e-6)
    (2, True, True)
    """
    ...

class VarResult:
    """
    Historical VaR / expected shortfall with its P&L distribution.

    Typed wrapper of the Rust ``VarResult`` returned by
    :func:`calculate_var`. VaR and ES follow the P&L sign: losses
    are negative, and ``expected_shortfall <= var``.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import VarResult
    >>> result = VarResult.from_json(
    ...     '{"var": -10.0, "expected_shortfall": -12.0, "pnl_distribution": [-12.0, 3.0],'
    ...     ' "num_scenarios": 2, "confidence_level": 0.95}'
    ... )
    >>> (result.var, list(result.to_dataframe()["pnl"]))
    (-10.0, [-12.0, 3.0])
    """

    @property
    def var(self) -> float:
        """
        Value-at-Risk at ``confidence_level``.

        Returns
        -------
        float
            Signed P&L at the ``1 - confidence_level`` quantile (negative for a loss, ``0.0`` for an all-gain distribution).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expected_shortfall(self) -> float:
        """
        Expected shortfall at ``confidence_level``.

        Returns
        -------
        float
            Mean signed P&L over the worst ``1 - confidence_level`` tail.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def pnl_distribution(self) -> list[float]:
        """
        Portfolio P&L per scenario.

        Returns
        -------
        list[float]
            Values sorted ascending (worst first).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def num_scenarios(self) -> int:
        """
        Number of scenarios in the distribution.

        Returns
        -------
        int
            The scenario count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def confidence_level(self) -> float:
        """
        Confidence level used.

        Returns
        -------
        float
            Decimal confidence (``0.95`` = 95%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        The P&L distribution as a one-column pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Column ``pnl`` (``float64``), one row per scenario, worst first.

        Raises
        ------
        ImportError
            If pandas is not installed.
        """
        ...
    @staticmethod
    def from_json(json: str) -> VarResult:
        """
        Deserialize a result from the JSON produced by :meth:`VarResult.to_json`.

        Parameters
        ----------
        json : str
            Serialized ``VarResult``.

        Returns
        -------
        VarResult
            The parsed result.

        Raises
        ------
        ValueError
            If ``json`` is malformed, misses a field or carries an unknown one.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import VarResult
        >>> text = (
        ...     '{"var": 0.0, "expected_shortfall": 0.0, "pnl_distribution": [],'
        ...     ' "num_scenarios": 0, "confidence_level": 0.99}'
        ... )
        >>> VarResult.from_json(text).confidence_level
        0.99
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to compact JSON (the WASM ``calculateVar`` object).

        Returns
        -------
        str
            The result JSON.

        Raises
        ------
        ValueError
            If the result cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...
