"""
Generic cross-asset composite instruments with immutable resolved quantities.

Specifications embed canonical instruments, resolve signed quantities only at
initialization or explicit rebalance, and expose recursive primitive value,
risk, concentration, execution, and dated total-return reports.

Examples
--------
>>> import datetime, json
>>> from finstack_quant.core.currency import Currency
>>> from finstack_quant.core.market_data import MarketContext
>>> from finstack_quant.core.money import Money
>>> from finstack_quant.valuations.composite import CompositeLegSpec, CompositeSpec, RebalanceRule, WeightingMethod
>>> def _equity(instrument_id: str, price: float) -> str:
...     return json.dumps({
...         "schema": "finstack_quant.instrument/1",
...         "instrument": {
...             "type": "equity",
...             "spec": {
...                 "id": instrument_id,
...                 "ticker": instrument_id,
...                 "currency": "USD",
...                 "shares": 1.0,
...                 "price_quote": price,
...                 "price_id": None,
...                 "div_yield_id": None,
...                 "discrete_dividends": [],
...                 "discount_curve_id": "USD",
...                 "attributes": {"tags": [], "meta": {}},
...             },
...         },
...     })
>>> _leg_a = CompositeLegSpec("A", _equity("A", 100.0), 1.0)
>>> _leg_b = CompositeLegSpec("B", _equity("B", 90.0), -1.0)
>>> _spec = CompositeSpec(
...     "A-B",
...     Currency("USD"),
...     Money(100.0, Currency("USD")),
...     [_leg_a, _leg_b],
...     WeightingMethod.fixed_quantity(),
...     RebalanceRule.manual(),
... )
>>> _resolved = _spec.initialize(MarketContext(), datetime.date(2025, 1, 1)).instrument
>>> _resolved.state.resolved_quantities
{'A': 1.0, 'B': -1.0}
"""

from __future__ import annotations

import datetime
from typing import Any

import pandas as pd

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money

DateLike = datetime.date | datetime.datetime | pd.Timestamp | str
Observations = list[dict[str, Any]] | str

__all__ = [
    "CompositeExposureReport",
    "CompositeHistoryEngine",
    "CompositeHistoryResult",
    "CompositeInstrument",
    "CompositeLegSpec",
    "CompositeRebalanceResult",
    "CompositeSpec",
    "CompositeState",
    "RebalanceRule",
    "WeightingMethod",
]

