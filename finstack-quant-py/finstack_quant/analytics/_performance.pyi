"""Stub declarations mirroring ``finstack-quant-py/src/bindings/analytics/performance.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Sequence
from finstack_quant.core.dates import FiscalConfig
import numpy as np
import numpy.typing as npt
import pandas as pd
from finstack_quant.core.dates import DayCount
from finstack_quant.analytics._types import (
    BetaResult,
    DatedSeries,
    DrawdownEpisode,
    GreeksResult,
    LookbackReturns,
    MultiFactorResult,
    PeriodStats,
    RollingGreeks,
)

# Performance engine

class Performance:
    """
    Stateful performance analytics engine over a panel of ticker series.

    Construct from a pandas DataFrame or Series of prices
    (``Performance(df)``), a DataFrame or Series of returns
    (``Performance.from_returns(df)``), or from raw arrays via
    :meth:`from_arrays` / :meth:`from_returns_arrays`.

    Scalar-per-ticker metrics return a ``pandas.Series`` indexed by ticker name
    and named after the metric, so results are selected by label rather than by
    column position, and ``pd.concat([perf.sharpe(), perf.sortino()], axis=1)``
    yields correctly-named columns.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.analytics import Performance
    >>> prices = pd.DataFrame(
    ...     {"FUND": [100.0, 101.0, 103.0]},
    ...     index=pd.to_datetime(["2024-01-01", "2024-01-02", "2024-01-03"]),
    ... )
    >>> Performance(prices).get_ticker_names()
    ['FUND']
    """

    def __init__(
        self,
        prices: pd.DataFrame | pd.Series,
        benchmark_ticker: str | None = None,
        frequency: str | None = None,
    ) -> None:
        """
        Build from a pandas DataFrame (or Series) of prices.

        Parameters
        ----------
        prices : pandas.DataFrame or pandas.Series
            Price panel with a date-like index (``datetime.date`` or
            ``pd.Timestamp``) and one column per ticker. A ``Series`` is a
            single-asset panel whose ticker is the series ``name`` (``"asset"``
            when unnamed).
        benchmark_ticker : str, optional
            Benchmark column name. Defaults to the first column when ``None``.
        frequency : str, optional
            Return aggregation frequency. One of ``"daily"``, ``"weekly"``,
            ``"monthly"``, ``"quarterly"``, ``"semi_annual"``, or ``"annual"``. Sets the annualization factor
            (252, 52, 12, 4, 2, 1).
            ``None`` uses the Rust default ``DEFAULT_FREQUENCY`` (daily).

        Raises
        ------
        AnalyticsError
            If ``prices`` is invalid or empty, or dates are not strictly
            ascending.
        KeyError
            If ``benchmark_ticker`` is given but is not one of the column labels.
        TypeError
            If ``prices`` is not a pandas ``DataFrame`` or ``Series`` (use
            :meth:`from_arrays` for raw lists), or its column labels are not
            all ``str`` (rename with ``df.columns = df.columns.astype(str)``).

        """

    @staticmethod
    def from_arrays(
        dates: Sequence[object],
        prices: list[list[float]],
        ticker_names: list[str],
        benchmark_ticker: str | None = None,
        frequency: str | None = None,
    ) -> Performance:
        """
        Construct from raw arrays (dates, prices matrix, ticker names).

        Parameters
        ----------
        dates : sequence
            Observation dates as ``datetime.date``, ``pd.Timestamp``, or ISO
            strings parseable by the binding layer.
        prices : list[list[float]]
            Column-major price matrix; ``prices[i]`` is the series for ticker
            ``ticker_names[i]``.
        ticker_names : list[str]
            Column labels, one per price series.
        benchmark_ticker : str, optional
            Benchmark ticker name. Defaults to the first column when ``None``.
        frequency : str, optional
            One of ``"daily"``, ``"weekly"``, ``"monthly"``, ``"quarterly"``,
            ``"semi_annual"``, or ``"annual"``.
            Default ``"daily"``.

        Returns
        -------
        Performance
            Analytics engine over the supplied panel.

        Raises
        ------
        AnalyticsError
            If dimensions are inconsistent, dates are invalid, or ``frequency`` is
            unrecognized.
        KeyError
            If ``benchmark_ticker`` is given but is not in ``ticker_names``.

        Examples
        --------
        >>> from datetime import date
        >>> from finstack_quant.analytics import Performance
        >>> perf = Performance.from_arrays(
        ...     [date(2024, 1, 1), date(2024, 1, 2), date(2024, 1, 3)],
        ...     [[100.0, 101.0, 103.0]],
        ...     ["FUND"],
        ... )
        >>> perf.get_ticker_names()
        ['FUND']
        """

    @staticmethod
    def from_returns(
        returns: pd.DataFrame | pd.Series,
        benchmark_ticker: str | None = None,
        frequency: str | None = None,
    ) -> Performance:
        """
        Build from a pandas DataFrame (or Series) of simple returns.

        Parameters
        ----------
        returns : pandas.DataFrame or pandas.Series
            Simple-return panel aligned with a date-like index and one column per
            ticker (decimal returns, e.g. ``0.01`` for +1%). A ``Series`` is a
            single-asset panel whose ticker is the series ``name`` (``"asset"``
            when unnamed).
        benchmark_ticker : str, optional
            Benchmark column name. Defaults to the first column when ``None``.
        frequency : str, optional
            One of ``"daily"``, ``"weekly"``, ``"monthly"``, ``"quarterly"``,
            ``"semi_annual"``, or ``"annual"``.
            Default ``"daily"``.

        Raises
        ------
        AnalyticsError
            If ``returns`` is invalid or empty.
        TypeError
            If ``returns`` is not a pandas ``DataFrame`` or ``Series`` (use
            :meth:`from_returns_arrays` for raw lists).
        KeyError
            If ``benchmark_ticker`` is given but is not one of the column labels.

        Returns
        -------
        Performance
            Analytics engine over the supplied return panel.

        Examples
        --------
        >>> import pandas as pd
        >>> from finstack_quant.analytics import Performance
        >>> returns = pd.DataFrame(
        ...     {"FUND": [0.01, 0.02]},
        ...     index=pd.to_datetime(["2024-01-01", "2024-01-02"]),
        ... )
        >>> Performance.from_returns(returns).get_ticker_names()
        ['FUND']
        """

    @staticmethod
    def from_returns_arrays(
        dates: Sequence[object],
        returns: list[list[float]],
        ticker_names: list[str],
        benchmark_ticker: str | None = None,
        frequency: str | None = None,
    ) -> Performance:
        """
        Construct from raw return arrays (dates, returns matrix, ticker names).

        Parameters
        ----------
        dates : sequence
            Return observation dates.
        returns : list[list[float]]
            Column-major simple-return matrix; ``returns[i]`` is the series for
            ``ticker_names[i]``.
        ticker_names : list[str]
            Column labels.
        benchmark_ticker : str, optional
            Benchmark ticker name.
        frequency : str, optional
            One of ``"daily"``, ``"weekly"``, ``"monthly"``, ``"quarterly"``,
            ``"semi_annual"``, or ``"annual"``.
            Default ``"daily"``.

        Returns
        -------
        Performance
            Analytics engine over the supplied return panel.

        Raises
        ------
        AnalyticsError
            If dimensions are inconsistent or ``frequency`` is unrecognized.
        KeyError
            If ``benchmark_ticker`` is given but is not in ``ticker_names``.

        Examples
        --------
        >>> from datetime import date
        >>> from finstack_quant.analytics import Performance
        >>> perf = Performance.from_returns_arrays([date(2024, 1, 1), date(2024, 1, 2)], [[0.01, 0.02]], ["FUND"])
        >>> perf.get_ticker_names()
        ['FUND']
        """

    # -- Mutators --

    def reset_date_range(self, start: datetime.date | str, end: datetime.date | str) -> None:
        """
        Restrict analytics to ``[start, end]``.

        Parameters
        ----------
        start : datetime.date | datetime.datetime | pandas.Timestamp | str
            Inclusive start date; strings are ISO 8601 (``YYYY-MM-DD``).
        end : datetime.date | datetime.datetime | pandas.Timestamp | str
            Inclusive end date, in the same forms as ``start``.

        Raises
        ------
        TypeError
            If ``start`` or ``end`` is not a date-like object or string.
        ValueError
            If a string is not a valid ISO calendar date.
        """

    def reset_bench_ticker(self, ticker: str) -> None:
        """
        Change the benchmark ticker.

        Parameters
        ----------
        ticker : str
            New benchmark column name.

        Raises
        ------
        KeyError
            If ``ticker`` is not a loaded ticker name; the benchmark is unchanged.
        """

    # -- Getters --

    def get_ticker_names(self) -> list[str]:
        """
        Ticker names in column order.

        Returns
        -------
        list[str]
            Column labels from the input panel.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def get_ticker_index(self, ticker: str) -> int:
        """
        Resolve an exact ticker name to its zero-based column index.

        Parameters
        ----------
        ticker : str
            Case-sensitive column label supplied when constructing the panel.

        Returns
        -------
        int
            Zero-based index in ``get_ticker_names()`` order.

        Raises
        ------
        KeyError
            If ``ticker`` is not a loaded column label.
        """

    def get_benchmark_idx(self) -> int:
        """
        Zero-based column index of the benchmark series.

        Returns
        -------
        int
            Zero-based column index of the benchmark series.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def get_frequency(self) -> str:
        """
        Observation frequency as the canonical lowercase token.

        Returns
        -------
        str
            One of ``"daily"``, ``"weekly"``, ``"monthly"``, etc.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def get_dates(self) -> list[datetime.date]:
        """
        Full return-aligned date grid (independent of any active window).

        Returns
        -------
        list[datetime.date]
            All observation dates in the panel.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def get_active_dates(self) -> list[datetime.date]:
        """
        Observation dates of the currently active analysis window.

        Returns
        -------
        list[datetime.date]
            Dates within the active ``[start, end]`` range.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def get_active_dates_for_ticker(self, ticker_idx: int | str) -> list[datetime.date]:
        """
        Observation dates for one ticker's active return series.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index, or a ticker name resolved through
            Rust ``Performance::get_ticker_index``.

        Returns
        -------
        list[datetime.date]
            Dates where the specified ticker has valid returns.

        Raises
        ------
        AnalyticsError
            If an integer ``ticker_idx`` is outside the loaded ticker columns.
        KeyError
            If a string ``ticker_idx`` is not a loaded ticker name.
        """

    # -- Scalar-per-ticker methods --

    def cagr(
        self,
        day_count: str | DayCount | None = None,
        calendar_id: str | None = None,
    ) -> pd.Series:
        """
        Compound annual growth rate for each ticker.

        The default convention is Act/365.25. Pass ``"act365_25"`` for the
        same default, a core DayCount name such as ``"act_365f"`` or
        ``"bus_252"``, or a :class:`~finstack_quant.core.dates.DayCount`.
        ``bus_252`` requires ``calendar_id``.

        Parameters
        ----------
        day_count : str or DayCount, optional
            ``None`` / ``"act365_25"`` for Act/365.25, or a core DayCount
            name / instance (``"act_365f"``, ``DayCount.ACT_365F``,
            ``"bus_252"``, …).
        calendar_id : str, optional
            Holiday-calendar id, or ``+``-joined ids for a union calendar
            (``"nyse+gblo"``); required when ``day_count`` is Bus/252.

        Returns
        -------
        pd.Series
            Compound annual growth rate indexed by ticker name.

        Raises
        ------
        ValueError
            If ``day_count`` is not a recognized convention.
        KeyError
            If ``calendar_id`` is set but cannot be resolved.
        AnalyticsError
            If the active date window cannot be annualized, or Bus/252 is
            requested without a calendar.
        """

    def mean_return(self, annualize: bool = True) -> pd.Series:
        """
        Mean return for each ticker.

        Parameters
        ----------
        annualize : bool, default True
            Whether to annualize the mean return.

        Returns
        -------
        pd.Series
            Mean return indexed by ticker name.

        Raises
        ------
        ValueError
            If the computed values cannot be wrapped as a labelled pandas object.
        """

    def volatility(self, annualize: bool = True) -> pd.Series:
        """
        Volatility for each ticker.

        Parameters
        ----------
        annualize : bool, default True
            Whether to annualize the volatility.

        Returns
        -------
        pd.Series
            Standard deviation of returns indexed by ticker name.

        Raises
        ------
        ValueError
            If the computed values cannot be wrapped as a labelled pandas object.
        """

    def sharpe(self, risk_free_rate: float = 0.0) -> pd.Series:
        """
        Sharpe ratio for each ticker.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal (``0.02`` for 2%),
            geometrically decompounded to the panel frequency before it is
            subtracted from the per-period mean.

        Returns
        -------
        pd.Series
            Sharpe ratios over the active return window, indexed by ticker name.

        Raises
        ------
        ValueError
            If the computed values cannot be wrapped as a labelled pandas object.

        Examples
        --------
        >>> from datetime import date
        >>> from finstack_quant.analytics import Performance
        >>> perf = Performance.from_returns_arrays([date(2024, 1, 1), date(2024, 1, 2)], [[0.01, 0.02]], ["FUND"])
        >>> sharpe = perf.sharpe()
        >>> (sharpe.name, list(sharpe.index))
        ('sharpe', ['FUND'])

        Sources
        -------
        - Sharpe (1966): see docs/REFERENCES.md#sharpe1966
        """

    def sortino(self, mar: float = 0.0) -> pd.Series:
        """
        Sortino ratio for each ticker.

        Parameters
        ----------
        mar : float, default 0.0
            Minimum acceptable return **per period** as a decimal (not
            annualized; e.g. ``0.0002`` for ~5% p.a. on daily data).

        Returns
        -------
        pd.Series
            Sortino ratios over the active return window, indexed by ticker name.

        Notes
        -----
        ``mar`` is per-period; Sharpe ``risk_free_rate`` inputs are annualized.

        Raises
        ------
        ValueError
            If the computed values cannot be wrapped as a labelled pandas object.

        Sources
        -------
        - Sortino and van der Meer (1991): see docs/REFERENCES.md#sortinoVanDerMeer1991
        """

    def calmar(self) -> pd.Series:
        """
        Calmar ratio for each ticker over the active window.

        This is CAGR / |max drawdown| on the loaded (or reset) date range,
        not Young's 36-month CTA definition.

        Returns
        -------
        pd.Series
            CAGR divided by absolute max drawdown indexed by ticker name.
            With zero drawdown, nonzero CAGR gives signed infinity and zero
            CAGR gives ``0.0``.

        Raises
        ------
        ValueError
            If the active date window cannot be annualized.

        Sources
        -------
        - Young (1991): see docs/REFERENCES.md#youngCalmar1991
        """

    def max_drawdown(self) -> pd.Series:
        """
        Max drawdown for each ticker.

        Returns
        -------
        pd.Series
            Peak-to-trough drawdown (negative), indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def mean_drawdown(self) -> pd.Series:
        """
        Mean drawdown (path-weighted average) for each ticker.

        Returns
        -------
        pd.Series
            Average drawdown (negative), indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def value_at_risk(self, confidence: float = 0.95) -> pd.Series:
        """
        Historical VaR for each ticker.

        Parameters
        ----------
        confidence : float, default 0.95
            Confidence level in the open interval ``(0, 1)`` (``0.95`` for
            95% VaR).

        Returns
        -------
        pd.Series
            Historical VaR (negative decimal), indexed by ticker name.

        Raises
        ------
        AnalyticsError
            If ``confidence`` is not strictly inside ``(0, 1)``.

        Sources
        -------
        - J.P. Morgan RiskMetrics (1996): see docs/REFERENCES.md#jpmorgan1996RiskMetrics
        """

    def expected_shortfall(self, confidence: float = 0.95) -> pd.Series:
        """
        Expected Shortfall for each ticker.

        Parameters
        ----------
        confidence : float, default 0.95
            Confidence level in the open interval ``(0, 1)``.

        Returns
        -------
        pd.Series
            Mean of exactly the worst ``1-confidence`` empirical probability mass,
            with fractional boundary weighting; negative for losses, indexed by ticker.

        Raises
        ------
        AnalyticsError
            If ``confidence`` is not strictly inside ``(0, 1)``.

        Sources
        -------
        - Artzner, Delbaen, Eber, and Heath (1999): see
          docs/REFERENCES.md#artzner1999CoherentRisk
        """

    def tracking_error(self) -> pd.Series:
        """
        Tracking error for each ticker vs benchmark.

        Returns
        -------
        pd.Series
            Annualized standard deviation of excess returns indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.

        Sources
        -------
        - Grinold and Kahn (1999): see docs/REFERENCES.md#grinoldKahn1999ActivePortfolio
        """

    def information_ratio(self) -> pd.Series:
        """
        Information ratio for each ticker vs benchmark.

        Returns
        -------
        pd.Series
            Annualized excess return divided by tracking error indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.

        Sources
        -------
        - Grinold and Kahn (1999): see docs/REFERENCES.md#grinoldKahn1999ActivePortfolio
        """

    def skewness(self) -> pd.Series:
        """
        Skewness for each ticker.

        Returns
        -------
        pd.Series
            Third moment of returns indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.

        Sources
        -------
        - Joanes and Gill (1998): see docs/REFERENCES.md#joanesGill1998
        """

    def kurtosis(self) -> pd.Series:
        """
        Kurtosis for each ticker.

        Returns
        -------
        pd.Series
            Fourth moment of returns indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.

        Sources
        -------
        - Joanes and Gill (1998): see docs/REFERENCES.md#joanesGill1998
        """

    def geometric_mean(self) -> pd.Series:
        """
        Geometric mean for each ticker.

        Returns
        -------
        pd.Series
            Geometric mean return indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def downside_deviation(self, mar: float = 0.0) -> pd.Series:
        """
        Downside deviation for each ticker.

        ``mar`` is per-period; Sharpe risk-free inputs are annualized.

        Parameters
        ----------
        mar : float, default 0.0
            Minimum acceptable return per period.

        Returns
        -------
        pd.Series
            Downside deviation indexed by ticker name.

        Raises
        ------
        ValueError
            If the computed values cannot be wrapped as a labelled pandas object.
        """

    def max_drawdown_duration(self) -> pd.Series:
        """
        Max drawdown duration (calendar days) for each ticker.

        Returns
        -------
        pd.Series
            Longest drawdown duration in calendar days, indexed by ticker name
            and kept at an integer dtype.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def up_capture(self) -> pd.Series:
        """
        Empyrical-style annualized geometric up-capture vs benchmark.

        Returns
        -------
        pd.Series
            Up-capture ratio indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def down_capture(self) -> pd.Series:
        """
        Empyrical-style annualized geometric down-capture vs benchmark.

        Returns
        -------
        pd.Series
            Down-capture ratio indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def capture_ratio(self) -> pd.Series:
        """
        Empyrical-style annualized geometric capture ratio vs benchmark.

        Returns
        -------
        pd.Series
            Up-capture divided by down-capture indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def omega_ratio(self, threshold: float = 0.0) -> pd.Series:
        """
        Omega ratio for each ticker.

        Parameters
        ----------
        threshold : float, default 0.0
            Return threshold for the gain/loss split.

        Returns
        -------
        pd.Series
            Omega ratio indexed by ticker name.

        Raises
        ------
        ValueError
            If the computed values cannot be wrapped as a labelled pandas object.

        Sources
        -------
        - Keating and Shadwick (2002): see docs/REFERENCES.md#keatingShadwick2002
        """

    def treynor(self, risk_free_rate: float = 0.0) -> pd.Series:
        """
        Treynor ratio for each ticker.

        A non-finite risk-free rate gives ``NaN``. With absolute beta below
        ``1e-10``, nonzero finite excess return gives signed infinity and
        zero excess return gives ``0``.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.

        Returns
        -------
        pd.Series
            Excess return per unit of beta indexed by ticker name.

        Raises
        ------
        ValueError
            If the computed values cannot be wrapped as a labelled pandas object.

        Sources
        -------
        - Treynor (1965): see docs/REFERENCES.md#treynor1965
        """

    def gain_to_pain(self) -> pd.Series:
        """
        Gain-to-pain ratio for each ticker.

        Returns
        -------
        pd.Series
            Sum of all returns divided by the sum of absolute losses, indexed by
            ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.

        Sources
        -------
        - Schwager (2012): see docs/REFERENCES.md#schwager2012
        """

    def ulcer_index(self) -> pd.Series:
        """
        Ulcer index for each ticker.

        Returns
        -------
        pd.Series
            Root-mean-square of drawdown depths indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.

        Sources
        -------
        - Martin (1987): see docs/REFERENCES.md#martinUlcer1987
        """

    def martin_ratio(self) -> pd.Series:
        """
        Martin ratio (CAGR divided by Ulcer Index) for each ticker.

        Returns
        -------
        pd.Series
            CAGR divided by Ulcer Index indexed by ticker name. With zero
            Ulcer Index, nonzero CAGR gives signed infinity and zero CAGR
            gives ``0.0``.

        Raises
        ------
        ValueError
            If the active date window cannot be annualized.

        Sources
        -------
        - Martin (1987): see docs/REFERENCES.md#martinUlcer1987
        """

    def recovery_factor(self) -> pd.Series:
        """
        Recovery factor for each ticker.

        Returns
        -------
        pd.Series
            Total return divided by max drawdown indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def pain_index(self) -> pd.Series:
        """
        Pain index for each ticker.

        Returns
        -------
        pd.Series
            Average drawdown depth indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.

        Sources
        -------
        - Schwager (2012): see docs/REFERENCES.md#schwager2012
        """

    def pain_ratio(self, risk_free_rate: float = 0.0) -> pd.Series:
        """
        Pain ratio for each ticker.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.

        Returns
        -------
        pd.Series
            CAGR minus the annualized risk-free rate, divided by the pain
            index and indexed by ticker name. Non-finite risk-free rates
            give ``NaN``, including when the pain index is zero.

        Raises
        ------
        ValueError
            If the active date window cannot be annualized.

        Sources
        -------
        - Schwager (2012): see docs/REFERENCES.md#schwager2012
        """

    def tail_ratio(self, confidence: float = 0.95) -> pd.Series:
        """
        Tail ratio for each ticker.

        Parameters
        ----------
        confidence : float, default 0.95
            Confidence level for the tail quantile, in ``(0, 1)``.

        Returns
        -------
        pd.Series
            Right-tail gain divided by left-tail loss indexed by ticker name.

        Raises
        ------
        AnalyticsError
            If ``confidence`` is not strictly inside ``(0, 1)``.

        Notes
        -----
        The upper tail uses the ``confidence`` quantile and the lower tail uses
        ``1 - confidence``. A zero lower tail with a positive upper tail returns
        ``inf``; both tails zero returns ``NaN``.
        """

    def r_squared(self) -> pd.Series:
        """
        R-squared for each ticker vs benchmark.

        Returns
        -------
        pd.Series
            Coefficient of determination indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def batting_average(self) -> pd.Series:
        """
        Batting average for each ticker vs benchmark.

        Returns
        -------
        pd.Series
            Fraction of periods where the ticker outperformed the benchmark,
            indexed by ticker name.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def parametric_var(
        self,
        confidence: float = 0.95,
        horizon_periods: float | None = None,
    ) -> pd.Series:
        """
        Equal-weight Gaussian VaR for each ticker.

        ``horizon_periods=None`` is one-period VaR. A positive ``h`` scales
        the mean by ``h`` and volatility by ``sqrt(h)``. Empty or invalid
        series return ``NaN``.

        Parameters
        ----------
        confidence : float, default 0.95
            Tail confidence as a decimal probability in ``(0, 1)``.
        horizon_periods : float, optional
            Horizon in observation periods. ``None`` is one period.

        Returns
        -------
        pd.Series
            Parametric VaR (negative decimal), indexed by ticker name.

        Raises
        ------
        AnalyticsError
            If ``confidence`` is not strictly inside ``(0, 1)``.
        """

    def cornish_fisher_var(
        self,
        confidence: float = 0.95,
        horizon_periods: float | None = None,
    ) -> pd.Series:
        """
        Cornish-Fisher VaR for each ticker.

        ``horizon_periods=None`` is one-period VaR. A positive ``h`` scales
        the Cornish–Fisher moments to that horizon. Empty or invalid series
        return ``NaN``.

        Parameters
        ----------
        confidence : float, default 0.95
            Tail confidence as a decimal probability in ``(0, 1)``.
        horizon_periods : float, optional
            Horizon in observation periods. ``None`` is one period.

        Returns
        -------
        pd.Series
            Cornish-Fisher modified VaR (negative decimal), indexed by ticker name.

        Raises
        ------
        AnalyticsError
            If ``confidence`` is not strictly inside ``(0, 1)``.

        Sources
        -------
        - Cornish and Fisher (1937): see docs/REFERENCES.md#cornishFisher1937
        """

    def cdar(self, confidence: float = 0.95) -> pd.Series:
        """
        Conditional drawdown at risk for each ticker.

        Parameters
        ----------
        confidence : float, default 0.95
            Confidence level in the open interval ``(0, 1)``.

        Returns
        -------
        pd.Series
            Conditional drawdown-at-risk (negative decimal), indexed by ticker name.

        Raises
        ------
        AnalyticsError
            If ``confidence`` is not strictly inside ``(0, 1)``.

        Sources
        -------
        - Chekhlov, Uryasev, and Zabarankin (2005): see docs/REFERENCES.md#chekhlov2005
        """

    def m_squared(self, risk_free_rate: float = 0.0) -> pd.Series:
        """
        M-squared for each ticker.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.

        Returns
        -------
        pd.Series
            Linearly annualized M-squared indexed by ticker name. Both cash
            subtraction and addition use the decompounded period cash rate
            multiplied by periods per year.

        Raises
        ------
        ValueError
            If the computed values cannot be wrapped as a labelled pandas object.

        Sources
        -------
        - Modigliani and Modigliani (1997): see docs/REFERENCES.md#modigliani1997
        """

    def modified_sharpe(
        self,
        risk_free_rate: float = 0.0,
        confidence: float = 0.95,
    ) -> pd.Series:
        """
        Modified Sharpe ratio for each ticker.

        The numerator is annualized excess return and the denominator is
        Cornish-Fisher VaR at the corresponding annual horizon. The panel
        frequency supplies the periods-per-year scaling for both terms,
        including the horizon decay of skewness and excess kurtosis; this
        method does not divide an annualized numerator by one-period VaR.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal, decompounded to the panel
            frequency before constructing annualized excess return.
        confidence : float, default 0.95
            Confidence level in ``(0, 1)`` for annual-horizon Cornish-Fisher VaR.

        Returns
        -------
        pd.Series
            Modified Sharpe ratio indexed by ticker name.

        Raises
        ------
        AnalyticsError
            If ``confidence`` is not strictly inside ``(0, 1)``.

        Sources
        -------
        - Gregoriou and Gueyie (2003): see docs/REFERENCES.md#gregoriou2003
        """

    def sterling_ratio(self, risk_free_rate: float = 0.0, n: int = 5) -> pd.Series:
        """
        Sterling ratio for each ticker.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.
        n : int, default 5
            Number of largest drawdowns to average.

        Returns
        -------
        pd.Series
            Sterling ratio indexed by ticker name. Non-finite risk-free
            rates give ``NaN``, including when no drawdowns are observed.

        Raises
        ------
        ValueError
            If the active date window cannot be annualized.

        Sources
        -------
        - Kestner (1996): see docs/REFERENCES.md#kestner1996
        """

    def burke_ratio(self, risk_free_rate: float = 0.0, n: int = 5) -> pd.Series:
        """
        Burke ratio for each ticker.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.
        n : int, default 5
            Number of largest drawdowns to use.

        Returns
        -------
        pd.Series
            Burke ratio indexed by ticker name. Non-finite risk-free rates
            give ``NaN``, including when no drawdowns are observed.

        Raises
        ------
        ValueError
            If the active date window cannot be annualized.

        Sources
        -------
        - Burke (1994): see docs/REFERENCES.md#burke1994
        """

    # -- Vector-per-ticker methods --

    def get_returns(self) -> list[list[float]]:
        """
        Per-period simple returns for each ticker.

        Canonical accessor for the raw return panel over the active window.
        Prefer this over :meth:`excess_returns` with an all-zero risk-free
        series or un-compounding :meth:`cumulative_returns`. Series are
        span-aware and therefore ragged across tickers on edge-ragged panels.

        Returns
        -------
        list[list[float]]
            Per-ticker simple return series as decimal fractions
            (``0.01`` for ``+1%``), in date order.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """

    def get_returns_for_ticker(self, ticker_idx: int | str) -> list[float]:
        """
        Per-period simple returns for a single ticker.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index, or a ticker name resolved through
            Rust ``Performance::get_ticker_index``.

        Returns
        -------
        list[float]
            Simple return series as decimal fractions (``0.01`` for ``+1%``),
            in date order, spanning that ticker's active dates.

        Raises
        ------
        AnalyticsError
            If an integer ``ticker_idx`` is outside the loaded ticker columns.
        KeyError
            If a string ``ticker_idx`` is not a loaded ticker name.
        """

    def cumulative_returns(self) -> list[list[float]]:
        """
        Cumulative returns for each ticker.

        Returns
        -------
        list[list[float]]
            Per-ticker cumulative return time series.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """

    def get_drawdown_series(self) -> list[list[float]]:
        """
        Drawdown series for each ticker.

        Returns
        -------
        list[list[float]]
            Per-ticker drawdown time series (negative or zero).

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """

    def correlation_matrix(self) -> tuple[list[list[float]], bool]:
        """
        Correlation matrix across all tickers, and whether it was repaired.

        Uses the complete-case common window when every ticker has at least
        two overlapping points; otherwise pairwise intersecting spans. A raw
        estimate that is not a valid correlation matrix (ragged panels can
        yield one that is not positive semi-definite) is Higham-repaired to
        the nearest correlation matrix.

        Returns
        -------
        tuple[list[list[float]], bool]
            ``(matrix, repaired)``. ``matrix`` is the symmetric correlation
            matrix indexed by ticker column order. ``repaired`` is ``True``
            when the raw pairwise estimate was projected to the nearest
            correlation matrix and ``False`` for a clean estimate.

        Raises
        ------
        AnalyticsError
            If a pair is degenerate (zero variance or non-finite) or Higham
            repair fails.

        Examples
        --------
        >>> from datetime import date
        >>> from finstack_quant.analytics import Performance
        >>> perf = Performance.from_returns_arrays(
        ...     [date(2024, 1, 1), date(2024, 1, 2), date(2024, 1, 3)],
        ...     [[0.01, 0.02, -0.01], [0.02, -0.01, 0.01]],
        ...     ["A", "B"],
        ... )
        >>> matrix, repaired = perf.correlation_matrix()
        >>> matrix[0][0], repaired
        (1.0, False)
        """

    def cumulative_returns_outperformance(self) -> list[list[float]]:
        """
        Cumulative returns outperformance vs benchmark.

        Returns
        -------
        list[list[float]]
            Per-ticker cumulative excess return time series.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """

    def drawdown_difference(self) -> list[list[float]]:
        """
        Drawdown difference vs benchmark.

        Returns
        -------
        list[list[float]]
            Per-ticker drawdown difference time series.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """

    def excess_returns(
        self,
        rf: pd.Series | Sequence[float],
        nperiods: float | None = None,
    ) -> list[list[float]]:
        """
        Excess returns over a risk-free series aligned to the panel grid.

        Each ticker subtracts ``rf[panel_index]`` on its active span.
        ``nperiods=None`` geometrically decompounds an annual series using
        the engine frequency; pass ``1.0`` when ``rf`` is already periodic.

        Parameters
        ----------
        rf : pandas.Series or numpy.ndarray or sequence of float
            Risk-free series with one value per active panel date.
        nperiods : float, optional
            Periods per year used to decompound annual ``rf``. ``None`` uses
            the engine frequency; ``1.0`` treats ``rf`` as already periodic.

        Returns
        -------
        list[list[float]]
            Per-ticker excess return series.

        Raises
        ------
        AnalyticsError
            If ``len(rf)`` differs from the number of active panel dates.
        TypeError
            If ``rf`` is not a float sequence, NumPy array or Series.
        """

    # -- Per-ticker structured methods --

    def beta(self) -> list[BetaResult]:
        """
        Beta for each ticker vs benchmark.

        Returns
        -------
        list[BetaResult]
            Per-ticker :class:`BetaResult` with CI.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """

    def greeks(self, risk_free_rate: float = 0.0) -> list[GreeksResult]:
        """
        Greeks (annualized Jensen alpha, beta, R²) for each ticker vs benchmark.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.

        Returns
        -------
        list[GreeksResult]
            Per-ticker :class:`GreeksResult`.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """

    def rolling_greeks(
        self,
        ticker_idx: int | str,
        window: int = 63,
        risk_free_rate: float = 0.0,
    ) -> RollingGreeks:
        """
        Rolling greeks for a specific ticker.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index, or a ticker name resolved through
            Rust ``Performance::get_ticker_index``.
        window : int, default 63
            Rolling window size in observations (must be ``>= 1``; longer
            than the active series yields an empty result).
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.

        Returns
        -------
        RollingGreeks
            :class:`RollingGreeks` with dates, alphas, and betas.

        Raises
        ------
        AnalyticsError
            If an integer ``ticker_idx`` is outside the loaded ticker columns.
        KeyError
            If a string ``ticker_idx`` is not a loaded ticker name.
        """

    def rolling_volatility(self, ticker_idx: int | str, window: int = 63) -> DatedSeries:
        """
        Rolling volatility for a specific ticker (column name ``volatility``).

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index, or a ticker name resolved through
            Rust ``Performance::get_ticker_index``.
        window : int, default 63
            Rolling window size in observations (must be ``>= 1``; longer
            than the active series yields an empty result).

        Returns
        -------
        DatedSeries
            :class:`DatedSeries` with ``value_column="volatility"``.

        Raises
        ------
        AnalyticsError
            If an integer ``ticker_idx`` is outside the loaded ticker columns.
        KeyError
            If a string ``ticker_idx`` is not a loaded ticker name.
        """

    def rolling_sortino(self, ticker_idx: int | str, window: int = 63, mar: float = 0.0) -> DatedSeries:
        """
        Rolling Sortino for a specific ticker (column name ``sortino``).

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index, or a ticker name resolved through
            Rust ``Performance::get_ticker_index``.
        window : int, default 63
            Rolling window size in observations (must be ``>= 1``; longer
            than the active series yields an empty result).
        mar : float, default 0.0
            Minimum acceptable return per period.

        Returns
        -------
        DatedSeries
            :class:`DatedSeries` with ``value_column="sortino"``.

        Raises
        ------
        AnalyticsError
            If an integer ``ticker_idx`` is outside the loaded ticker columns.
        KeyError
            If a string ``ticker_idx`` is not a loaded ticker name.
        """

    def rolling_sharpe(
        self,
        ticker_idx: int | str,
        window: int = 63,
        risk_free_rate: float = 0.0,
    ) -> DatedSeries:
        """
        Rolling Sharpe for a specific ticker (column name ``sharpe``).

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index, or a ticker name resolved through
            Rust ``Performance::get_ticker_index``.
        window : int, default 63
            Rolling window size in observations (must be ``>= 1``; longer
            than the active series yields an empty result).
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.

        Returns
        -------
        DatedSeries
            :class:`DatedSeries` with ``value_column="sharpe"``.

        Raises
        ------
        AnalyticsError
            If an integer ``ticker_idx`` is outside the loaded ticker columns.
        KeyError
            If a string ``ticker_idx`` is not a loaded ticker name.
        """

    def rolling_returns(self, ticker_idx: int | str, window: int) -> DatedSeries:
        """
        Rolling N-period compounded total return (column name ``return``).

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index, or a ticker name resolved through
            Rust ``Performance::get_ticker_index``.
        window : int
            Rolling window size in observations (must be ``>= 1``; longer
            than the active series yields an empty result).

        Returns
        -------
        DatedSeries
            :class:`DatedSeries` with ``value_column="return"``.

        Raises
        ------
        AnalyticsError
            If an integer ``ticker_idx`` is outside the loaded ticker columns.
        KeyError
            If a string ``ticker_idx`` is not a loaded ticker name.
        """

    def drawdown_details(self, ticker_idx: int | str, n: int = 5) -> list[DrawdownEpisode]:
        """
        Top-N drawdown episodes for a specific ticker.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index, or a ticker name resolved through
            Rust ``Performance::get_ticker_index``.
        n : int, default 5
            Number of largest drawdown episodes to return.

        Returns
        -------
        list[DrawdownEpisode]
            List of :class:`DrawdownEpisode` objects, deepest first.

        Raises
        ------
        AnalyticsError
            If an integer ``ticker_idx`` is outside the loaded ticker columns.
        KeyError
            If a string ``ticker_idx`` is not a loaded ticker name.
        """

    def multi_factor_greeks(
        self,
        ticker_idx: int | str,
        factor_returns: list[list[float]],
        return_kind: str | None = None,
        risk_free_rate: float = 0.0,
    ) -> MultiFactorResult:
        """
        Multi-factor regression for a specific ticker.

        Factor series are already-excess (Fama–French style).
        ``return_kind="excess"`` leaves the ticker series unchanged.
        ``return_kind="total"`` subtracts the geometrically decompounded
        period risk-free rate from the ticker series only. Excess returns
        have no use for a risk-free rate, so a non-zero ``risk_free_rate``
        with ``"excess"`` is rejected.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index, or a ticker name resolved through
            Rust ``Performance::get_ticker_index``.
        factor_returns : list[list[float]]
            Already-excess factor return matrix; ``factor_returns[i]`` is the
            return series for factor ``i``.
        return_kind : str, optional
            ``"excess"`` or ``"total"``; ``None`` uses the Rust default
            ``ReturnKind::Excess``.
        risk_free_rate : float, default 0.0
            Annualized decimal risk-free rate used when ``return_kind`` is
            ``"total"``. Must be ``0.0`` for ``"excess"``.

        Returns
        -------
        MultiFactorResult
            :class:`MultiFactorResult` with alpha, betas, and fit statistics.

        Raises
        ------
        ValueError
            If ``return_kind`` is not ``"excess"`` or ``"total"``, or
            ``risk_free_rate`` is non-zero with ``"excess"``.
        AnalyticsError
            If ``ticker_idx`` is out of range, no factors are supplied, factor
            lengths differ from the ticker return series, factor returns are
            non-finite, observations are insufficient, or the regression is
            numerically singular; also if a fitted coefficient, annualized
            intercept, or residual volatility cannot be represented as a
            finite value. Constant responses retain undefined ``NaN``
            R-squared statistics.
        """

    def lookback_returns(
        self,
        ref_date: object,
        fiscal_config: FiscalConfig | None = None,
    ) -> LookbackReturns:
        """
        Period-to-date lookback returns.

        MTD, QTD and YTD start on the first calendar day of ``ref_date``'s
        month, calendar quarter and year. FYTD is the first observation on or
        after the fiscal calendar start of *fiscal_config* through
        ``ref_date``. Holidays are not skipped. The first included simple
        return still spans the prior close.

        Parameters
        ----------
        ref_date : object
            Reference date (``datetime.date``, ``pd.Timestamp``, or ISO string).
        fiscal_config : FiscalConfig, optional
            Fiscal calendar that opens the FYTD window. ``None`` uses the
            calendar year (Rust ``FiscalConfig::default``).

        Returns
        -------
        LookbackReturns
            :class:`LookbackReturns` with mandatory MTD, QTD, YTD, and FYTD
            vectors, each containing one value per ticker.

        Raises
        ------
        ValueError
            If *ref_date* is a string that is not ISO-8601.
        TypeError
            If *ref_date* is not date-like, or *fiscal_config* is neither a
            ``FiscalConfig`` nor ``None``.
        """

    def period_stats(
        self,
        ticker_idx: int | str,
        aggregation_frequency: str | None = None,
        fiscal_config: FiscalConfig | None = None,
    ) -> PeriodStats:
        """
        Period statistics for one ticker at a given aggregation frequency.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index, or a ticker name resolved through
            Rust ``Performance::get_ticker_index``.
        aggregation_frequency : str, optional
            Aggregation frequency (``"daily"``, ``"weekly"``, ``"monthly"``,
            ``"quarterly"``, ``"semi_annual"``, ``"annual"``); ``None`` uses the Rust default
            ``DEFAULT_PERIODIC_FREQUENCY`` (monthly).
        fiscal_config : FiscalConfig, optional
            ``None`` buckets Gregorian periods with ISO weeks. A ``FiscalConfig``
            applies fiscal day, week, month, quarter, half-year and year
            boundaries, clamping a start day past the month end.

        Returns
        -------
        PeriodStats
            :class:`PeriodStats` with win/loss streaks, ratios, etc.

        Raises
        ------
        AnalyticsError
            If the frequency is invalid, the ticker index is out of range, or a
            containing period boundary cannot be represented.
        KeyError
            If a ticker name is not present in the panel.
        TypeError
            If *fiscal_config* is neither a ``FiscalConfig`` nor ``None``.
        """

    def periodic_returns(
        self,
        frequency: str | None = None,
    ) -> list[list[tuple[datetime.date, float]]]:
        """
        Calendar-bucketed compounded returns for all tickers.

        Parameters
        ----------
        frequency : str, optional
            Calendar-bucketing frequency: one of ``"daily"``, ``"weekly"``,
            ``"monthly"``, ``"quarterly"``, ``"semi_annual"``, or
            ``"annual"``; ``None`` uses the Rust
            default ``DEFAULT_PERIODIC_FREQUENCY`` (monthly).

        Returns
        -------
        list[list[tuple[datetime.date, float]]]
            Ticker-major panel in :meth:`get_ticker_names` order. Each inner list
            contains chronological ``(period_end_date, compounded_return)``
            points. Returns are simple decimal fractions (``0.01`` means 1%);
            chaining one ticker's points reconciles with the final value from
            :meth:`cumulative_returns`.

        Raises
        ------
        AnalyticsError
            If ``frequency`` is not a supported token or a containing calendar
            period boundary cannot be represented.

        Examples
        --------
        >>> from datetime import date
        >>> perf = Performance.from_returns_arrays(
        ...     [date(2024, 1, 1), date(2024, 1, 2)],
        ...     [[0.01, 0.02]],
        ...     ["FUND"],
        ... )
        >>> panel = perf.periodic_returns()
        >>> (len(panel), panel[0][0][0], round(panel[0][0][1], 4))
        (1, datetime.date(2024, 1, 2), 0.0302)
        """

    # -- DataFrame export --

    def to_dataframe(
        self,
        risk_free_rate: float = 0.0,
        confidence: float = 0.95,
    ) -> pd.DataFrame:
        """
        Summary statistics for all tickers as a pandas DataFrame.

        *risk_free_rate* affects only the ``sharpe`` column; the MAR-based
        metrics (``sortino``, ``downside_deviation``) and the ``omega_ratio``
        threshold are fixed at ``0.0``. *confidence* applies to
        ``value_at_risk``, ``expected_shortfall``, and ``tail_ratio``.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal (``0.02`` for 2%), used
            only by the ``sharpe`` column.
        confidence : float, default 0.95
            Confidence level in ``(0, 1)`` for VaR, ES, and tail ratio.

        Returns
        -------
        pd.DataFrame
            Summary statistics indexed by ticker name. Units: ``cagr``,
            ``mean_return``, ``volatility``, ``geometric_mean``,
            ``downside_deviation`` are annualized decimals (``mean_return``
            and ``volatility`` annualized; ``geometric_mean`` per period);
            ``max_drawdown``, ``value_at_risk``, ``expected_shortfall``,
            ``pain_index``, ``ulcer_index`` are non-positive/positive decimal
            fractions of wealth; ``sharpe``, ``sortino``, ``calmar``,
            ``information_ratio``, ``omega_ratio``, ``gain_to_pain``,
            ``recovery_factor``, ``tail_ratio`` are dimensionless ratios;
            ``skewness``, ``kurtosis`` (excess), ``r_squared`` are
            dimensionless.

        Raises
        ------
        AnalyticsError
            If ``confidence`` is outside ``(0, 1)`` or a ticker's active range
            has no positive holding period and therefore cannot be annualized.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize validated inputs and window selection to compact JSON.

        Returns
        -------
        str
            Canonical JSON (dates, returns, spans, ticker names, benchmark,
            frequency, active window) accepted by :meth:`from_json`; also the
            pickle payload.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> Performance:
        """
        Rebuild an engine from :meth:`to_json` output.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        Performance
            Engine with validated inputs and the same active date window;
            drawdown caches are rebuilt from returns.

        Raises
        ------
        ValueError
            If ``json`` has unknown fields or invalid dates, returns, spans,
            ticker identities, benchmark index, or active window.

        Examples
        --------
        >>> from datetime import date
        >>> from finstack_quant.analytics import Performance
        >>> perf = Performance.from_returns_arrays([date(2024, 1, 1), date(2024, 1, 2)], [[0.01, 0.02]], ["FUND"])
        >>> Performance.from_json(perf.to_json()).get_ticker_names()
        ['FUND']
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

