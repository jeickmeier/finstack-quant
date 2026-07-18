"""
Performance analytics: returns, drawdowns, risk metrics, and benchmarks.

The sole entry point is :class:`Performance`. Construct from a price panel
(``Performance(prices_df)`` / ``Performance.from_arrays(...)``) or from a
return panel (``Performance.from_returns(returns_df)`` /
``Performance.from_returns_arrays(...)``); every analytic — return / risk
scalars, drawdown statistics, rolling windows, periodic returns
(MTD / QTD / YTD / FYTD), benchmark alpha/beta, basic factor models — is a
method on the resulting instance.

The remaining classes are value-object outputs returned by `Performance`
methods (`LookbackReturns`, `PeriodStats`, etc.).

Examples
--------
>>> import finstack_quant.analytics as analytics
>>> analytics.__name__
'finstack_quant.analytics'
"""

from __future__ import annotations

import datetime
from typing import Sequence

import numpy as np
import numpy.typing as npt
import pandas as pd

__all__ = [
    "AnalyticsError",
    "Performance",
    "LookbackReturns",
    "PeriodStats",
    "BetaResult",
    "GreeksResult",
    "RollingGreeks",
    "MultiFactorResult",
    "DrawdownEpisode",
    "DatedSeries",
]

# ---------------------------------------------------------------------------
# Errors
# ---------------------------------------------------------------------------

class AnalyticsError(ValueError):
    """
    Analytics validation or calculation failure.

    Examples
    --------
    >>> from finstack_quant.analytics import AnalyticsError
    >>> AnalyticsError.__name__
    'AnalyticsError'
    """

# ---------------------------------------------------------------------------
# Value-object results
# ---------------------------------------------------------------------------

class PeriodStats:
    """
    Aggregated statistics for grouped periodic returns.

    Examples
    --------
    >>> from finstack_quant.analytics import Performance
    >>> perf = Performance.from_arrays(dates, prices, ["A"])  # doctest: +SKIP
    >>> stats = perf.period_stats(0)  # doctest: +SKIP
    >>> stats.win_rate  # doctest: +SKIP
    0.55
    """

    @property
    def best(self) -> float:
        """
        Best period return.

        Returns
        -------
        float
            Highest single-period return.
        """

    @property
    def worst(self) -> float:
        """
        Worst period return.

        Returns
        -------
        float
            Lowest single-period return.
        """

    @property
    def consecutive_wins(self) -> int:
        """
        Longest consecutive winning streak.

        Returns
        -------
        int
            Maximum number of consecutive positive-return periods.
        """

    @property
    def consecutive_losses(self) -> int:
        """
        Longest consecutive losing streak.

        Returns
        -------
        int
            Maximum number of consecutive negative-return periods.
        """

    @property
    def win_rate(self) -> float:
        """
        Fraction of positive-return periods.

        Returns
        -------
        float
            Win rate in ``[0, 1]``.
        """

    @property
    def avg_return(self) -> float:
        """
        Average return across all periods.

        Returns
        -------
        float
            Mean periodic return.
        """

    @property
    def avg_win(self) -> float:
        """
        Average return of positive periods.

        Returns
        -------
        float
            Mean return across winning periods.
        """

    @property
    def avg_loss(self) -> float:
        """
        Average return of negative periods.

        Returns
        -------
        float
            Mean return across losing periods.
        """

    @property
    def payoff_ratio(self) -> float:
        """
        Payoff ratio (avg win / |avg loss|).

        Returns
        -------
        float
            Ratio of average win to absolute average loss.
        """

    @property
    def profit_factor(self) -> float:
        """
        Profit factor (gross profits / gross losses).

        Returns
        -------
        float
            Sum of wins divided by sum of absolute losses.
        """

    @property
    def cpc_ratio(self) -> float:
        """
        CPC index (profit_factor x win_rate x payoff_ratio).

        Returns
        -------
        float
            Composite measure of profitability consistency.
        """

    @property
    def kelly_criterion(self) -> float:
        """
        Kelly criterion optimal fraction.

        Returns
        -------
        float
            Optimal bet fraction for maximizing long-run growth.
        """

    def __repr__(self) -> str: ...

class BetaResult:
    """
    Regression beta with confidence interval.

    The 95% interval uses Student-t critical values for finite samples and an
    asymptotic normal approximation once ``n - 2 >= 240``.

    Examples
    --------
    >>> from finstack_quant.analytics import Performance
    >>> perf = Performance.from_arrays(dates, prices, ["A", "B"])  # doctest: +SKIP
    >>> betas = perf.beta()  # doctest: +SKIP
    >>> betas[0].beta  # doctest: +SKIP
    1.02
    """

    @property
    def beta(self) -> float:
        """
        Beta coefficient.

        Returns
        -------
        float
            OLS regression slope vs benchmark.
        """

    @property
    def std_err(self) -> float:
        """
        Standard error of the beta estimate.

        Returns
        -------
        float
            Standard error from the OLS fit.
        """

    @property
    def ci_lower(self) -> float:
        """
        Lower 95% confidence bound.

        Returns
        -------
        float
            Lower bound of the 95% CI for beta.
        """

    @property
    def ci_upper(self) -> float:
        """
        Upper 95% confidence bound.

        Returns
        -------
        float
            Upper bound of the 95% CI for beta.
        """

    def __repr__(self) -> str: ...

