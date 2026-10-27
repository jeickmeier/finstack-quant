"""Stub declarations mirroring ``finstack-quant-py/src/bindings/covenants/forecast.rs``."""

from __future__ import annotations

import datetime
import pandas as pd
from finstack_quant.covenants import DateLike
from finstack_quant.covenants._engine import CovenantEngine
from finstack_quant.covenants._spec import CovenantSpec

class CovenantForecastConfig:
    """
    Forecast policy for :func:`forecast_covenant` / :func:`forecast_breaches`.

    Deterministic by default (breach probability ``0`` or ``1`` per date).
    With ``stochastic=True`` a lognormal overlay with ``volatility`` scales
    shocks by ``sqrt(T)`` from ``reference_date``, which a stochastic
    :func:`forecast_covenant` / :func:`forecast_breaches` requires (a
    date-indexed frame does not say where its first reporting period starts):
    ``num_paths=0`` is the closed-form analytic
    mode, ``num_paths>0`` is Monte Carlo (deterministic for a given
    ``random_seed``).

    Examples
    --------
    >>> from finstack_quant.covenants import CovenantForecastConfig
    >>> cfg = CovenantForecastConfig(stochastic=True, volatility=0.25, reference_date="2025-12-31")
    >>> cfg.stochastic, cfg.num_paths, cfg.breach_probability_threshold
    (True, 0, 0.05)
    """

    def __init__(
        self,
        stochastic: bool = False,
        num_paths: int = 0,
        volatility: float | None = None,
        random_seed: int | None = None,
        antithetic: bool = False,
        reference_date: DateLike | None = None,
        breach_probability_threshold: float = 0.05,
    ) -> None:
        """
        Create a forecast configuration.

        Parameters
        ----------
        stochastic : bool
            Use the lognormal stochastic overlay instead of deterministic
            pass/fail probabilities.
        num_paths : int
            Monte Carlo path count; ``0`` selects analytic mode. MC requires at
            least two independent samples, or two antithetic pairs.
        volatility : float | None
            Annualized lognormal volatility of the metric; required when
            ``stochastic`` is true.
        random_seed : int | None
            Seed for Monte Carlo mode; ``None`` uses the crate default ``0``.
        antithetic : bool
            Simulate Monte Carlo paths in ``(Z, -Z)`` pairs; requires
            ``num_paths > 0``.
        reference_date : datetime.date | str | None
            Anchor for ``sqrt(T)`` horizon scaling, typically the day before
            the first reporting period starts. Required for a stochastic
            :func:`forecast_covenant` / :func:`forecast_breaches`, which
            raise ``ValueError`` without it.
        breach_probability_threshold : float
            Minimum breach probability (decimal) for :func:`forecast_breaches`
            to report a date; must lie in ``[0, 1]``.

        Raises
        ------
        ValueError
            If ``reference_date`` is a non-ISO string. Other invalid
            combinations (missing volatility, antithetic without paths,
            threshold outside ``[0, 1]``) raise ``ValueError`` at forecast time.
        """

    def with_scope(self, scope: str) -> CovenantForecastConfig:
        """Return a copy selecting the covenant scope for batch forecasts.

        Parameters
        ----------
        scope : str
            ``maintenance`` for scheduled compliance or ``incurrence`` for
            hypothetical action capacity. The default is ``maintenance``.

        Returns
        -------
        CovenantForecastConfig
            Configuration with the selected batch scope.

        Raises
        ------
        ValueError
            If scope is neither ``maintenance`` nor ``incurrence``.
        """

    @property
    def scope(self) -> str:
        """Scope selected by batch forecasts; single-spec forecasts ignore it.

        This property does not raise.

        Returns
        -------
        str
            ``maintenance`` or ``incurrence``.
        """

    @staticmethod
    def from_json(json: str) -> CovenantForecastConfig:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON matching the ``CovenantForecastConfig`` wire schema.

        Returns
        -------
        CovenantForecastConfig
            Parsed configuration.

        Raises
        ------
        ValueError
            If ``json`` is malformed.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantForecastConfig
        >>> CovenantForecastConfig.from_json(
        ...     '{"scope": "maintenance", "stochastic": false, "num_paths": 0, "volatility": null, "random_seed": null}'
        ... ).antithetic
        False
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    @property
    def stochastic(self) -> bool:
        """
        Whether the stochastic overlay is used.

        This property does not raise.

        Returns
        -------
        bool
            ``False`` for deterministic pass/fail.
        """

    @property
    def num_paths(self) -> int:
        """
        Monte Carlo path count.

        This property does not raise.

        Returns
        -------
        int
            ``0`` selects the closed-form analytic mode.
        """

    @property
    def volatility(self) -> float | None:
        """
        Annualized lognormal volatility.

        This property does not raise.

        Returns
        -------
        float | None
            ``None`` in deterministic mode.
        """

    @property
    def random_seed(self) -> int | None:
        """
        Monte Carlo seed.

        This property does not raise.

        Returns
        -------
        int | None
            ``None`` uses the crate default ``0``.
        """

    @property
    def antithetic(self) -> bool:
        """
        Whether antithetic pairing is enabled.

        This property does not raise.

        Returns
        -------
        bool
            Only meaningful when ``num_paths > 0``.
        """

    @property
    def reference_date(self) -> datetime.date | None:
        """
        Horizon anchor used to scale stochastic forecast variance.

        This property does not raise.

        Returns
        -------
        datetime.date | None
            ``None`` for the default (day before the first forecast date).
        """

    @property
    def breach_probability_threshold(self) -> float:
        """
        Reporting threshold for :func:`forecast_breaches`.

        This property does not raise.

        Returns
        -------
        float
            Decimal probability in ``[0, 1]``.
        """

    def __eq__(self, other: object) -> bool: ...

