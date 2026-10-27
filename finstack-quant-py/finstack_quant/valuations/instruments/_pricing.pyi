"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/pricing.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.dates import Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.composite import CompositeInstrument
from finstack_quant.valuations.instruments._instruments import Bond, TermLoan
from finstack_quant.valuations.instruments._typed_convertible import ConvertibleBond
from finstack_quant.valuations.instruments._typed_credit.cds import CreditDefaultSwap
from finstack_quant.valuations.instruments._typed_credit.cds_index import CdsIndex
from finstack_quant.valuations.instruments._typed_credit.cds_tranche import CdsTranche
from finstack_quant.valuations.instruments._typed_equity import EquityOption
from finstack_quant.valuations.instruments._typed_fx import FxForward, FxOption
from finstack_quant.valuations.instruments._typed_rates import CapFloor, InterestRateSwap, Swaption
from finstack_quant.valuations.instruments._typed_revolving_credit.revolving_credit import RevolvingCredit
from finstack_quant.valuations.instruments._typed_structured_credit.structured_credit import StructuredCredit

def price_instrument(
    instrument: str
    | dict[str, Any]
    | Bond
    | TermLoan
    | InterestRateSwap
    | Swaption
    | CapFloor
    | CreditDefaultSwap
    | CdsIndex
    | FxForward
    | FxOption
    | CdsTranche
    | ConvertibleBond
    | EquityOption
    | StructuredCredit
    | RevolvingCredit
    | CompositeInstrument,
    market: MarketContext | str,
    as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    model: str = "default",
    metrics: list[str] | None = None,
    metric_pricing_overrides: MetricPricingOverrides | dict[str, Any] | str | None = None,
    market_history: MarketHistory | dict[str, Any] | str | None = None,
    explain: bool = False,
) -> ValuationResult:
    """
    Price one instrument and compute explicit risk metric requests.

    Parameters
    ----------
    instrument : str or dict[str, Any] or Bond or TermLoan or InterestRateSwap or Swaption or CapFloor or CreditDefaultSwap or CdsIndex or FxForward or FxOption or CdsTranche or ConvertibleBond or EquityOption or StructuredCredit or CompositeInstrument
        Typed instrument instance (:class:`Bond`, :class:`TermLoan`,
        :class:`InterestRateSwap`, :class:`Swaption`, :class:`CapFloor`,
        :class:`CreditDefaultSwap`, :class:`CdsIndex`, :class:`FxForward`,
        :class:`FxOption`, :class:`CdsTranche`, :class:`ConvertibleBond`,
        :class:`EquityOption`, :class:`StructuredCredit`,
        :class:`~finstack_quant.valuations.composite.CompositeInstrument`) or a
        canonical ``finstack_quant.instrument/1`` envelope, as the dict
        :func:`instrument_envelope_from_spec` returns or as JSON text.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON.
    as_of : datetime.date | datetime.datetime | pd.Timestamp | str
        Valuation date, either a date-like object or an ISO 8601 string.
    model : str, default "default"
        Model key: ``"default"`` (the instrument's registered default),
        ``"discounting"``, ``"black76"``, ``"hazard_rate"``,
        ``"hull_white_1f"``, ``"tree"``, ``"rates_credit"``, ``"normal"``,
        ... — see :func:`list_models_grouped`. For bonds, ``"discounting"`` is
        non-callable rates-only PV, ``"hazard_rate"`` is non-callable
        fractional recovery of par,
        ``"tree"`` values rates-only exercise rights, and ``"rates_credit"``
        values joint rates-credit bonds including call, put, and return floors.
    metrics : list[str] or None, default None
        Metric IDs to compute, such as ``"ytm"``, ``"dv01"``,
        ``"duration_mod"``, ``"z_spread"``, ``"pv01"``, ``"bucketed_dv01"``,
        ``"hvar"`` or ``"expected_shortfall"`` when supported by the
        instrument (see :func:`list_standard_metrics`). ``None`` or ``[]``
        means valuation only. Mortgage OAS and CMO Z-spread take clean prices
        per 100 of current face and add settlement accrued interest. MBS
        ``dv01`` and ``bucketed_dv01`` include the same rate-dependent
        prepayments as ``duration_mod``. FI TRS ``duration_dv01`` requires
        ``duration_id`` and a finite signed duration scalar in years.
        Roll specialness is in basis points against the forward curve
        ``repo_curve_id``, or the discount curve when absent; implied
        financing is an ACT/360 decimal rate.
    metric_pricing_overrides : MetricPricingOverrides or dict or str, optional
        Metric-time overrides merged into
        ``instrument.spec.metric_pricing_overrides`` before pricing: ``theta_period`` (a tenor; in
        dict/JSON form ``{"count": 1, "unit": "weeks"}``), ``breakeven_config``
        (``{"target": "z_spread", "mode": "linear"}``), ``bump_config``,
        ``bond_risk_basis``, ``theta_day_basis``, ``var_config``. A dict or
        JSON string is accepted in place of the typed object.
        Dict and JSON patches retain omitted fields, including individual
        ``bump_config`` fields; explicit ``None``/``null`` clears optional fields.
    market_history : MarketHistory or dict or str, optional
        Historical scenarios required by the ``"hvar"`` and
        ``"expected_shortfall"`` metrics; a dict or JSON string is accepted in
        place of the typed object.
    explain : bool, default False
        When ``True``, attach a step trace as ``result.explanation``. For the
        ``"discounting"`` and ``"hazard_rate"`` models it holds one
        ``cashflow_pv`` entry per projected cashflow (date, amount, discount
        factor, curve, survival probability, present value) that sums to the
        price, capped at 1000 entries. Other models get one
        ``computation_step`` entry saying no per-flow decomposition exists.

    Returns
    -------
    ValuationResult
        Typed valuation envelope including the requested metric values. A
        stochastic ``"rates_credit"`` bond result also carries Monte Carlo
        convergence and reproducibility diagnostics in ``details``.
        ``result.provenance`` records the model, requested date, declared
        market dependencies, scenario adjustment and, when metrics are
        requested, the finite-difference bump sizes.

    Raises
    ------
    KeyError
        If a curve, surface, fixing series or scalar the instrument depends
        on is missing from ``market``.
    ValueError
        If the instrument, market, date or option payloads are malformed, a
        metric is unknown or not applicable to the instrument, or the
        instrument fails validation for the requested model (for example a
        seasoned floating leg without a ``FIXING:<index>`` series).
    RuntimeError
        If the model or a metric solver fails numerically, or instrument pricing
        wraps a failure with instrument/model context. This includes missing
        quanto inputs, asset-currency mismatches, and an analytical barrier model
        requested for discrete monitoring; the message retains the cause.
    TypeError
        If ``instrument`` is not a typed instrument, an envelope dict or a string, or
        ``market`` is neither a ``MarketContext`` nor a string.

    Notes
    -----
    When stochastic rate or credit factors are configured for a
    ``"rates_credit"`` bond, ``result.details`` is tagged
    ``{"type": "monte_carlo", "data": ...}``. Its data contains the
    sampling-only standard error, configured independent exercise-policy paths
    (``training_paths``), configured independent make-whole-reference paths
    (``make_whole_training_paths``), the corresponding simulated-path counts
    including antithetic partners, independent estimator paths and their total
    simulated count, random seed, simulation time grid, and variance-reduction
    flags. The standard error measures pricing-path sampling
    uncertainty under the frozen fitted exercise policy and excludes regression
    approximation, time-grid discretization, and model error.

    The wire payload is still one call away: ``result.to_json()`` returns the
    JSON that :meth:`ValuationResult.from_json` accepts, for pipelines that
    serialize results.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.valuations.instruments import TermLoan
    >>> loan = TermLoan.example()
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", datetime.date(2024, 1, 1), 0.04))
    >>> from finstack_quant.valuations.instruments import price_instrument
    >>> result = price_instrument(loan, market, "2024-01-01", metrics=["all_in_rate"])
    >>> (list(result.metrics), round(result.get_metric("all_in_rate"), 4))
    (['all_in_rate'], 0.06)

    """
    ...

