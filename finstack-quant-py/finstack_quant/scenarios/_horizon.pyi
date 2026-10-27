"""Stub declarations mirroring ``finstack-quant-py/src/bindings/scenarios/horizon.rs``."""

from __future__ import annotations

import datetime
import pandas as pd
from collections.abc import Mapping
from typing import Any
from finstack_quant.attribution import PnlAttribution
from finstack_quant.core.config import FinstackConfig
from finstack_quant.core.market_data import MarketContext
from finstack_quant.scenarios._engine import ApplicationReport
from finstack_quant.scenarios._spec import ScenarioSpec

class HorizonResult:
    """
    Horizon total return result with full P&L attribution.

    Produced by :func:`compute_horizon_return`. Access factor-level
    contributions via :meth:`factor_contribution` and the full breakdown
    via :attr:`attribution`.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.scenarios import compose_scenarios, compute_horizon_return
    >>> try:
    ...     compute_horizon_return("{}", MarketContext(), "2025-01-15", compose_scenarios([]))
    ... except ValueError as exc:
    ...     print(str(exc).split(":")[0])
    Validation error
    """

    @property
    def attribution(self) -> PnlAttribution:
        """
        Full P&L attribution breakdown.

        Returns
        -------
        PnlAttribution
            Carry, rate, credit, inflation, FX, volatility, and model-parameter
            contributions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def initial_value(self) -> float:
        """
        Initial instrument value.

        Returns
        -------
        float
            Present value at the original valuation date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def terminal_value(self) -> float:
        """
        Final instrument value after the scenario is applied.

        Returns
        -------
        float
            Present value after scenario shocks and time roll, as a bare
            amount in ``currency``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        ISO-4217 currency of ``initial_value`` and ``terminal_value``.

        Returns
        -------
        str
            Currency code, e.g. ``"USD"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def horizon_days(self) -> int | None:
        """
        Horizon in calendar days (``None`` if no time-roll).

        Returns
        -------
        int or None
            Number of days rolled forward, or ``None`` when the scenario
            contains no ``time_roll_forward`` operation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_return(self) -> float:
        """
        Total return as a decimal fraction (``0.05`` = +5%).

        Returns
        -------
        float
            ``total_pnl / initial_value``; ``nan`` when the initial value and
            total P&L are in different currencies (no implicit FX) or the
            initial value is zero or negative (no positive capital base).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def annualized_return(self) -> float | None:
        """
        Annualized return (``None`` if no time-roll).

        Returns
        -------
        float or None
            ``(1 + total_return) ** (365 / horizon_days) - 1`` as a decimal
            fraction; ``None`` when the horizon is absent or nonpositive,
            total return is nonfinite or below -100%, or the compounded value
            is nonfinite. Exactly -100% remains a total loss.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scenario_report(self) -> ApplicationReport:
        """
        Report from applying the scenario to the market copy.

        Returns
        -------
        ApplicationReport
            Operation counters, change manifest, structured warnings and the
            time-roll report (``horizon_days`` comes from it).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def warnings(self) -> list[dict[str, Any]]:
        """
        Structured warnings emitted during scenario application.

        Returns
        -------
        list[dict[str, Any]]
            Same as ``scenario_report.warnings``: dicts with a ``kind``
            discriminator plus variant-specific fields.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def factor_contribution(self, factor: str) -> float:
        """
        Factor contribution as decimal fraction of initial value.

        Parameters
        ----------
        factor : str
            Canonical ``AttributionFactor`` serde name: one of ``"carry"``,
            ``"rates_curves"``, ``"credit_curves"``, ``"inflation_curves"``,
            ``"correlations"``, ``"fx"``, ``"volatility"``,
            ``"market_scalars"``, or ``"model_parameters"``. Historical
            Python-only aliases (``"rates"``, ``"credit"``, ``"vol"``, ...)
            are no longer accepted.

        Returns
        -------
        float
            Contribution of the given factor as a decimal fraction. Returns ``nan``
            for a zero or negative initial value, or mismatched currencies.

        Raises
        ------
        ValueError
            If ``factor`` is not a canonical factor name.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the canonical horizon result and Rust-computed returns to JSON.

        Returns
        -------
        str
            JSON-serialized ``HorizonResult`` with currency-tagged values,
            attribution, scenario report and a ``summary`` containing decimal
            ``total_return``, ``annualized_return`` and ``factor_contributions`` keyed
            by canonical attribution factor names. Undefined derived values are
            JSON ``null``, matching the WASM result shape.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> HorizonResult:
        """
        Deserialize canonical horizon JSON and recompute derived return fields.

        Parameters
        ----------
        json : str
            JSON-serialized ``HorizonResult`` produced by :meth:`to_json` or the
            WASM horizon API. Serialized derived returns and contributions are
            not trusted as inputs; Rust recomputes them from the underlying values.

        Returns
        -------
        HorizonResult
            The deserialized result.

        Raises
        ------
        ValueError
            If *json* does not match the ``HorizonResult`` schema.

        Examples
        --------
        >>> from finstack_quant.scenarios import HorizonResult
        >>> try:
        ...     HorizonResult.from_json("{}")
        ... except ValueError as exc:
        ...     print(type(exc).__name__)
        ValueError
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the horizon summary as a single-row pandas ``DataFrame``.

        Columns: ``initial_value``, ``terminal_value``, ``currency``,
        ``total_pnl``, ``total_return``, ``annualized_return``,
        ``horizon_days``, ``user_operations``, ``expanded_operations``,
        ``operations_applied``, ``warning_count``.

        ``total_return`` and ``annualized_return`` are decimal fractions
        (``0.05`` = +5%). For the factor-level breakdown use
        ``result.attribution.to_dataframe()``.

        Returns
        -------
        pandas.DataFrame
            One-row summary frame.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def explain(self) -> str:
        """
        Human-readable summary of horizon return and attribution.

        Returns
        -------
        str
            Multi-line text (total and annualized return, horizon, values,
            and the carry / rates / credit / residual legs) rendered by the
            Rust ``Display`` implementation.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def _repr_html_(self) -> str | None:
        """
        Render as an HTML table in Jupyter notebooks.

        Returns
        -------
        str or None
            HTML for the frame from :meth:`to_dataframe`, or ``None`` when the
            frame cannot be built (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise.
        """
        ...

def compute_horizon_return(
    instrument: Any,
    market: MarketContext | str,
    as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    scenario: ScenarioSpec | str,
    method: str | Mapping[str, Any] | None = None,
    config: FinstackConfig | str | None = None,
    calendar_id: str | None = None,
) -> HorizonResult:
    """
    Compute horizon total return under a scenario.

    Parameters
    ----------
    instrument : Instrument | str
        Typed instrument (``Bond``, ``CreditDefaultSwap``,
        ``InterestRateSwap``, ...) or a canonical v1 instrument envelope JSON
        string.
    market : MarketContext | str
        ``MarketContext`` object or JSON string; never mutated.
    as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
        Valuation date (ISO 8601 accepted).
    scenario : ScenarioSpec | str
        Typed scenario or JSON-serialized ``ScenarioSpec``.
    method : str | dict | None, default None
        ``AttributionMethod`` wire value: ``"parallel"``,
        ``"metrics_based"``, ``{"waterfall": [...factors]}`` (for example
        ``{"waterfall": default_waterfall_order()}``) or
        ``{"taylor": {...config}}`` (``{"taylor": {}}`` for the defaults), or
        the same value as JSON text. ``None`` selects the Rust default
        (``AttributionMethod::default()``, currently ``"parallel"``).
        ``"metrics_based"`` calculates the instrument's applicable subset of
        the default attribution metrics (DV01, CS01, vega, ...) at the opening
        snapshot, using the same configuration and recalibration provider as
        the scenario engine. The canonical Rust registry omits metrics
        unsupported by that instrument type. Selected metric failures raise
        ``ValueError`` for invalid inputs, ``KeyError`` for missing data, or
        ``RuntimeError`` for computation failures.
    config : FinstackConfig | str | None, default None
        Library configuration threaded into both the scenario engine and the
        attribution pricing; ``None`` uses the default.
    calendar_id : str, optional
        Holiday calendar used to business-day adjust ``time_roll_forward``
        targets under ``TimeRollMode.business_days`` (e.g. ``"nyse"``,
        ``"target"``). Defaults to a weekends-only calendar, so business-day
        rolls always avoid weekends but not market holidays. Raises
        ``KeyError`` if the identifier is not a built-in calendar.

    Returns
    -------
    HorizonResult
        Decomposed total return and factor attribution, with the scenario
        ``ApplicationReport`` as ``scenario_report``.

    Raises
    ------
    ValueError
        If an input fails to parse or validate, ``method`` is unknown,
        or the scenario contains an instrument-scoped operation (horizon
        analysis prices one instrument instance at both dates).
    KeyError
        If ``calendar_id`` is not a built-in calendar, or the scenario references
        market data or tenors that do not exist.
    RuntimeError
        If pricing or attribution fails.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.scenarios import compose_scenarios, compute_horizon_return
    >>> try:
    ...     compute_horizon_return("{}", MarketContext(), "2025-01-15", compose_scenarios([]))
    ... except ValueError as exc:
    ...     print(str(exc).split(":")[0])
    Validation error
    """
    ...
