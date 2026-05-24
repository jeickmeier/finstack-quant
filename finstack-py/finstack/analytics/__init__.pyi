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
        """Best period return."""

    @property
    def worst(self) -> float:
        """Worst period return."""

    @property
    def consecutive_wins(self) -> int:
        """Longest consecutive winning streak."""

    @property
    def consecutive_losses(self) -> int:
        """Longest consecutive losing streak."""

    @property
    def win_rate(self) -> float:
        """Fraction of positive-return periods."""

    @property
    def avg_return(self) -> float:
        """Average return across all periods."""

    @property
    def avg_win(self) -> float:
        """Average return of positive periods."""

    @property
    def avg_loss(self) -> float:
        """Average return of negative periods."""

    @property
    def payoff_ratio(self) -> float:
        """Payoff ratio (avg win / |avg loss|)."""

    @property
    def profit_factor(self) -> float:
        """Profit factor (gross profits / gross losses)."""

    @property
    def cpc_ratio(self) -> float:
        """Common-sense ratio (CPC)."""

    @property
    def kelly_criterion(self) -> float:
        """Kelly criterion optimal fraction."""

    def __repr__(self) -> str: ...

class BetaResult:
    """Regression beta with confidence interval."""

    @property
    def beta(self) -> float:
        """Beta coefficient."""

    @property
    def std_err(self) -> float:
        """Standard error of the beta estimate."""

    @property
    def ci_lower(self) -> float:
        """Lower 95% confidence bound."""

    @property
    def ci_upper(self) -> float:
        """Upper 95% confidence bound."""

    def __repr__(self) -> str: ...

class GreeksResult:
    """Alpha, beta, and goodness-of-fit from a single-index regression."""

    @property
    def alpha(self) -> float:
        """Jensen's alpha (annualized)."""

    @property
    def beta(self) -> float:
        """Beta coefficient."""

    @property
    def r_squared(self) -> float:
        """R-squared."""

    @property
    def adjusted_r_squared(self) -> float:
        """Adjusted R-squared."""

    def __repr__(self) -> str: ...

class RollingGreeks:
    """Rolling alpha and beta time series."""

    def dates(self) -> list[datetime.date]:
        """Date labels for each rolling window."""

    @property
    def alphas(self) -> list[float]:
        """Rolling alpha values."""

    @property
    def betas(self) -> list[float]:
        """Rolling beta values."""

    def to_dataframe(self) -> pd.DataFrame:
        """Convert to a pandas DataFrame with date index and alpha/beta columns."""
        ...

    def __repr__(self) -> str: ...

class MultiFactorResult:
    """Multi-factor regression result."""

    @property
    def alpha(self) -> float:
        """Intercept (alpha)."""

    @property
    def betas(self) -> list[float]:
        """Factor betas."""

    @property
    def r_squared(self) -> float:
        """R-squared."""

    @property
    def adjusted_r_squared(self) -> float:
        """Adjusted R-squared."""

    @property
    def residual_vol(self) -> float:
        """Residual volatility."""

    def __repr__(self) -> str: ...

class DrawdownEpisode:
    """A single drawdown episode with timing and depth information."""

    def start(self) -> datetime.date:
        """Start date of the drawdown."""

    def valley(self) -> datetime.date:
        """Date of the maximum drawdown within this episode."""

    def end(self) -> datetime.date | None:
        """Recovery date (``None`` if still in drawdown)."""

    @property
    def duration_days(self) -> int:
        """Duration in calendar days."""

    @property
    def max_drawdown(self) -> float:
        """Maximum drawdown depth (negative)."""

    @property
    def near_recovery_threshold(self) -> float:
        """Near-recovery threshold."""

    @property
    def truncated_at_start(self) -> bool:
        """True when the episode began before the first observation (left-censored)."""

    def __repr__(self) -> str: ...

