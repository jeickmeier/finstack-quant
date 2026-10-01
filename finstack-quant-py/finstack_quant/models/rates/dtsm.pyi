"""
Dynamic term-structure model bindings: Diebold-Li and yield-curve PCA.

Typed wrappers over the Rust engines:

- :class:`YieldPanel` — dated yield matrix (rows = dates, columns = tenors).
- :class:`DieboldLi` / :class:`FactorTimeSeries` / :class:`YieldForecast` —
  dynamic Nelson-Siegel factor extraction, VAR(1) dynamics and forecasting.
- :class:`YieldPca` / :class:`YieldPcaView` — PCA of yield changes and
  scenario shocks.

The free functions are thin twins over the same Rust entry points for callers
holding plain nested lists.

Yields are continuously compounded zero rates in decimal form (``0.045`` for
4.5%). Tenors are in years and the Diebold-Li decay ``lambda_`` is per year
(default ``0.7308``, the years-equivalent of Diebold-Li's ``0.0609`` months
value).

Examples
--------
>>> from finstack_quant.models.rates.dtsm import nelson_siegel_yields
>>> len(nelson_siegel_yields(0.7308, (0.03, -0.01, 0.005), [1.0, 5.0, 10.0]))
3

"""

from __future__ import annotations

import datetime
from typing import Any, Sequence

import pandas as pd

__all__ = [
    "DieboldLi",
    "FactorTimeSeries",
    "YieldForecast",
    "YieldPanel",
    "YieldPca",
    "YieldPcaView",
    "diebold_li_fit_factors",
    "diebold_li_forecast",
    "nelson_siegel_yields",
    "yield_pca_fit",
    "yield_pca_scenario",
]

