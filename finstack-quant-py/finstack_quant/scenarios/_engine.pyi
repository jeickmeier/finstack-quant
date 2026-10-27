"""Stub declarations mirroring ``finstack-quant-py/src/bindings/scenarios/engine.rs``."""

from __future__ import annotations

import datetime
import pandas as pd
from collections.abc import Sequence
from typing import Any
from finstack_quant.core.config import FinstackConfig
from finstack_quant.core.market_data import MarketContext
from finstack_quant.statements import FinancialModelSpec
from finstack_quant.scenarios._spec import ScenarioSpec

# Scenario application

class ApplicationReport:
    """
    Report describing what a scenario application changed.

    Exposed as the :attr:`ApplicationResult.report` attribute of the result
    returned by :func:`apply_scenario`,
    as the second element of the tuple returned by
    :func:`finstack_quant.portfolio.apply_scenario_and_revalue`, and as the
    ``report`` of the ``ScenarioPnlBatchItem`` returned by
    :func:`finstack_quant.portfolio.scenario_pnl`.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.scenarios import apply_scenario, compose_scenarios
    >>> report = apply_scenario(compose_scenarios([]), MarketContext(), "2025-01-15").report
    >>> (report.operations_applied, report.user_operations, report.warnings)
    (0, 0, [])
    """

    @property
    def operations_applied(self) -> int:
        """
        Number of effects successfully applied to the execution context.

        Returns
        -------
        int
            Low-level effect count. One user-level operation can produce zero,
            one, or many effects; inspect ``changes`` and ``warnings`` for
            coverage.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def user_operations(self) -> int:
        """
        Number of user-provided operations before hierarchy expansion.

        Returns
        -------
        int
            Count of operations as written in the ``ScenarioSpec``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expanded_operations(self) -> int:
        """
        Number of operations the engine attempted after hierarchy expansion.

        Returns
        -------
        int
            Count of post-expansion operations, which may exceed
            :attr:`user_operations` when an operation fans out over a
            hierarchy.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def warnings(self) -> list[dict[str, Any]]:
        """
        Non-fatal warnings raised while applying the scenario.

        Returns
        -------
        list[dict[str, Any]]
            Structured warnings in emission order. Each dict carries a
            ``kind`` discriminator (``"equity_not_found"``,
            ``"discount_curve_heuristic"``, ``"commodity_shock_outside_range"``,
            ...) plus variant-specific fields. Empty when the scenario applied
            cleanly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def warning_count(self) -> int:
        """
        Number of warnings raised while applying the scenario.

        Returns
        -------
        int
            ``len(warnings)``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def meta(self) -> dict[str, Any] | None:
        """
        Audit stamp: numeric mode, rounding context, and FX policy in force.

        Returns
        -------
        dict[str, Any] or None
            Policy stamp with ``numeric_mode``, ``rounding``,
            ``fx_policy_applied``, and ``version`` keys, or ``None`` when the
            engine recorded no stamp.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def changes(self) -> dict[str, Any]:
        """
        Metadata describing exactly which market state the effects changed.

        Returns
        -------
        dict[str, Any]
            Invalidation record with ``market_targets``,
            ``changed_instrument_indices``, ``as_of_changed``,
            ``portfolio_shape_changed``, and ``all_dirty`` keys, used
            downstream for precise cache invalidation.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def applied_shocks(self) -> list[dict[str, Any]]:
        """
        Size of every market and instrument shock applied, in application order.

        ``changes`` names which targets changed (deduplicated); this lists by
        how much, with one entry per accepted effect, so a target shocked
        several times appears several times.

        Returns
        -------
        list[dict[str, Any]]
            One dict per applied shock with these keys:

            - ``applies_to``: ``{"scope": "market", "target": {...}}`` with the
              same resolved target dict that ``changes["market_targets"]``
              carries, or ``{"scope": "instruments", "indices": [...]}`` with
              the zero-based inventory indices the shock changed.
            - ``shock``: a dict tagged by ``kind``. ``uniform`` (``value``,
              ``unit``) is one size for the whole target: a parallel curve or
              surface shift, or the shock to a price, an FX rate or
              instruments. ``key_rate`` (``time_years``, ``value``, ``unit``)
              is a triangular bump centred on one curve knot. ``nodes``
              (``nodes`` = list of ``{"tenor", "value"}``, ``unit``) lists the
              requested nodes of a curve rebuilt in one step. ``vol_bucket``
              (optional ``expiries_years`` and ``strikes``, ``value``,
              ``unit``) and ``detachment_bucket`` (``detachments_bp``,
              ``value``, ``unit``) are bucket shocks. ``unit`` is ``"bp"``
              (additive basis points, 1.0 = 0.0001), ``"percent"`` (relative,
              5.0 = +5%) or ``"absolute"`` (additive in the target's own quote
              units: volatility-index points or decimal correlation).
            - ``level_change``: ``{"before", "after"}`` stored levels around
              the shock. Present only for ``equity_price`` targets; FX, curve,
              surface and instrument shocks record their size only.

            Sizes are the shock as requested on the operation; delivery
            adjustments (interpolation, recalibration, clamping) are reported
            in ``warnings``. Statement-forecast operations, rate bindings and
            the time roll are not listed. Empty when no market or instrument
            shock was applied.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def time_roll(self) -> dict[str, Any] | None:
        """
        Roll-forward report, present only when the scenario contained a
        ``time_roll_forward`` operation.

        Returns
        -------
        dict[str, Any] or None
            Roll details, or ``None`` when the scenario performed no time roll.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the report counters as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row with columns ``operations_applied``, ``user_operations``,
            ``expanded_operations``, ``warning_count``, ``as_of_changed``,
            ``all_dirty`` and ``shock_count`` (number of ``applied_shocks``
            entries).

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.scenarios import ScenarioSpec, apply_scenario
        >>> frame = apply_scenario(ScenarioSpec("s", []), MarketContext(), "2025-01-15").report.to_dataframe()
        >>> list(frame.columns)[:4]
        ['operations_applied', 'user_operations', 'expanded_operations', 'warning_count']
        """
        ...

    def changes_to_dataframe(self) -> pd.DataFrame:
        """
        Export the market targets the scenario actually changed, one row each.

        Returns
        -------
        pd.DataFrame
            Columns ``kind`` (``curve``, ``volatility_index``,
            ``base_correlation``, ``vol_surface``, ``equity_price``, ``fx``),
            ``id`` (identifier, or ``BASE/QUOTE`` for FX) and ``curve_kind``
            (curve family for ``curve`` rows, else ``None``). Empty, with the
            same columns, when nothing changed.

        Raises
        ------
        ValueError
            If the manifest cannot be serialized into a pandas object.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.scenarios import ScenarioSpec, apply_scenario
        >>> frame = apply_scenario(ScenarioSpec("s", []), MarketContext(), "2025-01-15").report.changes_to_dataframe()
        >>> list(frame.columns)
        ['kind', 'id', 'curve_kind']
        """
        ...

    def carry_to_dataframe(self) -> pd.DataFrame:
        """
        Export per-instrument carry from the time roll, one row per
        instrument and currency.

        Returns
        -------
        pd.DataFrame
            Columns ``instrument_id``, ``amount`` (carry P&L as a float) and
            ``currency`` (ISO-4217 code). Empty when the scenario had no
            ``time_roll_forward`` or no instruments were supplied.

        Raises
        ------
        ValueError
            If the carry rows cannot be serialized into a pandas object.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.scenarios import ScenarioSpec, apply_scenario
        >>> frame = apply_scenario(ScenarioSpec("s", []), MarketContext(), "2025-01-15").report.carry_to_dataframe()
        >>> list(frame.columns)
        ['instrument_id', 'amount', 'currency']
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this report to canonical JSON.

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
    def from_json(json: str) -> ApplicationReport:
        """
        Deserialize an ``ApplicationReport`` from JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json`.

        Returns
        -------
        ApplicationReport
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``ApplicationReport`` schema.

        Examples
        --------
        >>> from finstack_quant.scenarios import ApplicationReport
        >>> try:
        ...     ApplicationReport.from_json("{}")
        ... except ValueError as exc:
        ...     "missing field" in str(exc)
        True
        """
        ...

class ApplicationResult:
    """
    Result of applying a scenario: the mutated market, the mutated model (when
    one was supplied), and the application report.

    Returned by :func:`apply_scenario`.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.scenarios import apply_scenario, compose_scenarios
    >>> applied = apply_scenario(compose_scenarios([]), MarketContext(), "2025-01-15")
    >>> (type(applied.market).__name__, applied.model, applied.report.operations_applied)
    ('MarketContext', None, 0)
    """

    @property
    def market(self) -> MarketContext:
        """
        The mutated market context.

        Returns
        -------
        MarketContext
            The market after every scenario effect was applied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model(self) -> FinancialModelSpec | None:
        """
        The mutated financial model, or ``None`` when no model was supplied.

        Returns
        -------
        FinancialModelSpec or None
            Always ``None`` for :func:`apply_scenario`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def instruments(self) -> list[str] | None:
        """Shocked copies of the supplied instrument inventory.

        Returns
        -------
        list[str] or None
            Canonical instrument-envelope JSON strings in input order, accepted
            by subsequent scenario calls and instrument ``from_json`` constructors.
            ``None`` means no inventory was supplied; an empty list stays empty.
            The original Python objects are never mutated.

        Raises
        ------
        ValueError
            If an instrument cannot be serialized to its canonical envelope.
        """
        ...

    @property
    def report(self) -> ApplicationReport:
        """
        What the scenario changed.

        Returns
        -------
        ApplicationReport
            Operation counters, warnings, invalidation metadata, and the
            policy stamp.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the application report counters as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Same columns as :meth:`ApplicationReport.to_dataframe`.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.scenarios import ScenarioSpec, apply_scenario
        >>> len(apply_scenario(ScenarioSpec("s", []), MarketContext(), "2025-01-15").to_dataframe())
        1
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Emits the canonical ``ApplicationEnvelope`` shape, with ``market`` and
        ``model`` as nested objects alongside the canonical ``report`` object.

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
    def from_json(json: str) -> ApplicationResult:
        """
        Deserialize an ``ApplicationResult`` from JSON.

        Parameters
        ----------
        json : str
            Canonical ``ApplicationEnvelope`` payload produced by
            :meth:`to_json`; the market, model, and report are rebuilt from it.

        Returns
        -------
        ApplicationResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``ApplicationEnvelope`` schema.

        Examples
        --------
        >>> from finstack_quant.scenarios import ApplicationResult
        >>> try:
        ...     ApplicationResult.from_json("{}")
        ... except ValueError as exc:
        ...     "missing field" in str(exc)
        True
        """
        ...

def apply_scenario(
    scenario: ScenarioSpec | str,
    market: MarketContext | str,
    as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    model: FinancialModelSpec | str | None = None,
    instruments: Sequence[Any] | None = None,
    config: FinstackConfig | str | None = None,
) -> ApplicationResult:
    """
    Apply a scenario to copied market data and an optional financial model.

    Parameters
    ----------
    scenario : ScenarioSpec | str
        Typed scenario or JSON-serialized ``ScenarioSpec``.
    market : MarketContext | str
        ``MarketContext`` object or JSON ``MarketContext`` string. Never
        mutated; the result carries a modified copy.
    model : FinancialModelSpec | str | None, default None
        Copied financial model or canonical JSON; omit for market-only scenarios.
    as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
        Valuation date (ISO 8601 accepted).
    instruments : Sequence[Instrument | str] | None, default None
        Typed instruments (``Bond``, ``CreditDefaultSwap``, ...) or canonical
        instrument-envelope JSON strings. Required when the scenario contains
        instrument-scoped operations; also used for carry under
        ``time_roll_forward``. Shocked copies are returned as canonical envelope JSON strings in
        ``ApplicationResult.instruments``.
    config : FinstackConfig | str | None, default None
        Library configuration (rounding policy stamped into ``report.meta``);
        ``None`` uses the library default.

    Returns
    -------
    ApplicationResult
        Typed result exposing :attr:`~ApplicationResult.market`,
        :attr:`~ApplicationResult.model` and :attr:`~ApplicationResult.report`.

    Notes
    -----
    No holiday calendar is supplied, so ``time_roll_forward`` in
    ``business_days`` mode adjusts against a weekends-only calendar. Quote
    replay operations use a fresh cached recalibration provider.

    Raises
    ------
    TypeError
        If scenario is neither a ScenarioSpec nor canonical JSON.
    ValueError
        If an input fails to parse or validate, or the scenario contains
        instrument-scoped operations and ``instruments`` is ``None`` (the Rust
        engine rejects it after validating the scenario).
    KeyError
        If the scenario references market data, statement nodes, tenors or
        instruments that do not exist.
    RuntimeError
        If the engine fails internally.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.scenarios import apply_scenario, compose_scenarios
    >>> model = (
    ...     '{"schema_version":1,"id":"m","periods":['
    ...     '{"id":"2025Q1","start":"2025-01-01","end":"2025-04-01",'
    ...     '"is_actual":false}],"nodes":{}}'
    ... )
    >>> applied = apply_scenario(compose_scenarios([]), MarketContext(), "2025-01-15", model)
    >>> applied.report.operations_applied
    0
    """
    ...