def list_models() -> list[str]:
    """
    Return every pricing model key registered in the standard pricer registry.

    The list is registry-derived rather than enum-derived, so it reflects real
    dispatch coverage: a model with no registered pricer is omitted. The names
    are the canonical keys accepted by the ``model`` argument of
    :func:`price_instrument`.

    Returns
    -------
    list[str]
        Canonical model keys such as ``"discounting"`` or ``"rates_credit"``,
        deduplicated and sorted.

    Notes
    -----
    This method does not raise; it returns the stored or derived value.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import list_models
    >>> models = list_models()
    >>> all(model in models for model in ("discounting", "hazard_rate", "tree", "rates_credit"))
    True
    """
    ...

def list_models_grouped() -> dict[str, list[str]]:
    """
    Return the standard registry's pricing models grouped by instrument type.

    Only instrument types with at least one registered pricer appear as keys,
    and each entry lists only the models that can price that instrument.

    Returns
    -------
    dict[str, list[str]]
        Mapping from canonical instrument-type name to its sorted model keys.

    Notes
    -----
    This method does not raise; it returns the stored or derived value.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import list_models_grouped
    >>> grouped = list_models_grouped()
    >>> set(("discounting", "hazard_rate", "tree", "rates_credit")).issubset(grouped["bond"])
    True
    """
    ...