class YieldPanel:
    """Panel of continuously compounded zero yields (rows = dates, columns = tenors).

    Parameters
    ----------
    tenors : Sequence[float]
        Tenor grid in years, strictly ascending and positive (length ``N``).
    yields : Sequence[Sequence[float]]
        ``yields[date_idx][tenor_idx]`` decimal zero rates (``T >= 2`` rows
        of ``N`` finite values).
    dates : Sequence[datetime.date | datetime.datetime | pandas.Timestamp | str] or None, default None
        Optional observation labels (length ``T``); ISO strings are accepted.

    Raises
    ------
    ValueError
        If the tenor grid is not strictly ascending and positive, the yield
        rows are empty/ragged/non-finite or do not match the tenor count,
        fewer than two observations are supplied, or ``dates`` has the wrong
        length.

    Examples
    --------
    >>> from finstack_quant.models.rates.dtsm import YieldPanel
    >>> panel = YieldPanel([1.0, 2.0], [[0.01, 0.02], [0.011, 0.021]], ["2025-01-01", "2025-01-02"])
    >>> (panel.num_dates, panel.num_tenors, panel.dates[0].isoformat())
    (2, 2, '2025-01-01')
    """

    def __init__(
        self,
        tenors: Sequence[float],
        yields: Sequence[Sequence[float]],
        dates: Sequence[Any] | None = None,
    ) -> None: ...
    @classmethod
    def from_dataframe(cls, df: pd.DataFrame) -> YieldPanel:
        """Build a panel from a DataFrame with tenor columns.

        Column labels are parsed as tenors in years (``float(label)``); the
        index supplies observation dates when it is date-like (otherwise the
        panel is unlabeled); values are decimal zero rates.

        Parameters
        ----------
        df : pandas.DataFrame
            Wide frame of yields, one column per tenor.

        Returns
        -------
        YieldPanel
            Validated panel.

        Raises
        ------
        TypeError
            If ``df`` is not a ``pandas.DataFrame``.
        ValueError
            If a column label is not numeric or the panel fails validation.

        Examples
        --------
        >>> import pandas as pd
        >>> from finstack_quant.models.rates.dtsm import YieldPanel
        >>> df = pd.DataFrame(
        ...     {1.0: [0.01, 0.011], 2.0: [0.02, 0.021]}, index=pd.to_datetime(["2025-01-01", "2025-01-02"])
        ... )
        >>> YieldPanel.from_dataframe(df).tenors
        [1.0, 2.0]
        """
        ...

    @property
    def tenors(self) -> list[float]:
        """
        Maturities the panel's yields are observed at.

        Returns
        -------
        list[float]
            Tenors as year fractions in ascending order (``0.25`` is three months); one entry per column of :attr:`yields`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dates(self) -> list[datetime.date] | None:
        """
        Calendar dates labelling the rows of the panel.

        Returns
        -------
        list[datetime.date] | None
            One :class:`datetime.date` per observation row in chronological order, or ``None`` when the panel was built without dates.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def yields(self) -> list[list[float]]:
        """
        Observed zero-rate surface backing the panel.

        Returns
        -------
        list[list[float]]
            ``yields[date_idx][tenor_idx]`` decimal zero rates (``0.045`` is 4.5%), with one row per observation date and one column per tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_dates(self) -> int:
        """
        Row count of the yield panel.

        Returns
        -------
        int
            Number of observation dates ``T`` in the panel; the length of :attr:`yields`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_tenors(self) -> int:
        """
        Column count of the yield panel.

        Returns
        -------
        int
            Number of tenors ``N`` on the curve grid; the length of :attr:`tenors`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def yield_changes(self) -> list[list[float]]:
        """Return first differences of the yields (``T-1`` rows).

        Returns
        -------
        list[list[float]]
            ``changes[t][tenor] = yields[t+1][tenor] - yields[t][tenor]``.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPanel
        >>> YieldPanel([1.0], [[0.01], [0.012]]).yield_changes()
        [[0.002]]
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Return the panel as a DataFrame with one column per tenor.

        Returns
        -------
        pandas.DataFrame
            ``DatetimeIndex`` when dates are present, ``RangeIndex`` otherwise.

        Raises
        ------
        ValueError
            If the result cannot be converted to the tabular form.
        ImportError
            If pandas is not installed in the environment.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPanel
        >>> YieldPanel([1.0, 2.0], [[0.01, 0.02], [0.011, 0.021]]).to_dataframe().shape
        (2, 2)
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with ``yields``, ``tenors`` and ``dates``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPanel
        >>> panel = YieldPanel([1.0], [[0.01], [0.012]])
        >>> YieldPanel.from_json(panel.to_json()).num_dates
        2
        """
        ...

    @staticmethod
    def from_json(json: str) -> YieldPanel:
        """Deserialize a panel produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        YieldPanel
            The reconstructed panel.

        Raises
        ------
        ValueError
            If the payload is malformed or violates the panel's observation,
            tenor, yield, or date-alignment requirements.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPanel
        >>> panel = YieldPanel([1.0], [[0.01], [0.012]])
        >>> YieldPanel.from_json(panel.to_json()).tenors
        [1.0]
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class FactorTimeSeries:
    """Time series of Nelson-Siegel factors extracted by :class:`DieboldLi`.

    ``level`` (beta1), ``slope`` (beta2) and ``curvature`` (beta3) are in
    decimal yield units, one value per observation date.

    Examples
    --------
    >>> from finstack_quant.models.rates.dtsm import diebold_li_fit_factors
    >>> fts = diebold_li_fit_factors([1.0, 2.0, 5.0, 10.0], [[0.02, 0.025, 0.03, 0.035], [0.021, 0.026, 0.031, 0.036]])
    >>> (len(fts.level), fts.num_dates)
    (2, 2)
    """

    @property
    def dates(self) -> list[datetime.date] | None:
        """
        Calendar dates labelling the extracted factor rows.

        Returns
        -------
        list[datetime.date] | None
            One :class:`datetime.date` per factor observation in chronological order, or ``None`` when the source panel carried no dates.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def level(self) -> list[float]:
        """
        Diebold-Li level factor through time.

        Returns
        -------
        list[float]
            ``beta1`` per observation date in decimal yield units; it is the long-run level the curve flattens to.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def slope(self) -> list[float]:
        """
        Diebold-Li slope factor through time.

        Returns
        -------
        list[float]
            ``beta2`` per observation date in decimal yield units; the short-minus-long spread, negative for an upward-sloping curve.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def curvature(self) -> list[float]:
        """
        Diebold-Li curvature factor through time.

        Returns
        -------
        list[float]
            ``beta3`` per observation date in decimal yield units; it governs the medium-tenor hump of the curve.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factors(self) -> list[list[float]]:
        """
        All three Diebold-Li factors stacked by date.

        Returns
        -------
        list[list[float]]
            ``factors[date_idx] = [level, slope, curvature]`` in decimal yield units, one row per observation date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def residuals(self) -> list[list[float]]:
        """
        Cross-sectional fit errors of the factor extraction.

        Returns
        -------
        list[list[float]]
            ``residuals[date_idx][tenor_idx]`` in decimal yield units: observed minus fitted zero rate for each date and tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def r_squared(self) -> list[float]:
        """
        Goodness of fit of the three-factor model at each tenor.

        Returns
        -------
        list[float]
            One R-squared per tenor, normally in ``[0, 1]``; low values flag tenors the level/slope/curvature loadings explain poorly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def r_squared_avg(self) -> float:
        """
        Summary goodness of fit across the whole curve.

        Returns
        -------
        float
            Unweighted mean of :attr:`r_squared` over the tenor grid, normally in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_dates(self) -> int:
        """
        Row count of the factor series.

        Returns
        -------
        int
            Number of dates for which factors were extracted; the length of :attr:`factors`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Return the factors as a DataFrame with ``level``, ``slope``, ``curvature`` columns.

        Returns
        -------
        pandas.DataFrame
            ``DatetimeIndex`` when dates are available, ``RangeIndex`` otherwise.

        Raises
        ------
        ValueError
            If the result cannot be converted to the tabular form.
        ImportError
            If pandas is not installed in the environment.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import diebold_li_fit_factors
        >>> fts = diebold_li_fit_factors(
        ...     [1.0, 2.0, 5.0, 10.0], [[0.02, 0.025, 0.03, 0.035], [0.021, 0.026, 0.031, 0.036]]
        ... )
        >>> fts.to_dataframe().columns.tolist()
        ['level', 'slope', 'curvature']
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with dates, factor and residual matrices and R-squared.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import FactorTimeSeries, diebold_li_fit_factors
        >>> fts = diebold_li_fit_factors(
        ...     [1.0, 2.0, 5.0, 10.0], [[0.02, 0.025, 0.03, 0.035], [0.021, 0.026, 0.031, 0.036]]
        ... )
        >>> FactorTimeSeries.from_json(fts.to_json()).num_dates
        2
        """
        ...

    @staticmethod
    def from_json(json: str) -> FactorTimeSeries:
        """Deserialize a factor series produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        FactorTimeSeries
            The reconstructed series.

        Raises
        ------
        ValueError
            If the payload is malformed or has non-finite values or misaligned
            factor, residual, date, or fit-statistic dimensions.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import FactorTimeSeries, diebold_li_fit_factors
        >>> fts = diebold_li_fit_factors(
        ...     [1.0, 2.0, 5.0, 10.0], [[0.02, 0.025, 0.03, 0.035], [0.021, 0.026, 0.031, 0.036]]
        ... )
        >>> FactorTimeSeries.from_json(fts.to_json()).r_squared_avg == fts.r_squared_avg
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class DieboldLi:
    """Diebold-Li (2006) dynamic Nelson-Siegel model.

    Every fitting step returns a new model; instances are immutable.

    Parameters
    ----------
    lambda_ : float or None, default None
        Decay parameter for tenors **in years**; finite and positive. ``None``
        uses the Rust default ``0.7308`` (years-equivalent of Diebold-Li's
        canonical ``0.0609`` months value). Named ``lambda_`` because
        ``lambda`` is a Python keyword.

    Raises
    ------
    ValueError
        If ``lambda_`` is supplied but non-finite or not strictly positive.

    Examples
    --------
    >>> from finstack_quant.models.rates.dtsm import DieboldLi, YieldPanel
    >>> panel = YieldPanel(
    ...     [1.0, 2.0, 5.0, 10.0],
    ...     [
    ...         [0.020, 0.025, 0.030, 0.035],
    ...         [0.021, 0.024, 0.031, 0.034],
    ...         [0.019, 0.026, 0.029, 0.036],
    ...         [0.022, 0.025, 0.032, 0.033],
    ...         [0.020, 0.027, 0.030, 0.037],
    ...         [0.023, 0.026, 0.033, 0.035],
    ...     ],
    ... )
    >>> model = DieboldLi().fit(panel)
    >>> (model.lambda_, model.forecast(2).horizon)
    (0.7308, 2)
    """

    def __init__(self, lambda_: float | None = None) -> None: ...
    @property
    def lambda_(self) -> float:
        """
        Diebold-Li exponential decay parameter.

        Returns
        -------
        float
            ``lambda`` in inverse years, applied to tenors measured in years; it fixes where the curvature loading peaks (``0.0609`` peaks near 30 months). Named ``lambda_`` because ``lambda`` is a Python keyword.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tenors(self) -> list[float]:
        """
        Tenor grid the model was last fitted on.

        Returns
        -------
        list[float]
            Tenors in years, in the order supplied to :meth:`extract_factors`; an empty list before any extraction has run.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factors(self) -> FactorTimeSeries | None:
        """
        Factor series produced by the most recent extraction.

        Returns
        -------
        FactorTimeSeries | None
            The fitted :class:`FactorTimeSeries`, or ``None`` when :meth:`extract_factors` has not been called yet.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def phi(self) -> list[list[float]] | None:
        """
        Transition matrix of the fitted factor VAR(1).

        Returns
        -------
        list[list[float]] | None
            Row-major 3x3 coefficient matrix mapping this period's factor vector to the next, or ``None`` before :meth:`fit_var` has run.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mu(self) -> list[float] | None:
        """
        Long-run mean the fitted factor VAR(1) reverts to.

        Returns
        -------
        list[float] | None
            Three decimal-yield-unit means for level, slope and curvature, or ``None`` before :meth:`fit_var` has run.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def q_cov(self) -> list[list[float]] | None:
        """
        Innovation covariance of the fitted factor VAR(1).

        Returns
        -------
        list[list[float]] | None
            Row-major 3x3 covariance of the one-step residuals in squared decimal yield units, or ``None`` before :meth:`fit_var` has run.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def loading_matrix(self) -> list[list[float]]:
        """Return the Nelson-Siegel loading matrix (``N x 3``) for the recorded tenors.

        Returns
        -------
        list[list[float]]
            Rows ``[1, slope_loading, curvature_loading]`` per tenor.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import DieboldLi
        >>> DieboldLi().loading_matrix()
        []
        """
        ...

    def extract_factors(self, panel: YieldPanel) -> DieboldLi:
        """Extract level/slope/curvature factors from ``panel`` via OLS.

        Parameters
        ----------
        panel : YieldPanel
            Yield panel with at least three tenors.

        Returns
        -------
        DieboldLi
            New model carrying ``factors`` and ``tenors`` with prior VAR state cleared;
            call ``fit_var`` before forecasting the replacement factors.

        Raises
        ------
        ValueError
            If the panel has fewer than three tenors or the loading matrix is
            singular.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import DieboldLi, YieldPanel
        >>> panel = YieldPanel([1.0, 2.0, 5.0, 10.0], [[0.02, 0.025, 0.03, 0.035], [0.021, 0.026, 0.031, 0.036]])
        >>> DieboldLi().extract_factors(panel).factors.num_dates
        2
        """
        ...

    def fit_var(self) -> DieboldLi:
        """Fit VAR(1) dynamics to the extracted factors.

        Returns
        -------
        DieboldLi
            New model carrying ``phi``, ``mu`` and ``q_cov``.

        Raises
        ------
        ValueError
            If factors have not been extracted, fewer than five observations are
            available, or the fitted dynamics have no finite unconditional mean
            because ``I - Phi`` is singular.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import DieboldLi, YieldPanel
        >>> panel = YieldPanel(
        ...     [1.0, 2.0, 5.0, 10.0],
        ...     [
        ...         [0.020, 0.025, 0.030, 0.035],
        ...         [0.021, 0.024, 0.031, 0.034],
        ...         [0.019, 0.026, 0.029, 0.036],
        ...         [0.022, 0.025, 0.032, 0.033],
        ...         [0.020, 0.027, 0.030, 0.037],
        ...         [0.023, 0.026, 0.033, 0.035],
        ...     ],
        ... )
        >>> len(DieboldLi().extract_factors(panel).fit_var().mu)
        3
        """
        ...

    def fit(self, panel: YieldPanel) -> DieboldLi:
        """Run :meth:`extract_factors` then :meth:`fit_var`.

        Parameters
        ----------
        panel : YieldPanel
            Yield panel with at least three tenors and five observations.

        Returns
        -------
        DieboldLi
            Fitted model ready for :meth:`forecast`.

        Raises
        ------
        ValueError
            On any validation failure of either step.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import DieboldLi, YieldPanel
        >>> panel = YieldPanel(
        ...     [1.0, 2.0, 5.0, 10.0],
        ...     [
        ...         [0.020, 0.025, 0.030, 0.035],
        ...         [0.021, 0.024, 0.031, 0.034],
        ...         [0.019, 0.026, 0.029, 0.036],
        ...         [0.022, 0.025, 0.032, 0.033],
        ...         [0.020, 0.027, 0.030, 0.037],
        ...         [0.023, 0.026, 0.033, 0.035],
        ...     ],
        ... )
        >>> DieboldLi().fit(panel).phi is not None
        True
        """
        ...

    def forecast(self, horizon: int) -> YieldForecast:
        """Forecast the curve ``horizon`` observation periods ahead.

        Parameters
        ----------
        horizon : int
            Forecast horizon in observation periods (``>= 1``).

        Returns
        -------
        YieldForecast
            Point forecast, factor triple and 95% bands.

        Raises
        ------
        ValueError
            If the VAR has not been fitted or ``horizon`` is zero.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import DieboldLi, YieldPanel
        >>> panel = YieldPanel(
        ...     [1.0, 2.0, 5.0, 10.0],
        ...     [
        ...         [0.020, 0.025, 0.030, 0.035],
        ...         [0.021, 0.024, 0.031, 0.034],
        ...         [0.019, 0.026, 0.029, 0.036],
        ...         [0.022, 0.025, 0.032, 0.033],
        ...         [0.020, 0.027, 0.030, 0.037],
        ...         [0.023, 0.026, 0.033, 0.035],
        ...     ],
        ... )
        >>> len(DieboldLi().fit(panel).forecast(3).yields)
        4
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with ``lambda``, factors, VAR parameters and tenors.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import DieboldLi
        >>> DieboldLi.from_json(DieboldLi(0.5).to_json()).lambda_
        0.5
        """
        ...

    @staticmethod
    def from_json(json: str) -> DieboldLi:
        """Deserialize a model produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        DieboldLi
            The reconstructed model.

        Raises
        ------
        ValueError
            If the payload is malformed, ``lambda`` is invalid, or its factor
            history, tenor grid, or fitted VAR state is inconsistent.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import DieboldLi
        >>> DieboldLi.from_json(DieboldLi().to_json()).factors is None
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class YieldForecast:
    """h-step-ahead Diebold-Li yield-curve forecast with 95% Gaussian bands.

    Examples
    --------
    >>> from finstack_quant.models.rates.dtsm import diebold_li_forecast
    >>> tenors = [1.0, 2.0, 5.0, 10.0]
    >>> yields = [
    ...     [0.020, 0.025, 0.030, 0.035],
    ...     [0.021, 0.024, 0.031, 0.034],
    ...     [0.019, 0.026, 0.029, 0.036],
    ...     [0.022, 0.025, 0.032, 0.033],
    ...     [0.020, 0.027, 0.030, 0.037],
    ...     [0.023, 0.026, 0.033, 0.035],
    ... ]
    >>> fc = diebold_li_forecast(tenors, yields, 2)
    >>> (fc.horizon, len(fc.yields), len(fc.factors))
    (2, 4, 3)
    """

    @property
    def horizon(self) -> int:
        """
        How far ahead this forecast was projected.

        Returns
        -------
        int
            Horizon as a count of observation periods of the source panel (months for monthly data), not calendar days.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def yields(self) -> list[float]:
        """
        Central projection of the curve at the horizon.

        Returns
        -------
        list[float]
            One decimal zero rate per tenor (``0.045`` is 4.5%), aligned with :attr:`tenors`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tenors(self) -> list[float]:
        """
        Maturities the forecast curve is quoted on.

        Returns
        -------
        list[float]
            Tenors as year fractions, in the same order as :attr:`yields`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factors(self) -> tuple[float, float, float]:
        """
        Projected Diebold-Li factor vector at the horizon.

        Returns
        -------
        tuple[float, float, float]
            The three factors ``(level, slope, curvature)`` in decimal yield units, from iterating the fitted VAR(1) forward.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def lower_95(self) -> list[float]:
        """
        Lower edge of the 95% forecast interval.

        Returns
        -------
        list[float]
            One decimal zero rate per tenor at the 2.5th percentile of the forecast distribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def upper_95(self) -> list[float]:
        """
        Upper edge of the 95% forecast interval.

        Returns
        -------
        list[float]
            One decimal zero rate per tenor at the 97.5th percentile of the forecast distribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Return the forecast as a DataFrame with one row per tenor.

        Returns
        -------
        pandas.DataFrame
            Columns ``tenor``, ``yield``, ``lower_95``, ``upper_95``.

        Raises
        ------
        ValueError
            If the result cannot be converted to the tabular form.
        ImportError
            If pandas is not installed in the environment.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import diebold_li_forecast
        >>> tenors = [1.0, 2.0, 5.0, 10.0]
        >>> yields = [
        ...     [0.020, 0.025, 0.030, 0.035],
        ...     [0.021, 0.024, 0.031, 0.034],
        ...     [0.019, 0.026, 0.029, 0.036],
        ...     [0.022, 0.025, 0.032, 0.033],
        ...     [0.020, 0.027, 0.030, 0.037],
        ...     [0.023, 0.026, 0.033, 0.035],
        ... ]
        >>> diebold_li_forecast(tenors, yields, 2).to_dataframe().columns.tolist()
        ['tenor', 'yield', 'lower_95', 'upper_95']
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with the forecast fields.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldForecast
        >>> YieldForecast.from_json(
        ...     '{"horizon":1,"yields":[0.02],"tenors":[1.0],"factors":[0.02,0.0,0.0],"lower_95":[0.01],"upper_95":[0.03]}'
        ... ).to_json().startswith("{")
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> YieldForecast:
        """Deserialize a forecast produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        YieldForecast
            The reconstructed forecast.

        Raises
        ------
        ValueError
            If the payload is malformed, the horizon is zero, or yields, tenors,
            factors, and confidence bands violate their invariants.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldForecast
        >>> YieldForecast.from_json(
        ...     '{"horizon":1,"yields":[0.02],"tenors":[1.0],"factors":[0.02,0.0,0.0],"lower_95":[0.01],"upper_95":[0.03]}'
        ... ).horizon
        1
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class YieldPca:
    """PCA decomposition of yield-curve changes (Litterman-Scheinkman).

    Construct with :meth:`fit` (from a :class:`YieldPanel`) or
    :meth:`fit_yield_changes` (from already-differenced rows).

    Examples
    --------
    >>> from finstack_quant.models.rates.dtsm import YieldPanel, YieldPca
    >>> panel = YieldPanel(
    ...     [1.0, 2.0, 5.0], [[0.01, 0.02, 0.03], [0.012, 0.021, 0.031], [0.011, 0.023, 0.032], [0.013, 0.022, 0.034]]
    ... )
    >>> pca = YieldPca.fit(panel)
    >>> (pca.num_components, len(pca.loading(0)))
    (3, 3)
    """

    @classmethod
    def fit(cls, panel: YieldPanel) -> YieldPca:
        """Fit PCA to the first differences of ``panel``.

        Parameters
        ----------
        panel : YieldPanel
            Panel with at least two tenors and three observations.

        Returns
        -------
        YieldPca
            Fitted decomposition.

        Raises
        ------
        ValueError
            For too few tenors/observations or a degenerate covariance.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPanel, YieldPca
        >>> panel = YieldPanel([1.0, 2.0], [[0.01, 0.02], [0.012, 0.021], [0.011, 0.023]])
        >>> YieldPca.fit(panel).tenors
        [1.0, 2.0]
        """
        ...

    @classmethod
    def fit_yield_changes(cls, yield_changes: Sequence[Sequence[float]]) -> YieldPca:
        """Fit PCA to already-differenced yields.

        The result carries a synthetic tenor grid ``1.0, 2.0, ..., N`` because
        yield changes do not identify the maturities.

        Parameters
        ----------
        yield_changes : Sequence[Sequence[float]]
            ``yield_changes[t][tenor]`` in decimal units.

        Returns
        -------
        YieldPca
            Fitted decomposition with placeholder tenors.

        Raises
        ------
        ValueError
            For empty/ragged rows, fewer than two rows or tenors, or a
            degenerate covariance matrix.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPca
        >>> YieldPca.fit_yield_changes([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]]).tenors
        [1.0, 2.0]
        """
        ...

    @property
    def num_components(self) -> int:
        """
        How many principal components the fit retained.

        Returns
        -------
        int
            Component count, capped at ``min(T - 1, N)`` for ``T`` observations and ``N`` tenors.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def eigenvalues(self) -> list[float]:
        """
        Variance carried by each principal component.

        Returns
        -------
        list[float]
            Eigenvalues of the yield-change covariance in squared decimal yield units, sorted descending.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def loadings(self) -> list[list[float]]:
        """
        How each tenor moves with each component.

        Returns
        -------
        list[list[float]]
            ``loadings[tenor][k]``: the unit-norm eigenvector weight of tenor ``tenor`` on component ``k`` (component 1 is the level shift).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scores(self) -> list[list[float]]:
        """
        Component time series implied by the sample.

        Returns
        -------
        list[list[float]]
            ``scores[t][k]``: the projection of observation ``t`` onto component ``k``, in decimal yield-change units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tenors(self) -> list[float]:
        """
        Tenor labels for the loading rows.

        Returns
        -------
        list[float]
            Tenors in years, or the synthetic sequence ``1..N`` when the fit was run through :meth:`fit_yield_changes`, which takes no tenor grid.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def variance_explained(self) -> list[float]:
        """
        Share of curve variance attributable to each component.

        Returns
        -------
        list[float]
            One decimal fraction per component (``0.9`` is 90%), summing to at most ``1.0``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cumulative_variance(self) -> list[float]:
        """
        Running total of explained variance across components.

        Returns
        -------
        list[float]
            Non-decreasing decimal fractions, entry ``k`` being the variance explained by the first ``k + 1`` components.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mean_change(self) -> list[float]:
        """
        Sample mean removed to centre the data before decomposition.

        Returns
        -------
        list[float]
            One mean yield change per tenor in decimal units; add it back to reconstruct levels from scores and loadings.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def loading(self, k: int) -> list[float]:
        """Return the loading vector of component ``k``.

        Parameters
        ----------
        k : int
            Zero-based component index.

        Returns
        -------
        list[float]
            Loading per tenor.

        Raises
        ------
        ValueError
            If ``k`` is out of range.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPca
        >>> len(YieldPca.fit_yield_changes([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]]).loading(0))
        2
        """
        ...

    def components_for_threshold(self, threshold: float) -> int:
        """Return the number of leading components explaining ``threshold`` of variance.

        Parameters
        ----------
        threshold : float
            Target cumulative variance fraction in ``[0, 1]``.

        Returns
        -------
        int
            Smallest component count reaching the threshold (all if never reached).

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPca
        >>> YieldPca.fit_yield_changes([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]]).components_for_threshold(
        ...     1.0
        ... ) >= 1
        True
        """
        ...

    def scenario(self, shocks: Sequence[float]) -> list[float]:
        """Return the yield-change vector for standard-deviation ``shocks``.

        Parameters
        ----------
        shocks : Sequence[float]
            One shock (in standard deviations) per leading component.

        Returns
        -------
        list[float]
            ``sum_k shocks[k] * sqrt(eigenvalue_k) * loading_k`` per tenor.

        Raises
        ------
        ValueError
            If more shocks than components are given.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPca
        >>> len(YieldPca.fit_yield_changes([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]]).scenario([2.0]))
        2
        """
        ...

    def apply_scenario(self, base_yields: Sequence[float], shocks: Sequence[float]) -> list[float]:
        """Return ``base_yields`` shifted by :meth:`scenario`.

        Parameters
        ----------
        base_yields : Sequence[float]
            Base curve, one decimal yield per tenor.
        shocks : Sequence[float]
            Shocks in standard deviations per leading component.

        Returns
        -------
        list[float]
            Shifted yields per tenor.

        Raises
        ------
        ValueError
            If ``base_yields`` does not match the tenor count or too many
            shocks are given.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPca
        >>> pca = YieldPca.fit_yield_changes([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]])
        >>> pca.apply_scenario([0.02, 0.03], [0.0]) == [0.02, 0.03]
        True
        """
        ...

    def reconstruct(self, num_components: int) -> list[list[float]]:
        """Reconstruct yield changes from the leading ``num_components``.

        Parameters
        ----------
        num_components : int
            Number of components to keep (``1..=num_components``).

        Returns
        -------
        list[list[float]]
            Row-major reconstructed changes with the mean added back.

        Raises
        ------
        ValueError
            If ``num_components`` is zero or too large.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPca
        >>> len(YieldPca.fit_yield_changes([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]]).reconstruct(1))
        3
        """
        ...

    def truncated(self, n_components: int) -> YieldPcaView:
        """Return the leading ``n_components`` as a :class:`YieldPcaView`.

        Parameters
        ----------
        n_components : int
            Components to keep (``1..=num_components``).

        Returns
        -------
        YieldPcaView
            Plain nested-list view.

        Raises
        ------
        ValueError
            If ``n_components`` is zero or exceeds ``num_components``.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPca
        >>> YieldPca.fit_yield_changes([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]]).truncated(1).num_components
        1
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Return the loadings as a DataFrame indexed by tenor.

        Returns
        -------
        pandas.DataFrame
            Columns ``PC1, PC2, ...``; index is the tenor grid.

        Raises
        ------
        ValueError
            If the result cannot be converted to the tabular form.
        ImportError
            If pandas is not installed in the environment.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPca
        >>> YieldPca.fit_yield_changes([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]]).to_dataframe().columns.tolist()
        ['PC1', 'PC2']
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with eigenvalues, loadings, scores and tenors.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPca
        >>> pca = YieldPca.fit_yield_changes([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]])
        >>> YieldPca.from_json(pca.to_json()).num_components == pca.num_components
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> YieldPca:
        """Deserialize a decomposition produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        YieldPca
            The reconstructed decomposition.

        Raises
        ------
        ValueError
            If the payload is malformed or its loadings, scores, eigenvalues,
            means, or variance vectors are invalid or misaligned.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPca
        >>> pca = YieldPca.fit_yield_changes([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]])
        >>> YieldPca.from_json(pca.to_json()).tenors
        [1.0, 2.0]
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class YieldPcaView:
    """Leading components of a :class:`YieldPca` fit in plain nested-list form.

    ``explained_variance_ratio`` is the per-component variance share (the
    ``variance_explained`` accessor on :class:`YieldPca`); ``cumulative_variance``
    accumulates it. ``tenors`` are placeholders ``1..N`` when the fit came from
    yield changes rather than a dated panel.

    Examples
    --------
    >>> from finstack_quant.models.rates.dtsm import yield_pca_fit
    >>> view = yield_pca_fit([[0.001, 0.002, 0.003], [0.002, 0.001, 0.002], [-0.001, 0.0, 0.001]], 2)
    >>> (view.num_components, len(view.explained_variance_ratio), view.tenors)
    (2, 2, [1.0, 2.0, 3.0])
    """

    @property
    def loadings(self) -> list[list[float]]:
        """
        How each tenor moves with each retained component.

        Returns
        -------
        list[list[float]]
            ``loadings[tenor][k]``: unit-norm eigenvector weight of tenor ``tenor`` on component ``k``, restricted to the components in this view.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scores(self) -> list[list[float]]:
        """
        Component time series over the sample, for the retained components.

        Returns
        -------
        list[list[float]]
            ``scores[t][k]``: projection of observation ``t`` onto component ``k``, in decimal yield-change units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def eigenvalues(self) -> list[float]:
        """
        Variance carried by each retained component.

        Returns
        -------
        list[float]
            Eigenvalues in squared decimal yield units, sorted descending, truncated to the components in this view.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def explained_variance_ratio(self) -> list[float]:
        """
        Share of curve variance attributable to each retained component.

        Returns
        -------
        list[float]
            One decimal fraction per retained component, measured against the total variance of the full decomposition.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cumulative_variance(self) -> list[float]:
        """
        Running total of explained variance across retained components.

        Returns
        -------
        list[float]
            Non-decreasing decimal fractions; the last entry is the share of variance this view captures.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mean_change(self) -> list[float]:
        """
        Sample mean removed to centre the data before decomposition.

        Returns
        -------
        list[float]
            One mean yield change per tenor in decimal units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tenors(self) -> list[float]:
        """
        Tenor labels for the loading rows of this view.

        Returns
        -------
        list[float]
            Tenors in years, aligned with the rows of :attr:`loadings`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_components(self) -> int:
        """
        How many components this view exposes.

        Returns
        -------
        int
            Component count retained by the view; at most the number produced by the underlying fit.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Return the loadings as a DataFrame indexed by tenor.

        Returns
        -------
        pandas.DataFrame
            Columns ``PC1, PC2, ...``.

        Raises
        ------
        ValueError
            If the result cannot be converted to the tabular form.
        ImportError
            If pandas is not installed in the environment.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import yield_pca_fit
        >>> yield_pca_fit([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]], 1).to_dataframe().shape
        (2, 1)
        """
        ...

    def to_scores_dataframe(self) -> pd.DataFrame:
        """Return the scores as a DataFrame (one row per yield-change observation).

        Returns
        -------
        pandas.DataFrame
            Columns ``PC1, PC2, ...``.

        Raises
        ------
        ValueError
            If the result cannot be converted to the tabular form.
        ImportError
            If pandas is not installed in the environment.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import yield_pca_fit
        >>> yield_pca_fit([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]], 1).to_scores_dataframe().shape
        (3, 1)
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with the view fields.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPcaView, yield_pca_fit
        >>> view = yield_pca_fit([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]], 1)
        >>> YieldPcaView.from_json(view.to_json()) == view
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> YieldPcaView:
        """Deserialize a view produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        YieldPcaView
            The reconstructed view.

        Raises
        ------
        ValueError
            If the payload is malformed or its loadings, scores, eigenvalues,
            means, or variance vectors are invalid or misaligned.

        Examples
        --------
        >>> from finstack_quant.models.rates.dtsm import YieldPcaView, yield_pca_fit
        >>> view = yield_pca_fit([[0.001, 0.002], [0.002, 0.001], [-0.001, 0.0]], 1)
        >>> YieldPcaView.from_json(view.to_json()).num_components
        1
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