class GreeksResult:
    """
    Alpha, beta, and goodness-of-fit from a single-index regression.

    Examples
    --------
    >>> from finstack_quant.analytics import Performance
    >>> perf = Performance.from_arrays(dates, prices, ["A", "B"])  # doctest: +SKIP
    >>> g = perf.greeks(0)  # doctest: +SKIP
    >>> g[0].alpha  # doctest: +SKIP
    0.03
    """

    @property
    def alpha(self) -> float:
        """
        Annualized Jensen alpha.

        Returns
        -------
        float
            Annualized intercept from the single-index regression.
        """

    @property
    def beta(self) -> float:
        """
        Beta coefficient.

        Returns
        -------
        float
            OLS regression slope vs benchmark.
        """

    @property
    def r_squared(self) -> float:
        """
        Return the r squared for `GreeksResult`.
        R-squared.

        Returns
        -------
        float
            Coefficient of determination.
        """

    @property
    def adjusted_r_squared(self) -> float:
        """
        Adjusted R-squared.

        Returns
        -------
        float
            Degrees-of-freedom-adjusted R².
        """

    def __repr__(self) -> str: ...

class RollingGreeks:
    """
    Rolling alpha and beta time series.

    Examples
    --------
    >>> from finstack_quant.analytics import Performance
    >>> perf = Performance.from_arrays(dates, prices, ["A", "B"])  # doctest: +SKIP
    >>> rg = perf.rolling_greeks(0, window=63)  # doctest: +SKIP
    >>> rg.betas[0]  # doctest: +SKIP
    1.05
    """

    @property
    def dates(self) -> list[datetime.date]:
        """
        Date labels for each rolling window.

        Returns
        -------
        list[datetime.date]
            Window-end dates aligned 1:1 with :attr:`alphas` and :attr:`betas`.
        """

    @property
    def alphas(self) -> npt.NDArray[np.float64]:
        """
        Rolling alpha values.

        Returns
        -------
        npt.NDArray[np.float64]
            Annualized Jensen alpha per window.
        """

    @property
    def betas(self) -> npt.NDArray[np.float64]:
        """
        Rolling beta values.

        Returns
        -------
        npt.NDArray[np.float64]
            OLS beta per window.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """
        Convert to a pandas DataFrame with date index and alpha/beta columns.

        Returns
        -------
        pd.DataFrame
            DataFrame with columns ``alpha`` and ``beta``, indexed by date.
        """
        ...

    def __repr__(self) -> str: ...

class MultiFactorResult:
    """
    Multi-factor regression result.

    Examples
    --------
    >>> from finstack_quant.analytics import Performance
    >>> perf = Performance.from_arrays(dates, prices, ["A", "B"])  # doctest: +SKIP
    >>> mf = perf.multi_factor_greeks(0, factor_returns)  # doctest: +SKIP
    >>> mf.alpha  # doctest: +SKIP
    0.02
    """

    @property
    def alpha(self) -> float:
        """
        Raw regression intercept, annualized with the supplied factor frequency.

        Returns
        -------
        float
            Annualized regression intercept.
        """

    @property
    def betas(self) -> npt.NDArray[np.float64]:
        """
        Return the betas for `MultiFactorResult`.
        Factor betas.

        Returns
        -------
        npt.NDArray[np.float64]
            One beta per factor, in factor order.
        """

    @property
    def r_squared(self) -> float:
        """
        Return the r squared for `MultiFactorResult`.
        R-squared.

        Returns
        -------
        float
            Coefficient of determination.
        """

    @property
    def adjusted_r_squared(self) -> float:
        """
        Adjusted R-squared.

        Returns
        -------
        float
            Degrees-of-freedom-adjusted R².
        """

    @property
    def residual_vol(self) -> float:
        """
        Residual volatility.

        Returns
        -------
        float
            Standard deviation of regression residuals.
        """

    def __repr__(self) -> str: ...

class DrawdownEpisode:
    """
    A single drawdown episode with timing and depth information.

    Examples
    --------
    >>> from finstack_quant.analytics import Performance
    >>> perf = Performance.from_arrays(dates, prices, ["A"])  # doctest: +SKIP
    >>> episodes = perf.drawdown_details(0, n=3)  # doctest: +SKIP
    >>> episodes[0].max_drawdown  # doctest: +SKIP
    -0.15
    """

    @property
    def start(self) -> datetime.date:
        """
        Start date of the drawdown.

        Returns
        -------
        datetime.date
            Date when the drawdown began.
        """

    @property
    def valley(self) -> datetime.date:
        """
        Date of the maximum drawdown within this episode.

        Returns
        -------
        datetime.date
            Date of the deepest point.
        """

    @property
    def end(self) -> datetime.date | None:
        """
        Recovery date (``None`` if still in drawdown).

        Returns
        -------
        datetime.date or None
            Recovery date, or ``None`` if the episode is ongoing.
        """

    @property
    def duration_days(self) -> int:
        """
        Duration in calendar days.

        Returns
        -------
        int
            Number of calendar days from start to end (or valley if ongoing).
        """

    @property
    def max_drawdown(self) -> float:
        """
        Maximum drawdown depth (negative).

        Returns
        -------
        float
            Peak-to-trough drawdown as a negative decimal.
        """

    @property
    def near_recovery_threshold(self) -> float:
        """
        Near-recovery threshold.

        Returns
        -------
        float
            Price level that would signal near-recovery.
        """

    @property
    def truncated_at_start(self) -> bool:
        """
        True when the episode began before the first observation (left-censored).

        Returns
        -------
        bool
            ``True`` if the drawdown started before the available data window.
        """

    def __repr__(self) -> str: ...

class LookbackReturns:
    """
    Period-to-date returns for each ticker.

    Examples
    --------
    >>> from finstack_quant.analytics import Performance
    >>> perf = Performance.from_arrays(dates, prices, ["A"])  # doctest: +SKIP
    >>> lb = perf.lookback_returns("2025-06-15")  # doctest: +SKIP
    >>> lb.mtd[0]  # doctest: +SKIP
    0.03
    """

    @property
    def mtd(self) -> npt.NDArray[np.float64]:
        """
        Month-to-date returns per ticker.

        Returns
        -------
        npt.NDArray[np.float64]
            Array of MTD returns, one per ticker.
        """

    @property
    def qtd(self) -> npt.NDArray[np.float64]:
        """
        Quarter-to-date returns per ticker.

        Returns
        -------
        npt.NDArray[np.float64]
            Array of QTD returns, one per ticker.
        """

    @property
    def ytd(self) -> npt.NDArray[np.float64]:
        """
        Year-to-date returns per ticker.

        Returns
        -------
        npt.NDArray[np.float64]
            Array of YTD returns, one per ticker.
        """

    @property
    def fytd(self) -> npt.NDArray[np.float64] | None:
        """
        Fiscal-year-to-date returns when a fiscal config is provided.

        Returns
        -------
        npt.NDArray[np.float64] or None
            Array of FYTD returns, or ``None`` when no fiscal config is set.
        """

    def to_dataframe(self, ticker_names: list[str]) -> pd.DataFrame:
        """
        Convert to a pandas DataFrame with ticker names as index.

        Columns: ``mtd``, ``qtd``, ``ytd`` (and ``fytd`` when available).

        Parameters
        ----------
        ticker_names : list[str]
            Ticker labels to use as the DataFrame index.

        Returns
        -------
        pd.DataFrame
            DataFrame indexed by ticker name with lookback return columns.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def __repr__(self) -> str: ...

