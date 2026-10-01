"""Product-independent liquidity risk and market-impact models.

Series inputs (``returns``, ``volumes``) accept any sequence of floats,
including numpy arrays and pandas Series. Results are typed wrappers
(:class:`LvarBangiaScalar`, :class:`ImpactEstimate`,
:class:`ExecutionTrajectory`) with ``to_series()`` / ``to_dataframe()``,
``to_json()`` / ``from_json()`` and pickle support.

Examples
--------
>>> from finstack_quant.models.liquidity import days_to_liquidate
>>> days_to_liquidate(1_000_000, 250_000, 0.20)
20.0
"""

from __future__ import annotations

from typing import Any, Sequence

import pandas as pd

__all__ = [
    "AlmgrenChrissModel",
    "ExecutionTrajectory",
    "ImpactEstimate",
    "KyleLambdaModel",
    "LiquidityProfile",
    "LvarBangiaScalar",
    "TradeParams",
    "almgren_chriss_impact",
    "amihud_illiquidity",
    "days_to_liquidate",
    "kyle_lambda",
    "liquidity_tier",
    "lvar_bangia",
    "roll_effective_spread",
]

class LiquidityProfile:
    """Market microstructure snapshot for one instrument.

    Prices are in the instrument's native currency, volumes in
    shares/contracts, and ``spread_volatility`` is interpreted according to
    ``spread_volatility_kind`` (``"relative"`` = spread / mid, the Bangia
    convention; ``"absolute"`` = ask - bid in price units).

    Parameters
    ----------
    instrument_id : str
        Identifier of the instrument.
    mid : float
        Positive mid price.
    bid : float
        Positive best bid; must not exceed ``ask``.
    ask : float
        Positive best ask.
    avg_daily_volume : float
        Non-negative average daily volume in shares/contracts.
    avg_trade_size : float
        Non-negative average trade size in shares/contracts.
    spread_volatility : float
        Non-negative spread standard deviation; ``0.0`` when unavailable.
    spread_volatility_kind : str, default "relative"
        ``"relative"`` or ``"absolute"``.
    observation_days : int, default 20
        Trading-day window behind the volume and spread statistics.

    Raises
    ------
    ValueError
        If a price is non-positive, the market is crossed, a statistic is
        negative or non-finite, or ``spread_volatility_kind`` is unknown.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import LiquidityProfile
    >>> profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1_000_000.0, 500.0, 0.0002)
    >>> (round(profile.spread, 6), round(profile.relative_spread, 6), profile.spread_volatility_kind)
    (0.1, 0.001, 'relative')
    """

    def __init__(
        self,
        instrument_id: str,
        mid: float,
        bid: float,
        ask: float,
        avg_daily_volume: float,
        avg_trade_size: float,
        spread_volatility: float,
        spread_volatility_kind: str = "relative",
        observation_days: int = 20,
    ) -> None: ...
    @property
    def instrument_id(self) -> str:
        """
        Identifier of the instrument this profile describes.

        Returns
        -------
        str
            The caller-supplied instrument id; it is opaque to the model and used only for labelling results.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mid(self) -> float:
        """
        Mid price the profile is quoted around.

        Returns
        -------
        float
            Mid price in the instrument's native currency and price units (per share or per contract); impact costs are expressed relative to it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def bid(self) -> float:
        """
        Best (highest) bid price of the quoted market.

        Returns
        -------
        float
            Bid price in the instrument's native currency and price units; must not exceed :attr:`ask`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ask(self) -> float:
        """
        Best (lowest) ask price of the quoted market.

        Returns
        -------
        float
            Ask price in the instrument's native currency and price units; must be at least :attr:`bid`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def avg_daily_volume(self) -> float:
        """
        Average daily traded volume of the instrument.

        Returns
        -------
        float
            Volume in shares or contracts per trading day (not currency notional); it sets the participation rate used by the impact models.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def avg_trade_size(self) -> float:
        """
        Average size of a single trade in this instrument.

        Returns
        -------
        float
            Trade size in shares or contracts, used to scale the fixed portion of the spread cost.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spread_volatility(self) -> float:
        """
        Standard deviation of the quoted bid-ask spread.

        Returns
        -------
        float
            Spread volatility expressed either as a fraction of mid (when :attr:`spread_volatility_kind` is ``"relative"``) or in absolute price units (``"absolute"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spread_volatility_kind(self) -> str:
        """
        Unit convention that :attr:`spread_volatility` is quoted in.

        Returns
        -------
        str
            Either ``"relative"`` (fraction of mid) or ``"absolute"`` (price units); the constructor rejects any other string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def observation_days(self) -> int:
        """
        Length of the window the profile statistics were estimated over.

        Returns
        -------
        int
            Number of trading days of history behind the volume, trade-size and spread statistics; larger windows smooth the estimates.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spread(self) -> float:
        """
        Quoted bid-ask spread of the instrument.

        Returns
        -------
        float
            ``ask - bid`` in the instrument's native price units; non-negative for a well-formed quote.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def relative_spread(self) -> float:
        """
        Quoted spread normalised by the mid price.

        Returns
        -------
        float
            ``(ask - bid) / mid`` as a decimal fraction (``0.0005`` is 5 bp), independent of the price level.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def half_spread(self) -> float:
        """
        Cost of crossing from mid to the touch.

        Returns
        -------
        float
            ``(ask - bid) / 2`` in native price units: the immediate cost per unit of walking one side of the book.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def relative_spread_volatility(self) -> float:
        """
        Spread volatility restated in relative units regardless of input convention.

        Returns
        -------
        float
            The stored spread volatility as a fraction of mid; an ``"absolute"`` input is divided by :attr:`mid`, a ``"relative"`` one is returned unchanged.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with the profile fields.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import LiquidityProfile
        >>> p = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
        >>> LiquidityProfile.from_json(p.to_json()) == p
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> LiquidityProfile:
        """Deserialize a profile produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        LiquidityProfile
            The reconstructed profile.

        Raises
        ------
        ValueError
            If the payload is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import LiquidityProfile
        >>> p = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
        >>> LiquidityProfile.from_json(p.to_json()).instrument_id
        'ACME'
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class TradeParams:
    """Inputs to a market-impact calculation.

    Parameters
    ----------
    quantity : float
        Signed quantity to execute (positive = buy, negative = sell) in
        shares/contracts.
    horizon_days : float
        Positive execution horizon in trading days.
    daily_volatility : float
        Positive daily return volatility as a decimal (``0.02`` for 2%).
    profile : LiquidityProfile
        Market microstructure snapshot of the instrument.
    risk_aversion : float or None, default None
        Trajectory risk-aversion; ``None`` uses the model default (``1e-6``).
    reference_price : float or None, default None
        Arrival/decision price converting return-space volatility into
        currency; ``None`` falls back to ``profile.mid``.

    Notes
    -----
    The constructor does not raise; the values are stored as given and
    validated by the model that consumes them.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import LiquidityProfile, TradeParams
    >>> profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
    >>> params = TradeParams(10_000.0, 1.0, 0.02, profile)
    >>> params.effective_reference_price
    100.0
    """

    def __init__(
        self,
        quantity: float,
        horizon_days: float,
        daily_volatility: float,
        profile: LiquidityProfile,
        risk_aversion: float | None = None,
        reference_price: float | None = None,
    ) -> None: ...
    @property
    def quantity(self) -> float:
        """
        Order size to be executed over the horizon.

        Returns
        -------
        float
            Quantity in shares or contracts; positive to buy, negative to sell. Impact costs depend on its magnitude.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def horizon_days(self) -> float:
        """
        Length of the execution schedule.

        Returns
        -------
        float
            Horizon in trading days (fractional values allowed); a shorter horizon raises temporary impact and lowers timing risk.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def daily_volatility(self) -> float:
        """
        Return volatility used for the timing-risk term.

        Returns
        -------
        float
            Daily return volatility as a decimal (``0.02`` is 2% per day), not annualised and not in percent.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def profile(self) -> LiquidityProfile:
        """
        Market microstructure inputs used to cost the trade.

        Returns
        -------
        LiquidityProfile
            The :class:`LiquidityProfile` supplying mid, spread, volume and trade-size statistics for this order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def risk_aversion(self) -> float | None:
        """
        Optional risk-aversion coefficient for the mean-variance trade-off.

        Returns
        -------
        float | None
            The Almgren-Chriss risk-aversion parameter in inverse currency units, or ``None`` when the model's own default applies.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def reference_price(self) -> float | None:
        """
        Optional arrival price the cost is measured against.

        Returns
        -------
        float | None
            Reference price in native price units, or ``None`` to fall back to ``profile.mid``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def effective_reference_price(self) -> float:
        """
        Reference price actually used to convert costs into basis points.

        Returns
        -------
        float
            ``reference_price`` when it was supplied, otherwise ``profile.mid``, in native price units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with the trade parameters and embedded profile.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import LiquidityProfile, TradeParams
        >>> profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
        >>> TradeParams.from_json(TradeParams(10.0, 1.0, 0.02, profile).to_json()).quantity
        10.0
        """
        ...

    @staticmethod
    def from_json(json: str) -> TradeParams:
        """Deserialize trade parameters produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        TradeParams
            The reconstructed parameters.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import LiquidityProfile, TradeParams
        >>> profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
        >>> TradeParams.from_json(TradeParams(10.0, 1.0, 0.02, profile).to_json()).horizon_days
        1.0
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class ImpactEstimate:
    """Expected market-impact execution costs of one trade.

    ``permanent_impact``, ``temporary_impact`` and ``total_cost`` are costs
    in currency units (impact integrated over the executed quantity), not
    per-share price displacements; ``cost_bp`` is the total cost in basis
    points of notional and ``execution_risk`` the standard deviation of the
    cost.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import almgren_chriss_impact
    >>> est = almgren_chriss_impact(100_000, 1_000_000, 0.20, 5.0, 0.10, 0.20)
    >>> round(est.cost_bp, 2)
    56.62
    """

    @property
    def permanent_impact(self) -> float:
        """
        Cost attributed to the lasting price move the order causes.

        Returns
        -------
        float
            Permanent-impact cost in the instrument's currency, positive for a cost.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def temporary_impact(self) -> float:
        """
        Cost attributed to the transient price concession while trading.

        Returns
        -------
        float
            Temporary-impact cost in the instrument's currency, positive for a cost; it grows as the horizon shortens.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_cost(self) -> float:
        """
        Expected all-in execution shortfall of the order.

        Returns
        -------
        float
            Sum of the permanent and temporary components in the instrument's currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cost_bp(self) -> float:
        """
        Expected execution cost expressed as a rate.

        Returns
        -------
        float
            ``total_cost`` divided by ``|quantity| * effective_reference_price``, in basis points (``10.0`` is 0.10%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def execution_risk(self) -> float:
        """
        Timing risk of the execution schedule.

        Returns
        -------
        float
            Standard deviation of the realised cost in the instrument's currency, driven by ``daily_volatility`` and the horizon.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_series(self) -> pd.Series:
        """Return the five cost fields as a float Series named ``impact``.

        Returns
        -------
        pandas.Series
            Index ``permanent_impact``, ``temporary_impact``, ``total_cost``,
            ``cost_bp``, ``execution_risk``.

        Raises
        ------
        ValueError
            If the result cannot be converted to the tabular form.
        ImportError
            If pandas is not installed in the environment.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import almgren_chriss_impact
        >>> almgren_chriss_impact(100_000, 1_000_000, 0.20, 5.0, 0.10, 0.20).to_series().index.tolist()
        ['permanent_impact', 'temporary_impact', 'total_cost', 'cost_bp', 'execution_risk']
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Return a single-row DataFrame with the five cost columns.

        Returns
        -------
        pandas.DataFrame
            One row; columns in the order listed on :meth:`to_series`.

        Raises
        ------
        ValueError
            If the result cannot be converted to the tabular form.
        ImportError
            If pandas is not installed in the environment.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import almgren_chriss_impact
        >>> almgren_chriss_impact(100_000, 1_000_000, 0.20, 5.0, 0.10, 0.20).to_dataframe().shape
        (1, 5)
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with the five cost fields.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import ImpactEstimate, almgren_chriss_impact
        >>> est = almgren_chriss_impact(100_000, 1_000_000, 0.20, 5.0, 0.10, 0.20)
        >>> ImpactEstimate.from_json(est.to_json()) == est
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> ImpactEstimate:
        """Deserialize an estimate produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        ImpactEstimate
            The reconstructed estimate.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import ImpactEstimate
        >>> ImpactEstimate.from_json(
        ...     '{"permanent_impact":1.0,"temporary_impact":2.0,"total_cost":3.0,"cost_bp":0.3,"execution_risk":0.5}'
        ... ).total_cost
        3.0
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class ExecutionTrajectory:
    """Optimal execution schedule for a trade.

    ``time_points`` holds the ``num_buckets + 1`` bucket boundaries in
    trading days (starting at ``0.0``), ``remaining`` the inventory at each
    boundary and ``quantities`` the ``num_buckets`` per-bucket trades.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import KyleLambdaModel, LiquidityProfile, TradeParams
    >>> profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
    >>> traj = KyleLambdaModel(0.001).optimal_trajectory(TradeParams(1_000.0, 2.0, 0.02, profile), 4)
    >>> (len(traj.quantities), len(traj.remaining), traj.remaining[-1])
    (4, 5, 0.0)
    """

    @property
    def quantities(self) -> list[float]:
        """
        Per-bucket trade sizes of the optimal schedule.

        Returns
        -------
        list[float]
            One signed quantity per time bucket, in shares or contracts, in execution order; length equals ``num_buckets``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def remaining(self) -> list[float]:
        """
        Inventory left to trade at every bucket boundary.

        Returns
        -------
        list[float]
            Holdings in shares or contracts at each boundary, starting at the full order and ending at ``0.0``; length ``num_buckets + 1``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def time_points(self) -> list[float]:
        """
        Time grid the schedule is defined on.

        Returns
        -------
        list[float]
            Bucket boundaries as trading days from the start of execution, from ``0.0`` to ``horizon_days``; length ``num_buckets + 1``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expected_cost(self) -> float:
        """
        Expected implementation shortfall of the whole schedule.

        Returns
        -------
        float
            Expected cost in the instrument's currency, positive for a cost.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cost_variance(self) -> float:
        """
        Dispersion of the schedule's realised cost.

        Returns
        -------
        float
            Variance of the execution cost in squared currency units; its square root is the timing risk.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Return the schedule as a DataFrame with ``t``, ``holdings``, ``trade`` columns.

        Returns
        -------
        pandas.DataFrame
            One row per bucket boundary; ``trade`` is the quantity executed in
            the bucket ending at ``t`` (``0.0`` on the first row).

        Raises
        ------
        ValueError
            If the result cannot be converted to the tabular form.
        ImportError
            If pandas is not installed in the environment.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import KyleLambdaModel, LiquidityProfile, TradeParams
        >>> profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
        >>> traj = KyleLambdaModel(0.001).optimal_trajectory(TradeParams(1_000.0, 2.0, 0.02, profile), 4)
        >>> traj.to_dataframe().columns.tolist()
        ['t', 'holdings', 'trade']
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with the schedule arrays and cost statistics.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import (
        ...     ExecutionTrajectory,
        ...     KyleLambdaModel,
        ...     LiquidityProfile,
        ...     TradeParams,
        ... )
        >>> profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
        >>> traj = KyleLambdaModel(0.001).optimal_trajectory(TradeParams(1_000.0, 2.0, 0.02, profile), 2)
        >>> ExecutionTrajectory.from_json(traj.to_json()).quantities == traj.quantities
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> ExecutionTrajectory:
        """Deserialize a trajectory produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        ExecutionTrajectory
            The reconstructed schedule.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import ExecutionTrajectory
        >>> ExecutionTrajectory.from_json(
        ...     '{"quantities":[1.0],"remaining":[1.0,0.0],"expected_cost":0.0,"cost_variance":0.0,"time_points":[0.0,1.0]}'
        ... ).time_points
        [0.0, 1.0]
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class LvarBangiaScalar:
    """Bangia liquidity-adjusted VaR for one position (loss sign convention).

    ``var`` and ``lvar`` are non-positive loss numbers with ``lvar <= var``,
    ``spread_cost`` is the non-negative liquidity add-on and ``lvar_ratio``
    is ``lvar / var`` (``NaN`` when ``var == 0``).

    Examples
    --------
    >>> from finstack_quant.models.liquidity import lvar_bangia
    >>> result = lvar_bangia(-100.0, 0.01, 0.005, 0.99, 1_000_000)
    >>> round(result.lvar, 2)
    -10915.87
    """

    @property
    def var(self) -> float:
        """
        Market Value-at-Risk before any liquidity adjustment.

        Returns
        -------
        float
            The supplied market VaR in currency units, sign convention non-positive (a loss is negative).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spread_cost(self) -> float:
        """
        Bangia liquidity add-on charged on top of market VaR.

        Returns
        -------
        float
            Exogenous spread cost in currency units, non-negative, computed from the half spread and its volatility at the chosen confidence level.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def lvar(self) -> float:
        """
        Value-at-Risk including the liquidity cost of unwinding.

        Returns
        -------
        float
            ``var - spread_cost`` in currency units, non-positive under the same loss-negative convention as :attr:`var`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def lvar_ratio(self) -> float:
        """
        Multiple by which liquidity inflates the market VaR.

        Returns
        -------
        float
            ``lvar / var``, at least ``1.0`` when both are non-zero; ``NaN`` when ``var`` is exactly zero.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_series(self) -> pd.Series:
        """Return the four fields as a float Series named ``lvar``.

        Returns
        -------
        pandas.Series
            Index ``var``, ``spread_cost``, ``lvar``, ``lvar_ratio``.

        Raises
        ------
        ValueError
            If the result cannot be converted to the tabular form.
        ImportError
            If pandas is not installed in the environment.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import lvar_bangia
        >>> lvar_bangia(-100.0, 0.01, 0.005, 0.99, 1_000_000).to_series().index.tolist()
        ['var', 'spread_cost', 'lvar', 'lvar_ratio']
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Return a single-row DataFrame with the four columns.

        Read the VaR column as ``df["var"]``; attribute access resolves to
        ``DataFrame.var`` (the variance method).

        Returns
        -------
        pandas.DataFrame
            One row with columns ``var``, ``spread_cost``, ``lvar``, ``lvar_ratio``.

        Raises
        ------
        ValueError
            If the result cannot be converted to the tabular form.
        ImportError
            If pandas is not installed in the environment.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import lvar_bangia
        >>> lvar_bangia(-100.0, 0.01, 0.005, 0.99, 1_000_000).to_dataframe().shape
        (1, 4)
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with the four fields.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import LvarBangiaScalar, lvar_bangia
        >>> result = lvar_bangia(-100.0, 0.01, 0.005, 0.99, 1_000_000)
        >>> LvarBangiaScalar.from_json(result.to_json()) == result
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> LvarBangiaScalar:
        """Deserialize a result produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        LvarBangiaScalar
            The reconstructed result.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import LvarBangiaScalar
        >>> LvarBangiaScalar.from_json('{"var":-100.0,"spread_cost":5.0,"lvar":-105.0,"lvar_ratio":1.05}').lvar
        -105.0
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class AlmgrenChrissModel:
    """Almgren-Chriss (2000) market-impact model.

    Permanent impact is linear (``g(v) = gamma * v``); temporary impact
    follows the power law ``h(v) = eta * sign(v) * |v|**delta``.

    Parameters
    ----------
    gamma : float
        Non-negative permanent impact coefficient in price units per share.
    eta : float
        Positive temporary impact coefficient in price units per share.
    delta : float
        Power-law exponent in ``(0, 1]``; ``0.5``-``0.6`` is typical for
        equities and ``1.0`` selects the linear model required by
        :meth:`optimal_trajectory`.

    Raises
    ------
    ValueError
        If a coefficient is outside its documented range or non-finite.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import AlmgrenChrissModel
    >>> model = AlmgrenChrissModel(1e-7, 1e-4, 0.5)
    >>> (model.gamma, model.eta, model.delta)
    (1e-07, 0.0001, 0.5)
    """

    def __init__(self, gamma: float, eta: float, delta: float) -> None: ...
    @classmethod
    def from_profile(cls, profile: LiquidityProfile, daily_volatility: float) -> AlmgrenChrissModel:
        """Calibrate coefficients from a liquidity profile.

        ``gamma = spread / (2 * ADV)``, ``eta = daily_volatility * mid / sqrt(ADV)``
        and ``delta = 0.5``.

        Parameters
        ----------
        profile : LiquidityProfile
            Snapshot supplying spread, mid and average daily volume.
        daily_volatility : float
            Positive daily return volatility as a decimal.

        Returns
        -------
        AlmgrenChrissModel
            Calibrated model.

        Raises
        ------
        ValueError
            If ``daily_volatility`` is non-positive or the profile has zero
            average daily volume.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import AlmgrenChrissModel, LiquidityProfile
        >>> profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
        >>> AlmgrenChrissModel.from_profile(profile, 0.02).delta
        0.5
        """
        ...

    @property
    def gamma(self) -> float:
        """
        Almgren-Chriss permanent-impact coefficient.

        Returns
        -------
        float
            Price move per unit of quantity traded, in price units per share or contract; it scales the linear permanent-impact term.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def eta(self) -> float:
        """
        Almgren-Chriss temporary-impact coefficient.

        Returns
        -------
        float
            Scale of the transient price concession, in price units per unit trading rate; it multiplies the temporary-impact term.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def delta(self) -> float:
        """
        Exponent of the temporary-impact power law.

        Returns
        -------
        float
            Dimensionless exponent applied to the trading rate; ``1.0`` gives linear temporary impact, ``0.5`` a square-root law.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model_name(self) -> str:
        """
        Stable label identifying the impact model behind this estimate.

        Returns
        -------
        str
            Short model name used in diagnostics and result metadata; it is
            constant per model type and not user-configurable.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def estimate_cost(self, params: TradeParams) -> ImpactEstimate:
        """Expected execution cost of ``params`` under uniform execution.

        Parameters
        ----------
        params : TradeParams
            Trade size, horizon, volatility and liquidity profile.

        Returns
        -------
        ImpactEstimate
            Permanent/temporary/total cost, basis points and execution risk.

        Raises
        ------
        ValueError
            For non-finite or non-positive trade inputs.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import AlmgrenChrissModel, LiquidityProfile, TradeParams
        >>> profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
        >>> model = AlmgrenChrissModel.from_profile(profile, 0.02)
        >>> model.estimate_cost(TradeParams(10_000.0, 1.0, 0.02, profile)).total_cost > 0.0
        True
        """
        ...

    def optimal_trajectory(self, params: TradeParams, num_buckets: int) -> ExecutionTrajectory:
        """Cost-plus-risk optimal schedule over ``num_buckets`` intervals.

        Only defined for ``delta == 1.0`` (linear temporary impact).

        Parameters
        ----------
        params : TradeParams
            Trade inputs; ``risk_aversion`` weights the variance term.
        num_buckets : int
            Positive number of execution intervals.

        Returns
        -------
        ExecutionTrajectory
            Per-bucket trades, remaining inventory and cost statistics.

        Raises
        ------
        ValueError
            If ``num_buckets`` is zero, the trade inputs are invalid, or the
            model's ``delta`` is not ``1.0``.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import AlmgrenChrissModel, LiquidityProfile, TradeParams
        >>> profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
        >>> model = AlmgrenChrissModel(1e-7, 1e-4, 1.0)
        >>> len(model.optimal_trajectory(TradeParams(10_000.0, 2.0, 0.02, profile), 4).quantities)
        4
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with ``gamma``, ``eta`` and ``delta``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import AlmgrenChrissModel
        >>> model = AlmgrenChrissModel(1e-7, 1e-4, 0.5)
        >>> AlmgrenChrissModel.from_json(model.to_json()) == model
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> AlmgrenChrissModel:
        """Deserialize a model produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        AlmgrenChrissModel
            The reconstructed model.

        Raises
        ------
        ValueError
            If the payload is malformed or the impact parameters violate the
            constructor's finiteness and range requirements.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import AlmgrenChrissModel
        >>> AlmgrenChrissModel.from_json('{"gamma":0.0,"eta":0.001,"delta":1.0}').delta
        1.0
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class KyleLambdaModel:
    """Kyle (1985) linear price-impact model ``dP = lambda * signed_volume``.

    Parameters
    ----------
    lambda_ : float
        Non-negative price impact per unit of order flow, in price units per
        share/contract. Named ``lambda_`` because ``lambda`` is a Python
        keyword.

    Raises
    ------
    ValueError
        If ``lambda_`` is negative or non-finite.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import KyleLambdaModel
    >>> KyleLambdaModel(0.001).lambda_
    0.001
    """

    def __init__(self, lambda_: float) -> None: ...
    @classmethod
    def from_amihud(cls, amihud_ratio: float, reference_price: float) -> KyleLambdaModel:
        """Build from an Amihud ratio: ``lambda = amihud_ratio * reference_price``.

        Parameters
        ----------
        amihud_ratio : float
            Non-negative mean absolute return per unit volume.
        reference_price : float
            Positive price per share or contract.

        Returns
        -------
        KyleLambdaModel
            Price-space model.

        Raises
        ------
        ValueError
            If the ratio is negative/non-finite or the price is non-positive.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import KyleLambdaModel
        >>> KyleLambdaModel.from_amihud(0.0001, 50.0).lambda_
        0.005
        """
        ...

    @property
    def lambda_(self) -> float:
        """
        Kyle's lambda: market depth of the instrument.

        Returns
        -------
        float
            Price move in native price units per unit of signed order flow; larger values mean a thinner, more impactful market. Named ``lambda_`` because ``lambda`` is a Python keyword.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model_name(self) -> str:
        """
        Stable label identifying the impact model behind this estimate.

        Returns
        -------
        str
            Short model name used in diagnostics and result metadata; it is
            constant per model type and not user-configurable.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def estimate_cost(self, params: TradeParams) -> ImpactEstimate:
        """Expected execution cost ``0.5 * lambda * quantity**2`` plus timing risk.

        Parameters
        ----------
        params : TradeParams
            Trade size, horizon, volatility and liquidity profile.

        Returns
        -------
        ImpactEstimate
            Cost decomposition (all impact is permanent under Kyle).

        Raises
        ------
        ValueError
            For non-finite or non-positive trade inputs.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import KyleLambdaModel, LiquidityProfile, TradeParams
        >>> profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
        >>> KyleLambdaModel(0.001).estimate_cost(TradeParams(1_000.0, 1.0, 0.02, profile)).total_cost
        500.0
        """
        ...

    def optimal_trajectory(self, params: TradeParams, num_buckets: int) -> ExecutionTrajectory:
        """Uniform execution schedule over ``num_buckets`` intervals.

        Parameters
        ----------
        params : TradeParams
            Trade inputs.
        num_buckets : int
            Positive number of execution intervals.

        Returns
        -------
        ExecutionTrajectory
            Equal per-bucket trades with cost statistics.

        Raises
        ------
        ValueError
            If ``num_buckets`` is zero or trade inputs are invalid.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import KyleLambdaModel, LiquidityProfile, TradeParams
        >>> profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)
        >>> KyleLambdaModel(0.001).optimal_trajectory(TradeParams(1_000.0, 2.0, 0.02, profile), 4).quantities
        [250.0, 250.0, 250.0, 250.0]
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with ``lambda``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import KyleLambdaModel
        >>> KyleLambdaModel.from_json(KyleLambdaModel(0.001).to_json()).lambda_
        0.001
        """
        ...

    @staticmethod
    def from_json(json: str) -> KyleLambdaModel:
        """Deserialize a model produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        KyleLambdaModel
            The reconstructed model.

        Raises
        ------
        ValueError
            If the payload is malformed or ``lambda`` is negative or non-finite.

        Examples
        --------
        >>> from finstack_quant.models.liquidity import KyleLambdaModel
        >>> KyleLambdaModel.from_json('{"lambda":0.002}').lambda_
        0.002
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

def roll_effective_spread(returns: Sequence[float]) -> float | None:
    """Estimate Roll effective spread from an ordered return series.

    Parameters
    ----------
    returns : Sequence[float]
        Log or arithmetic decimal returns in time order; at least two values
        are required.

    Returns
    -------
    float | None
        Effective spread in return units, or ``None`` when the sample is too
        short or its serial covariance is non-negative.

    Notes
    -----
    This helper does not raise; unavailable estimates return ``None``.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import roll_effective_spread
    >>> roll_effective_spread([0.01, -0.01, 0.01, -0.01])
    0.02
    """
    ...

def amihud_illiquidity(returns: Sequence[float], volumes: Sequence[float]) -> float | None:
    """Compute Amihud illiquidity from aligned returns and volumes.

    Parameters
    ----------
    returns : Sequence[float]
        Ordered decimal period returns.
    volumes : Sequence[float]
        Strictly positive traded volumes aligned with ``returns``.

    Returns
    -------
    float | None
        Mean ``abs(return) / volume``, or ``None`` for empty, mismatched,
        non-finite, or non-positive-volume inputs.

    Notes
    -----
    This helper does not raise; unavailable estimates return ``None``.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import amihud_illiquidity
    >>> amihud_illiquidity([0.01, 0.02], [100.0, 200.0])
    0.0001
    """
    ...

def days_to_liquidate(
    position_quantity: float,
    adv: float,
    participation_rate: float,
) -> float:
    """Estimate a position's liquidation horizon in trading days.

    Parameters
    ----------
    position_quantity : float
        Shares or contracts to liquidate; the absolute value is used.
    adv : float
        Average daily volume in the same share or contract units.
    participation_rate : float
        Fraction of ADV available per trading day.

    Returns
    -------
    float
        ``abs(position_quantity) / (adv * participation_rate)``, or infinity
        when ADV or participation is non-positive.

    Notes
    -----
    This helper does not raise; invalid capacity produces infinity.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import days_to_liquidate
    >>> days_to_liquidate(1_000_000, 250_000, 0.20)
    20.0
    """
    ...

def liquidity_tier(
    days_to_liquidate: float,
    thresholds: tuple[float, float, float, float] | None = None,
) -> str:
    """Classify a liquidation horizon into a liquidity tier.

    Parameters
    ----------
    days_to_liquidate : float
        Estimated unwind horizon in trading days.
    thresholds : tuple[float, float, float, float] or None, default None
        Ascending tier boundaries ``(tier1_max, tier2_max, tier3_max,
        tier4_max)`` in trading days; ``None`` uses the Rust
        ``LiquidityConfig`` default ``(1.0, 5.0, 20.0, 60.0)``.

    Returns
    -------
    str
        One of ``"tier1"`` through ``"tier5"``, with Tier 1 the most liquid.

    Raises
    ------
    ValueError
        If ``thresholds`` is given but not strictly ascending or non-finite.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import liquidity_tier
    >>> (liquidity_tier(3.0), liquidity_tier(3.0, (0.5, 2.0, 10.0, 30.0)))
    ('tier2', 'tier3')
    """
    ...

def lvar_bangia(
    var: float,
    spread_mean: float,
    spread_vol: float,
    confidence: float,
    position_value: float,
) -> LvarBangiaScalar:
    """Compute Bangia liquidity-adjusted VaR under the loss-sign convention.

    Parameters
    ----------
    var : float
        Finite non-positive market VaR; ``-100`` denotes a loss of 100.
    spread_mean : float
        Finite non-negative mean relative bid-ask spread as a decimal.
    spread_vol : float
        Finite non-negative volatility of the relative spread.
    confidence : float
        Confidence level strictly between 0.5 and 1.
    position_value : float
        Finite position market value; only its magnitude is used.

    Returns
    -------
    LvarBangiaScalar
        ``var``, non-negative ``spread_cost``, adjusted ``lvar`` and
        ``lvar_ratio`` with ``to_series()`` / ``to_dataframe()`` exits.

    Raises
    ------
    ValueError
        If an input violates the stated finiteness, sign, or range contract.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import lvar_bangia
    >>> result = lvar_bangia(-100.0, 0.01, 0.005, 0.99, 1_000_000)
    >>> round(result.lvar, 2)
    -10915.87
    """
    ...

def almgren_chriss_impact(
    position_size: float,
    avg_daily_volume: float,
    volatility: float,
    execution_horizon_days: float,
    permanent_impact_coef: float,
    temporary_impact_coef: float,
    reference_price: float | None = None,
) -> ImpactEstimate:
    """Estimate uniform Almgren-Chriss execution-impact components.

    The impact coefficients are derived from ``avg_daily_volume`` with the
    same calibration as :meth:`AlmgrenChrissModel.from_profile` (20 bp
    proportional spread); ``permanent_impact_coef`` and
    ``temporary_impact_coef`` scale that base multiplicatively. Callers with
    externally calibrated absolute ``gamma`` / ``eta`` should build
    :class:`AlmgrenChrissModel` directly.

    Parameters
    ----------
    position_size : float
        Finite signed quantity in shares or contracts.
    avg_daily_volume : float
        Positive finite ADV in matching quantity units.
    volatility : float
        Positive finite daily volatility as a decimal.
    execution_horizon_days : float
        Positive finite execution horizon in trading days.
    permanent_impact_coef : float
        Non-negative finite multiplier on permanent impact.
    temporary_impact_coef : float
        Positive finite multiplier on temporary impact.
    reference_price : float | None, default None
        Optional positive finite price for notional and basis-point scaling;
        ``None`` uses unit price.

    Returns
    -------
    ImpactEstimate
        ``permanent_impact``, ``temporary_impact``, ``total_cost``,
        ``cost_bp`` and ``execution_risk`` with ``to_series()`` /
        ``to_dataframe()`` exits.

    Raises
    ------
    ValueError
        If an input violates the stated finiteness, sign, or range contract.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import almgren_chriss_impact
    >>> result = almgren_chriss_impact(100_000, 1_000_000, 0.20, 5.0, 0.10, 0.20)
    >>> round(result.cost_bp, 2)
    56.62
    """
    ...

def kyle_lambda(
    returns: Sequence[float],
    volumes: Sequence[float],
    reference_price: float,
) -> float | None:
    """Estimate price-space Kyle lambda from return and volume observations.

    Argument order matches :func:`amihud_illiquidity`: returns first, then
    volumes.

    Parameters
    ----------
    returns : Sequence[float]
        Finite decimal returns aligned one-for-one with ``volumes``.
    volumes : Sequence[float]
        Strictly positive finite volume observations in consistent units.
    reference_price : float
        Positive finite price per share or contract.

    Returns
    -------
    float | None
        Estimated price-space impact coefficient, or ``None`` for invalid
        samples or reference price.

    Notes
    -----
    This helper does not raise; unavailable estimates return ``None``.

    Examples
    --------
    >>> from finstack_quant.models.liquidity import kyle_lambda
    >>> kyle_lambda([0.01, -0.02], [100.0, 200.0], 50.0)
    0.005
    """
    ...