def diebold_li_fit_factors(
    tenors: Sequence[float],
    yields_matrix: Sequence[Sequence[float]],
    lambda_: float | None = None,
) -> FactorTimeSeries:
    """
    Extract Nelson-Siegel factors from a yield panel via Diebold-Li (2006).

    Thin twin of ``DieboldLi(lambda_).extract_factors(YieldPanel(tenors, yields_matrix)).factors``.

    Parameters
    ----------
    tenors : Sequence[float]
        Tenor grid in years, length ``N``, strictly ascending and all positive.
    yields_matrix : Sequence[Sequence[float]]
        Yield panel ``yields_matrix[date_idx][tenor_idx]`` with ``T`` rows of
        ``N`` continuously compounded zero rates each.
    lambda_ : float or None, default None
        Diebold-Li decay parameter for tenors **in years**; ``None`` uses the
        Rust default ``0.7308``. Named ``lambda_`` because ``lambda`` is a
        Python keyword.

    Returns
    -------
    FactorTimeSeries
        Level/slope/curvature per date, residuals and R-squared, with
        ``to_dataframe()``.

    Raises
    ------
    ValueError
        If tenors or the yield panel are malformed, non-finite, have fewer
        than three tenors, or ``lambda_`` is invalid.

    Sources
    -------
    See ``docs/REFERENCES.md#diebold-li-2006``.

    Examples
    --------
    >>> from finstack_quant.models.rates.dtsm import diebold_li_fit_factors
    >>> tenors = [1.0, 2.0, 5.0, 10.0]
    >>> yields = [[0.02, 0.025, 0.03, 0.035], [0.021, 0.026, 0.031, 0.036]]
    >>> len(diebold_li_fit_factors(tenors, yields).level)
    2

    """
    ...