class DatedSeries:
    """
    Date-indexed numeric series returned by the rolling-window analytics.

    Rolling-window methods return this shared carrier with a metric-specific
    DataFrame column name.

    Examples
    --------
    >>> from finstack_quant.analytics import Performance
    >>> perf = Performance.from_arrays(dates, prices, ["A"])  # doctest: +SKIP
    >>> rv = perf.rolling_volatility(0, window=63)  # doctest: +SKIP
    >>> rv.values[0]  # doctest: +SKIP
    0.15
    """

    @property
    def values(self) -> npt.NDArray[np.float64]:
        """
        Rolling values, one per window.

        Returns
        -------
        npt.NDArray[np.float64]
            Metric values aligned with :attr:`dates`.
        """

    @property
    def dates(self) -> list[datetime.date]:
        """
        Window-end dates aligned 1:1 with :attr:`values`.

        Returns
        -------
        list[datetime.date]
            Date labels for each rolling window.
        """

    @property
    def value_column(self) -> str:
        """
        Column name used by :meth:`to_dataframe`.

        Returns
        -------
        str
            Metric-specific column name (e.g. ``sharpe``, ``volatility``).
        """

    def to_dataframe(self) -> pd.DataFrame:
        """
        Convert to a pandas DataFrame with date index and a value column.

        The column is named after :attr:`value_column` (e.g. ``sharpe``,
        ``sortino``, ``volatility``, or ``return``).

        Returns
        -------
        pd.DataFrame
            DataFrame with a date index and one column named
            :attr:`value_column`.
        """
        ...

    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Performance engine
# ---------------------------------------------------------------------------