def sharpe(
    returns: Sequence[float] | npt.NDArray[np.float64] | pd.Series,
    risk_free_rate: float = 0.0,
    periods_per_year: float = 252,
) -> float:
    """
    Sharpe ratio of one return series.

    Annualized excess arithmetic mean over annualized sample volatility — the
    same kernel as :meth:`Performance.sharpe`, without building a panel.

    Parameters
    ----------
    returns : sequence of float, numpy.ndarray, or pandas.Series
        Per-period simple decimal returns (``0.01`` is +1%) in date order.
    risk_free_rate : float, default 0.0
        Annualized risk-free rate as a decimal (``0.02`` for 2%),
        geometrically decompounded to the observation frequency.
    periods_per_year : float, default 252
        Observations per year used to annualize (252 daily, 52 weekly,
        12 monthly).

    Returns
    -------
    float
        Sharpe ratio; ``inf`` / ``-inf`` when volatility is zero with a
        non-zero excess return, ``nan`` for fewer than two observations,
        non-finite inputs, or a non-positive ``periods_per_year``.

    Raises
    ------
    TypeError
        If ``returns`` is not a float sequence, NumPy array, or Series.

    Examples
    --------
    >>> from finstack_quant.analytics import sharpe
    >>> round(sharpe([0.01, -0.02, 0.015, 0.003], 0.0, 252), 4)
    2.0522
    """
    ...