class CovenantForecast:
    """
    Forward compliance projection for one covenant across the forecast dates.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.covenants import Covenant, CovenantSpec, CovenantType, forecast_covenant
    >>> spec = CovenantSpec(Covenant(CovenantType.max_debt_to_ebitda(4.5), "3M", "max_leverage"), "debt_to_ebitda")
    >>> frame = pd.DataFrame(
    ...     {"debt_to_ebitda": [4.0, 4.4, 4.8]}, index=pd.to_datetime(["2026-03-31", "2026-06-30", "2026-09-30"])
    ... )
    >>> forecast = forecast_covenant(spec, frame)
    >>> forecast.first_breach_date.isoformat(), forecast.breach_probability
    ('2026-09-30', [0.0, 0.0, 1.0])
    >>> list(forecast.to_dataframe().columns)
    ['test_date', 'projected_value', 'threshold', 'headroom', 'breach_probability', 'breach_probability_stderr']
    """

    @staticmethod
    def from_json(json: str) -> CovenantForecast:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON matching the ``CovenantForecast`` wire schema.

        Returns
        -------
        CovenantForecast
            Parsed forecast.

        Raises
        ------
        ValueError
            If ``json`` is malformed.

        Examples
        --------
        >>> import pandas as pd
        >>> from finstack_quant.covenants import (
        ...     Covenant,
        ...     CovenantForecast,
        ...     CovenantSpec,
        ...     CovenantType,
        ...     forecast_covenant,
        ... )
        >>> spec = CovenantSpec(Covenant(CovenantType.min_dscr(1.2), "3M", "min_dscr"), "dscr")
        >>> frame = pd.DataFrame({"dscr": [1.3, 1.1]}, index=pd.to_datetime(["2026-03-31", "2026-06-30"]))
        >>> forecast = forecast_covenant(spec, frame)
        >>> CovenantForecast.from_json(forecast.to_json()) == forecast
        True
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per test date.

        Returns
        -------
        pd.DataFrame
            Columns ``test_date`` (ISO string), ``projected_value``,
            ``threshold``, ``headroom``, ``breach_probability``,
            ``breach_probability_stderr``; ``None`` where a value is not
            meaningful.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """

    @property
    def covenant_id(self) -> str:
        """
        Label of the forecast covenant.

        This property does not raise.

        Returns
        -------
        str
            Matches ``Covenant.label``.
        """

    @property
    def covenant_description(self) -> str:
        """
        Human-readable covenant description.

        This property does not raise.

        Returns
        -------
        str
            For example ``"Debt/EBITDA <= 4.50x"``.
        """

    @property
    def comparator(self) -> str:
        """
        Direction the activation metric is tested in.

        This property does not raise.

        Returns
        -------
        str
            ``"at_most"`` or ``"at_least"``.
        """

    @property
    def test_dates(self) -> list[datetime.date]:
        """
        Forecast test dates.

        This property does not raise.

        Returns
        -------
        list[datetime.date]
            One per frame row, ascending.
        """

    @property
    def projected_values(self) -> list[float | None]:
        """
        Projected metric per test date.

        This property does not raise.

        Returns
        -------
        list[float | None]
            ``None`` where the projection is not finite.
        """

    @property
    def thresholds(self) -> list[float]:
        """
        Threshold in force per test date.

        This property does not raise.

        Returns
        -------
        list[float]
            Static threshold or step-down schedule value.
        """

    @property
    def headroom(self) -> list[float | None]:
        """
        Relative headroom per test date.

        This property does not raise.

        Returns
        -------
        list[float | None]
            ``None`` while a springing covenant is inactive or the ratio is
            not meaningful (negative EBITDA).
        """

    @property
    def breach_probability(self) -> list[float]:
        """
        Breach probability per test date.

        This property does not raise.

        Returns
        -------
        list[float]
            ``0.0`` / ``1.0`` in deterministic mode.
        """

    @property
    def breach_probability_stderr(self) -> list[float]:
        """
        Monte Carlo standard error per test date.

        This property does not raise.

        Returns
        -------
        list[float]
            Zeros in deterministic and analytic modes.
        """

    @property
    def first_breach_date(self) -> datetime.date | None:
        """
        First projected breach.

        This property does not raise.

        Returns
        -------
        datetime.date | None
            ``None`` when no date breaches.
        """

    @property
    def min_headroom_date(self) -> datetime.date | None:
        """
        Date of minimum finite headroom.

        This property does not raise.

        Returns
        -------
        datetime.date | None
            ``None`` when no headroom is finite.
        """

    @property
    def min_headroom_value(self) -> float | None:
        """
        Minimum finite headroom across active dates.

        This property does not raise.

        Returns
        -------
        float | None
            ``None`` when no headroom is finite.
        """

    def __len__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...

class FutureBreach:
    """
    A projected covenant breach on one forecast date.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.covenants import CovenantEngine, CovenantForecastConfig, cov_lite, forecast_breaches
    >>> engine = CovenantEngine.from_specs(cov_lite(7.0, 4.5))
    >>> frame = pd.DataFrame(
    ...     {"total_leverage": [6.0, 7.5], "senior_leverage": [3.0, 3.5]},
    ...     index=pd.to_datetime(["2026-03-31", "2026-06-30"]),
    ... )
    >>> breaches = forecast_breaches(engine, frame, CovenantForecastConfig().with_scope("incurrence"))
    >>> [(b.covenant_id, b.breach_date.isoformat()) for b in breaches]
    [('max_total_leverage', '2026-06-30')]
    """

    @staticmethod
    def from_json(json: str) -> FutureBreach:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON matching the ``FutureBreach`` wire schema.

        Returns
        -------
        FutureBreach
            Parsed breach.

        Raises
        ------
        ValueError
            If ``json`` is malformed.

        Examples
        --------
        >>> from finstack_quant.covenants import FutureBreach
        >>> raw = '{"covenant_id": "x", "covenant_description": "DSCR >= 1.20x", "breach_date": "2026-06-30", "projected_value": 1.1, "threshold": 1.2, "headroom": -0.0833, "breach_probability": 1.0}'
        >>> FutureBreach.from_json(raw).breach_probability
        1.0
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    @property
    def covenant_id(self) -> str:
        """
        Label of the covenant.

        This property does not raise.

        Returns
        -------
        str
            Matches ``Covenant.label``.
        """

    @property
    def covenant_description(self) -> str:
        """
        Human-readable covenant description.

        This property does not raise.

        Returns
        -------
        str
            For example ``"Total Leverage <= 7.00x"``.
        """

    @property
    def breach_date(self) -> datetime.date:
        """
        Forecast date of the breach.

        This property does not raise.

        Returns
        -------
        datetime.date
            Breach date.
        """

    @property
    def projected_value(self) -> float | None:
        """
        Projected metric value.

        This property does not raise.

        Returns
        -------
        float | None
            ``None`` when not finite.
        """

    @property
    def threshold(self) -> float:
        """
        Threshold in force on the breach date.

        This property does not raise.

        Returns
        -------
        float
            Static or scheduled threshold.
        """

    @property
    def headroom(self) -> float | None:
        """
        Relative headroom.

        This property does not raise.

        Returns
        -------
        float | None
            Negative means breach; ``None`` when not meaningful.
        """

    @property
    def breach_probability(self) -> float:
        """
        Breach probability.

        This property does not raise.

        Returns
        -------
        float
            ``1.0`` in deterministic mode.
        """

    def __eq__(self, other: object) -> bool: ...

def forecast_covenant(
    spec: CovenantSpec,
    metrics: pd.DataFrame,
    config: CovenantForecastConfig | None = None,
) -> CovenantForecast:
    """
    Forecast one numeric covenant across a date-indexed projection frame.

    Each frame row is a forecast test date. The metric is resolved from the
    spec's explicit ``metric_id`` exclusively when present; otherwise it uses
    the covenant type's conventional metric name,
    then a custom covenant's ``metric``. Threshold schedules and springing
    conditions are honoured (the activation metric must be a frame column).

    Parameters
    ----------
    spec : CovenantSpec
        Numeric covenant to forecast.
    metrics : pd.DataFrame
        Index holds forecast dates; columns are metric ids; ``NaN`` cells are
        treated as absent.
    config : CovenantForecastConfig | None
        Forecast policy; ``None`` is deterministic.

    Returns
    -------
    CovenantForecast
        Per-date projections, thresholds, headroom and breach probabilities.

    Raises
    ------
    KeyError
        If the covenant's metric is absent on any date.
    ValueError
        If the frame is empty, the covenant is non-numeric or has a
        non-finite threshold, or the config is invalid (stochastic without
        ``volatility`` or ``reference_date``, antithetic without paths,
        threshold outside ``[0, 1]``).

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.covenants import (
    ...     Covenant,
    ...     CovenantForecastConfig,
    ...     CovenantSpec,
    ...     CovenantType,
    ...     forecast_covenant,
    ... )
    >>> spec = CovenantSpec(Covenant(CovenantType.max_debt_to_ebitda(4.5), "3M", "max_leverage"), "debt_to_ebitda")
    >>> frame = pd.DataFrame({"debt_to_ebitda": [4.0, 4.4]}, index=pd.to_datetime(["2026-03-31", "2026-06-30"]))
    >>> cfg = CovenantForecastConfig(stochastic=True, volatility=0.25, reference_date="2025-12-31")
    >>> forecast = forecast_covenant(spec, frame, cfg)
    >>> [0.0 < p < 1.0 for p in forecast.breach_probability]
    [True, True]
    """

def forecast_breaches(
    engine: CovenantEngine,
    metrics: pd.DataFrame,
    config: CovenantForecastConfig | None = None,
) -> list[FutureBreach]:
    """
    Forecast every active numeric covenant in an engine and collect the
    dates whose breach probability reaches the config threshold.

    Effective windows, waivers, schedules, and scope are honored. Missing or
    non-finite required observations fail the entire batch. Inactive and
    non-numeric covenants are skipped. Deterministic breaches are always retained.

    Parameters
    ----------
    engine : CovenantEngine
        Engine whose effective specifications and amendments are forecast.
    metrics : pd.DataFrame
        Index holds forecast dates; columns are metric ids.
    config : CovenantForecastConfig | None
        Forecast policy; ``None`` is deterministic with a 5% reporting
        threshold.

    Returns
    -------
    list[FutureBreach]
        Breaches sorted by date then covenant id; empty when no applicable test breaches.

    Raises
    ------
    KeyError
        If any active numeric test lacks a required metric at a requested date.
    ValueError
        If the frame is empty, has duplicate metric columns or non-finite required
        observations, or the engine, date ordering, or configuration is invalid
        (including a stochastic config without ``reference_date``).

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.covenants import (
    ...     CovenantEngine,
    ...     CovenantForecastConfig,
    ...     breaches_to_dataframe,
    ...     cov_lite,
    ...     forecast_breaches,
    ... )
    >>> engine = CovenantEngine.from_specs(cov_lite(7.0, 4.5))
    >>> frame = pd.DataFrame(
    ...     {"total_leverage": [6.0, 7.5], "senior_leverage": [3.0, 5.0]},
    ...     index=pd.to_datetime(["2026-03-31", "2026-06-30"]),
    ... )
    >>> breaches_to_dataframe(forecast_breaches(engine, frame, CovenantForecastConfig().with_scope("incurrence")))[
    ...     "covenant_id"
    ... ].tolist()
    ['max_senior_leverage', 'max_total_leverage']
    """

def breaches_to_dataframe(breaches: list[FutureBreach]) -> pd.DataFrame:
    """
    Flatten forecast breaches into one frame row per breach.

    Parameters
    ----------
    breaches : list[FutureBreach]
        Output of :func:`forecast_breaches`.

    Returns
    -------
    pd.DataFrame
        Columns ``covenant_id``, ``covenant_description``, ``breach_date``
        (ISO string), ``projected_value``, ``threshold``, ``headroom``,
        ``breach_probability``; typed and empty when there are no breaches.

    Raises
    ------
    TypeError
        If an element is not a ``FutureBreach``.

    Examples
    --------
    >>> from finstack_quant.covenants import breaches_to_dataframe
    >>> list(breaches_to_dataframe([]).columns)
    ['covenant_id', 'covenant_description', 'breach_date', 'projected_value', 'threshold', 'headroom', 'breach_probability']
    """
