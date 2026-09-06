"""Instrument pricing, risk metrics, cashflow inspection, and composites.

Where things live:

- Market data (``DiscountCurve``, ``ForwardCurve``, ``HazardCurve``,
  ``MarketContext``, ``FxMatrix``): :mod:`finstack_quant.core.market_data`;
  curve bootstrapping and quote ingestion: :mod:`finstack_quant.calibration`.
- Instruments, builders and :func:`~finstack_quant.valuations.instruments.price_instrument`:
  :mod:`finstack_quant.valuations.instruments`.
- Results: :class:`ValuationResult` (here) and :func:`instrument_cashflows`.
- Composites, credit-derivative examples, listed-market catalog and JSON
  schemas: ``.composite``, ``.credit_derivatives``, ``.market``, ``.schema``.

Examples
--------
>>> from finstack_quant.valuations import instruments
>>> hasattr(instruments, "price_instrument")
True

"""

from __future__ import annotations

import datetime
from typing import Any

import pandas as pd

from finstack_quant.core.dates import StubKind
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.valuations import composite as composite
from finstack_quant.valuations import credit_derivatives as credit_derivatives
from finstack_quant.valuations import instruments as instruments
from finstack_quant.valuations import market as market
from finstack_quant.valuations import schema as schema

__all__ = [
    "composite",
    "credit_derivatives",
    "instruments",
    "market",
    "schema",
    "ValuationResult",
    "tarn_coupon_profile",
    "snowball_coupon_profile",
    "inverse_floater_coupon_profile",
    "cms_spread_option_intrinsic",
    "callable_range_accrual_accrued",
    "instrument_cashflows",
]