def sortino(
    returns: Sequence[float] | npt.NDArray[np.float64] | pd.Series,
    mar: float = 0.0,
    periods_per_year: float = 252,
) -> float:
    """
    Annualized Sortino ratio of one return series.

    Parameters
    ----------
    returns : sequence of float, numpy.ndarray, or pandas.Series
        Per-period simple decimal returns in date order.
    mar : float, default 0.0
        Minimum acceptable return **per period** as a decimal (not
        annualized), matching :meth:`Performance.sortino`.
    periods_per_year : float, default 252
        Observations per year used to annualize.

    Returns
    -------
    float
        Sortino ratio; ``±inf`` when there is no downside deviation but a
        non-zero excess mean, ``nan`` for fewer than two observations,
        non-finite inputs, or an invalid ``periods_per_year``.

    Raises
    ------
    TypeError
        If ``returns`` is not a float sequence, NumPy array, or Series.

    Examples
    --------
    >>> from finstack_quant.analytics import sortino
    >>> sortino([0.01, -0.02, 0.015, 0.003]) > 0
    True
    """
    ...

def volatility(
    returns: Sequence[float] | npt.NDArray[np.float64] | pd.Series,
    periods_per_year: float = 252,
) -> float:
    """
    Annualized sample volatility (n−1 denominator) of one return series.

    Parameters
    ----------
    returns : sequence of float, numpy.ndarray, or pandas.Series
        Per-period simple decimal returns in date order.
    periods_per_year : float, default 252
        Observations per year; the per-period standard deviation is scaled
        by its square root.

    Returns
    -------
    float
        Annualized volatility as a decimal (``0.15`` is 15%); ``0.0`` for fewer
        than two observations, ``nan`` for an invalid ``periods_per_year``.

    Raises
    ------
    TypeError
        If ``returns`` is not a float sequence, NumPy array, or Series.

    Examples
    --------
    >>> from finstack_quant.analytics import volatility
    >>> round(volatility([0.01, -0.01, 0.01, -0.01], 252), 4)
    0.1833
    """
    ...

def max_drawdown(
    returns: Sequence[float] | npt.NDArray[np.float64] | pd.Series,
) -> float:
    """
    Maximum peak-to-trough drawdown of one return series.

    Parameters
    ----------
    returns : sequence of float, numpy.ndarray, or pandas.Series
        Per-period simple decimal returns in date order; they are compounded
        into a wealth path before the running-peak decline is measured.

    Returns
    -------
    float
        Non-positive fraction (``-0.25`` is a 25% loss); ``0.0`` when the
        series never falls below its running peak or is empty. Returns ``nan``
        for a non-finite return or reconstructed drawdown path.

    Raises
    ------
    TypeError
        If ``returns`` is not a float sequence, NumPy array, or Series.

    Examples
    --------
    >>> from finstack_quant.analytics import max_drawdown
    >>> round(max_drawdown([0.10, -0.20, 0.05]), 4)
    -0.2
    """
    ...