def diebold_li_forecast(
    tenors: Sequence[float],
    yields_matrix: Sequence[Sequence[float]],
    horizon: int,
    lambda_: float | None = None,
) -> YieldForecast:
    """
    VAR(1) forecast of Diebold-Li factors and yields out to ``horizon`` periods.

    Thin twin of ``DieboldLi(lambda_).fit(panel).forecast(horizon)``.

    Parameters
    ----------
    tenors : Sequence[float]
        Tenor grid in years, length ``N``.
    yields_matrix : Sequence[Sequence[float]]
        Yield panel ``yields_matrix[date_idx][tenor_idx]`` (at least five rows
        for the VAR fit).
    horizon : int
        Forecast horizon in observation periods (must be ``>= 1``).
    lambda_ : float or None, default None
        Diebold-Li decay for tenors in years; ``None`` uses the Rust default.

    Returns
    -------
    YieldForecast
        Point forecast, factor triple and 95% bands with ``to_dataframe()``.

    Raises
    ------
    ValueError
        If inputs are invalid, the panel is too short for the VAR fit,
        ``horizon`` is zero, or ``lambda_`` is invalid.

    Sources
    -------
    See ``docs/REFERENCES.md#diebold-li-2006``.

    Examples
    --------
    >>> from finstack_quant.models.rates.dtsm import diebold_li_forecast
    >>> tenors = [1.0, 2.0, 5.0, 10.0]
    >>> yields = [
    ...     [0.02, 0.025, 0.03, 0.035],
    ...     [0.021, 0.024, 0.031, 0.034],
    ...     [0.019, 0.026, 0.029, 0.036],
    ...     [0.022, 0.025, 0.032, 0.033],
    ...     [0.020, 0.027, 0.030, 0.037],
    ...     [0.023, 0.026, 0.033, 0.035],
    ... ]
    >>> fc = diebold_li_forecast(tenors, yields, 2)
    >>> (fc.horizon, len(fc.yields))
    (2, 4)

    """
    ...