class ValuationResult:
    """
    Valuation envelope: PV, currency, risk metrics, covenant reports, and JSON round-trip.

    Returned directly by the ``price_*`` helpers; :meth:`from_json` rebuilds one
    from a previously serialized payload.

    Metric keys are fully qualified and literal (``pv01::USD-OIS``,
    ``bucketed_dv01::USD-OIS::10y``); ``result["dv01"]`` / :meth:`get_metric`
    read one measure, :attr:`metrics` is the whole dict, :meth:`metric_units`
    labels every key (``ytm`` is a decimal, ``par_spread`` is basis points,
    ``dv01`` is currency per bp). :meth:`to_dataframe` is one wide row;
    :meth:`to_long_dataframe` is tidy ``metric / curve / bucket / value``.

    ``details`` is the optional tagged model-specific pricing payload; ``meta``
    is the Rust ``ResultsMeta`` policy stamp (numeric mode, rounding, FX, timing).
    Two results compare equal when their JSON documents are identical.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import StubKind
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.core.types import Rate
    >>> from finstack_quant.valuations.instruments import Bond, price_instrument
    >>> as_of = datetime.date(2024, 1, 15)
    >>> bond = Bond.fixed(
    ...     "B", Money(1000.0, Currency("USD")), Rate(0.05), as_of, datetime.date(2026, 1, 15), StubKind.NONE, "USD-OIS"
    ... )
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
    >>> result = price_instrument(bond, market, "2024-01-15", metrics=["ytm", "dv01"])
    >>> (result.instrument_id, round(result.price, 2), result.currency)
    ('B', 1018.16, 'USD')
    >>> sorted(result.metrics) == ["dv01", "ytm"] and "ytm" in result
    True
    >>> result.metric_units()["ytm"]
    'decimal'
    >>> isinstance(result.meta, dict) and result.details is None
    True

    """

    @staticmethod
    def from_json(json: str) -> ValuationResult:
        """
        Deserialize a ``ValuationResult`` from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by ``to_json``.

        Returns
        -------
        ValuationResult
            Parsed ``ValuationResult`` instance.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import StubKind
        >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.core.types import Rate
        >>> from finstack_quant.valuations import ValuationResult
        >>> from finstack_quant.valuations.instruments import Bond, price_instrument
        >>> as_of = datetime.date(2024, 1, 15)
        >>> bond = Bond.fixed(
        ...     "B",
        ...     Money(1000.0, Currency("USD")),
        ...     Rate(0.05),
        ...     as_of,
        ...     datetime.date(2026, 1, 15),
        ...     StubKind.NONE,
        ...     "USD-OIS",
        ... )
        >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
        >>> result = ValuationResult.from_json(price_instrument(bond, market, "2024-01-15").to_json())
        >>> (result.instrument_id, round(result.price, 2), result.currency)
        ('B', 1018.16, 'USD')

        Raises
        ------
        ValueError
            If ``json`` is malformed or cannot be deserialized as a valuation result.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to pretty-printed JSON.

        Returns
        -------
        str
            Pretty-printed JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def instrument_id(self) -> str:
        """
        Instrument identifier assigned by the pricer.

        Returns
        -------
        str
            Instrument ID string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_of(self) -> datetime.date:
        """
        Valuation date (T+0) for the calculation.

        Returns
        -------
        datetime.date
            The valuation date stamped on this result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def schema_version(self) -> int:
        """
        Wire-format schema version of the result envelope.

        Returns
        -------
        int
            Schema version number (currently ``1``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def price(self) -> float:
        """
        Present value amount (NPV).

        Returns
        -------
        float
            PV amount as a float.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def value(self) -> Money:
        """
        Present value as a currency-tagged ``Money`` (exact Decimal amount).

        Returns
        -------
        Money
            The valuation amount with its currency; ``value.amount`` is the
            ``float`` view, ``price_decimal()`` the exact string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def price_decimal(self) -> str:
        """
        Return the exact Decimal price as a string, without a float round-trip.

        Unlike the ``price`` property (a lossy ``float``), this preserves the
        internal Decimal representation exactly. Pass the result to
        ``decimal.Decimal`` for lossless arithmetic in Python.

        Returns
        -------
        str
            Exact decimal string of the valuation amount, e.g. ``"1000000.00"``.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        Currency code for the present value.

        Returns
        -------
        str
            Currency code string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def metrics(self) -> dict[str, float]:
        """
        All computed measures as ``{metric_key: value}`` in computation order.

        Keys are literal composite keys (``"bucketed_dv01::USD-OIS::10y"``).
        The present value is not a measure; read :attr:`price` / :attr:`value`.

        Returns
        -------
        dict[str, float]
            Insertion-ordered measure map (a fresh dict on every access).

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...

    def get_metric(self, key: str) -> float | None:
        """
        Return a scalar risk measure by string key.

        Parameters
        ----------
        key : str
            Canonical metric identifier (``"ytm"``, ``"dv01"``, ``"pv01::USD-OIS"``).
            Lookup matches the stored wire key exactly.

        Returns
        -------
        float or None
            Metric value, or ``None`` if missing.

        Notes
        -----
        This method does not raise; a missing result is ``None`` rather than an exception.
        """
        ...

    def __getitem__(self, key: str) -> float:
        """
        ``result[key]``: scalar measure by key.

        Parameters
        ----------
        key : str
            Metric key in canonical wire form; lookup is exact.

        Returns
        -------
        float
            Metric value.

        Raises
        ------
        KeyError
            If ``key`` is not a measure of this result; the message names the
            five closest keys present.
        """
        ...

    def __contains__(self, key: str) -> bool:
        """
        ``key in result``: whether a measure with this key is present.

        Parameters
        ----------
        key : str
            Metric key in canonical wire form; lookup is exact.

        Returns
        -------
        bool
            ``True`` when the measure exists.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality: ``True`` when both JSON documents are identical.

        Parameters
        ----------
        other : object
            Any object; non-``ValuationResult`` values compare unequal.

        Returns
        -------
        bool
            Whether the two results serialize identically (including ``meta``).

        Notes
        -----
        This method does not raise. Results are unhashable.
        """
        ...

    def metric_series(self, base: str) -> list[tuple[list[str], float]]:
        """
        Return decoded components and values for a composite base metric.

        Entries retain the deterministic insertion order of the serialized
        ``measures`` map. The scalar aggregate stored directly under ``base``
        is excluded. Canonical wire keys decode to unique coordinates.

        Parameters
        ----------
        base : str
            Unqualified metric base key, such as ``"bucketed_dv01"``, used to
            select its encoded coordinate series from the valuation measures.

        Returns
        -------
        list[tuple[list[str], float]]
            Ordered ``(coordinate_components, value)`` pairs for matching
            composite metrics; the scalar aggregate stored at ``base`` is omitted.

        Raises
        ------
        ValueError
            If the base metric key is not canonically encoded.
        """
        ...

    def metric_series_dataframe(self, base: str) -> pd.DataFrame:
        """
        Tidy ``DataFrame`` of one composite base metric.

        Parameters
        ----------
        base : str
            Unqualified base metric such as ``"bucketed_dv01"`` or ``"pv01"``.

        Returns
        -------
        pd.DataFrame
            Columns ``metric`` (the base), ``curve`` (first component),
            ``bucket`` (remaining components joined with ``::``, or ``None``)
            and ``value``; the scalar aggregate stored under ``base`` is
            excluded, matching :meth:`metric_series`.

        Raises
        ------
        ValueError
            If the rows cannot be serialized into a pandas object.
        """
        ...

    def to_long_dataframe(self) -> pd.DataFrame:
        """
        Every measure as one tidy row.

        Returns
        -------
        pd.DataFrame
            Columns ``metric`` (base name), ``curve`` (first composite
            component or ``None``), ``bucket`` (second and later components
            joined with ``::``, or ``None``) and ``value``, in computation
            order. Pivot with ``df.pivot(index="bucket", columns="curve")``.

        Raises
        ------
        ValueError
            If the rows cannot be serialized into a pandas object.
        """
        ...

    def metric_units(self) -> dict[str, str]:
        """
        Unit family of every measure, keyed by metric key.

        Returns
        -------
        dict[str, str]
            One of ``"currency"`` (PV components and currency-per-bump
            sensitivities such as ``dv01``, ``cs01``, ``vega``), ``"decimal"``
            (``ytm``, ``z_spread``, ``par_rate``, probabilities),
            ``"basis_points"`` (``par_spread``), ``"years"`` (durations, WAL),
            ``"percent"``, ``"dimensionless"`` (ratios, counts, discount
            factors) or ``"unknown"`` (custom metrics). Composite keys inherit
            their base metric's unit.

        Notes
        -----
        This method does not raise; it returns the derived mapping.
        """
        ...

    def metric_keys(self) -> list[str]:
        """
        List metric keys present on this result.

        Returns
        -------
        list[str]
            All measure keys as strings.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def metric_count(self) -> int:
        """
        Count of measures stored on this result.

        Returns
        -------
        int
            Number of entries in the measures map.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def all_covenants_passed(self) -> bool:
        """
        Whether every covenant passed (or none were evaluated).

        Returns
        -------
        bool
            ``True`` if no covenant failures are recorded.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

    def failed_covenants(self) -> list[str]:
        """
        Covenant IDs that failed, if any.

        Returns
        -------
        list[str]
            List of failed covenant identifiers.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def covenants(self) -> dict[str, dict[str, Any]] | None:
        """
        Per-covenant compliance reports keyed by covenant id, or ``None``.

        Returns
        -------
        dict[str, dict[str, Any]] or None
            Rust ``CovenantReport`` documents (``covenant_type``, ``passed``,
            ``actual_value``, ``threshold``, ``headroom``, ``details``,
            ``meta``); ``None`` for instruments without covenants.

        Raises
        ------
        ValueError
            If the reports cannot be serialized to a Python object.
        """
        ...

    @property
    def explanation(self) -> dict[str, Any] | None:
        """
        Computation explanation trace, or ``None`` when tracing was not enabled.

        Returns
        -------
        dict[str, Any] or None
            Decoded Rust ``ExplanationTrace`` document.

        Raises
        ------
        ValueError
            If the trace cannot be serialized to a Python object.
        """
        ...

    def _repr_html_(self) -> str:
        """
        Jupyter rich display: the :meth:`to_dataframe` table as HTML.

        Returns
        -------
        str
            HTML table rendered by pandas.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the headline result as a single-row pandas DataFrame.

        Columns: ``instrument_id``, ``as_of_date`` (ISO 8601 string), ``pv``,
        ``currency``, then one column per metric key in ``measures``
        insertion order.

        This is the default export, built from the Rust crate's own
        ``ValuationResult::to_row`` flattener. Stack a book with
        ``pd.concat([r.to_dataframe() for r in results])``; instruments with
        different metric sets align on column name and leave ``NaN``
        elsewhere.

        Returns
        -------
        pd.DataFrame
            Single-row DataFrame with the identity columns followed by one
            column per metric.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """
        Policy stamps from the Rust ``ResultsMeta`` envelope.

        Keys include numeric mode, rounding context, optional FX policy, and
        the computation timestamp. Same serde shape as the WASM result object.

        Returns
        -------
        dict[str, Any]
            Decoded ``ResultsMeta`` document.

        Raises
        ------
        ValueError
            If the metadata cannot be serialized to a Python object.
        """
        ...

    @property
    def details(self) -> dict[str, Any] | None:
        """
        Model-specific structured pricing detail, if the pricer emitted one.

        Tagged ``{"type": ..., "data": ...}`` document matching Rust
        ``ValuationDetails``. A stochastic ``"rates_credit"`` bond uses
        ``type="monte_carlo"``. Its ``data`` contains ``model_key``,
        sampling-only ``standard_error``, configured independent exercise-policy
        paths (``training_paths``) and their simulated count including
        antithetic partners (``training_simulated_paths``), configured
        independent make-whole-reference paths (``make_whole_training_paths``)
        and their simulated count (``make_whole_training_simulated_paths``),
        independent estimator paths (``estimator_paths``) and their simulated
        count (``simulated_paths``), ``seed``, ``time_grid``, and the
        ``antithetic``, ``sobol``, and ``brownian_bridge`` flags. An unused
        training stage reports zero paths. ``standard_error`` measures
        pricing-path sampling uncertainty under the frozen fitted exercise
        policy and excludes regression approximation, time-grid discretization,
        and model error.
        ``None`` when the envelope is scalar-only.

        Returns
        -------
        dict[str, Any] or None
            Decoded detail payload, or ``None`` when absent.

        Raises
        ------
        ValueError
            If the detail payload cannot be serialized to a Python object.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug string for this result.

        Returns
        -------
        str
            ``ValuationResult(id=..., price=..., currency=..., metrics=...)`` text.
        """
        ...

def instrument_cashflows(
    instrument: Any,
    market: MarketContext | str,
    as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    *,
    model: str,
) -> tuple[dict[str, Any], pd.DataFrame]:
    """
    DataFrame-friendly wrapper around :func:`instrument_cashflows_json`.

    Parses the JSON envelope returned by the low-level binding and constructs
    a per-flow ``pandas.DataFrame`` with ``date`` / ``reset_date`` parsed as
    ``datetime64``. See :func:`instrument_cashflows_json` for argument and
    error semantics. Hazard-rate export rejects bonds with call, put, or
    return-floor rights because static rows cannot represent their
    exercise-contingent value.

    Parameters
    ----------
    instrument : Bond | TermLoan | InterestRateSwap | ... | str
        Typed instrument instance or a canonical
        ``finstack_quant.instrument/1`` JSON envelope.
    market : MarketContext or str
        Market context object or canonical market JSON containing the curves,
        fixings, and scalar data required by the requested pricing model.
    as_of : datetime.date | datetime.datetime | pd.Timestamp | str
        Valuation date used to exclude settled flows and calculate
        schedule-relative discount factors.
    model : str
        Must be ``"discounting"`` or ``"hazard_rate"``. ``"default"`` is
        not accepted on cashflow export.

    Returns
    -------
    tuple[dict[str, Any], pd.DataFrame]
        ``(envelope, df)`` where ``envelope`` is the parsed dict and ``df``
        carries one row per flow with columns ``date``, ``amount``,
        ``currency``, ``kind``, ``accrual_factor``, ``year_fraction``,
        ``rate``, ``reset_date``, ``discount_factor``, ``discount_curve_id``,
        ``survival_probability``, ``conditional_default_prob``, ``inflation_index_ratio``,
        ``prepayment_smm``, ``beginning_balance``, ``ending_balance``, and
        ``pv``.

    Raises
    ------
    TypeError
        If ``instrument`` is neither a supported typed instrument nor
        a JSON string, or ``market`` is neither a ``MarketContext`` nor a
        JSON string.
    ValueError
        If instrument or market JSON is malformed, ``as_of`` or ``model``
        is invalid, the instrument/model pair is unsupported, a hazard-rate
        bond contains embedded exercise rights, or the generated cashflow
        schedule fails validation.
    KeyError
        If a curve, fixing, or other market datum required for cashflow
        generation or pricing is missing.
    RuntimeError
        If native pricing reports an internal, calibration, or solver failure.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import StubKind
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.core.types import Rate
    >>> from finstack_quant.valuations.instruments import Bond
    >>> as_of = datetime.date(2024, 1, 1)
    >>> bond = Bond.fixed(
    ...     "B",
    ...     Money(1000.0, Currency("USD")),
    ...     Rate(0.05),
    ...     as_of,
    ...     datetime.date(2026, 1, 1),
    ...     StubKind.NONE,
    ...     "USD-OIS",
    ... )
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
    >>> from finstack_quant.valuations import instrument_cashflows
    >>> header, frame = instrument_cashflows(bond, market, as_of, model="discounting")
    >>> (header["instrument_id"], len(frame))
    ('B', 6)

    """
    ...

def tarn_coupon_profile(
    fixed_rate: float,
    coupon_floor: float,
    floating_fixings: list[float],
    target_coupon: float,
    day_count_fraction: float,
) -> dict[str, Any]:
    """
    Simulate a TARN coupon profile along a deterministic rate path.

    Each period coupon is ``max(fixed_rate - L_i, coupon_floor) * dcf``;
    payments accumulate until the cumulative reaches ``target_coupon``, at
    which point the final coupon is capped so the cumulative hits the
    target exactly and the note redeems early.

    Parameters
    ----------
    fixed_rate : float
        Fixed strike rate.
    coupon_floor : float
        Per-period floor on ``fixed_rate - L_i``.
    floating_fixings : list[float]
        Floating rate fixings (one per period).
    target_coupon : float
        Cumulative target that triggers knockout (> 0).
    day_count_fraction : float
        Year fraction applied to each period coupon.

    Returns
    -------
    dict[str, Any]
        Dict with keys ``coupons_paid`` (list[float]), ``cumulative``
        (list[float]), ``redemption_index`` (int | None) and
        ``redeemed_early`` (bool).

    Raises
    ------
    ValueError
        If ``fixed_rate`` or a fixing is non-finite; ``coupon_floor`` is
        non-finite or negative; or ``target_coupon`` or
        ``day_count_fraction`` is non-finite or non-positive.

    Examples
    --------
    >>> from finstack_quant.valuations import tarn_coupon_profile
    >>> profile = tarn_coupon_profile(0.05, 0.0, [0.02, 0.03, 0.04], 0.025, 0.5)
    >>> (profile["redeemed_early"], profile["redemption_index"], round(profile["cumulative"][-1], 3))
    (True, 1, 0.025)

    """
    ...

def snowball_coupon_profile(
    initial_coupon: float,
    fixed_rate: float,
    floating_fixings: list[float],
    floor: float,
    cap: float,
) -> list[float]:
    """
    Compute a snowball coupon schedule.

    Snowball: ``c_i = clip(c_{i-1} + fixed_rate - L_i, floor, cap)``
    with ``c_0 = initial_coupon``.

    Pass ``float('inf')`` as ``cap`` for an uncapped coupon.

    Parameters
    ----------
    initial_coupon : float
        First-period coupon for snowball mode.
    fixed_rate : float
        Fixed strike rate.
    floating_fixings : list[float]
        Floating rate fixings (one per period).
    floor : float
        Per-period coupon floor.
    cap : float
        Per-period coupon cap (use ``float('inf')`` for uncapped).
    is_inverse_floater : bool
        ``True`` for inverse floater mode, ``False`` for snowball.
    leverage : float, default 1.0
        Leverage multiplier for inverse floater mode.

    Returns
    -------
    list[float]
        Coupon schedule, one per period.

    Raises
    ------
    ValueError
        If ``fixed_rate``, ``initial_coupon``, ``floor``, or a fixing is
        non-finite; ``initial_coupon`` or ``floor`` is negative; or ``cap`` is
        NaN or is not strictly greater than ``floor``. Positive infinity is
        accepted as an uncapped ``cap``.

    Examples
    --------
    >>> from finstack_quant.valuations import snowball_coupon_profile
    >>> snowball_coupon_profile(0.03, 0.04, [0.02, 0.03, 0.05], 0.0, 0.10)
    [0.05, 0.06, 0.05]

    """
    ...

def inverse_floater_coupon_profile(
    fixed_rate: float,
    floating_fixings: list[float],
    floor: float,
    cap: float,
    leverage: float,
) -> list[float]:
    """
    Compute a path-independent inverse-floater coupon schedule.

    Parameters
    ----------
    fixed_rate : float
        Fixed strike rate in decimal annual-rate units.
    floating_fixings : list[float]
        Floating reference-rate fixings in decimal annual-rate units, one per
        coupon period in the returned schedule.
    floor : float
        Per-period minimum coupon rate in decimal annual-rate units.
    cap : float
        Per-period maximum coupon rate in decimal annual-rate units; use
        ``float("inf")`` for no cap.
    leverage : float
        Multiplier applied to each floating fixing before it offsets the fixed rate.

    Returns
    -------
    list[float]
        Coupon rate for each fixing after applying ``fixed_rate - leverage *
        fixing`` and clamping the result to ``[floor, cap]``.

    Raises
    ------
    ValueError
        If ``fixed_rate``, ``floor``, ``leverage``, or a fixing is non-finite;
        ``floor`` is negative; ``leverage`` is non-positive; or ``cap`` is NaN
        or is not strictly greater than ``floor``. Positive infinity is
        accepted as an uncapped ``cap``.

    Examples
    --------
    >>> from finstack_quant.valuations import inverse_floater_coupon_profile
    >>> [round(value, 3) for value in inverse_floater_coupon_profile(0.08, [0.02, 0.03, 0.05], 0.0, 0.10, 1.5)]
    [0.05, 0.035, 0.005]

    """
    ...

def cms_spread_option_intrinsic(
    long_cms: float,
    short_cms: float,
    strike: float,
    is_call: bool,
    notional: float,
) -> float:
    """
    Undiscounted intrinsic payoff of a CMS spread option.

    Call: ``notional * max(long_cms - short_cms - strike, 0)``.
    Put: ``notional * max(strike - (long_cms - short_cms), 0)``.

    Ignores CMS convexity, vol smile, and correlation adjustments — the
    full product pricer applies those on top of a copula model with
    SABR marginals.

    Parameters
    ----------
    long_cms : float
        Long CMS rate.
    short_cms : float
        Short CMS rate.
    strike : float
        Spread strike.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    notional : float
        Notional amount.

    Returns
    -------
    float
        Undiscounted intrinsic payoff.

    Raises
    ------
    ValueError
        If a CMS rate or ``strike`` is non-finite, or ``notional`` is
        non-finite or negative.

    Examples
    --------
    >>> from finstack_quant.valuations import cms_spread_option_intrinsic
    >>> round(cms_spread_option_intrinsic(0.05, 0.03, 0.01, True, 1_000_000.0), 2)
    10000.0

    """
    ...

def callable_range_accrual_accrued(
    lower: float,
    upper: float,
    observations: list[float],
    coupon_rate: float,
    day_count_fraction: float,
) -> float:
    """
    Accrued coupon over a range-accrual period.

    Counts the fraction of ``observations`` within the inclusive interval
    ``[lower, upper]`` and returns
    ``coupon_rate * day_count_fraction * fraction``.

    The call provision is not applied here — this is the coupon that
    would accrue assuming the note is not called before period end.

    Parameters
    ----------
    lower : float
        Lower bound of the accrual range.
    upper : float
        Upper bound of the accrual range.
    observations : list[float]
        Observed values (one per day in the period).
    coupon_rate : float
        Coupon rate (decimal).
    day_count_fraction : float
        Year fraction for the period.

    Returns
    -------
    float
        Accrued coupon amount.

    Raises
    ------
    ValueError
        If ``lower`` or ``upper`` is non-finite or ``lower >= upper``;
        ``observations`` is empty or contains a non-finite value; or
        ``coupon_rate`` or ``day_count_fraction`` is non-finite or negative.

    Examples
    --------
    >>> from finstack_quant.valuations import callable_range_accrual_accrued
    >>> callable_range_accrual_accrued(0.01, 0.03, [0.005, 0.02, 0.03, 0.04], 0.08, 0.25)
    0.01

    """
    ...