def list_standard_metrics() -> list[str]:
    """
    Return all standard metric IDs registered by the Rust valuation engine.

    Returns
    -------
    list[str]
        Sorted list of fully qualified metric keys.

    Notes
    -----
    This method does not raise; it returns the stored or derived value.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import list_standard_metrics
    >>> metrics = list_standard_metrics()
    >>> ("dirty_price" in metrics, "dv01" in metrics)
    (True, True)
    """
    ...

def list_standard_metrics_grouped() -> dict[str, list[str]]:
    """
    Return standard metric IDs grouped by human-readable category.

    Returns
    -------
    dict[str, list[str]]
        Mapping from group label to sorted metric ID lists.

    Notes
    -----
    This method does not raise; it returns the stored or derived value.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import list_standard_metrics_grouped
    >>> grouped = list_standard_metrics_grouped()
    >>> ("Credit" in grouped, "Rates" in grouped)
    (True, True)
    """
    ...

def metric_metadata(keys: list[str]) -> list[dict[str, Any]]:
    """
    Describe canonical metric keys using Rust-owned units and coordinates.

    Parameters
    ----------
    keys : list[str]
        Canonical scalar or qualified wire keys; input order and duplicates
        are retained and qualified coordinates require the native composite
        escaping. Custom names are accepted with unknown units and no group.

    Returns
    -------
    list[dict[str, Any]]
        One record per key with ``key``, ``metric``, ``components`` (decoded
        coordinate labels in original order), ``unit`` (canonical unit family
        string), ``group`` (native display group or ``None``) and
        ``bucketed``. No prices are changed and a unit family does not imply
        a bump size or FX conversion.

    Raises
    ------
    ValueError
        If a key uses malformed or obsolete composite encoding.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import metric_metadata
    >>> metric_metadata(["ytm"])[0]["unit"]
    'decimal'
    """
    ...