def nelson_siegel_yields(
    lambda_: float,
    factors: tuple[float, float, float],
    tenors: Sequence[float],
) -> list[float]:
    """
    Evaluate the static Nelson-Siegel (1987) curve for one factor triple.

    This is the Diebold-Li cross-sectional equation for a single date::

        y(tau) = beta1 + beta2 * s(tau) + beta3 * (s(tau) - exp(-lambda * tau))
        s(tau) = (1 - exp(-lambda * tau)) / (lambda * tau)

    Use it to reconstruct a fitted or forecast curve from the factors returned
    by :class:`DieboldLi` or :func:`diebold_li_forecast`.

    Parameters
    ----------
    lambda_ : float
        Exponential decay parameter for tenors **in years**; must be finite and
        strictly positive. ``0.7308`` is the years-equivalent of Diebold-Li's
        canonical ``0.0609`` months value and places the curvature peak at
        about 2.45 years. Named ``lambda_`` because ``lambda`` is a Python
        keyword.
    factors : tuple[float, float, float]
        The triple ``(beta1, beta2, beta3)`` = ``(level, slope, curvature)`` in
        decimal yield units (``0.045`` for 4.5%). All three must be finite.
    tenors : Sequence[float]
        Maturities in years, each finite and non-negative. Order is preserved in
        the output; no sorting or de-duplication is applied.

    Returns
    -------
    list[float]
        Fitted yields in decimal units, one per input tenor and in the same
        order as ``tenors``.

    Raises
    ------
    ValueError
        If ``lambda_`` is non-positive or non-finite, a factor is
        non-finite, or a tenor is negative or non-finite.

    Sources
    -------
    See ``docs/REFERENCES.md#diebold-li-2006``.

    Examples
    --------
    >>> from finstack_quant.models.rates.dtsm import nelson_siegel_yields
    >>> len(nelson_siegel_yields(0.7308, (0.03, -0.01, 0.005), [1.0, 5.0, 10.0]))
    3

    """
    ...