class CompositeLegSpec:
    """
    Self-contained signed leg referencing an embedded canonical instrument.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import TermLoan
    >>> _loan = TermLoan.example()
    >>> _leg = CompositeLegSpec(_loan.id, _loan, -2.0)
    >>> (_leg.instrument_id, _leg.weight)
    ('TERM-LOAN-USD-5Y', -2.0)
    """

    def __init__(self, instrument_id: str, instrument: Any | str, weight: float) -> None:
        """
        Construct one composite leg.

        Parameters
        ----------
        instrument_id : str
            Stable identifier that must equal the embedded instrument identifier.
        instrument : Any | str
            Typed instrument wrapper or canonical instrument-envelope JSON string.
        weight : float
            Finite non-zero signed quantity or relative weighting score.

        Raises
        ------
        ValueError
            If the envelope is malformed or contains an unsupported instrument.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CompositeLegSpec:
        """
        Deserialize a bare canonical leg object.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json` for exactly one leg.

        Returns
        -------
        CompositeLegSpec
            Parsed leg retaining the embedded instrument definition.

        Raises
        ------
        ValueError
            If the JSON is malformed or does not match the strict leg schema.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> _loan = TermLoan.example()
        >>> _leg = CompositeLegSpec(_loan.id, _loan, -2.0)
        >>> CompositeLegSpec.from_json(_leg.to_json()).weight
        -2.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this leg as a bare canonical JSON object.

        Returns
        -------
        str
            Strict ``CompositeLegSpec`` JSON including its embedded instrument.

        Raises
        ------
        ValueError
            If the canonical Rust value cannot be serialized.
        """
        ...

    @property
    def instrument_id(self) -> str:
        """
        Return the declared embedded-instrument identifier.

        Returns
        -------
        str
            Stable identifier that matches the embedded instrument.

        Notes
        -----
        This accessor does not raise; it returns the stored identifier.
        """
        ...

    @property
    def weight(self) -> float:
        """
        Return the signed fixed quantity or dynamic weighting score.

        Returns
        -------
        float
            Finite non-zero signed leg input.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    def instrument_dict(self) -> dict[str, Any]:
        """
        Return the embedded instrument as a plain ``dict`` (canonical envelope).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.instrument_json)``.

        Raises
        ------
        ValueError
            If canonical serialization fails.

        Examples
        --------
        >>> import datetime, json
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.composite import (
        ...     CompositeLegSpec,
        ...     CompositeSpec,
        ...     RebalanceRule,
        ...     WeightingMethod,
        ... )
        >>> def _equity(instrument_id, price):
        ...     return json.dumps({
        ...         "schema": "finstack_quant.instrument/1",
        ...         "instrument": {
        ...             "type": "equity",
        ...             "spec": {
        ...                 "id": instrument_id,
        ...                 "ticker": instrument_id,
        ...                 "currency": "USD",
        ...                 "shares": 1.0,
        ...                 "price_quote": price,
        ...                 "price_id": None,
        ...                 "div_yield_id": None,
        ...                 "discrete_dividends": [],
        ...                 "discount_curve_id": "USD",
        ...                 "attributes": {"tags": [], "meta": {}},
        ...             },
        ...         },
        ...     })
        >>> _leg_a = CompositeLegSpec("A", _equity("A", 100.0), 1.0)
        >>> _leg_a.instrument_dict()["instrument"]["type"]
        'equity'
        """
        ...

    @property
    def instrument_json(self) -> str:
        """
        Return the embedded instrument as a canonical v1 envelope.

        Returns
        -------
        str
            ``finstack_quant.instrument/1`` JSON for the embedded instrument.

        Raises
        ------
        ValueError
            If canonical JSON serialization fails.
        """
        ...

class WeightingMethod:
    """
    Serializable policy that resolves leg scores into signed quantities.

    Examples
    --------
    >>> WeightingMethod.fixed_quantity().to_json()
    '{"kind":"fixed_quantity"}'
    """

    @staticmethod
    def fixed_quantity() -> WeightingMethod:
        """
        Use signed leg weights directly as quantities without market data.

        Returns
        -------
        WeightingMethod
            Fixed-quantity policy.

        Examples
        --------
        >>> _fixed = WeightingMethod.fixed_quantity()

        Notes
        -----
        This factory does not raise; validation occurs when a specification is built.
        """
        ...

    @staticmethod
    def notional_weighted(gross_notional: Money) -> WeightingMethod:
        """
        Normalize absolute scores to a target gross reporting-currency notional.

        Parameters
        ----------
        gross_notional : Money
            Positive gross allocation denominated in the composite reporting currency.

        Returns
        -------
        WeightingMethod
            Gross-notional weighting policy preserving score signs.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> _notional = WeightingMethod.notional_weighted(Money(1_000_000.0, Currency("USD")))

        Notes
        -----
        This factory does not raise; currency and positivity are validated by ``CompositeSpec``.
        """
        ...

    @staticmethod
    def metric_weighted(
        metric: str, anchor_leg_id: str, anchor_quantity: float, neutralize: bool = False
    ) -> WeightingMethod:
        """
        Resolve quantities from unit metric contributions and an anchor scale.

        Parameters
        ----------
        metric : str
            Canonical unit metric identifier such as ``dv01`` or ``delta``.
        anchor_leg_id : str
            Existing leg whose signed quantity fixes overall scale.
        anchor_quantity : float
            Finite non-zero signed quantity assigned to the anchor leg.
        neutralize : bool
            Whether positive and negative score groups normalize separately.

        Returns
        -------
        WeightingMethod
            Anchored metric-weighting policy.

        Examples
        --------
        >>> _metric = WeightingMethod.metric_weighted("delta", "A", 1.0, True)

        Raises
        ------
        ValueError
            If the metric key uses noncanonical composite encoding. Anchors and
            quantities are validated by ``CompositeSpec``.
        """
        ...

    @staticmethod
    def dv01_neutral(anchor_leg_id: str, anchor_quantity: float) -> WeightingMethod:
        """
        Construct parallel-DV01-neutral weighting.

        Parameters
        ----------
        anchor_leg_id : str
            Existing rates leg that fixes quantity scale.
        anchor_quantity : float
            Signed non-zero quantity assigned to the anchor.

        Returns
        -------
        WeightingMethod
            Neutral metric policy using ``dv01``.

        Examples
        --------
        >>> _dv01 = WeightingMethod.dv01_neutral("TU", 1.0)

        Notes
        -----
        This factory does not raise; the anchor is validated by ``CompositeSpec``.
        """
        ...

    @staticmethod
    def delta_neutral(anchor_leg_id: str, anchor_quantity: float) -> WeightingMethod:
        """
        Construct delta-neutral weighting for cross-asset hedges.

        Parameters
        ----------
        anchor_leg_id : str
            Existing delta-bearing leg that fixes quantity scale.
        anchor_quantity : float
            Signed non-zero quantity assigned to the anchor.

        Returns
        -------
        WeightingMethod
            Neutral metric policy using ``delta``.

        Examples
        --------
        >>> _delta = WeightingMethod.delta_neutral("A", 1.0)

        Notes
        -----
        This factory does not raise; the anchor is validated by ``CompositeSpec``.
        """
        ...

    @staticmethod
    def duration_weighted(anchor_leg_id: str, anchor_quantity: float) -> WeightingMethod:
        """
        Construct modified-duration weighting without sign-group neutrality.

        Parameters
        ----------
        anchor_leg_id : str
            Existing duration-bearing leg that fixes quantity scale.
        anchor_quantity : float
            Signed non-zero quantity assigned to the anchor.

        Returns
        -------
        WeightingMethod
            Anchored policy using modified duration.

        Examples
        --------
        >>> _duration = WeightingMethod.duration_weighted("TY", 1.0)

        Notes
        -----
        This factory does not raise; the anchor is validated by ``CompositeSpec``.
        """
        ...

    @staticmethod
    def volatility_weighted(
        anchor_leg_id: str, anchor_quantity: float, lookback: int, min_observations: int, annualization_factor: float
    ) -> WeightingMethod:
        """
        Construct inverse annualized unit-P&L-volatility weighting.

        Parameters
        ----------
        anchor_leg_id : str
            Existing leg whose quantity fixes overall scale.
        anchor_quantity : float
            Signed non-zero quantity assigned to the anchor.
        lookback : int
            Maximum number of most-recent P&L observations used.
        min_observations : int
            Minimum finite P&L observations required for every leg.
        annualization_factor : float
            Positive periods-per-year multiplier, such as ``252`` for daily data.

        Returns
        -------
        WeightingMethod
            Inverse-volatility policy using one-unit total P&L.

        Examples
        --------
        >>> _vol = WeightingMethod.volatility_weighted("A", 1.0, 60, 20, 252.0)

        Notes
        -----
        This factory does not raise; window and anchor validation occurs in ``CompositeSpec``.
        """
        ...

    @staticmethod
    def from_json(json: str) -> WeightingMethod:
        """
        Deserialize any canonical weighting policy, including expressions.

        Parameters
        ----------
        json : str
            Strict weighting-method JSON using its ``kind`` discriminator.

        Returns
        -------
        WeightingMethod
            Parsed canonical weighting policy.

        Raises
        ------
        ValueError
            If JSON is malformed or carries an unknown field or variant.

        Examples
        --------
        >>> WeightingMethod.from_json('{"kind":"fixed_quantity"}').to_json()
        '{"kind":"fixed_quantity"}'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the canonical weighting policy.

        Returns
        -------
        str
            Strict tagged weighting-method JSON.

        Raises
        ------
        ValueError
            If serialization of the canonical Rust policy fails.
        """
        ...

class RebalanceRule:
    """
    Manual, explicit-date, or calendar-cadence rebalance rule.

    Examples
    --------
    >>> RebalanceRule.manual().to_json()
    '{"kind":"manual"}'
    """

    @staticmethod
    def manual() -> RebalanceRule:
        """
        Require callers to invoke rebalance explicitly.

        Returns
        -------
        RebalanceRule
            Manual rule with no scheduled dates.

        Examples
        --------
        >>> _manual = RebalanceRule.manual()

        Notes
        -----
        This factory does not raise; it returns a fixed manual policy.
        """
        ...

    @staticmethod
    def dates(dates: list[DateLike]) -> RebalanceRule:
        """
        Schedule rebalances on strictly increasing dates.

        Parameters
        ----------
        dates : list[datetime.date | datetime.datetime | pandas.Timestamp | str]
            Rebalance dates (date-like objects or ISO-8601 strings); duplicates
            and descending dates are rejected.

        Returns
        -------
        RebalanceRule
            Validated explicit-date schedule.

        Raises
        ------
        ValueError
            If a date is invalid or the sequence is not strictly increasing.

        Examples
        --------
        >>> import datetime
        >>> _dates = RebalanceRule.dates(["2025-01-31", datetime.date(2025, 2, 28)])
        """
        ...

    @staticmethod
    def calendar(
        start: DateLike,
        frequency: str,
        calendar_id: str,
        business_day_convention: str,
        end: DateLike | None = None,
    ) -> RebalanceRule:
        """
        Build a calendar-adjusted daily, weekly, monthly, or quarterly cadence.

        Parameters
        ----------
        start : datetime.date | datetime.datetime | pandas.Timestamp | str
            Unadjusted schedule start date (date-like or ISO-8601 string).
        frequency : str
            One of ``daily``, ``weekly``, ``monthly``, or ``quarterly``.
        calendar_id : str
            Registered calendar identifier such as ``weekends``.
        business_day_convention : str
            Canonical convention such as ``following`` or ``modified_following``.
        end : datetime.date | datetime.datetime | pandas.Timestamp | str | None
            Optional final date; omit for an open-ended cadence.

        Returns
        -------
        RebalanceRule
            Validated calendar-aware schedule.

        Raises
        ------
        ValueError
            If dates, enums, bounds, or the calendar identifier are invalid.

        Examples
        --------
        >>> _calendar = RebalanceRule.calendar("2025-01-01", "monthly", "weekends_only", "following", "2026-01-01")
        """
        ...

    @staticmethod
    def from_json(json: str) -> RebalanceRule:
        """
        Deserialize and validate a canonical rebalance rule.

        Parameters
        ----------
        json : str
            Strict tagged rebalance-rule JSON.

        Returns
        -------
        RebalanceRule
            Parsed and validated scheduling policy.

        Raises
        ------
        ValueError
            If JSON, dates, schedule ordering, or calendar lookup is invalid.

        Examples
        --------
        >>> RebalanceRule.from_json('{"kind":"manual"}').to_json()
        '{"kind":"manual"}'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the canonical tagged rebalance rule.

        Returns
        -------
        str
            Strict rebalance-rule JSON.

        Raises
        ------
        ValueError
            If serialization of the canonical Rust rule fails.
        """
        ...

class CompositeSpec:
    """
    Unresolved economic definition containing embedded legs and future policy.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import TermLoan
    >>> _loan = TermLoan.example()
    >>> _other = json.loads(_loan.to_json())
    >>> _other["instrument"]["spec"]["id"] = "TERM-LOAN-ALT"
    >>> _spec = CompositeSpec(
    ...     "LOAN-SPREAD",
    ...     Currency("USD"),
    ...     Money(1_000_000.0, Currency("USD")),
    ...     [
    ...         CompositeLegSpec(_loan.id, _loan, 1.0),
    ...         CompositeLegSpec("TERM-LOAN-ALT", json.dumps(_other), -1.0),
    ...     ],
    ...     WeightingMethod.fixed_quantity(),
    ...     RebalanceRule.manual(),
    ... )
    >>> _spec.id
    'LOAN-SPREAD'
    """

    def __init__(
        self,
        id: str,
        reporting_currency: Currency,
        capital: Money,
        legs: list[CompositeLegSpec],
        weighting_method: WeightingMethod,
        rebalance_rule: RebalanceRule,
    ) -> None:
        """
        Construct and validate a self-contained composite specification.

        Parameters
        ----------
        id : str
            Stable composite identifier used for pricing and serialization.
        reporting_currency : Currency
            Currency used for capital, values, risk, P&L, and return reporting.
        capital : Money
            Positive return denominator in exactly ``reporting_currency``.
        legs : list[CompositeLegSpec]
            At least two unique signed legs with matching embedded identifiers.
        weighting_method : WeightingMethod
            Policy used only during initialization or explicit rebalance.
        rebalance_rule : RebalanceRule
            Manual or scheduled rule controlling state transitions.

        Raises
        ------
        ValueError
            If any specification invariant or embedded definition is invalid.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CompositeSpec:
        """
        Deserialize and validate a bare composite specification.

        Parameters
        ----------
        json : str
            Bare strict ``CompositeSpec`` JSON produced by :meth:`to_json`.

        Returns
        -------
        CompositeSpec
            Parsed unresolved economic definition.

        Raises
        ------
        ValueError
            If JSON or any nested specification invariant is invalid.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> _loan = TermLoan.example()
        >>> _other = json.loads(_loan.to_json())
        >>> _other["instrument"]["spec"]["id"] = "TERM-LOAN-ALT"
        >>> _spec = CompositeSpec(
        ...     "LOAN-SPREAD",
        ...     Currency("USD"),
        ...     Money(1_000_000.0, Currency("USD")),
        ...     [
        ...         CompositeLegSpec(_loan.id, _loan, 1.0),
        ...         CompositeLegSpec("TERM-LOAN-ALT", json.dumps(_other), -1.0),
        ...     ],
        ...     WeightingMethod.fixed_quantity(),
        ...     RebalanceRule.manual(),
        ... )
        >>> CompositeSpec.from_json(_spec.to_json()).id
        'LOAN-SPREAD'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this unresolved definition as bare JSON.

        Returns
        -------
        str
            Strict ``CompositeSpec`` JSON with embedded instruments.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    @property
    def id(self) -> str:
        """
        Return the stable composite identifier.

        Returns
        -------
        str
            Identifier stored on the unresolved specification.

        Notes
        -----
        This accessor does not raise; it returns the stored identifier.
        """
        ...

    @property
    def reporting_currency(self) -> str:
        """
        Return the ISO code used for values, risk, P&L, and returns.

        Returns
        -------
        str
            Three-letter reporting-currency code.

        Notes
        -----
        This accessor does not raise; it returns the validated stored currency.
        """
        ...

    @property
    def capital(self) -> Money:
        """
        Return the capital denominator (``Money`` in the reporting currency).

        Returns
        -------
        Money
            Positive return denominator used by the history engine.

        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def legs(self) -> list[CompositeLegSpec]:
        """
        Return the signed leg definitions in specification order.

        Returns
        -------
        list[CompositeLegSpec]
            Independent copies of the embedded legs.

        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def weighting_method(self) -> WeightingMethod:
        """
        Return the weighting policy.

        Returns
        -------
        WeightingMethod
            Policy applied at initialization or explicit rebalance.

        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rebalance_rule(self) -> RebalanceRule:
        """
        Return the rebalance rule.

        Returns
        -------
        RebalanceRule
            Manual or scheduled state-transition policy.

        This accessor does not raise; it returns the stored value.
        """
        ...

    def initialize(
        self, market: MarketContext | str, as_of: DateLike, history: Observations | None = None
    ) -> CompositeRebalanceResult:
        """
        Resolve immutable quantities from information available through a date.

        Parameters
        ----------
        market : MarketContext | str
            Complete current market object or canonical market JSON.
        as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
            Effective date as a date-like value or ISO-8601 string.
        history : list[dict[str, Any]] | str | None
            Strict chronological ``CompositeMarketObservation`` array as a
            list of dicts or a JSON string; ``None`` means no history.

        Returns
        -------
        CompositeRebalanceResult
            New priceable instrument and primitive establishment trades.

        Raises
        ------
        ValueError
            If validation, history, metric, notional, FX, or quantity resolution fails.

        Notes
        -----
        There is no separate ``initialize_fixed`` binding. ``fixed_quantity``
        specs resolve through this method and do not require historical
        observations. Volatility weighting requires ``history`` to be
        strictly increasing and to end on ``as_of``.
        """
        ...

class CompositeState:
    """
    Frozen effective date, resolved quantities, and weighting audit inputs.

    Examples
    --------
    >>> _state = CompositeState.from_json('{"effective_date":"2025-01-01","resolved_legs":[],"weighting_inputs":{}}')
    >>> _state.effective_date
    '2025-01-01'
    """

    @staticmethod
    def from_json(json: str) -> CompositeState:
        """
        Deserialize a bare resolved-state object.

        Parameters
        ----------
        json : str
            Strict state JSON produced by :meth:`to_json`.

        Returns
        -------
        CompositeState
            Parsed immutable state data.

        Raises
        ------
        ValueError
            If JSON does not match the strict state schema.

        Examples
        --------
        >>> _state_copy = CompositeState.from_json(
        ...     '{"effective_date":"2025-01-01","resolved_legs":[],"weighting_inputs":{}}'
        ... )
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the frozen state as canonical JSON.

        Returns
        -------
        str
            State effective date, resolved legs, and finite weighting inputs.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    @property
    def effective_date(self) -> str:
        """
        Return the ISO date from which these quantities are held.

        Returns
        -------
        str
            Effective date formatted as ``YYYY-MM-DD``.

        Notes
        -----
        This accessor does not raise; it returns the stored state date.
        """
        ...

    @property
    def resolved_quantities(self) -> dict[str, float]:
        """
        Return signed top-level quantities keyed by leg identifier.

        Returns
        -------
        dict[str, float]
            New mapping from top-level leg IDs to frozen signed quantities.

        Notes
        -----
        This accessor does not raise; it copies the validated resolved legs.
        """
        ...

class CompositeInstrument:
    """
    Priceable composite containing a specification and immutable resolved state.

    Examples
    --------
    >>> import datetime, json
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import TermLoan
    >>> _loan = TermLoan.example()
    >>> _other = json.loads(_loan.to_json())
    >>> _other["instrument"]["spec"]["id"] = "TERM-LOAN-ALT"
    >>> _spec = CompositeSpec(
    ...     "LOAN-SPREAD",
    ...     Currency("USD"),
    ...     Money(1_000_000.0, Currency("USD")),
    ...     [
    ...         CompositeLegSpec(_loan.id, _loan, 1.0),
    ...         CompositeLegSpec("TERM-LOAN-ALT", json.dumps(_other), -1.0),
    ...     ],
    ...     WeightingMethod.fixed_quantity(),
    ...     RebalanceRule.manual(),
    ... )
    >>> _instrument = _spec.initialize(MarketContext(), datetime.date(2025, 1, 1)).instrument
    >>> (_instrument.id, _instrument.state.resolved_quantities)
    ('LOAN-SPREAD', {'TERM-LOAN-ALT': -1.0, 'TERM-LOAN-USD-5Y': 1.0})
    """

    @staticmethod
    def from_json(json: str) -> CompositeInstrument:
        """
        Deserialize and validate a canonical composite instrument envelope.

        Parameters
        ----------
        json : str
            Required ``finstack_quant.instrument/1`` composite envelope.

        Returns
        -------
        CompositeInstrument
            Parsed priceable resolved composite.

        Raises
        ------
        ValueError
            If JSON is malformed, non-composite, unresolved, or internally inconsistent.

        Examples
        --------
        >>> import datetime, json
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> _loan = TermLoan.example()
        >>> _other = json.loads(_loan.to_json())
        >>> _other["instrument"]["spec"]["id"] = "TERM-LOAN-ALT"
        >>> _spec = CompositeSpec(
        ...     "LOAN-SPREAD",
        ...     Currency("USD"),
        ...     Money(1_000_000.0, Currency("USD")),
        ...     [
        ...         CompositeLegSpec(_loan.id, _loan, 1.0),
        ...         CompositeLegSpec("TERM-LOAN-ALT", json.dumps(_other), -1.0),
        ...     ],
        ...     WeightingMethod.fixed_quantity(),
        ...     RebalanceRule.manual(),
        ... )
        >>> _instrument = _spec.initialize(MarketContext(), datetime.date(2025, 1, 1)).instrument
        >>> CompositeInstrument.from_json(_instrument.to_json()).id
        'LOAN-SPREAD'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize as the canonical instrument envelope accepted by pricing APIs.

        Returns
        -------
        str
            Validated ``finstack_quant.instrument/1`` composite JSON.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    @property
    def id(self) -> str:
        """
        Return the stable composite identifier.

        Returns
        -------
        str
            Identifier stored on the composite specification.

        Notes
        -----
        This accessor does not raise; it returns the stored identifier.
        """
        ...

    @property
    def spec(self) -> CompositeSpec:
        """
        Return a clone of the unresolved economic definition.

        Returns
        -------
        CompositeSpec
            Independent wrapper around a cloned specification.

        Notes
        -----
        This accessor does not raise; cloning preserves the immutable definition.
        """
        ...

    @property
    def state(self) -> CompositeState:
        """
        Return a clone of the immutable resolved holdings state.

        Returns
        -------
        CompositeState
            Independent wrapper around the frozen effective-date state.

        Notes
        -----
        This accessor does not raise; cloning cannot rebalance the instrument.
        """
        ...

    def rebalance(
        self, market: MarketContext | str, as_of: DateLike, history: Observations | None = None
    ) -> CompositeRebalanceResult:
        """
        Explicitly return a distinct resolved state and primitive trade deltas.

        Parameters
        ----------
        market : MarketContext | str
            Complete rebalance-date market object or canonical JSON.
        as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
            Effective date for the new state.
        history : list[dict[str, Any]] | str | None
            Strict chronological observation array (list of dicts or JSON
            string) available through ``as_of``; ``None`` means no history.

        Returns
        -------
        CompositeRebalanceResult
            New immutable instrument plus net primitive quantity deltas.

        Raises
        ------
        ValueError
            If market/history inputs or quantity resolution are invalid.
        """
        ...

    def primitive_exposures(
        self, market: MarketContext | str, as_of: datetime.date | str, metrics: list[str] | None = None
    ) -> CompositeExposureReport:
        """
        Price recursive primitive paths and report net/gross value and risk.

        Parameters
        ----------
        market : MarketContext | str
            Complete valuation and FX market context.
        as_of : datetime.date | str
            Valuation date used for prices, metrics, and FX conversion.
        metrics : list[str] | None
            Additive metric IDs; normalized non-additive measures are rejected.

        Returns
        -------
        CompositeExposureReport
            Path-level and primitive net/gross concentration report.

        Raises
        ------
        ValueError
            If state, metrics, market data, FX, or primitive pricing are invalid.
        """
        ...

    def execution_trades(self, previous: CompositeInstrument | None = None) -> list[dict[str, Any]]:
        """
        Flatten target holdings or a transition into primitive quantity deltas.

        Parameters
        ----------
        previous : CompositeInstrument | None
            Prior resolved state, or ``None`` for establishment trades.

        Returns
        -------
        list[dict[str, Any]]
            One ``{"instrument_id", "instrument_type", "quantity_delta"}`` dict
            per primitive with signed quantity deltas. ``execution_trades_json``
            is the JSON twin.

        Raises
        ------
        ValueError
            If either state is invalid or primitive definitions conflict.

        Examples
        --------
        >>> import datetime, json
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.composite import (
        ...     CompositeLegSpec,
        ...     CompositeSpec,
        ...     RebalanceRule,
        ...     WeightingMethod,
        ... )
        >>> def _equity(instrument_id, price):
        ...     return json.dumps({
        ...         "schema": "finstack_quant.instrument/1",
        ...         "instrument": {
        ...             "type": "equity",
        ...             "spec": {
        ...                 "id": instrument_id,
        ...                 "ticker": instrument_id,
        ...                 "currency": "USD",
        ...                 "shares": 1.0,
        ...                 "price_quote": price,
        ...                 "price_id": None,
        ...                 "div_yield_id": None,
        ...                 "discrete_dividends": [],
        ...                 "discount_curve_id": "USD",
        ...                 "attributes": {"tags": [], "meta": {}},
        ...             },
        ...         },
        ...     })
        >>> _spec = CompositeSpec(
        ...     "A-B",
        ...     Currency("USD"),
        ...     Money(100.0, Currency("USD")),
        ...     [
        ...         CompositeLegSpec("A", _equity("A", 100.0), 1.0),
        ...         CompositeLegSpec("B", _equity("B", 90.0), -1.0),
        ...     ],
        ...     WeightingMethod.fixed_quantity(),
        ...     RebalanceRule.manual(),
        ... )
        >>> _resolved = _spec.initialize(MarketContext(), datetime.date(2025, 1, 1)).instrument
        >>> [t["quantity_delta"] for t in _resolved.execution_trades()]
        [1.0, -1.0]
        """
        ...

    def execution_trades_json(self, previous: CompositeInstrument | None = None) -> str:
        """
        JSON twin of :meth:`execution_trades`.

        Parameters
        ----------
        previous : CompositeInstrument | None
            Prior resolved state, or ``None`` for establishment trades.

        Returns
        -------
        str
            JSON array of primitive identifiers, types, and signed quantity deltas.

        Raises
        ------
        ValueError
            If either state is invalid or primitive definitions conflict.
        """
        ...

    def execution_trades_dataframe(self, previous: CompositeInstrument | None = None) -> pd.DataFrame:
        """
        :meth:`execution_trades` as a pandas ``DataFrame``.

        Parameters
        ----------
        previous : CompositeInstrument | None
            Prior resolved state, or ``None`` for establishment trades.

        Returns
        -------
        pandas.DataFrame
            Columns ``instrument_id``, ``instrument_type``, ``quantity_delta``.

        Raises
        ------
        ValueError
            If either state is invalid or primitive definitions conflict.
        """
        ...

class CompositeRebalanceResult:
    """
    Resolved immutable instrument and net primitive execution deltas.

    Examples
    --------
    >>> import datetime, json
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import TermLoan
    >>> _loan = TermLoan.example()
    >>> _other = json.loads(_loan.to_json())
    >>> _other["instrument"]["spec"]["id"] = "TERM-LOAN-ALT"
    >>> _spec = CompositeSpec(
    ...     "LOAN-SPREAD",
    ...     Currency("USD"),
    ...     Money(1_000_000.0, Currency("USD")),
    ...     [
    ...         CompositeLegSpec(_loan.id, _loan, 1.0),
    ...         CompositeLegSpec("TERM-LOAN-ALT", json.dumps(_other), -1.0),
    ...     ],
    ...     WeightingMethod.fixed_quantity(),
    ...     RebalanceRule.manual(),
    ... )
    >>> _result = _spec.initialize(MarketContext(), datetime.date(2025, 1, 1))
    >>> (_result.instrument.id, len(json.loads(_result.trades_json)))
    ('LOAN-SPREAD', 2)
    """

    @staticmethod
    def from_json(json: str) -> CompositeRebalanceResult:
        """
        Deserialize a complete resolved instrument and primitive trade list.

        Parameters
        ----------
        json : str
            Strict JSON produced by :meth:`to_json`.

        Returns
        -------
        CompositeRebalanceResult
            Parsed immutable instrument and its primitive execution deltas.

        Raises
        ------
        ValueError
            If JSON is malformed or the embedded composite state is invalid.

        Examples
        --------
        >>> import datetime, json
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> _loan = TermLoan.example()
        >>> _other = json.loads(_loan.to_json())
        >>> _other["instrument"]["spec"]["id"] = "TERM-LOAN-ALT"
        >>> _spec = CompositeSpec(
        ...     "LOAN-SPREAD",
        ...     Currency("USD"),
        ...     Money(1_000_000.0, Currency("USD")),
        ...     [
        ...         CompositeLegSpec(_loan.id, _loan, 1.0),
        ...         CompositeLegSpec("TERM-LOAN-ALT", json.dumps(_other), -1.0),
        ...     ],
        ...     WeightingMethod.fixed_quantity(),
        ...     RebalanceRule.manual(),
        ... )
        >>> _result = _spec.initialize(MarketContext(), datetime.date(2025, 1, 1))
        >>> CompositeRebalanceResult.from_json(_result.to_json()).instrument.id
        'LOAN-SPREAD'
        """
        ...

    @property
    def instrument(self) -> CompositeInstrument:
        """
        Return the newly resolved priceable composite instrument.

        Returns
        -------
        CompositeInstrument
            Independent wrapper around the new immutable resolved state.

        Notes
        -----
        This accessor does not raise; it clones the stored result instrument.
        """
        ...

    @property
    def trades_json(self) -> str:
        """
        Return net primitive quantity deltas as a JSON array.

        Returns
        -------
        str
            Primitive IDs, type tags, and signed quantity deltas.

        Raises
        ------
        ValueError
            If canonical JSON serialization fails.
        """
        ...

    @property
    def trades(self) -> list[dict[str, Any]]:
        """
        Return net primitive quantity deltas as a list of dicts.

        Returns
        -------
        list[dict[str, Any]]
            ``{"instrument_id", "instrument_type", "quantity_delta"}`` per primitive.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export primitive execution deltas as a pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Columns ``instrument_id``, ``instrument_type``, ``quantity_delta``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the complete rebalance result.

        Returns
        -------
        str
            JSON containing the resolved instrument data and primitive trades.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

class CompositeExposureReport:
    """
    Recursive primitive paths plus net and gross concentration aggregates.

    Examples
    --------
    >>> import datetime, json
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.core.money import Money
    >>> def _equity(instrument_id: str, price: float) -> str:
    ...     return json.dumps({
    ...         "schema": "finstack_quant.instrument/1",
    ...         "instrument": {
    ...             "type": "equity",
    ...             "spec": {
    ...                 "id": instrument_id,
    ...                 "ticker": instrument_id,
    ...                 "currency": "USD",
    ...                 "shares": 1.0,
    ...                 "price_quote": price,
    ...                 "price_id": None,
    ...                 "div_yield_id": None,
    ...                 "discrete_dividends": [],
    ...                 "discount_curve_id": "USD",
    ...                 "attributes": {"tags": [], "meta": {}},
    ...             },
    ...         },
    ...     })
    >>> _spec = CompositeSpec(
    ...     "A-B",
    ...     Currency("USD"),
    ...     Money(100.0, Currency("USD")),
    ...     [CompositeLegSpec("A", _equity("A", 100.0), 1.0), CompositeLegSpec("B", _equity("B", 90.0), -1.0)],
    ...     WeightingMethod.fixed_quantity(),
    ...     RebalanceRule.manual(),
    ... )
    >>> _report = _spec.initialize(MarketContext(), datetime.date(2025, 1, 1)).instrument.primitive_exposures(
    ...     MarketContext(), datetime.date(2025, 1, 2)
    ... )
    >>> [item["instrument_id"] for item in json.loads(_report.to_json())["aggregates"]]
    ['A', 'B']
    """

    @staticmethod
    def from_json(json: str) -> CompositeExposureReport:
        """
        Deserialize primitive paths and net/gross aggregate exposures.

        Parameters
        ----------
        json : str
            Strict JSON produced by :meth:`to_json`.

        Returns
        -------
        CompositeExposureReport
            Parsed report in its declared reporting currency.

        Raises
        ------
        ValueError
            If JSON is malformed or does not match the report contract.

        Examples
        --------
        >>> _report = CompositeExposureReport.from_json('{"reporting_currency":"USD","paths":[],"aggregates":[]}')
        """
        ...

    @property
    def reporting_currency(self) -> Currency:
        """
        Return the reporting currency of every value and risk figure.

        Returns
        -------
        Currency
            Composite reporting currency.

        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def path_count(self) -> int:
        """
        Return the number of recursive primitive paths.

        Returns
        -------
        int
            Count of path-level rows.

        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def aggregate_count(self) -> int:
        """
        Return the number of primitive aggregates.

        Returns
        -------
        int
            Count of net/gross aggregate rows.

        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export primitive aggregates as a pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Columns ``instrument_id``, ``instrument_type``, ``net_quantity``,
            ``gross_quantity``, ``net_value``, ``gross_value``, ``currency``,
            then one column per additive metric requested.

        Raises
        ------
        ValueError
            If canonical serialization fails.

        Examples
        --------
        >>> import datetime, json
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.composite import (
        ...     CompositeLegSpec,
        ...     CompositeSpec,
        ...     RebalanceRule,
        ...     WeightingMethod,
        ... )
        >>> def _equity(instrument_id, price):
        ...     return json.dumps({
        ...         "schema": "finstack_quant.instrument/1",
        ...         "instrument": {
        ...             "type": "equity",
        ...             "spec": {
        ...                 "id": instrument_id,
        ...                 "ticker": instrument_id,
        ...                 "currency": "USD",
        ...                 "shares": 1.0,
        ...                 "price_quote": price,
        ...                 "price_id": None,
        ...                 "div_yield_id": None,
        ...                 "discrete_dividends": [],
        ...                 "discount_curve_id": "USD",
        ...                 "attributes": {"tags": [], "meta": {}},
        ...             },
        ...         },
        ...     })
        >>> _spec = CompositeSpec(
        ...     "A-B",
        ...     Currency("USD"),
        ...     Money(100.0, Currency("USD")),
        ...     [
        ...         CompositeLegSpec("A", _equity("A", 100.0), 1.0),
        ...         CompositeLegSpec("B", _equity("B", 90.0), -1.0),
        ...     ],
        ...     WeightingMethod.fixed_quantity(),
        ...     RebalanceRule.manual(),
        ... )
        >>> _report = _spec.initialize(MarketContext(), datetime.date(2025, 1, 1)).instrument.primitive_exposures(
        ...     MarketContext(), datetime.date(2025, 1, 2)
        ... )
        >>> list(_report.to_dataframe()["instrument_id"])
        ['A', 'B']
        """
        ...

    def to_json(self) -> str:
        """
        Serialize paths and aggregate quantity, value, and additive risk.

        Returns
        -------
        str
            Canonical exposure-report JSON in the composite reporting currency.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

class CompositeHistoryResult:
    """
    Chronological dated-market rows returned by the composite history engine.

    Examples
    --------
    >>> import datetime, json
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.core.money import Money
    >>> def _equity(instrument_id: str, price: float) -> str:
    ...     return json.dumps({
    ...         "schema": "finstack_quant.instrument/1",
    ...         "instrument": {
    ...             "type": "equity",
    ...             "spec": {
    ...                 "id": instrument_id,
    ...                 "ticker": instrument_id,
    ...                 "currency": "USD",
    ...                 "shares": 1.0,
    ...                 "price_quote": price,
    ...                 "price_id": None,
    ...                 "div_yield_id": None,
    ...                 "discrete_dividends": [],
    ...                 "discount_curve_id": "USD",
    ...                 "attributes": {"tags": [], "meta": {}},
    ...             },
    ...         },
    ...     })
    >>> _spec = CompositeSpec(
    ...     "A-B",
    ...     Currency("USD"),
    ...     Money(100.0, Currency("USD")),
    ...     [CompositeLegSpec("A", _equity("A", 100.0), 1.0), CompositeLegSpec("B", _equity("B", 90.0), -1.0)],
    ...     WeightingMethod.fixed_quantity(),
    ...     RebalanceRule.manual(),
    ... )
    >>> _state = json.loads(MarketContext().to_json())
    >>> _history = CompositeHistoryEngine.run_from_spec(
    ...     _spec,
    ...     json.dumps([
    ...         {"date": "2025-01-01", "state": _state},
    ...         {"date": "2025-01-02", "state": _state},
    ...     ]),
    ... )
    >>> (len(_history), json.loads(_history.row_json(0))["return_index"])
    (2, 100.0)
    """

    @staticmethod
    def from_json(json: str) -> CompositeHistoryResult:
        """
        Deserialize a chronological array of composite history rows.

        Parameters
        ----------
        json : str
            Strict history-row array JSON produced by :meth:`to_json`.

        Returns
        -------
        CompositeHistoryResult
            Parsed immutable row collection.

        Raises
        ------
        ValueError
            If JSON is malformed or a row violates its serialized contract.

        Examples
        --------
        >>> len(CompositeHistoryResult.from_json("[]"))
        0
        """
        ...

    def __len__(self) -> int:
        """
        Return the number of chronological output rows.

        Returns
        -------
        int
            Count of dated history rows in chronological order.

        Notes
        -----
        This accessor does not raise; an empty result has length ``0``.
        """
        ...

    @property
    def dates(self) -> list[str]:
        """
        Return the ISO-8601 observation dates in chronological order.

        Returns
        -------
        list[str]
            One date per output row.

        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the dated rows as a pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Columns ``date``, ``value``, ``cashflows``, ``pnl``, ``currency``,
            ``period_return``, ``return_index``, ``held_state_effective_date``,
            ``next_state_effective_date``, ``rebalance_trade_count``, then one
            column per additive metric requested.

        Raises
        ------
        ValueError
            If canonical serialization fails.

        Examples
        --------
        >>> import datetime, json
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.composite import (
        ...     CompositeLegSpec,
        ...     CompositeSpec,
        ...     RebalanceRule,
        ...     WeightingMethod,
        ... )
        >>> def _equity(instrument_id, price):
        ...     return json.dumps({
        ...         "schema": "finstack_quant.instrument/1",
        ...         "instrument": {
        ...             "type": "equity",
        ...             "spec": {
        ...                 "id": instrument_id,
        ...                 "ticker": instrument_id,
        ...                 "currency": "USD",
        ...                 "shares": 1.0,
        ...                 "price_quote": price,
        ...                 "price_id": None,
        ...                 "div_yield_id": None,
        ...                 "discrete_dividends": [],
        ...                 "discount_curve_id": "USD",
        ...                 "attributes": {"tags": [], "meta": {}},
        ...             },
        ...         },
        ...     })
        >>> _spec = CompositeSpec(
        ...     "A-B",
        ...     Currency("USD"),
        ...     Money(100.0, Currency("USD")),
        ...     [
        ...         CompositeLegSpec("A", _equity("A", 100.0), 1.0),
        ...         CompositeLegSpec("B", _equity("B", 90.0), -1.0),
        ...     ],
        ...     WeightingMethod.fixed_quantity(),
        ...     RebalanceRule.manual(),
        ... )
        >>> from finstack_quant.valuations.composite import CompositeHistoryEngine
        >>> _state = json.loads(MarketContext().to_json())
        >>> _history = CompositeHistoryEngine.run_from_spec(
        ...     _spec,
        ...     json.dumps([
        ...         {"date": "2025-01-01", "state": _state},
        ...         {"date": "2025-01-02", "state": _state},
        ...     ]),
        ... )
        >>> list(_history.to_dataframe()["return_index"])
        [100.0, 100.0]
        """
        ...

    def row_json(self, index: int) -> str:
        """
        Serialize one zero-based dated history row.

        Parameters
        ----------
        index : int
            Zero-based row index in chronological order.

        Returns
        -------
        str
            JSON for the selected value, P&L, return, exposure, and trade row.

        Raises
        ------
        IndexError
            If ``index`` is outside the result bounds.
        ValueError
            If the selected row cannot be serialized.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize every dated history row as a JSON array.

        Returns
        -------
        str
            Chronological array containing values, cashflows, P&L, returns, indices, exposures, and trades.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

class CompositeHistoryEngine:
    """
    Focused dated-market engine for composite total return and rebalancing.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.core.money import Money
    >>> def _equity(instrument_id: str, price: float) -> str:
    ...     return json.dumps({
    ...         "schema": "finstack_quant.instrument/1",
    ...         "instrument": {
    ...             "type": "equity",
    ...             "spec": {
    ...                 "id": instrument_id,
    ...                 "ticker": instrument_id,
    ...                 "currency": "USD",
    ...                 "shares": 1.0,
    ...                 "price_quote": price,
    ...                 "price_id": None,
    ...                 "div_yield_id": None,
    ...                 "discrete_dividends": [],
    ...                 "discount_curve_id": "USD",
    ...                 "attributes": {"tags": [], "meta": {}},
    ...             },
    ...         },
    ...     })
    >>> _spec = CompositeSpec(
    ...     "A-B",
    ...     Currency("USD"),
    ...     Money(100.0, Currency("USD")),
    ...     [CompositeLegSpec("A", _equity("A", 100.0), 1.0), CompositeLegSpec("B", _equity("B", 90.0), -1.0)],
    ...     WeightingMethod.fixed_quantity(),
    ...     RebalanceRule.manual(),
    ... )
    >>> _state = json.loads(MarketContext().to_json())
    >>> len(
    ...     CompositeHistoryEngine.run_from_spec(
    ...         _spec,
    ...         json.dumps([{"date": "2025-01-01", "state": _state}, {"date": "2025-01-02", "state": _state}]),
    ...     )
    ... )
    2
    """

    @staticmethod
    def run_from_spec(
        spec: CompositeSpec,
        observations: Observations,
        warmup: Observations | None = None,
        metrics: list[str] | None = None,
    ) -> CompositeHistoryResult:
        """
        Initialize at the first observation and calculate chronological rows.

        Parameters
        ----------
        spec : CompositeSpec
            Unresolved definition initialized using only available warmup and first-date information.
        observations : list[dict[str, Any]] | str
            Non-empty strictly increasing complete market-observation array
            (list of dicts or JSON string).
        warmup : list[dict[str, Any]] | str | None
            Optional strictly earlier complete observations used for weighting only.
        metrics : list[str] | None
            Optional canonical additive metric keys included on every output row.

        Returns
        -------
        CompositeHistoryResult
            Dated value, cashflow, P&L, return, index, exposure, state, and trade rows.

        Raises
        ------
        ValueError
            If metrics, observations, warmup, initialization, pricing, FX, or rebalancing fail.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.core.money import Money
        >>> def _equity(instrument_id: str, price: float) -> str:
        ...     return json.dumps({
        ...         "schema": "finstack_quant.instrument/1",
        ...         "instrument": {
        ...             "type": "equity",
        ...             "spec": {
        ...                 "id": instrument_id,
        ...                 "ticker": instrument_id,
        ...                 "currency": "USD",
        ...                 "shares": 1.0,
        ...                 "price_quote": price,
        ...                 "price_id": None,
        ...                 "div_yield_id": None,
        ...                 "discrete_dividends": [],
        ...                 "discount_curve_id": "USD",
        ...                 "attributes": {"tags": [], "meta": {}},
        ...             },
        ...         },
        ...     })
        >>> _spec = CompositeSpec(
        ...     "A-B",
        ...     Currency("USD"),
        ...     Money(100.0, Currency("USD")),
        ...     [CompositeLegSpec("A", _equity("A", 100.0), 1.0), CompositeLegSpec("B", _equity("B", 90.0), -1.0)],
        ...     WeightingMethod.fixed_quantity(),
        ...     RebalanceRule.manual(),
        ... )
        >>> _state = json.loads(MarketContext().to_json())
        >>> json.loads(
        ...     CompositeHistoryEngine.run_from_spec(
        ...         _spec,
        ...         json.dumps([{"date": "2025-01-01", "state": _state}, {"date": "2025-01-02", "state": _state}]),
        ...     ).to_json()
        ... )[0]["return_index"]
        100.0
        """
        ...

    @staticmethod
    def run(
        instrument: CompositeInstrument, observations: Observations, metrics: list[str] | None = None
    ) -> CompositeHistoryResult:
        """
        Calculate chronological rows from an already-resolved initial state.

        Parameters
        ----------
        instrument : CompositeInstrument
            Immutable resolved state held from the first supplied observation.
        observations : list[dict[str, Any]] | str
            Non-empty strictly increasing complete market-observation array
            (list of dicts or JSON string).
        metrics : list[str] | None
            Optional canonical additive metric keys included on every output row.

        Returns
        -------
        CompositeHistoryResult
            Dated total-return rows with close-effective rebalance transitions.

        Raises
        ------
        ValueError
            If metrics, state, observations, market inputs, pricing, FX, or rebalancing fail.

        Examples
        --------
        >>> import datetime, json
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.core.money import Money
        >>> def _equity(instrument_id: str, price: float) -> str:
        ...     return json.dumps({
        ...         "schema": "finstack_quant.instrument/1",
        ...         "instrument": {
        ...             "type": "equity",
        ...             "spec": {
        ...                 "id": instrument_id,
        ...                 "ticker": instrument_id,
        ...                 "currency": "USD",
        ...                 "shares": 1.0,
        ...                 "price_quote": price,
        ...                 "price_id": None,
        ...                 "div_yield_id": None,
        ...                 "discrete_dividends": [],
        ...                 "discount_curve_id": "USD",
        ...                 "attributes": {"tags": [], "meta": {}},
        ...             },
        ...         },
        ...     })
        >>> _spec = CompositeSpec(
        ...     "A-B",
        ...     Currency("USD"),
        ...     Money(100.0, Currency("USD")),
        ...     [CompositeLegSpec("A", _equity("A", 100.0), 1.0), CompositeLegSpec("B", _equity("B", 90.0), -1.0)],
        ...     WeightingMethod.fixed_quantity(),
        ...     RebalanceRule.manual(),
        ... )
        >>> _instrument = _spec.initialize(MarketContext(), datetime.date(2025, 1, 1)).instrument
        >>> _state = json.loads(MarketContext().to_json())
        >>> len(
        ...     CompositeHistoryEngine.run(
        ...         _instrument,
        ...         json.dumps([{"date": "2025-01-01", "state": _state}, {"date": "2025-01-02", "state": _state}]),
        ...     )
        ... )
        2
        """
        ...