class LookbackReturns:
    """Period-to-date returns for each ticker."""

    @property
    def mtd(self) -> list[float]:
        """Month-to-date returns per ticker."""

    @property
    def qtd(self) -> list[float]:
        """Quarter-to-date returns per ticker."""

    @property
    def ytd(self) -> list[float]:
        """Year-to-date returns per ticker."""

    @property
    def fytd(self) -> list[float] | None:
        """Fiscal-year-to-date returns when a fiscal config is provided."""

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
    def values(self) -> list[float]:
        """Rolling values, one per window."""

    def dates(self) -> list[datetime.date]:
        """Window-end dates aligned 1:1 with :attr:`values`."""

    @property
    def value_column(self) -> str:
        """Column name used by :meth:`to_dataframe`."""

    def to_dataframe(self) -> pd.DataFrame:
        """Convert to a pandas DataFrame with date index and a value column.

        The column is named after :attr:`value_column` (e.g. ``sharpe``,
        ``sortino``, ``volatility``, or ``return``).
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

        Args:
            prices: Price panel with a date-like index and one column per ticker.
            benchmark_ticker: Benchmark column name; first column if ``None``.
            freq: ``daily``, ``weekly``, ``monthly``, ``quarterly``,
                ``semiannual``, or ``annual``.
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

        ``prices[i]`` is the series for ticker *i*.
        """

    @staticmethod
    def from_returns(
        returns: pd.DataFrame,
        benchmark_ticker: str | None = None,
        freq: str = "daily",
    ) -> Performance:
        """Build from a pandas DataFrame of simple returns."""

    @staticmethod
    def from_returns_arrays(
        dates: Sequence[object],
        returns: list[list[float]],
        ticker_names: list[str],
        benchmark_ticker: str | None = None,
        freq: str = "daily",
    ) -> Performance:
        """Construct from raw return arrays (dates, returns matrix, ticker names)."""

    # -- Mutators --

    def reset_date_range(self, start: object, end: object) -> None:
        """Restrict analytics to ``[start, end]``."""

    def reset_bench_ticker(self, ticker: str) -> None:
        """Change the benchmark ticker."""

    # -- Getters --

    @property
    def ticker_names(self) -> list[str]:
        """Ticker names in column order."""

    @property
    def benchmark_idx(self) -> int:
        """Benchmark column index."""

    @property
    def freq(self) -> str:
        """Observation frequency as the canonical lowercase token."""

    def dates(self) -> list[datetime.date]:
        """Active observation dates after any window filter."""

    # -- Scalar-per-ticker methods --

    def cagr(self) -> list[float]:
        """CAGR for each ticker.

        Raises:
            ValueError: If the active date window cannot be annualized.
        """

    def mean_return(self, annualize: bool = True) -> list[float]:
        """Mean return for each ticker."""

    def volatility(self, annualize: bool = True) -> list[float]:
        """Volatility for each ticker."""

    def sharpe(self, risk_free_rate: float = 0.0) -> list[float]:
        """Sharpe ratio for each ticker."""

    def sortino(self, mar: float = 0.0) -> list[float]:
        """Sortino ratio for each ticker."""

    def calmar(self) -> list[float]:
        """Calmar ratio for each ticker.

        Raises:
            ValueError: If the active date window cannot be annualized.
        """

    def max_drawdown(self) -> list[float]:
        """Max drawdown for each ticker."""

    def mean_drawdown(self) -> list[float]:
        """Mean drawdown (path-weighted average) for each ticker."""

    def value_at_risk(self, confidence: float = 0.95) -> list[float]:
        """Historical VaR for each ticker."""

    def expected_shortfall(self, confidence: float = 0.95) -> list[float]:
        """Expected Shortfall for each ticker."""

    def tracking_error(self) -> list[float]:
        """Tracking error for each ticker vs benchmark."""

    def information_ratio(self) -> list[float]:
        """Information ratio for each ticker vs benchmark."""

    def skewness(self) -> list[float]:
        """Skewness for each ticker."""

    def kurtosis(self) -> list[float]:
        """Kurtosis for each ticker."""

    def geometric_mean(self) -> list[float]:
        """Geometric mean for each ticker."""

    def downside_deviation(self, mar: float = 0.0) -> list[float]:
        """Downside deviation for each ticker."""

    def max_drawdown_duration(self) -> list[int]:
        """Max drawdown duration (calendar days) for each ticker."""

    def up_capture(self) -> list[float]:
        """Up-capture ratio for each ticker vs benchmark."""

    def down_capture(self) -> list[float]:
        """Down-capture ratio for each ticker vs benchmark."""

    def capture_ratio(self) -> list[float]:
        """Capture ratio for each ticker vs benchmark."""

    def omega_ratio(self, threshold: float = 0.0) -> list[float]:
        """Omega ratio for each ticker."""

    def treynor(self, risk_free_rate: float = 0.0) -> list[float]:
        """Treynor ratio for each ticker."""

    def gain_to_pain(self) -> list[float]:
        """Gain-to-pain ratio for each ticker."""

    def ulcer_index(self) -> list[float]:
        """Ulcer index for each ticker."""

    def martin_ratio(self) -> list[float]:
        """Martin ratio for each ticker.

        Raises:
            ValueError: If the active date window cannot be annualized.
        """

    def recovery_factor(self) -> list[float]:
        """Recovery factor for each ticker."""

    def pain_index(self) -> list[float]:
        """Pain index for each ticker."""

    def pain_ratio(self, risk_free_rate: float = 0.0) -> list[float]:
        """Pain ratio for each ticker.

        Raises:
            ValueError: If the active date window cannot be annualized.
        """

    def tail_ratio(self, confidence: float = 0.95) -> list[float]:
        """Tail ratio for each ticker."""

    def r_squared(self) -> list[float]:
        """R-squared for each ticker vs benchmark."""

    def batting_average(self) -> list[float]:
        """Batting average for each ticker vs benchmark."""

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
        """Cumulative returns for each ticker."""

    def drawdown_series(self) -> list[list[float]]:
        """Drawdown series for each ticker."""

    def correlation_matrix(self) -> list[list[float]]:
        """Correlation matrix across all tickers."""

    def cumulative_returns_outperformance(self) -> list[list[float]]:
        """Cumulative returns outperformance vs benchmark."""

    def drawdown_difference(self) -> list[list[float]]:
        """Drawdown difference vs benchmark."""

    def excess_returns(
        self,
        rf: list[float],
        nperiods: float | None = None,
    ) -> list[list[float]]:
        """Excess returns over a risk-free series (per ticker)."""

    # -- Per-ticker structured methods --

    def beta(self) -> list[BetaResult]:
        """Beta for each ticker vs benchmark."""

    def greeks(self) -> list[GreeksResult]:
        """Greeks (alpha, beta, R²) for each ticker vs benchmark."""

    def rolling_greeks(self, ticker_idx: int, window: int = 63) -> RollingGreeks:
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
    ) -> LookbackReturns:
        """Period-to-date lookback returns.

        Uses ``1`` (January) as the default fiscal-year start month.

        Raises:
            ValueError: If *fiscal_year_start_month* is not in ``1..=12``.
        """

    def period_stats(
        self,
        ticker_idx: int,
        agg_freq: str = "monthly",
        fiscal_year_start_month: int | None = None,
    ) -> PeriodStats:
        """Period statistics for one ticker at a given aggregation frequency.

        Raises:
            ValueError: If *fiscal_year_start_month* is not in ``1..=12``.
        """

    # -- DataFrame export methods --

    def summary_to_dataframe(
        self,
        risk_free_rate: float = 0.0,
        confidence: float = 0.95,
    ) -> pd.DataFrame:
        """Summary statistics for all tickers as a pandas DataFrame."""
        ...

    def cumulative_returns_to_dataframe(self) -> pd.DataFrame:
        """Cumulative returns for all tickers as a pandas DataFrame."""
        ...

    def drawdown_series_to_dataframe(self) -> pd.DataFrame:
        """Drawdown series for all tickers as a pandas DataFrame."""
        ...

    def correlation_to_dataframe(self) -> pd.DataFrame:
        """Correlation matrix as a pandas DataFrame indexed by ticker name."""
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
    ) -> pd.DataFrame:
        """Period-to-date lookback returns as a pandas DataFrame.

        Indexed by ticker name with columns ``mtd``, ``qtd``, ``ytd``,
        and ``fytd``.

        Raises:
            ValueError: If *fiscal_year_start_month* is not in ``1..=12``.
        """
        ...