def yield_pca_fit(
    yield_changes: Sequence[Sequence[float]],
    n_components: int = 3,
) -> YieldPcaView:
    """
    PCA decomposition of a yield-change panel.

    Thin twin of ``YieldPca.fit_yield_changes(yield_changes).truncated(n_components)``.

    Parameters
    ----------
    yield_changes : Sequence[Sequence[float]]
        Panel of yield changes ``yield_changes[date_idx][tenor_idx]`` in decimal
        units (e.g. ``0.001`` for a 10 bp move).
    n_components : int, default 3
        Number of principal components to retain (``1..=min(T-1, N)``).

    Returns
    -------
    YieldPcaView
        Eigenvalues, ``explained_variance_ratio``, ``cumulative_variance``,
        loadings per tenor and scores, with ``to_dataframe()``. The ``tenors``
        are placeholders ``1..N`` because yield changes do not identify the
        maturities.

    Raises
    ------
    ValueError
        If the panel is empty, ragged, non-finite, or ``n_components`` is invalid.

    Examples
    --------
    >>> from finstack_quant.models.rates.dtsm import yield_pca_fit
    >>> changes = [[0.001, 0.002, 0.003], [0.002, 0.001, 0.002], [-0.001, 0.0, 0.001]]
    >>> len(yield_pca_fit(changes, 2).eigenvalues)
    2

    """
    ...

def yield_pca_scenario(
    yield_changes: Sequence[Sequence[float]],
    component_index: int,
    sigma_shock: float,
    n_components: int = 3,
) -> list[float]:
    """
    Apply a single-component N-sigma PCA shock to the mean yield curve.

    Parameters
    ----------
    yield_changes : Sequence[Sequence[float]]
        Historical yield-change panel used to fit PCA (same shape as
        :func:`yield_pca_fit`).
    component_index : int
        Zero-based principal component index to shock.
    sigma_shock : float
        Shock size in standard deviations (e.g. ``2.0`` for a +2σ move).
    n_components : int, default 3
        Number of components used in the PCA fit.

    Returns
    -------
    list[float]
        Scenario yield shift per tenor (decimal units), length equal to the
        number of columns in ``yield_changes``.

    Raises
    ------
    ValueError
        If PCA fitting fails or ``component_index`` is out of range.

    Examples
    --------
    >>> from finstack_quant.models.rates.dtsm import yield_pca_scenario
    >>> changes = [[0.001, 0.002, 0.003], [0.002, 0.001, 0.002], [-0.001, 0.0, 0.001]]
    >>> len(yield_pca_scenario(changes, 0, 1.0, 2))
    3

    """
    ...