class MetricPricingOverrides:
    """
    Metric-time pricing overrides merged into an instrument before pricing.

    Typed twin of the ``metric_pricing_overrides`` JSON accepted by
    :func:`price_instrument`, typed ``.price()`` and
    :func:`validate_instrument_json`. Every field mirrors the Rust
    ``MetricPricingOverrides`` struct; omitted fields keep the instrument's
    own overrides. Instances are immutable and compare by value.

    Parameters
    ----------
    bump_config : dict[str, Any], optional
        Finite-difference bump sizes: ``spot_bump_decimal`` (``0.01`` = 1%),
        ``vol_bump_decimal`` (absolute vol, ``0.01`` = 1 vol point),
        ``rate_bump_bp``, ``credit_spread_bump_bp``, ``ytm_bump_bp`` (basis
        points) and ``adaptive_bumps`` (bool). ``None`` keeps the defaults.
    theta_period : Tenor | str, optional
        Theta / carry horizon as a ``Tenor`` or tenor string (``"1D"``,
        ``"1W"``, ``"1M"``, ``"3M"``); day and week tenors roll fixed days,
        month and year tenors roll calendar months. The default horizon is
        one day. The JSON form is ``{"count": 1, "unit": "weeks"}``.
    breakeven_config : dict[str, Any], optional
        Breakeven solve configuration such as
        ``{"target": "z_spread", "mode": "linear"}``.
    bond_risk_basis : {"bullet_discountable", "callable_oas"}, optional
        Basis for bond duration/convexity/DV01: Bloomberg-style workout
        risk (default) or callable OAS repricing.
    theta_day_basis : {"calendar_365", "trading_252"}, optional
        Day basis for per-day analytic option theta (EquityOption,
        FxOption, FxDigitalOption): annual theta / 365 (default) or / 252.
    var_config : dict[str, Any], optional
        Historical VaR / expected-shortfall configuration override
        (confidence level, horizon, decay).

    Raises
    ------
    ValueError
        If a sub-document is malformed, ``bond_risk_basis`` or
        ``theta_day_basis`` is not one of the accepted names, or
        ``theta_period`` is not a valid positive tenor.
    TypeError
        If ``theta_period`` is neither a ``Tenor`` nor a string.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import MetricPricingOverrides
    >>> opts = MetricPricingOverrides(theta_period="1W", bond_risk_basis="callable_oas")
    >>> (str(opts.theta_period), opts.bond_risk_basis)
    ('1W', 'callable_oas')
    >>> MetricPricingOverrides.from_json(opts.to_json()) == opts
    True
    """

    def __init__(
        self,
        *,
        bump_config: dict[str, Any] | None = None,
        theta_period: Tenor | str | None = None,
        breakeven_config: dict[str, Any] | None = None,
        bond_risk_basis: Literal["bullet_discountable", "callable_oas"] | None = None,
        theta_day_basis: Literal["calendar_365", "trading_252"] | None = None,
        var_config: dict[str, Any] | None = None,
    ) -> None:
        """
        Build metric-time overrides from keyword fields.

        Parameters
        ----------
        bump_config : dict[str, Any], optional
            Finite-difference bump sizes: ``spot_bump_decimal`` (``0.01`` = 1%),
            ``vol_bump_decimal`` (absolute vol, ``0.01`` = 1 vol point),
            ``rate_bump_bp``, ``credit_spread_bump_bp``, ``ytm_bump_bp`` (basis
            points) and ``adaptive_bumps`` (bool). ``None`` keeps the defaults.
        theta_period : Tenor | str, optional
            Theta / carry horizon as a ``Tenor`` or tenor string (``"1D"``,
            ``"1W"``, ``"1M"``, ``"3M"``); day and week tenors roll fixed days,
            month and year tenors roll calendar months. The default horizon is
            one day. The JSON form is ``{"count": 1, "unit": "weeks"}``.
        breakeven_config : dict[str, Any], optional
            Breakeven solve configuration such as
            ``{"target": "z_spread", "mode": "linear"}``.
        bond_risk_basis : {"bullet_discountable", "callable_oas"}, optional
            Basis for bond duration/convexity/DV01: Bloomberg-style workout
            risk (default) or callable OAS repricing.
        theta_day_basis : {"calendar_365", "trading_252"}, optional
            Day basis for per-day analytic option theta (EquityOption,
            FxOption, FxDigitalOption): annual theta / 365 (default) or / 252.
        var_config : dict[str, Any], optional
            Historical VaR / expected-shortfall configuration override
            (confidence level, horizon, decay).

        Raises
        ------
        ValueError
            If a sub-document is malformed, ``bond_risk_basis`` or
            ``theta_day_basis`` is not one of the accepted names, or
            ``theta_period`` is not a valid positive tenor.
        TypeError
            If ``theta_period`` is neither a ``Tenor`` nor a string.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MetricPricingOverrides
        >>> str(MetricPricingOverrides(theta_period="1M").theta_period)
        '1M'
        """
        ...

    @property
    def bump_config(self) -> dict[str, Any]:
        """
        Finite-difference bump configuration.

        Returns
        -------
        dict[str, Any]
            Bump-size document; an empty dict when every size is defaulted.

        Raises
        ------
        ValueError
            If the configuration cannot be serialized to a Python object.
        """
        ...

    @property
    def theta_period(self) -> Tenor | None:
        """
        Theta / carry horizon.

        Returns
        -------
        Tenor or None
            Horizon such as ``Tenor("1W")``, or ``None`` for the one-day default.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def breakeven_config(self) -> dict[str, Any] | None:
        """
        Breakeven solve configuration.

        Returns
        -------
        dict[str, Any] or None
            Configuration document, or ``None`` when breakeven is not requested.

        Raises
        ------
        ValueError
            If the configuration cannot be serialized to a Python object.
        """
        ...

    @property
    def bond_risk_basis(self) -> Literal["bullet_discountable", "callable_oas"] | None:
        """
        Basis for bond duration, convexity and DV01-style metrics.

        Returns
        -------
        {"bullet_discountable", "callable_oas"} or None
            Serde name of the basis, or ``None`` for the default
            (``"bullet_discountable"``).

        Raises
        ------
        ValueError
            If the basis cannot be rendered as its serde name.
        """
        ...

    @property
    def theta_day_basis(self) -> Literal["calendar_365", "trading_252"] | None:
        """
        Day basis for per-day analytic option theta.

        Returns
        -------
        {"calendar_365", "trading_252"} or None
            Serde name of the basis, or ``None`` for the calendar-day default.

        Raises
        ------
        ValueError
            If the basis cannot be rendered as its serde name.
        """
        ...

    @property
    def var_config(self) -> dict[str, Any] | None:
        """
        Historical VaR configuration override.

        Returns
        -------
        dict[str, Any] or None
            Configuration document, or ``None`` when defaults apply.

        Raises
        ------
        ValueError
            If the configuration cannot be serialized to a Python object.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MetricPricingOverrides:
        """
        Deserialize overrides from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`; unknown fields are
            rejected.

        Returns
        -------
        MetricPricingOverrides
            Parsed overrides.

        Raises
        ------
        ValueError
            If ``json`` is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MetricPricingOverrides
        >>> str(MetricPricingOverrides.from_json('{"theta_period": {"count": 1, "unit": "weeks"}}').theta_period)
        '1W'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize these overrides to compact JSON.

        Returns
        -------
        str
            JSON document accepted by :meth:`from_json` and by the
            ``metric_pricing_overrides`` argument of :func:`price_instrument`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Value equality on every field.

        Parameters
        ----------
        other : object
            Any object; non-``MetricPricingOverrides`` values compare unequal.

        Returns
        -------
        bool
            Whether all fields match.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-style constructor form of this value.

        Returns
        -------
        str
            ``MetricPricingOverrides(bump_config=None, ..., theta_period='1W', ...)`` text.
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

class MarketHistory:
    """
    Historical market shifts for historical VaR / expected shortfall.

    Typed twin of the ``market_history`` JSON accepted by
    :func:`price_instrument`. Each scenario is one historical date carrying a
    list of risk-factor shifts relative to the base market; the ``"hvar"``
    and ``"expected_shortfall"`` metrics revalue the instrument under every
    scenario.

    All three constructor arguments are required; ``window_days=0`` and
    ``scenarios=[]`` represent an empty history.

    Parameters
    ----------
    base_date : datetime.date | datetime.datetime | pd.Timestamp | str
        Reference date of the base market the shifts are relative to.
    window_days : int
        Length of the historical lookback window in calendar days.
    scenarios : list[dict[str, Any]]
        Chronological scenarios, each ``{"date": "YYYY-MM-DD", "shifts":
        [{"factor": {...}, "shift": float}, ...]}``. ``factor`` is a
        tagged risk factor: ``{"type": "discount_rate" | "forward_rate" |
        "credit_spread", "curve_id": str, "tenor_years": float}``,
        ``{"type": "equity_spot", "ticker": str}``, ``{"type": "fx_spot",
        "base": "EUR", "quote": "USD"}`` or ``{"type": "implied_vol",
        "vol_surface_id": str, "expiry_years": float, "strike": float}``.
        Rate and spread shifts are decimal (``0.0015`` = 15bp); spot
        shifts are relative (``-0.025`` = -2.5%); vol shifts are absolute
        vol points.

    Raises
    ------
    ValueError
        If a scenario document is malformed or carries unknown fields.
    TypeError
        If ``base_date`` is not a date-like value.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import MarketHistory
    >>> history = MarketHistory(
    ...     "2024-01-01",
    ...     2,
    ...     [
    ...         {
    ...             "date": "2023-12-29",
    ...             "shifts": [
    ...                 {
    ...                     "factor": {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0},
    ...                     "shift": 0.0010,
    ...                 }
    ...             ],
    ...         },
    ...         {
    ...             "date": "2023-12-28",
    ...             "shifts": [
    ...                 {
    ...                     "factor": {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0},
    ...                     "shift": -0.0005,
    ...                 }
    ...             ],
    ...         },
    ...     ],
    ... )
    >>> (len(history), history.window_days, history.to_dataframe()["shift"].tolist())
    (2, 2, [0.001, -0.0005])
    """

    def __init__(
        self,
        base_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        window_days: int,
        scenarios: list[dict[str, Any]],
    ) -> None:
        """
        Build a market history from scenario documents.

        Parameters
        ----------
        base_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Reference date of the base market the shifts are relative to.
        window_days : int
            Length of the historical lookback window in calendar days.
        scenarios : list[dict[str, Any]]
            Chronological scenarios, each ``{"date": "YYYY-MM-DD", "shifts":
            [{"factor": {...}, "shift": float}, ...]}``. ``factor`` is a
            tagged risk factor: ``{"type": "discount_rate" | "forward_rate" |
            "credit_spread", "curve_id": str, "tenor_years": float}``,
            ``{"type": "equity_spot", "ticker": str}``, ``{"type": "fx_spot",
            "base": "EUR", "quote": "USD"}`` or ``{"type": "implied_vol",
            "vol_surface_id": str, "expiry_years": float, "strike": float}``.
            Rate and spread shifts are decimal (``0.0015`` = 15bp); spot
            shifts are relative (``-0.025`` = -2.5%); vol shifts are absolute
            vol points.

        Raises
        ------
        ValueError
            If a scenario document is malformed or carries unknown fields.
        TypeError
            If ``base_date`` is not a date-like value.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MarketHistory
        >>> len(MarketHistory("2024-01-01", 0, []))
        0
        """
        ...

    @staticmethod
    def from_dict(data: dict[str, Any]) -> MarketHistory:
        """
        Build from a plain ``dict`` with keys ``base_date``, ``window_days`` and ``scenarios``.

        Parameters
        ----------
        data : dict[str, Any]
            Same document shape as :meth:`to_json` emits, as a Python dict.

        Returns
        -------
        MarketHistory
            Parsed history.

        Raises
        ------
        ValueError
            If the document is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MarketHistory
        >>> MarketHistory.from_dict({"base_date": "2024-01-01", "window_days": 0, "scenarios": []}).window_days
        0
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Reference date of the base market.

        Returns
        -------
        datetime.date
            Base date the shifts are relative to.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def window_days(self) -> int:
        """
        Historical window length.

        Returns
        -------
        int
            Lookback window in calendar days.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scenarios(self) -> list[dict[str, Any]]:
        """
        Scenario documents in chronological order.

        Returns
        -------
        list[dict[str, Any]]
            ``{"date": ..., "shifts": [...]}`` documents.

        Raises
        ------
        ValueError
            If the scenarios cannot be serialized to Python objects.
        """
        ...

    def __len__(self) -> int:
        """
        Number of scenarios.

        Returns
        -------
        int
            Scenario count.

        Notes
        -----
        This method does not raise.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per risk-factor shift as a tidy ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``date`` (ISO 8601 string), ``type`` (risk-factor tag),
            ``curve_id``, ``tenor_years``, ``ticker``, ``base``, ``quote``,
            ``vol_surface_id``, ``expiry_years``, ``strike`` (``NaN``/``None``
            where the factor type has no such coordinate) and ``shift``.

        Raises
        ------
        ValueError
            If the rows cannot be serialized into a pandas object.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MarketHistory:
        """
        Deserialize a market history from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        MarketHistory
            Parsed history.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MarketHistory
        >>> MarketHistory.from_json('{"base_date": "2024-01-01", "window_days": 0, "scenarios": []}').window_days
        0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this history to compact JSON.

        Returns
        -------
        str
            JSON document accepted by :meth:`from_json` and by the
            ``market_history`` argument of :func:`price_instrument`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-style constructor form of this value.

        Returns
        -------
        str
            ``MarketHistory(base_date=2024-01-01, window_days=2, scenarios=<2 items>)`` text.
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

def instrument_cashflows(
    instrument: Any,
    market: MarketContext | str,
    as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    model: str,
) -> InstrumentCashflowEnvelope:
    """
    Per-flow cashflow envelope (DF / survival / PV) for a discountable instrument.

    The Rust
    ``instrument_cashflows`` export, returned as an
    :class:`InstrumentCashflowEnvelope`. Hazard-rate export rejects bonds with
    call, put, or return-floor rights because static rows cannot represent
    their exercise-contingent value. For supported static-flow combinations
    ``total_pv`` reconciles with the instrument's ``base_value``.

    Parameters
    ----------
    instrument : Bond | TermLoan | InterestRateSwap | CdsOption | ... | dict | str
        Typed instrument instance or a canonical
        ``finstack_quant.instrument/1`` envelope, as the dict
        :func:`instrument_envelope_from_spec` returns or as JSON text.
    market : MarketContext | str
        Typed ``MarketContext`` or serialized market-context JSON.
    as_of : datetime.date | datetime.datetime | pd.Timestamp | str
        Valuation date, either a date-like object or an ISO 8601 string.
    model : str
        Must be ``"discounting"`` or ``"hazard_rate"``. ``"default"`` is not
        accepted on cashflow export.

    Returns
    -------
    InstrumentCashflowEnvelope
        Header fields plus one ``flows`` row per cashflow.

    Raises
    ------
    TypeError
        If ``instrument`` is not a typed instrument, an envelope dict or a JSON string.
    KeyError
        If a curve or fixing series the instrument depends on is missing
        from ``market``.
    ValueError
        If ``model`` is unsupported, the instrument/model pair is not
        registered for cashflow export, a bond with embedded exercise rights
        is requested under a static cashflow model, or a payload is malformed.
    RuntimeError
        If the pricer fails numerically.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import StubKind
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.core.types import Rate
    >>> from finstack_quant.valuations.instruments import Bond, instrument_cashflows
    >>> as_of = datetime.date(2024, 1, 1)
    >>> bond = Bond.fixed(
    ...     "B", Money(1000.0, Currency("USD")), Rate(0.05), as_of, datetime.date(2026, 1, 1), StubKind.NONE, "USD-OIS"
    ... )
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
    >>> envelope = instrument_cashflows(bond, market, as_of, "discounting")
    >>> (envelope.instrument_id, len(envelope.to_dataframe()))
    ('B', 6)
    """
    ...

class InstrumentCashflowEnvelope:
    """
    Cashflow export for one instrument: header fields plus per-flow rows.

    Typed wrapper of the Rust ``InstrumentCashflowEnvelope`` returned by
    :func:`instrument_cashflows`. ``flows`` holds one dict per cashflow;
    :meth:`InstrumentCashflowEnvelope.to_dataframe` is the tabular view.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import InstrumentCashflowEnvelope
    >>> envelope = InstrumentCashflowEnvelope.from_json(
    ...     '{"instrument_id": "B", "currency": "USD", "model": "discounting",'
    ...     ' "as_of": "2025-01-15", "discount_curve_id": "USD-OIS", "flows": [],'
    ...     ' "total_pv": 0.0, "reconciles_with_base_value": true}'
    ... )
    >>> (envelope.instrument_id, envelope.currency, len(envelope.to_dataframe()))
    ('B', 'USD', 0)
    """

    @property
    def instrument_id(self) -> str:
        """
        Instrument identifier.

        Returns
        -------
        str
            The exported instrument's id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def currency(self) -> str:
        """
        Reporting currency of the row PVs and ``total_pv``.

        Returns
        -------
        str
            ISO-4217 currency code.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def model(self) -> str:
        """
        Model key used for the export.

        Returns
        -------
        str
            ``"discounting"`` or ``"hazard_rate"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def as_of(self) -> datetime.date:
        """
        Valuation date of the export.

        Returns
        -------
        datetime.date
            The valuation date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve used for every row.

        Returns
        -------
        str
            Discount curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def credit_curve_id(self) -> str | None:
        """
        Hazard curve used under ``hazard_rate``.

        Returns
        -------
        str | None
            The curve id, or ``None`` under ``"discounting"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def recovery_rate(self) -> float | None:
        """
        Recovery rate of the hazard curve.

        Returns
        -------
        float | None
            Recovery as a decimal, or ``None`` under ``"discounting"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def scenario_price_shock_decimal(self) -> float | None:
        """
        Scenario price shock applied to every row ``pv`` and to ``total_pv``.

        Returns
        -------
        float | None
            Shock as a decimal (``-0.10`` multiplies by 0.90), or ``None``
            when the rows are unshocked.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def flows(self) -> list[dict[str, Any]]:
        """
        Per-flow rows (serde form of ``CashflowRow``).

        Returns
        -------
        list[dict[str, Any]]
            One dict per cashflow, in schedule order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def total_pv(self) -> float:
        """
        Sum of the row PVs in the envelope currency.

        Returns
        -------
        float
            Total present value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def reconciles_with_base_value(self) -> bool:
        """
        Whether ``total_pv`` agrees with the instrument's ``base_value``.

        Returns
        -------
        bool
            ``True`` for every successful export.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per cashflow as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``date``, ``amount``, ``currency``, ``kind``,
            ``accrual_factor``, ``accrual_start``, ``accrual_end``,
            ``accrual_day_count``, ``accrual_notional`` (outstanding principal
            at accrual start), ``index_rate`` (floating index before spread,
            gearing, caps and floors), ``principal_delta``, ``year_fraction``,
            ``rate``, ``reset_date``, ``discount_factor``,
            ``discount_curve_id``, ``survival_probability``,
            ``conditional_default_prob``, ``inflation_index_ratio``,
            ``prepayment_smm``, ``beginning_balance``, ``ending_balance``,
            ``native_pv`` (row currency, before FX and scenario shock),
            ``fx_rate`` (reporting per row currency) and ``pv``. The four date
            columns are ``datetime64`` and fields the model does not populate
            are null. A coupon is ``accrual_notional * rate * accrual_factor``
            when the balance is constant over its accrual period.

        Raises
        ------
        ImportError
            If pandas is not installed.
        """
        ...
    @staticmethod
    def from_json(json: str) -> InstrumentCashflowEnvelope:
        """
        Deserialize an envelope from ``to_json`` output.

        Parameters
        ----------
        json : str
            Serialized ``InstrumentCashflowEnvelope``.

        Returns
        -------
        InstrumentCashflowEnvelope
            The parsed envelope.

        Raises
        ------
        ValueError
            If ``json`` is malformed or misses a required field.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InstrumentCashflowEnvelope
        >>> text = (
        ...     '{"instrument_id": "B", "currency": "USD", "model": "discounting",'
        ...     ' "as_of": "2025-01-15", "discount_curve_id": "USD-OIS", "flows": [],'
        ...     ' "total_pv": 0.0, "reconciles_with_base_value": true}'
        ... )
        >>> InstrumentCashflowEnvelope.from_json(text).model
        'discounting'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to compact JSON, for storage or transport.

        Returns
        -------
        str
            The envelope JSON.

        Raises
        ------
        ValueError
            If the envelope cannot be serialized.
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