class Performance:
    """
    Stateful performance analytics engine over a panel of ticker series.

    Construct from a pandas DataFrame of prices (``Performance(df)``), a
    DataFrame of returns (``Performance.from_returns(df)``), or from raw
    arrays via :meth:`from_arrays` / :meth:`from_returns_arrays`.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.analytics import Performance
    >>> perf = Performance(prices_df, benchmark_ticker="SPX")  # doctest: +SKIP
    >>> perf.sharpe()  # doctest: +SKIP
    [1.2, 0.8]
    """

    def __init__(
        self,
        prices: pd.DataFrame,
        benchmark_ticker: str | None = None,
        freq: str = "daily",
    ) -> None:
        """
        Build from a pandas DataFrame of prices.

        Parameters
        ----------
        prices : pandas.DataFrame
            Price panel with a date-like index (``datetime.date`` or
            ``pd.Timestamp``) and one column per ticker.
        benchmark_ticker : str, optional
            Benchmark column name. Defaults to the first column when ``None``.
        freq : str, optional
            Return aggregation frequency. One of ``"daily"``, ``"weekly"``,
            ``"monthly"``, ``"quarterly"``, ``"semiannual"``, or ``"annual"``.
            Default ``"daily"``.

        Raises
        ------
        AnalyticsError
            If ``prices`` is not a DataFrame, dates are invalid, or the panel is
            empty.
        TypeError
            If ``prices`` is not a pandas ``DataFrame`` (use
            :meth:`from_arrays` for raw lists).

        Examples
        --------
        >>> import pandas as pd
        >>> from finstack_quant.analytics import Performance
        >>> perf = Performance(prices_df, benchmark_ticker="SPX", freq="daily")  # doctest: +SKIP
        """

    @staticmethod
    def from_arrays(
        dates: Sequence[object],
        prices: list[list[float]],
        ticker_names: list[str],
        benchmark_ticker: str | None = None,
        freq: str = "daily",
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
        freq : str, optional
            One of ``"daily"``, ``"weekly"``, ``"monthly"``, ``"quarterly"``,
            ``"semiannual"``, or ``"annual"``. Default ``"daily"``.

        Returns
        -------
        Performance
            Analytics engine over the supplied panel.

        Raises
        ------
        AnalyticsError
            If dimensions are inconsistent, dates are invalid, or ``freq`` is
            unrecognized.

        Examples
        --------
        >>> from finstack_quant.analytics import Performance
        >>> perf = Performance.from_arrays(dates, prices, ["A", "B"])  # doctest: +SKIP
        """

    @staticmethod
    def from_returns(
        returns: pd.DataFrame,
        benchmark_ticker: str | None = None,
        freq: str = "daily",
    ) -> Performance:
        """
        Build from a pandas DataFrame of simple returns.

        Parameters
        ----------
        returns : pandas.DataFrame
            Simple-return panel aligned with a date-like index and one column per
            ticker (decimal returns, e.g. ``0.01`` for +1%).
        benchmark_ticker : str, optional
            Benchmark column name. Defaults to the first column when ``None``.
        freq : str, optional
            One of ``"daily"``, ``"weekly"``, ``"monthly"``, ``"quarterly"``,
            ``"semiannual"``, or ``"annual"``. Default ``"daily"``.

        Raises
        ------
        AnalyticsError
            If ``returns`` is invalid or empty.
        TypeError
            If ``returns`` is not a pandas ``DataFrame`` (use
            :meth:`from_returns_arrays` for raw lists).

        Returns
        -------
        Performance
            Analytics engine over the supplied return panel.

        Examples
        --------
        >>> from finstack_quant.analytics import Performance
        >>> perf = Performance.from_returns(returns_df, freq="monthly")  # doctest: +SKIP
        """

    @staticmethod
    def from_returns_arrays(
        dates: Sequence[object],
        returns: list[list[float]],
        ticker_names: list[str],
        benchmark_ticker: str | None = None,
        freq: str = "daily",
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
        freq : str, optional
            One of ``"daily"``, ``"weekly"``, ``"monthly"``, ``"quarterly"``,
            ``"semiannual"``, or ``"annual"``. Default ``"daily"``.

        Returns
        -------
        Performance
            Analytics engine over the supplied return panel.

        Raises
        ------
        AnalyticsError
            If dimensions are inconsistent or ``freq`` is unrecognized.

        Examples
        --------
        >>> from finstack_quant.analytics import Performance
        >>> perf = Performance.from_returns_arrays(dates, returns, ["A", "B"])  # doctest: +SKIP
        """

    # -- Mutators --

    def reset_date_range(self, start: object, end: object) -> None:
        """
        Restrict analytics to ``[start, end]``.

        Parameters
        ----------
        start : object
            Start date (``datetime.date``, ``pd.Timestamp``, or ISO string).
        end : object
            End date (``datetime.date``, ``pd.Timestamp``, or ISO string).

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
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
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    # -- Getters --

    @property
    def ticker_names(self) -> list[str]:
        """
        Ticker names in column order.

        Returns
        -------
        list[str]
            Column labels from the input panel.
        """

    @property
    def benchmark_idx(self) -> int:
        """
        Benchmark column index.

        Returns
        -------
        int
            Zero-based column index of the benchmark series.
        """

    @property
    def freq(self) -> str:
        """
        Observation frequency as the canonical lowercase token.

        Returns
        -------
        str
            One of ``"daily"``, ``"weekly"``, ``"monthly"``, etc.
        """

    def dates(self) -> list[datetime.date]:
        """
        Full return-aligned date grid (independent of any active window).

        Returns
        -------
        list[datetime.date]
            All observation dates in the panel.
        """

    def active_dates(self) -> list[datetime.date]:
        """
        Observation dates of the currently active analysis window.

        Returns
        -------
        list[datetime.date]
            Dates within the active ``[start, end]`` range.
        """

    def active_dates_for_ticker(self, ticker_idx: int) -> list[datetime.date]:
        """
        Observation dates for one ticker's active return series.

        Parameters
        ----------
        ticker_idx : int
            Zero-based ticker column index.

        Returns
        -------
        list[datetime.date]
            Dates where the specified ticker has valid returns.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    # -- Scalar-per-ticker methods --

    def cagr(self) -> list[float]:
        """
        CAGR for each ticker.

        Returns
        -------
        list[float]
            Compound annual growth rate per ticker.

        Raises
        ------
        ValueError
            If the active date window cannot be annualized.
        """

    def mean_return(self, annualize: bool = True) -> list[float]:
        """
        Mean return for each ticker.

        Parameters
        ----------
        annualize : bool, default True
            Whether to annualize the mean return.

        Returns
        -------
        list[float]
            Mean return per ticker.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def volatility(self, annualize: bool = True) -> list[float]:
        """
        Volatility for each ticker.

        Parameters
        ----------
        annualize : bool, default True
            Whether to annualize the volatility.

        Returns
        -------
        list[float]
            Standard deviation of returns per ticker.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def sharpe(self, risk_free_rate: float = 0.0) -> list[float]:
        """
        Sharpe ratio for each ticker.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.

        Returns
        -------
        list[float]
            Per-ticker Sharpe ratios over the active return window.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def sortino(self, mar: float = 0.0) -> list[float]:
        """
        Sortino ratio for each ticker.

        Parameters
        ----------
        mar : float, default 0.0
            Minimum acceptable return per period (not annualized).

        Returns
        -------
        list[float]
            Per-ticker Sortino ratios over the active return window.

        Notes
        -----
        ``mar`` is per-period; Sharpe ``risk_free_rate`` inputs are annualized.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def calmar(self) -> list[float]:
        """
        Calmar ratio for each ticker.

        Returns
        -------
        list[float]
            CAGR divided by absolute max drawdown per ticker.

        Raises
        ------
        ValueError
            If the active date window cannot be annualized.
        """

    def max_drawdown(self) -> list[float]:
        """
        Max drawdown for each ticker.

        Returns
        -------
        list[float]
            Peak-to-trough drawdown per ticker (negative).
        """

    def mean_drawdown(self) -> list[float]:
        """
        Mean drawdown (path-weighted average) for each ticker.

        Returns
        -------
        list[float]
            Average drawdown per ticker (negative).
        """

    def value_at_risk(self, confidence: float = 0.95) -> list[float]:
        """
        Historical VaR for each ticker.

        Parameters
        ----------
        confidence : float, default 0.95
            Confidence level (e.g. ``0.95`` for 95% VaR).

        Returns
        -------
        list[float]
            Historical VaR per ticker (negative).

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def expected_shortfall(self, confidence: float = 0.95) -> list[float]:
        """
        Expected Shortfall for each ticker.

        Parameters
        ----------
        confidence : float, default 0.95
            Confidence level.

        Returns
        -------
        list[float]
            Expected shortfall per ticker (negative).

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def tracking_error(self) -> list[float]:
        """
        Tracking error for each ticker vs benchmark.

        Returns
        -------
        list[float]
            Annualized standard deviation of excess returns per ticker.
        """

    def information_ratio(self) -> list[float]:
        """
        Information ratio for each ticker vs benchmark.

        Returns
        -------
        list[float]
            Annualized excess return divided by tracking error per ticker.
        """

    def skewness(self) -> list[float]:
        """
        Skewness for each ticker.

        Returns
        -------
        list[float]
            Third moment of returns per ticker.
        """

    def kurtosis(self) -> list[float]:
        """
        Kurtosis for each ticker.

        Returns
        -------
        list[float]
            Fourth moment of returns per ticker.
        """

    def geometric_mean(self) -> list[float]:
        """
        Geometric mean for each ticker.

        Returns
        -------
        list[float]
            Geometric mean return per ticker.
        """

    def skew_kurt(self) -> tuple[list[float], list[float]]:
        """
        Per-ticker ``(skewness, kurtosis)`` from one moments pass.

        Returns
        -------
        tuple[list[float], list[float]]
            ``(skewness_list, kurtosis_list)`` per ticker.
        """

    def value_at_risk_and_es(
        self,
        confidence: float = 0.95,
    ) -> tuple[list[float], list[float]]:
        """
        Per-ticker ``(value_at_risk, expected_shortfall)`` from one tail pass.

        Parameters
        ----------
        confidence : float, default 0.95
            Confidence level.

        Returns
        -------
        tuple[list[float], list[float]]
            ``(var_list, es_list)`` per ticker.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def downside_deviation(self, mar: float = 0.0) -> list[float]:
        """
        Downside deviation for each ticker.

        ``mar`` is per-period; Sharpe risk-free inputs are annualized.

        Parameters
        ----------
        mar : float, default 0.0
            Minimum acceptable return per period.

        Returns
        -------
        list[float]
            Downside deviation per ticker.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def max_drawdown_duration(self) -> list[int]:
        """
        Max drawdown duration (calendar days) for each ticker.

        Returns
        -------
        list[int]
            Longest drawdown duration in calendar days per ticker.
        """

    def up_capture(self) -> list[float]:
        """
        Empyrical-style annualized geometric up-capture vs benchmark.

        Returns
        -------
        list[float]
            Up-capture ratio per ticker.
        """

    def down_capture(self) -> list[float]:
        """
        Empyrical-style annualized geometric down-capture vs benchmark.

        Returns
        -------
        list[float]
            Down-capture ratio per ticker.
        """

    def capture_ratio(self) -> list[float]:
        """
        Empyrical-style annualized geometric capture ratio vs benchmark.

        Returns
        -------
        list[float]
            Up-capture divided by down-capture per ticker.
        """

    def omega_ratio(self, threshold: float = 0.0) -> list[float]:
        """
        Omega ratio for each ticker.

        Parameters
        ----------
        threshold : float, default 0.0
            Return threshold for the gain/loss split.

        Returns
        -------
        list[float]
            Omega ratio per ticker.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def treynor(self, risk_free_rate: float = 0.0) -> list[float]:
        """
        Treynor ratio for each ticker.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.

        Returns
        -------
        list[float]
            Excess return per unit of beta per ticker.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def gain_to_pain(self) -> list[float]:
        """
        Gain-to-pain ratio for each ticker.

        Returns
        -------
        list[float]
            Sum of gains divided by sum of absolute losses per ticker.
        """

    def ulcer_index(self) -> list[float]:
        """
        Ulcer index for each ticker.

        Returns
        -------
        list[float]
            Root-mean-square of drawdown depths per ticker.
        """

    def martin_ratio(self) -> list[float]:
        """
        Martin ratio for each ticker.

        Returns
        -------
        list[float]
            Excess return per unit of ulcer index per ticker.

        Raises
        ------
        ValueError
            If the active date window cannot be annualized.
        """

    def recovery_factor(self) -> list[float]:
        """
        Recovery factor for each ticker.

        Returns
        -------
        list[float]
            Total return divided by max drawdown per ticker.
        """

    def pain_index(self) -> list[float]:
        """
        Pain index for each ticker.

        Returns
        -------
        list[float]
            Average drawdown depth per ticker.
        """

    def pain_ratio(self, risk_free_rate: float = 0.0) -> list[float]:
        """
        Pain ratio for each ticker.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.

        Returns
        -------
        list[float]
            Excess return per unit of pain index per ticker.

        Raises
        ------
        ValueError
            If the active date window cannot be annualized.
        """

    def tail_ratio(self, confidence: float = 0.95) -> list[float]:
        """
        Tail ratio for each ticker.

        Parameters
        ----------
        confidence : float, default 0.95
            Confidence level for the tail quantile.

        Returns
        -------
        list[float]
            Right-tail gain divided by left-tail loss per ticker.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def r_squared(self) -> list[float]:
        """
        R-squared for each ticker vs benchmark.

        Returns
        -------
        list[float]
            Coefficient of determination per ticker.
        """

    def batting_average(self) -> list[float]:
        """
        Batting average for each ticker vs benchmark.

        Returns
        -------
        list[float]
            Fraction of periods where the ticker outperformed the benchmark.
        """

    def parametric_var(self, confidence: float = 0.95) -> list[float]:
        """
        Parametric VaR for each ticker.

        Parameters
        ----------
        confidence : float, default 0.95
            Confidence level.

        Returns
        -------
        list[float]
            Parametric VaR per ticker (negative).

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def cornish_fisher_var(self, confidence: float = 0.95) -> list[float]:
        """
        Cornish-Fisher VaR for each ticker.

        Parameters
        ----------
        confidence : float, default 0.95
            Confidence level.

        Returns
        -------
        list[float]
            Cornish-Fisher modified VaR per ticker (negative).

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def cdar(self, confidence: float = 0.95) -> list[float]:
        """
        CDaR for each ticker.

        Parameters
        ----------
        confidence : float, default 0.95
            Confidence level.

        Returns
        -------
        list[float]
            Conditional drawdown-at-risk per ticker (negative).

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def m_squared(self, risk_free_rate: float = 0.0) -> list[float]:
        """
        M-squared for each ticker.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.

        Returns
        -------
        list[float]
            M-squared measure per ticker.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def modified_sharpe(
        self,
        risk_free_rate: float = 0.0,
        confidence: float = 0.95,
    ) -> list[float]:
        """
        Modified Sharpe ratio for each ticker.

        Parameters
        ----------
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.
        confidence : float, default 0.95
            Confidence level for VaR.

        Returns
        -------
        list[float]
            Modified Sharpe ratio per ticker.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def sterling_ratio(self, risk_free_rate: float = 0.0, n: int = 5) -> list[float]:
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
        list[float]
            Sterling ratio per ticker.

        Raises
        ------
        ValueError
            If the active date window cannot be annualized.
        """

    def burke_ratio(self, risk_free_rate: float = 0.0, n: int = 5) -> list[float]:
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
        list[float]
            Burke ratio per ticker.

        Raises
        ------
        ValueError
            If the active date window cannot be annualized.
        """

    # -- Vector-per-ticker methods --

    def returns(self) -> list[list[float]]:
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
        """

    def returns_for_ticker(self, ticker_idx: int) -> list[float]:
        """
        Per-period simple returns for a single ticker.

        Parameters
        ----------
        ticker_idx : int
            Zero-based ticker column index.

        Returns
        -------
        list[float]
            Simple return series as decimal fractions (``0.01`` for ``+1%``),
            in date order, spanning that ticker's active dates.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def cumulative_returns(self) -> list[list[float]]:
        """
        Cumulative returns for each ticker.

        Returns
        -------
        list[list[float]]
            Per-ticker cumulative return time series.
        """

    def drawdown_series(self) -> list[list[float]]:
        """
        Drawdown series for each ticker.

        Returns
        -------
        list[list[float]]
            Per-ticker drawdown time series (negative or zero).
        """

    def correlation_matrix(self) -> list[list[float]]:
        """
        Correlation matrix across all tickers.

        Returns
        -------
        list[list[float]]
            Symmetric correlation matrix indexed by ticker column order.
        """

    def cumulative_returns_outperformance(self) -> list[list[float]]:
        """
        Cumulative returns outperformance vs benchmark.

        Returns
        -------
        list[list[float]]
            Per-ticker cumulative excess return time series.
        """

    def drawdown_difference(self) -> list[list[float]]:
        """
        Drawdown difference vs benchmark.

        Returns
        -------
        list[list[float]]
            Per-ticker drawdown difference time series.
        """

    def excess_returns(
        self,
        rf: list[float],
        nperiods: float | None = None,
    ) -> list[list[float]]:
        """
        Excess returns over a risk-free series (per ticker).

        Parameters
        ----------
        rf : list[float]
            Risk-free return series aligned with the observation dates.
        nperiods : float, optional
            Annualization factor (e.g. ``252`` for daily). When ``None``,
            uses the engine's frequency default.

        Returns
        -------
        list[list[float]]
            Per-ticker excess return series.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    # -- Per-ticker structured methods --

    def beta(self) -> list[BetaResult]:
        """
        Beta for each ticker vs benchmark.

        Returns
        -------
        list[BetaResult]
            Per-ticker :class:`BetaResult` with CI.
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

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def rolling_greeks(
        self,
        ticker_idx: int,
        window: int = 63,
        risk_free_rate: float = 0.0,
    ) -> RollingGreeks:
        """
        Rolling greeks for a specific ticker.

        Parameters
        ----------
        ticker_idx : int
            Zero-based ticker column index.
        window : int, default 63
            Rolling window size in observations.
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.

        Returns
        -------
        RollingGreeks
            :class:`RollingGreeks` with dates, alphas, and betas.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def rolling_volatility(self, ticker_idx: int, window: int = 63) -> DatedSeries:
        """
        Rolling volatility for a specific ticker (column name ``volatility``).

        Parameters
        ----------
        ticker_idx : int
            Zero-based ticker column index.
        window : int, default 63
            Rolling window size in observations.

        Returns
        -------
        DatedSeries
            :class:`DatedSeries` with ``value_column="volatility"``.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def rolling_sortino(self, ticker_idx: int, window: int = 63, mar: float = 0.0) -> DatedSeries:
        """
        Rolling Sortino for a specific ticker (column name ``sortino``).

        Parameters
        ----------
        ticker_idx : int
            Zero-based ticker column index.
        window : int, default 63
            Rolling window size in observations.
        mar : float, default 0.0
            Minimum acceptable return per period.

        Returns
        -------
        DatedSeries
            :class:`DatedSeries` with ``value_column="sortino"``.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def rolling_sharpe(
        self,
        ticker_idx: int,
        window: int = 63,
        risk_free_rate: float = 0.0,
    ) -> DatedSeries:
        """
        Rolling Sharpe for a specific ticker (column name ``sharpe``).

        Parameters
        ----------
        ticker_idx : int
            Zero-based ticker column index.
        window : int, default 63
            Rolling window size in observations.
        risk_free_rate : float, default 0.0
            Annualized risk-free rate as a decimal.

        Returns
        -------
        DatedSeries
            :class:`DatedSeries` with ``value_column="sharpe"``.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def rolling_returns(self, ticker_idx: int, window: int) -> DatedSeries:
        """
        Rolling N-period compounded total return (column name ``return``).

        Parameters
        ----------
        ticker_idx : int
            Zero-based ticker column index.
        window : int
            Rolling window size in observations.

        Returns
        -------
        DatedSeries
            :class:`DatedSeries` with ``value_column="return"``.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def drawdown_details(self, ticker_idx: int, n: int = 5) -> list[DrawdownEpisode]:
        """
        Top-N drawdown episodes for a specific ticker.

        Parameters
        ----------
        ticker_idx : int
            Zero-based ticker column index.
        n : int, default 5
            Number of largest drawdown episodes to return.

        Returns
        -------
        list[DrawdownEpisode]
            List of :class:`DrawdownEpisode` objects, deepest first.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def multi_factor_greeks(
        self,
        ticker_idx: int,
        factor_returns: list[list[float]],
    ) -> MultiFactorResult:
        """
        Multi-factor regression for a specific ticker.

        Parameters
        ----------
        ticker_idx : int
            Zero-based ticker column index.
        factor_returns : list[list[float]]
            Column-major factor return matrix; ``factor_returns[i]`` is the
            return series for factor ``i``.

        Returns
        -------
        MultiFactorResult
            :class:`MultiFactorResult` with alpha, betas, and fit statistics.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """

    def lookback_returns(
        self,
        ref_date: object,
        fiscal_year_start_month: int | None = None,
        fiscal_year_start_day: int | None = None,
        calendar: str = "nyse",
    ) -> LookbackReturns:
        """
        Period-to-date lookback returns.

        Defaults to a January-1 fiscal-year start. The FYTD window start is
        adjusted to the next business day on *calendar* (default ``"nyse"``);
        pass the calendar id matching your market for non-US panels.

        Parameters
        ----------
        ref_date : object
            Reference date (``datetime.date``, ``pd.Timestamp``, or ISO string).
        fiscal_year_start_month : int, optional
            Fiscal year start month in ``1..=12``.
        fiscal_year_start_day : int, optional
            Fiscal year start day in ``1..=31``.
        calendar : str, default "nyse"
            Business-day calendar id for FYTD adjustments.

        Returns
        -------
        LookbackReturns
            :class:`LookbackReturns` with MTD, QTD, YTD, and optional FYTD.

        Raises
        ------
        ValueError
            If *fiscal_year_start_month* is not in ``1..=12``,
            *fiscal_year_start_day* is not in ``1..=31``, or *calendar*
            is not a registered calendar id.
        """

    def period_stats(
        self,
        ticker_idx: int,
        agg_freq: str = "monthly",
        fiscal_year_start_month: int | None = None,
        fiscal_year_start_day: int | None = None,
    ) -> PeriodStats:
        """
        Period statistics for one ticker at a given aggregation frequency.

        Parameters
        ----------
        ticker_idx : int
            Zero-based ticker column index.
        agg_freq : str, default "monthly"
            Aggregation frequency (``"daily"``, ``"weekly"``, ``"monthly"``,
            ``"quarterly"``, ``"annual"``).
        fiscal_year_start_month : int, optional
            Fiscal year start month in ``1..=12``.
        fiscal_year_start_day : int, optional
            Fiscal year start day in ``1..=31``.

        Returns
        -------
        PeriodStats
            :class:`PeriodStats` with win/loss streaks, ratios, etc.

        Raises
        ------
        ValueError
            If *fiscal_year_start_month* is not in ``1..=12`` or
            *fiscal_year_start_day* is not in ``1..=31``.
        """

    # -- DataFrame export methods --

    def summary_to_dataframe(
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
            Annualized risk-free rate as a decimal.
        confidence : float, default 0.95
            Confidence level for VaR, ES, and tail ratio.

        Returns
        -------
        pd.DataFrame
            Summary statistics indexed by ticker name.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def returns_to_dataframe(self) -> pd.DataFrame:
        """
        Per-period simple returns for all tickers as a pandas DataFrame.

        Ragged per-ticker series are padded with ``NaN`` onto the active date
        grid. Prefer this over :meth:`excess_returns` with an all-zero
        risk-free series or un-compounding
        :meth:`cumulative_returns_to_dataframe`.

        Returns
        -------
        pd.DataFrame
            Simple returns indexed by date, one column per ticker.
        """
        ...

    def cumulative_returns_to_dataframe(self) -> pd.DataFrame:
        """
        Cumulative returns for all tickers as a pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Cumulative returns indexed by date, one column per ticker.
        """
        ...

    def periodic_returns_to_dataframe(self, freq: str = "monthly") -> pd.DataFrame:
        """
        Calendar-bucketed compounded returns for all tickers.

        Parameters
        ----------
        freq : str, default "monthly"
            Bucketing frequency: one of ``"daily"``, ``"weekly"``,
            ``"monthly"``, ``"quarterly"``, ``"semiannual"``, ``"annual"``.

        Returns
        -------
        pd.DataFrame
            Compounded period returns indexed by period-end date, one column
            per ticker. Buckets reconcile with
            :meth:`cumulative_returns_to_dataframe`.

        Raises
        ------
        ValueError
            If ``freq`` is not a recognized frequency.
        """
        ...

    def drawdown_series_to_dataframe(self) -> pd.DataFrame:
        """
        Drawdown series for all tickers as a pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Drawdown series indexed by date, one column per ticker.
        """
        ...

    def correlation_to_dataframe(self) -> pd.DataFrame:
        """
        Correlation matrix as a pandas DataFrame indexed by ticker name.

        Returns
        -------
        pd.DataFrame
            Symmetric correlation matrix with ticker names on both axes.
        """
        ...

    def drawdown_details_to_dataframe(
        self,
        ticker_idx: int,
        n: int = 5,
    ) -> pd.DataFrame:
        """
        Top-N drawdown episodes for a ticker as a pandas DataFrame.

        Columns: ``start``, ``valley``, ``end``, ``duration_days``,
        ``max_drawdown``, ``near_recovery_threshold``, ``truncated_at_start``.

        Parameters
        ----------
        ticker_idx : int
            Zero-based ticker column index.
        n : int, default 5
            Number of largest drawdown episodes to return.

        Returns
        -------
        pd.DataFrame
            Drawdown episodes, one row per episode, deepest first.

        Raises
        ------
        AnalyticsError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def lookback_returns_to_dataframe(
        self,
        ref_date: object,
        fiscal_year_start_month: int | None = None,
        fiscal_year_start_day: int | None = None,
        calendar: str = "nyse",
    ) -> pd.DataFrame:
        """
        Period-to-date lookback returns as a pandas DataFrame.

        Indexed by ticker name with columns ``mtd``, ``qtd``, ``ytd``,
        and ``fytd``. See :meth:`lookback_returns` for the FYTD fiscal-start
        and *calendar* semantics (default ``"nyse"``).

        Parameters
        ----------
        ref_date : object
            Reference date.
        fiscal_year_start_month : int, optional
            Fiscal year start month in ``1..=12``.
        fiscal_year_start_day : int, optional
            Fiscal year start day in ``1..=31``.
        calendar : str, default "nyse"
            Business-day calendar id.

        Returns
        -------
        pd.DataFrame
            Lookback returns indexed by ticker name.

        Raises
        ------
        ValueError
            If *fiscal_year_start_month* is not in ``1..=12``,
            *fiscal_year_start_day* is not in ``1..=31``, or *calendar*
            is not a registered calendar id.
        """
        ...
