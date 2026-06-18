"""Performance analytics: returns, drawdowns, risk metrics, and benchmarks.

The sole entry point is :class:`Performance`. Construct from a price panel
(``Performance(prices_df)`` / ``Performance.from_arrays(...)``) or from a
return panel (``Performance.from_returns(returns_df)`` /
``Performance.from_returns_arrays(...)``); every analytic — return / risk
scalars, drawdown statistics, rolling windows, periodic returns
(MTD / QTD / YTD / FYTD), benchmark alpha/beta, basic factor models — is a
method on the resulting instance.

The remaining classes are value-object outputs returned by `Performance`
methods (`LookbackReturns`, `PeriodStats`, etc.).
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
    """Analytics validation or calculation failure."""

# ---------------------------------------------------------------------------
# Value-object results
# ---------------------------------------------------------------------------

class PeriodStats:
    """Aggregated statistics for grouped periodic returns."""

    @property
    def best(self) -> float:
        """Best period return.
        Returns
        -------
        float
        """

    @property
    def worst(self) -> float:
        """Worst period return.
        Returns
        -------
        float
        """

    @property
    def consecutive_wins(self) -> int:
        """Longest consecutive winning streak.
        Returns
        -------
        int
        """

    @property
    def consecutive_losses(self) -> int:
        """Longest consecutive losing streak.
        Returns
        -------
        int
        """

    @property
    def win_rate(self) -> float:
        """Fraction of positive-return periods.
        Returns
        -------
        float
        """

    @property
    def avg_return(self) -> float:
        """Average return across all periods.
        Returns
        -------
        float
        """

    @property
    def avg_win(self) -> float:
        """Average return of positive periods.
        Returns
        -------
        float
        """

    @property
    def avg_loss(self) -> float:
        """Average return of negative periods.
        Returns
        -------
        float
        """

    @property
    def payoff_ratio(self) -> float:
        """Payoff ratio (avg win / |avg loss|).
        Returns
        -------
        float
        """

    @property
    def profit_factor(self) -> float:
        """Profit factor (gross profits / gross losses).
        Returns
        -------
        float
        """

    @property
    def cpc_ratio(self) -> float:
        """CPC index (profit_factor x win_rate x payoff_ratio).
        Returns
        -------
        float
        """

    @property
    def kelly_criterion(self) -> float:
        """Kelly criterion optimal fraction.
        Returns
        -------
        float
        """

    def __repr__(self) -> str: ...

class BetaResult:
    """Regression beta with confidence interval.

    The 95% interval uses Student-t critical values for finite samples and an
    asymptotic normal approximation once ``n - 2 >= 240``.
    """

    @property
    def beta(self) -> float:
        """Beta coefficient.
        Returns
        -------
        float
        """

    @property
    def std_err(self) -> float:
        """Standard error of the beta estimate.
        Returns
        -------
        float
        """

    @property
    def ci_lower(self) -> float:
        """Lower 95% confidence bound.
        Returns
        -------
        float
        """

    @property
    def ci_upper(self) -> float:
        """Upper 95% confidence bound.
        Returns
        -------
        float
        """

    def __repr__(self) -> str: ...

class GreeksResult:
    """Alpha, beta, and goodness-of-fit from a single-index regression."""

    @property
    def alpha(self) -> float:
        """Annualized Jensen alpha.
        Returns
        -------
        float
        """

    @property
    def beta(self) -> float:
        """Beta coefficient.
        Returns
        -------
        float
        """

    @property
    def r_squared(self) -> float:
        """R-squared.
        Returns
        -------
        float
        """

    @property
    def adjusted_r_squared(self) -> float:
        """Adjusted R-squared.
        Returns
        -------
        float
        """

    def __repr__(self) -> str: ...

class RollingGreeks:
    """Rolling alpha and beta time series."""

    @property
    def dates(self) -> list[datetime.date]:
        """Date labels for each rolling window.
        Returns
        -------
        list[datetime.date]
        """

    @property
    def alphas(self) -> npt.NDArray[np.float64]:
        """Rolling alpha values.
        Returns
        -------
        npt.NDArray[np.float64]
        """

    @property
    def betas(self) -> npt.NDArray[np.float64]:
        """Rolling beta values.
        Returns
        -------
        npt.NDArray[np.float64]
        """

    def to_dataframe(self) -> pd.DataFrame:
        """Convert to a pandas DataFrame with date index and alpha/beta columns.
        Returns
        -------
        pd.DataFrame
        """
        ...

    def __repr__(self) -> str: ...

class MultiFactorResult:
    """Multi-factor regression result."""

    @property
    def alpha(self) -> float:
        """Raw regression intercept, annualized with the supplied factor frequency.
        Returns
        -------
        float
        """

    @property
    def betas(self) -> npt.NDArray[np.float64]:
        """Factor betas.
        Returns
        -------
        npt.NDArray[np.float64]
        """

    @property
    def r_squared(self) -> float:
        """R-squared.
        Returns
        -------
        float
        """

    @property
    def adjusted_r_squared(self) -> float:
        """Adjusted R-squared.
        Returns
        -------
        float
        """

    @property
    def residual_vol(self) -> float:
        """Residual volatility.
        Returns
        -------
        float
        """

    def __repr__(self) -> str: ...

class DrawdownEpisode:
    """A single drawdown episode with timing and depth information."""

    @property
    def start(self) -> datetime.date:
        """Start date of the drawdown.
        Returns
        -------
        datetime.date
        """

    @property
    def valley(self) -> datetime.date:
        """Date of the maximum drawdown within this episode.
        Returns
        -------
        datetime.date
        """

    @property
    def end(self) -> datetime.date | None:
        """Recovery date (``None`` if still in drawdown).
        Returns
        -------
        datetime.date or None
        """

    @property
    def duration_days(self) -> int:
        """Duration in calendar days.
        Returns
        -------
        int
        """

    @property
    def max_drawdown(self) -> float:
        """Maximum drawdown depth (negative).
        Returns
        -------
        float
        """

    @property
    def near_recovery_threshold(self) -> float:
        """Near-recovery threshold.
        Returns
        -------
        float
        """

    @property
    def truncated_at_start(self) -> bool:
        """True when the episode began before the first observation (left-censored).
        Returns
        -------
        bool
        """

    def __repr__(self) -> str: ...

class LookbackReturns:
    """Period-to-date returns for each ticker."""

    @property
    def mtd(self) -> npt.NDArray[np.float64]:
        """Month-to-date returns per ticker.
        Returns
        -------
        npt.NDArray[np.float64]
        """

    @property
    def qtd(self) -> npt.NDArray[np.float64]:
        """Quarter-to-date returns per ticker.
        Returns
        -------
        npt.NDArray[np.float64]
        """

    @property
    def ytd(self) -> npt.NDArray[np.float64]:
        """Year-to-date returns per ticker.
        Returns
        -------
        npt.NDArray[np.float64]
        """

    @property
    def fytd(self) -> npt.NDArray[np.float64] | None:
        """Fiscal-year-to-date returns when a fiscal config is provided.
        Returns
        -------
        npt.NDArray[np.float64] or None
        """

    def to_dataframe(self, ticker_names: list[str]) -> pd.DataFrame:
        """Convert to a pandas DataFrame with ticker names as index.

        Columns: ``mtd``, ``qtd``, ``ytd`` (and ``fytd`` when available).
        """
        ...

    def __repr__(self) -> str: ...

class DatedSeries:
    """Date-indexed numeric series returned by the rolling-window analytics.

    Rolling-window methods return this shared carrier with a metric-specific
    DataFrame column name.
    """

    @property
    def values(self) -> npt.NDArray[np.float64]:
        """Rolling values, one per window.
        Returns
        -------
        npt.NDArray[np.float64]
        """

    @property
    def dates(self) -> list[datetime.date]:
        """Window-end dates aligned 1:1 with :attr:`values`.
        Returns
        -------
        list[datetime.date]
        """

    @property
    def value_column(self) -> str:
        """Column name used by :meth:`to_dataframe`.
        Returns
        -------
        str
        """

    def to_dataframe(self) -> pd.DataFrame:
        """Convert to a pandas DataFrame with date index and a value column.

        The column is named after :attr:`value_column` (e.g. ``sharpe``,
        ``sortino``, ``volatility``, or ``return``).

        Returns
        -------
        pd.DataFrame
        """
        ...

    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Performance engine
# ---------------------------------------------------------------------------

class Performance:
    """Stateful performance analytics engine over a panel of ticker series.

    Construct from a pandas DataFrame of prices (``Performance(df)``), a
    DataFrame of returns (``Performance.from_returns(df)``), or from raw
    arrays via :meth:`from_arrays` / :meth:`from_returns_arrays`.
    """

    def __init__(
        self,
        prices: pd.DataFrame,
        benchmark_ticker: str | None = None,
        freq: str = "daily",
    ) -> None:
        """Build from a pandas DataFrame of prices.

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
        """Construct from raw arrays (dates, prices matrix, ticker names).

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
        """Build from a pandas DataFrame of simple returns.

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
        """Construct from raw return arrays (dates, returns matrix, ticker names).

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
        """

    # -- Mutators --

    def reset_date_range(self, start: object, end: object) -> None:
        """Restrict analytics to ``[start, end]``."""

    def reset_bench_ticker(self, ticker: str) -> None:
        """Change the benchmark ticker."""

    # -- Getters --

    @property
    def ticker_names(self) -> list[str]:
        """Ticker names in column order.
        Returns
        -------
        list[str]
        """

    @property
    def benchmark_idx(self) -> int:
        """Benchmark column index.
        Returns
        -------
        int
        """

    @property
    def freq(self) -> str:
        """Observation frequency as the canonical lowercase token.
        Returns
        -------
        str
        """

    def dates(self) -> list[datetime.date]:
        """Full return-aligned date grid (independent of any active window).
        Returns
        -------
        list[datetime.date]
        """

    def active_dates(self) -> list[datetime.date]:
        """Observation dates of the currently active analysis window.
        Returns
        -------
        list[datetime.date]
        """

    def active_dates_for_ticker(self, ticker_idx: int) -> list[datetime.date]:
        """Observation dates for one ticker's active return series."""

    # -- Scalar-per-ticker methods --

    def cagr(self) -> list[float]:
        """CAGR for each ticker.

        Raises:
            ValueError: If the active date window cannot be annualized.

        Returns
        -------
        list[float]
        """

    def mean_return(self, annualize: bool = True) -> list[float]:
        """Mean return for each ticker."""

    def volatility(self, annualize: bool = True) -> list[float]:
        """Volatility for each ticker."""

    def sharpe(self, risk_free_rate: float = 0.0) -> list[float]:
        """Sharpe ratio for each ticker.

        Args:
            risk_free_rate: Annualized risk-free rate as a decimal (default 0).

        Returns:
            Per-ticker Sharpe ratios over the active return window.
        """

    def sortino(self, mar: float = 0.0) -> list[float]:
        """Sortino ratio for each ticker.

        Args:
            mar: Minimum acceptable return per period (not annualized).

        Returns:
            Per-ticker Sortino ratios over the active return window.

        Note:
            ``mar`` is per-period; Sharpe ``risk_free_rate`` inputs are annualized.
        """

    def calmar(self) -> list[float]:
        """Calmar ratio for each ticker.

        Raises:
            ValueError: If the active date window cannot be annualized.

        Returns
        -------
        list[float]
        """

    def max_drawdown(self) -> list[float]:
        """Max drawdown for each ticker.
        Returns
        -------
        list[float]
        """

    def mean_drawdown(self) -> list[float]:
        """Mean drawdown (path-weighted average) for each ticker.
        Returns
        -------
        list[float]
        """

    def value_at_risk(self, confidence: float = 0.95) -> list[float]:
        """Historical VaR for each ticker."""

    def expected_shortfall(self, confidence: float = 0.95) -> list[float]:
        """Expected Shortfall for each ticker."""

    def tracking_error(self) -> list[float]:
        """Tracking error for each ticker vs benchmark.
        Returns
        -------
        list[float]
        """

    def information_ratio(self) -> list[float]:
        """Information ratio for each ticker vs benchmark.
        Returns
        -------
        list[float]
        """

    def skewness(self) -> list[float]:
        """Skewness for each ticker.
        Returns
        -------
        list[float]
        """

    def kurtosis(self) -> list[float]:
        """Kurtosis for each ticker.
        Returns
        -------
        list[float]
        """

    def geometric_mean(self) -> list[float]:
        """Geometric mean for each ticker.
        Returns
        -------
        list[float]
        """

    def skew_kurt(self) -> tuple[list[float], list[float]]:
        """Per-ticker ``(skewness, kurtosis)`` from one moments pass.
        Returns
        -------
        tuple[list[float], list[float]]
        """

    def value_at_risk_and_es(
        self,
        confidence: float = 0.95,
    ) -> tuple[list[float], list[float]]:
        """Per-ticker ``(value_at_risk, expected_shortfall)`` from one tail pass."""

    def downside_deviation(self, mar: float = 0.0) -> list[float]:
        """Downside deviation for each ticker.

        ``mar`` is per-period; Sharpe risk-free inputs are annualized.
        """

    def max_drawdown_duration(self) -> list[int]:
        """Max drawdown duration (calendar days) for each ticker.
        Returns
        -------
        list[int]
        """

    def up_capture(self) -> list[float]:
        """Empyrical-style annualized geometric up-capture vs benchmark.
        Returns
        -------
        list[float]
        """

    def down_capture(self) -> list[float]:
        """Empyrical-style annualized geometric down-capture vs benchmark.
        Returns
        -------
        list[float]
        """

    def capture_ratio(self) -> list[float]:
        """Empyrical-style annualized geometric capture ratio vs benchmark.
        Returns
        -------
        list[float]
        """

    def omega_ratio(self, threshold: float = 0.0) -> list[float]:
        """Omega ratio for each ticker."""

    def treynor(self, risk_free_rate: float = 0.0) -> list[float]:
        """Treynor ratio for each ticker."""

    def gain_to_pain(self) -> list[float]:
        """Gain-to-pain ratio for each ticker.
        Returns
        -------
        list[float]
        """

    def ulcer_index(self) -> list[float]:
        """Ulcer index for each ticker.
        Returns
        -------
        list[float]
        """

    def martin_ratio(self) -> list[float]:
        """Martin ratio for each ticker.

        Raises:
            ValueError: If the active date window cannot be annualized.

        Returns
        -------
        list[float]
        """

    def recovery_factor(self) -> list[float]:
        """Recovery factor for each ticker.
        Returns
        -------
        list[float]
        """

    def pain_index(self) -> list[float]:
        """Pain index for each ticker.
        Returns
        -------
        list[float]
        """

    def pain_ratio(self, risk_free_rate: float = 0.0) -> list[float]:
        """Pain ratio for each ticker.

        Raises:
            ValueError: If the active date window cannot be annualized.
        """

    def tail_ratio(self, confidence: float = 0.95) -> list[float]:
        """Tail ratio for each ticker."""

    def r_squared(self) -> list[float]:
        """R-squared for each ticker vs benchmark.
        Returns
        -------
        list[float]
        """

    def batting_average(self) -> list[float]:
        """Batting average for each ticker vs benchmark.
        Returns
        -------
        list[float]
        """

    def parametric_var(self, confidence: float = 0.95) -> list[float]:
        """Parametric VaR for each ticker."""

    def cornish_fisher_var(self, confidence: float = 0.95) -> list[float]:
        """Cornish-Fisher VaR for each ticker."""

    def cdar(self, confidence: float = 0.95) -> list[float]:
        """CDaR for each ticker."""

    def m_squared(self, risk_free_rate: float = 0.0) -> list[float]:
        """M-squared for each ticker."""

    def modified_sharpe(
        self,
        risk_free_rate: float = 0.0,
        confidence: float = 0.95,
    ) -> list[float]:
        """Modified Sharpe ratio for each ticker."""

    def sterling_ratio(self, risk_free_rate: float = 0.0, n: int = 5) -> list[float]:
        """Sterling ratio for each ticker.

        Raises:
            ValueError: If the active date window cannot be annualized.
        """

    def burke_ratio(self, risk_free_rate: float = 0.0, n: int = 5) -> list[float]:
        """Burke ratio for each ticker.

        Raises:
            ValueError: If the active date window cannot be annualized.
        """

    # -- Vector-per-ticker methods --

    def cumulative_returns(self) -> list[list[float]]:
        """Cumulative returns for each ticker.
        Returns
        -------
        list[list[float]]
        """

    def drawdown_series(self) -> list[list[float]]:
        """Drawdown series for each ticker.
        Returns
        -------
        list[list[float]]
        """

    def correlation_matrix(self) -> list[list[float]]:
        """Correlation matrix across all tickers.
        Returns
        -------
        list[list[float]]
        """

    def cumulative_returns_outperformance(self) -> list[list[float]]:
        """Cumulative returns outperformance vs benchmark.
        Returns
        -------
        list[list[float]]
        """

    def drawdown_difference(self) -> list[list[float]]:
        """Drawdown difference vs benchmark.
        Returns
        -------
        list[list[float]]
        """

    def excess_returns(
        self,
        rf: list[float],
        nperiods: float | None = None,
    ) -> list[list[float]]:
        """Excess returns over a risk-free series (per ticker)."""

    # -- Per-ticker structured methods --

    def beta(self) -> list[BetaResult]:
        """Beta for each ticker vs benchmark.
        Returns
        -------
        list[BetaResult]
        """

    def greeks(self, risk_free_rate: float = 0.0) -> list[GreeksResult]:
        """Greeks (annualized Jensen alpha, beta, R²) for each ticker vs benchmark."""

    def rolling_greeks(
        self,
        ticker_idx: int,
        window: int = 63,
        risk_free_rate: float = 0.0,
    ) -> RollingGreeks:
        """Rolling greeks for a specific ticker."""

    def rolling_volatility(self, ticker_idx: int, window: int = 63) -> DatedSeries:
        """Rolling volatility for a specific ticker (column name ``volatility``)."""

    def rolling_sortino(self, ticker_idx: int, window: int = 63, mar: float = 0.0) -> DatedSeries:
        """Rolling Sortino for a specific ticker (column name ``sortino``)."""

    def rolling_sharpe(
        self,
        ticker_idx: int,
        window: int = 63,
        risk_free_rate: float = 0.0,
    ) -> DatedSeries:
        """Rolling Sharpe for a specific ticker (column name ``sharpe``)."""

    def rolling_returns(self, ticker_idx: int, window: int) -> DatedSeries:
        """Rolling N-period compounded total return (column name ``return``)."""

    def drawdown_details(self, ticker_idx: int, n: int = 5) -> list[DrawdownEpisode]:
        """Top-N drawdown episodes for a specific ticker."""

    def multi_factor_greeks(
        self,
        ticker_idx: int,
        factor_returns: list[list[float]],
    ) -> MultiFactorResult:
        """Multi-factor regression for a specific ticker."""

    def lookback_returns(
        self,
        ref_date: object,
        fiscal_year_start_month: int | None = None,
        fiscal_year_start_day: int | None = None,
        calendar: str = "nyse",
    ) -> LookbackReturns:
        """Period-to-date lookback returns.

        Defaults to a January-1 fiscal-year start. The FYTD window start is
        adjusted to the next business day on *calendar* (default ``"nyse"``);
        pass the calendar id matching your market for non-US panels.

        Raises:
            ValueError: If *fiscal_year_start_month* is not in ``1..=12``,
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
        """Period statistics for one ticker at a given aggregation frequency.

        Raises:
            ValueError: If *fiscal_year_start_month* is not in ``1..=12`` or
                *fiscal_year_start_day* is not in ``1..=31``.
        """

    # -- DataFrame export methods --

    def summary_to_dataframe(
        self,
        risk_free_rate: float = 0.0,
        confidence: float = 0.95,
    ) -> pd.DataFrame:
        """Summary statistics for all tickers as a pandas DataFrame.

        *risk_free_rate* affects only the ``sharpe`` column; the MAR-based
        metrics (``sortino``, ``downside_deviation``) and the ``omega_ratio``
        threshold are fixed at ``0.0``. *confidence* applies to
        ``value_at_risk``, ``expected_shortfall``, and ``tail_ratio``.
        """
        ...

    def cumulative_returns_to_dataframe(self) -> pd.DataFrame:
        """Cumulative returns for all tickers as a pandas DataFrame.
        Returns
        -------
        pd.DataFrame
        """
        ...

    def drawdown_series_to_dataframe(self) -> pd.DataFrame:
        """Drawdown series for all tickers as a pandas DataFrame.
        Returns
        -------
        pd.DataFrame
        """
        ...

    def correlation_to_dataframe(self) -> pd.DataFrame:
        """Correlation matrix as a pandas DataFrame indexed by ticker name.
        Returns
        -------
        pd.DataFrame
        """
        ...

    def drawdown_details_to_dataframe(
        self,
        ticker_idx: int,
        n: int = 5,
    ) -> pd.DataFrame:
        """Top-N drawdown episodes for a ticker as a pandas DataFrame.

        Columns: ``start``, ``valley``, ``end``, ``duration_days``,
        ``max_drawdown``, ``near_recovery_threshold``, ``truncated_at_start``.
        """
        ...

    def lookback_returns_to_dataframe(
        self,
        ref_date: object,
        fiscal_year_start_month: int | None = None,
        fiscal_year_start_day: int | None = None,
        calendar: str = "nyse",
    ) -> pd.DataFrame:
        """Period-to-date lookback returns as a pandas DataFrame.

        Indexed by ticker name with columns ``mtd``, ``qtd``, ``ytd``,
        and ``fytd``. See :meth:`lookback_returns` for the FYTD fiscal-start
        and *calendar* semantics (default ``"nyse"``).

        Raises:
            ValueError: If *fiscal_year_start_month* is not in ``1..=12``,
                *fiscal_year_start_day* is not in ``1..=31``, or *calendar*
                is not a registered calendar id.
        """
        ...
