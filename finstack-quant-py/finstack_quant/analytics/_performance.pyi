"""Stub declarations mirroring ``finstack-quant-py/src/bindings/analytics/performance.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Sequence
from finstack_quant.core.dates import FiscalConfig
import numpy as np
import numpy.typing as npt
import pandas as pd
from finstack_quant.core.dates import DayCount, DayCountContext
from finstack_quant.analytics._types import (
    BetaResult,
    CorrelationResult,
    DatedSeries,
    DrawdownEpisode,
    GreeksResult,
    LookbackReturns,
    MultiFactorResult,
    PeriodStats,
    RollingGreeks,
)

# Performance panel

class PerformancePanel:
    """
    Dated panel of price or return series, one per ticker: the Rust
    ``PerformancePanel`` a :class:`Performance` is built from.

    Immutable, compares by value, picklable. ``Performance(df)`` and
    ``Performance.from_returns(df)`` build one from a pandas frame; construct
    it directly for raw lists and pass it to :meth:`Performance.from_panel`.

    Parameters
    ----------
    dates : Sequence[object]
        Strictly ascending actual valuation dates (``datetime.date``,
        ``pd.Timestamp`` or ISO strings). L dates require L prices or L - 1
        interval returns per ticker; each return covers consecutive dates.
    series : list[list[float]]
        Ticker-major prices or interval simple returns in the kind-specific shape.
        Leading or trailing ``nan`` gaps are allowed; each ticker's observed
        span must be contiguous.
    kind : str
        ``"prices"`` (positive levels, permitting a final terminal zero; derived pairwise)
        or ``"returns"`` (simple decimal returns, ``0.01`` = +1%).
    ticker_names : list[str]
        Unique ticker names, one per series.
    benchmark_ticker : str, optional
        Explicit comparison ticker; ``None`` leaves benchmark context absent.
    frequency : str, optional
        ``"daily"``, ``"weekly"``, ``"monthly"``, ``"quarterly"``,
        ``"semi_annual"`` or ``"annual"``; ``None`` leaves frequency context absent.

    Raises
    ------
    ValueError
        If ``kind`` is not ``"prices"`` or ``"returns"``.
    AnalyticsError
        If ``frequency`` is not a recognized frequency.

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.analytics import PerformancePanel
    >>> panel = PerformancePanel(
    ...     dates=[date(2024, 1, 1), date(2024, 1, 2)], series=[[0.01]], kind="returns", ticker_names=["FUND"]
    ... )
    >>> panel.kind, panel.frequency
    ('returns', None)
    """

    def __init__(
        self,
        *,
        dates: Sequence[object],
        series: list[list[float]],
        kind: str,
        ticker_names: list[str],
        benchmark_ticker: str | None = None,
        frequency: str | None = None,
    ) -> None: ...
    @property
    def dates(self) -> list[datetime.date]:
        """
        Observation dates, one per value of every series.

        Returns
        -------
        list[datetime.date]
            Strictly ascending dates.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def series(self) -> list[list[float]]:
        """
        Ticker-major series, ``nan`` for a gap.

        Returns
        -------
        list[list[float]]
            ``series[i][t]`` is ticker ``i`` at ``dates[t]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Whether the series are prices or simple returns.

        Returns
        -------
        str
            ``"prices"`` or ``"returns"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ticker_names(self) -> list[str]:
        """
        Ticker names, one per series.

        Returns
        -------
        list[str]
            Unique names in series order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def benchmark_ticker(self) -> str | None:
        """
        Benchmark ticker.

        Returns
        -------
        str | None
            The benchmark name, or ``None`` for the first ticker.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def frequency(self) -> str | None:
        """
        Observation frequency used for annualization.

        Returns
        -------
        str | None
            The declared frequency label, or ``None`` when no frequency was supplied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON with the canonical field names.

        Returns
        -------
        str
            JSON object with ``dates``, ``series`` (gaps as ``null``),
            ``kind``, ``ticker_names`` and any ``benchmark_ticker`` and
            ``frequency``. Does not raise.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PerformancePanel:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON object with ``dates``, ``series``, ``kind``, ``ticker_names``
            and optional ``benchmark_ticker`` and ``frequency``.

        Returns
        -------
        PerformancePanel
            Reconstructed panel.

        Raises
        ------
        ValueError
            If a field is missing, unknown or malformed.

        Examples
        --------
        >>> from datetime import date
        >>> from finstack_quant.analytics import PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=[date(2024, 1, 1), date(2024, 1, 2)], series=[[100.0, 101.0]], kind="prices", ticker_names=["A"]
        ... )
        >>> PerformancePanel.from_json(panel.to_json()) == panel
        True
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct this value for ``pickle`` through its canonical JSON form.

        Returns
        -------
        tuple[Any, tuple[str]]
            The bound ``from_json`` reconstruction callable and its JSON payload.

        Raises
        ------
        ValueError
            If serialization fails or the JSON payload cannot reconstruct the
            stored value, including non-finite floats unsupported by its schema.
        """
        ...

# Performance engine

class Performance:
    """
    Stateful performance analytics engine over a panel of ticker series.

    Construct from a pandas DataFrame or Series of prices
    (``Performance(df)``), a DataFrame or Series of returns
    (``Performance.from_returns(df, initial_date=...)``), or a :class:`PerformancePanel` of raw
    lists (``Performance.from_panel(panel)``). The pandas constructors read
    the row index as the dates and the column headings as the ticker names.

    Scalar-per-ticker metrics return a ``pandas.Series`` indexed by ticker name
    and named after the metric, so results are selected by label rather than by
    column position, and ``pd.concat([perf.sharpe(0.0), perf.sortino(0.0)], axis=1)``
    yields correctly-named columns.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.analytics import Performance, PerformancePanel
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
        Build a validated analytics engine from actual dated price observations.

        Parameters
        ----------
        prices : pd.DataFrame or pd.Series
            Strictly increasing date-indexed price or prepared wealth-index levels, with unique nonblank string ticker labels; only contiguous observed spans with edge NaNs are admitted.
        benchmark_ticker : str, optional
            Explicit loaded comparison ticker; None leaves benchmark context absent.
        frequency : str, optional
            Declared observation frequency used for annualized risk and cash conversion; None leaves frequency absent.

        Raises
        ------
        AnalyticsError
            If chronology, shapes, labels, economic values or supplied frequency are invalid.
        KeyError
            If an explicit benchmark label is unknown.
        TypeError
            If prices is not a supported numeric pandas object.
        """
        ...

    @staticmethod
    def from_returns(
        returns: pd.DataFrame | pd.Series,
        initial_date: datetime.date | str,
        benchmark_ticker: str | None = None,
        frequency: str | None = None,
    ) -> Performance:
        """
        Build from simple investment returns and an explicit actual opening valuation date.

        Parameters
        ----------
        returns : pd.DataFrame or pd.Series
            Return-ending date index with decimal simple-return columns; values are at least -1 and a terminal -1 must be the last observed return.
        initial_date : datetime.date or str
            Actual valuation date before the first return endpoint; anchors the first declared holding interval.
        benchmark_ticker : str, optional
            Explicit comparison label; None leaves benchmark context absent.
        frequency : str, optional
            Declared observation frequency; None leaves annualization context absent.

        Returns
        -------
        Performance
            Validated engine with the supplied opening date prepended to the endpoint grid.

        Raises
        ------
        AnalyticsError
            If dates, interval data, values, labels, frequency or opening date are invalid.
        KeyError
            If an explicit benchmark label is unknown.
        TypeError
            If returns or initial_date has an unsupported host type.

        Examples
        --------
        >>> import pandas as pd
        >>> from finstack_quant.analytics import Performance
        >>> frame = pd.Series([0.01, -0.02], index=pd.to_datetime(["2024-01-02", "2024-01-03"]), name="FUND")
        >>> Performance.from_returns(frame, initial_date="2024-01-01").get_ticker_names()
        ['FUND']
        """
        ...

    @staticmethod
    def from_panel(panel: PerformancePanel) -> Performance:
        """
        Build from the canonical actual valuation-grid panel.

        Parameters
        ----------
        panel : PerformancePanel
            A validated panel with L actual dates and L prices or L - 1 interval returns per ticker.

        Returns
        -------
        Performance
            Engine preserving ticker identity, optional context and the complete supplied interval grid.

        Raises
        ------
        AnalyticsError
            If the panel violates chronology, shape, economic-domain, contiguous-span or identity rules.
        KeyError
            If an explicit benchmark is unknown.
        RuntimeError
            If adjacent price changes cannot be represented faithfully as finite simple returns.

        Examples
        --------
        >>> from finstack_quant.analytics import Performance, PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=["2024-01-01", "2024-01-02"], series=[[0.01]], kind="returns", ticker_names=["FUND"]
        ... )
        >>> Performance.from_panel(panel).get_ticker_names()
        ['FUND']
        """
        ...

    # -- Mutators --

    def reset_date_range(self, start: datetime.date | str, end: datetime.date | str) -> None:
        """
        Atomically select inclusive return-ending bounds on the original grid.

        Parameters
        ----------
        start : datetime.date or str
            Earliest included return endpoint; selected returns retain their entire supplied holding interval.
        end : datetime.date or str
            Latest included endpoint, at or after start; off-grid bounds and empty valid selections are allowed.

        Raises
        ------
        AnalyticsError
            If start is after end; previous state and caches remain unchanged.
        ValueError
            If a date string cannot be parsed.
        """
        ...

    def reset_bench_ticker(self, ticker: str) -> None:
        """
        Atomically select an explicit loaded benchmark.

        Parameters
        ----------
        ticker : str
            Exact case-sensitive ticker identity; self-benchmarking is permitted.

        Raises
        ------
        KeyError
            If ticker is not loaded; the previous benchmark remains unchanged.
        """
        ...

    # -- Getters --

    def get_ticker_names(self) -> list[str]:
        """
        Return ticker identities in their original column order.

        Returns
        -------
        list[str]
            Unique nonblank labels preserved exactly.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    def get_ticker_index(self, ticker: str) -> int:
        """
        Resolve an exact ticker identity to its column index.

        Parameters
        ----------
        ticker : str
            Loaded case-sensitive ticker label, without trimming or case conversion.

        Returns
        -------
        int
            Zero-based original column index.

        Raises
        ------
        KeyError
            If ticker is not loaded.
        """
        ...

    def get_benchmark_idx(self) -> int | None:
        """
        Inspect the explicitly selected benchmark.

        Returns
        -------
        int or None
            Original column index, or None when no benchmark has been selected.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    def get_frequency(self) -> str | None:
        """
        Inspect the declared observation frequency.

        Returns
        -------
        str or None
            Canonical frequency label, or None when no annualization context was supplied.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    def get_dates(self) -> list[datetime.date]:
        """
        Return the full return-ending grid.

        Returns
        -------
        list[datetime.date]
            All canonical valuation dates except the initial opening anchor; no active-window filtering.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    def get_active_dates(self) -> list[datetime.date]:
        """
        Return endpoints selected by the active inclusive window.

        Returns
        -------
        list[datetime.date]
            Global selected return endpoints; individual tickers may have shorter contiguous observed spans.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    def get_active_dates_for_ticker(self, ticker_idx: int | str) -> list[datetime.date]:
        """
        Return endpoints for one ticker within its active observed span.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index or an exact, case-sensitive loaded ticker name.

        Returns
        -------
        list[datetime.date]
            Dates aligned exactly with that ticker's compact active return values.

        Raises
        ------
        AnalyticsError
            If an integer ticker index is outside the loaded columns.
        KeyError
            If a string ticker name is not loaded.
        """
        ...

    def get_paired_dates(self, ticker_idx: int | str) -> list[datetime.date]:
        """
        Return exact active endpoint dates shared by a ticker and its benchmark.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based original ticker index or exact loaded ticker label.

        Returns
        -------
        list[datetime.date]
            Dates aligned with paired benchmark and relative-wealth outputs.

        Raises
        ------
        AnalyticsError
            If ticker_idx is out of range or no benchmark is selected.
        KeyError
            If a string ticker identity is unknown.
        """
        ...

    # -- Scalar-per-ticker methods --

    def cagr(self, day_count: str | DayCount | None = None, ctx: DayCountContext | None = None) -> pd.Series:
        """
        Calculate geometric annual growth using actual holding dates.

        Parameters
        ----------
        day_count : str or DayCount, optional
            Dated annualization convention; None selects Actual/365 Fixed. Explicit act365_25 retains days/365.25 when deliberately requested.
        ctx : DayCountContext, optional
            Complete core day-count context, including calendar or coupon information required by the chosen convention; None supplies empty context.

        Returns
        -------
        pd.Series
            One result per ticker in original order, indexed by ticker name. Undefined estimates are NaN; valid nonzero reward at exactly zero risk can yield signed infinity.

        Raises
        ------
        AnalyticsError
            If the day-count request or required context is invalid.
        KeyError
            If a referenced calendar is unknown.
        RuntimeError
            If a requested growth estimate cannot be represented.
        """
        ...

    def mean_return(self, annualize: bool) -> pd.Series:
        """
        Calculate the equal-observation arithmetic mean.

        Parameters
        ----------
        annualize : bool
            Explicitly select annualized arithmetic units when True or native observation-period units when False; True requires stored frequency.

        Returns
        -------
        pd.Series
            Arithmetic mean decimal returns in the explicitly selected units; empty ticker histories give NaN.

        Raises
        ------
        AnalyticsError
            If annualize is True and no explicit observation frequency is stored.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def volatility(self, annualize: bool) -> pd.Series:
        """
        Calculate sample standard deviation with an n - 1 denominator.

        Parameters
        ----------
        annualize : bool
            Explicitly select annualized arithmetic units when True or native observation-period units when False; True requires stored frequency.

        Returns
        -------
        pd.Series
            Sample standard deviation in explicitly selected units; fewer than two observations gives NaN and measured constant history gives zero.

        Raises
        ------
        AnalyticsError
            If annualize is True and no explicit observation frequency is stored.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def sharpe(self, risk_free_rate: float) -> pd.Series:
        """
        Calculate frequency-scaled arithmetic excess mean divided by sample volatility.

        Parameters
        ----------
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.

        Returns
        -------
        pd.Series
            Dimensionless annualized Sharpe; fewer than two observations gives NaN, finite nonzero excess over exact zero volatility gives signed infinity, and 0/0 gives NaN.

        Raises
        ------
        AnalyticsError
            If cash is not finite and greater than -1, or no explicit observation frequency is stored.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def sortino(self, mar: float) -> pd.Series:
        """
        Calculate frequency-scaled mean excess over all-observation downside deviation.

        Parameters
        ----------
        mar : float
            Explicit finite minimum acceptable decimal return per observation; this is not an annual rate.

        Returns
        -------
        pd.Series
            Dimensionless annualized Sortino; two observations are required, exact zero downside with nonzero excess gives signed infinity, and 0/0 gives NaN.

        Raises
        ------
        AnalyticsError
            If MAR is nonfinite or no explicit observation frequency is stored.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def calmar(self) -> pd.Series:
        """
        Divide Actual/365 Fixed dated CAGR by absolute maximum drawdown over the selected window.

        Returns
        -------
        pd.Series
            Dated CAGR divided by absolute maximum drawdown; empty history gives NaN, valid nonzero reward at zero drawdown gives signed infinity and 0/0 gives NaN.

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def max_drawdown(self) -> pd.Series:
        """
        Return the most negative compounded percentage drawdown.

        Returns
        -------
        pd.Series
            Most negative endpoint drawdown in [-1, 0]; empty history gives NaN, observed no-loss history gives zero, and a terminal wipeout gives -1.

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def mean_drawdown(self) -> pd.Series:
        """
        Average signed compounded endpoint drawdowns with equal observation weight.

        Returns
        -------
        pd.Series
            Equal-weight mean signed endpoint drawdown in [-1, 0]; empty history gives NaN and observed no-loss history gives zero.

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def value_at_risk(self, confidence: float) -> pd.Series:
        """
        Calculate signed observation-period historical VaR using R-7 interpolation.

        Parameters
        ----------
        confidence : float
            Explicit finite decimal tail confidence strictly between 0.5 and 1; the lower-tail probability is 1 - confidence.

        Returns
        -------
        pd.Series
            Signed decimal return quantile at lower-tail probability 1 - confidence, without flooring or absolute value; empty history gives NaN.

        Raises
        ------
        AnalyticsError
            If confidence is nonfinite or is not strictly between 0.5 and 1.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def expected_shortfall(self, confidence: float) -> pd.Series:
        """
        Average the exact fractional empirical lower-tail probability mass.

        Parameters
        ----------
        confidence : float
            Explicit finite decimal tail confidence strictly between 0.5 and 1; the lower-tail probability is 1 - confidence.

        Returns
        -------
        pd.Series
            Signed decimal mean return over the exact fractional empirical lower tail; empty history gives NaN and tails smaller than one observation return the worst return.

        Raises
        ------
        AnalyticsError
            If confidence is nonfinite or is not strictly between 0.5 and 1.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def tracking_error(self) -> pd.Series:
        """
        Calculate sample standard deviation of paired arithmetic active returns, annualized by declared frequency.

        Returns
        -------
        pd.Series
            One result per ticker in original order, indexed by ticker name. Undefined estimates are NaN; valid nonzero reward at exactly zero risk can yield signed infinity.

        Raises
        ------
        AnalyticsError
            If no benchmark has been explicitly selected, or declared frequency is absent.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def information_ratio(self) -> pd.Series:
        """
        Divide annualized paired arithmetic active mean by annualized tracking error.

        Returns
        -------
        pd.Series
            One result per ticker in original order, indexed by ticker name. Undefined estimates are NaN; valid nonzero reward at exactly zero risk can yield signed infinity.

        Raises
        ------
        AnalyticsError
            If no benchmark has been explicitly selected, or declared frequency is absent.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def skewness(self) -> pd.Series:
        """
        Calculate Fisher-adjusted Type 2 skewness.

        Returns
        -------
        pd.Series
            Ticker-indexed dimensionless estimates; fewer than three returns or zero variance gives NaN.

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def kurtosis(self) -> pd.Series:
        """
        Calculate Fisher-adjusted Type 2 excess kurtosis.

        Returns
        -------
        pd.Series
            Ticker-indexed excess-kurtosis estimates; fewer than four returns or zero variance gives NaN.

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def geometric_mean(self) -> pd.Series:
        """
        Calculate geometric mean simple growth per observation.

        Returns
        -------
        pd.Series
            Geometric decimal return per observation, without annualization; empty history gives NaN and a valid terminal loss gives exactly -1.

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def downside_deviation(self, mar: float) -> pd.Series:
        """
        Calculate annualized lower partial deviation using every observation in the denominator.

        Parameters
        ----------
        mar : float
            Explicit finite minimum acceptable decimal return per observation; this is not an annual rate.

        Returns
        -------
        pd.Series
            Nonnegative annualized lower-partial deviation, using all observations in its denominator; empty history gives NaN, while observed zero downside gives zero.

        Raises
        ------
        AnalyticsError
            If MAR is nonfinite or no explicit observation frequency is stored.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def max_drawdown_duration(self) -> pd.Series:
        """
        Measure the longest observed drawdown episode in actual calendar days.

        Returns
        -------
        pd.Series
            Nullable integer calendar-day durations per ticker; zero for observed history without a decline, and missing for empty history.

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def up_capture(self) -> pd.Series:
        """
        Compare geometric annualized returns on strictly positive benchmark observations.

        Returns
        -------
        pd.Series
            Signed ratio of geometrically annualized portfolio and benchmark returns on benchmark-positive pairs; no selected pairs gives NaN. There is no near-zero denominator floor.

        Raises
        ------
        AnalyticsError
            If no benchmark has been explicitly selected, or declared frequency is absent.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def down_capture(self) -> pd.Series:
        """
        Compare geometric annualized returns on strictly negative benchmark observations.

        Returns
        -------
        pd.Series
            Signed ratio of geometrically annualized portfolio and benchmark returns on benchmark-negative pairs; no selected pairs gives NaN. There is no near-zero denominator floor.

        Raises
        ------
        AnalyticsError
            If no benchmark has been explicitly selected, or declared frequency is absent.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def capture_ratio(self) -> pd.Series:
        """
        Divide up capture by down capture using their separately selected paired subsets.

        Returns
        -------
        pd.Series
            Up-capture divided by down-capture; unavailable subsets remain NaN, valid nonzero/zero gives signed infinity and 0/0 gives NaN.

        Raises
        ------
        AnalyticsError
            If no benchmark has been explicitly selected, or declared frequency is absent.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def omega_ratio(self, threshold: float) -> pd.Series:
        """
        Calculate the unannualized empirical gain-to-shortfall ratio at an explicit hurdle.

        Parameters
        ----------
        threshold : float
            Explicit finite decimal hurdle per observation, in the same units as returns.

        Returns
        -------
        pd.Series
            Unannualized gain/shortfall ratio; empty samples and 0/0 give NaN, while observed gains with zero shortfall give positive infinity.

        Raises
        ------
        AnalyticsError
            If the observation-period threshold is nonfinite.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def treynor(self, risk_free_rate: float) -> pd.Series:
        """
        Divide annualized arithmetic excess mean by paired OLS beta.

        Parameters
        ----------
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.

        Returns
        -------
        pd.Series
            Annual arithmetic excess return divided by paired beta; unavailable or exactly zero beta gives NaN, without a near-zero cutoff.

        Raises
        ------
        AnalyticsError
            If the benchmark or declared frequency is absent, or annual-effective cash is nonfinite or not greater than -1.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def gain_to_pain(self) -> pd.Series:
        """
        Divide the arithmetic net return sum by magnitudes of negative observations.

        Returns
        -------
        pd.Series
            Unannualized arithmetic net-return sum divided by loss magnitudes; empty history and 0/0 give NaN, while gains without losses give positive infinity.

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def ulcer_index(self) -> pd.Series:
        """
        Calculate the root mean square of all signed percentage endpoint drawdowns.

        Returns
        -------
        pd.Series
            Nonnegative root mean square percentage endpoint drawdown; empty history gives NaN and observed no-loss history gives zero.

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def martin_ratio(self, risk_free_rate: float) -> pd.Series:
        """
        Divide dated CAGR less annual-effective cash by Ulcer index.

        Parameters
        ----------
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.

        Returns
        -------
        pd.Series
            Dated CAGR minus annual-effective cash, divided by Ulcer index; empty history gives NaN, valid nonzero/zero gives signed infinity and 0/0 gives NaN.

        Raises
        ------
        AnalyticsError
            If the annual-effective cash rate is nonfinite or is not greater than -1.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def recovery_factor(self) -> pd.Series:
        """
        Divide compounded total return by absolute maximum drawdown without annualization.

        Returns
        -------
        pd.Series
            Compounded total return divided by absolute maximum drawdown without annualization; empty history gives NaN, valid nonzero/zero gives signed infinity and 0/0 gives NaN.

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def pain_index(self) -> pd.Series:
        """
        Average absolute compounded drawdown across all observed endpoints.

        Returns
        -------
        pd.Series
            Nonnegative mean absolute endpoint drawdown; empty history gives NaN and observed no-loss history gives zero.

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def pain_ratio(self, risk_free_rate: float) -> pd.Series:
        """
        Divide dated CAGR less annual-effective cash by the observation-weighted Pain index.

        Parameters
        ----------
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.

        Returns
        -------
        pd.Series
            Dated CAGR minus annual-effective cash, divided by Pain index; empty history gives NaN, valid nonzero/zero gives signed infinity and 0/0 gives NaN.

        Raises
        ------
        AnalyticsError
            If the annual-effective cash rate is nonfinite or is not greater than -1.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def tail_ratio(self, confidence: float) -> pd.Series:
        """
        Divide magnitudes of the R-7 upper and lower empirical quantiles without annualization.

        Parameters
        ----------
        confidence : float
            Explicit finite decimal tail confidence strictly between 0.5 and 1; the lower-tail probability is 1 - confidence.

        Returns
        -------
        pd.Series
            Magnitude ratio |Q(confidence)| / |Q(1 - confidence)|; empty samples and 0/0 give NaN, while a positive numerator over exact zero gives positive infinity.

        Raises
        ------
        AnalyticsError
            If confidence is nonfinite or is not strictly between 0.5 and 1.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def r_squared(self) -> pd.Series:
        """
        Calculate paired equal-weight intercept OLS squared correlation.

        Returns
        -------
        pd.Series
            Paired sample squared Pearson correlation, in [0, 1]; fewer than two pairs or a constant portfolio or benchmark gives NaN.

        Raises
        ------
        AnalyticsError
            If no benchmark has been explicitly selected.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def batting_average(self) -> pd.Series:
        """
        Count strict portfolio outperformance over the exact paired sample.

        Returns
        -------
        pd.Series
            Strict wins divided by all paired observations, including ties; no paired observations gives NaN.

        Raises
        ------
        AnalyticsError
            If no benchmark has been explicitly selected.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def parametric_var(self, confidence: float, horizon_periods: int | None = None) -> pd.Series:
        """
        Calculate a signed Gaussian quantile under additive independent observation-period scaling.

        Parameters
        ----------
        confidence : float
            Explicit finite decimal tail confidence strictly between 0.5 and 1; the lower-tail probability is 1 - confidence.
        horizon_periods : int, optional
            Whole observation count from 1 through 2**53 - 1; None means one observation. Historical methods do not use this horizon.

        Returns
        -------
        pd.Series
            Signed additive-model decimal quantile, without an investment-return lower bound; fewer than two returns gives NaN and constant histories yield their point-mass quantile.

        Raises
        ------
        AnalyticsError
            If confidence is outside the open interval (0.5, 1) or horizon_periods is outside the supported positive exact-integer range.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def cornish_fisher_var(self, confidence: float, horizon_periods: int | None = None) -> pd.Series:
        """
        Calculate a classical plug-in Cornish-Fisher quantile after a global quantile-ordering check.

        Parameters
        ----------
        confidence : float
            Explicit finite decimal tail confidence strictly between 0.5 and 1; the lower-tail probability is 1 - confidence.
        horizon_periods : int, optional
            Whole additive independent observation count from 1 through 2**53 - 1; None means one observation.

        Returns
        -------
        pd.Series
            Signed additive-model decimal quantile; fewer than four returns or zero variance gives NaN. An estimable globally unordered fit raises instead of substituting an estimate.

        Raises
        ------
        AnalyticsError
            If confidence or horizon is invalid, or the fitted CF polynomial is not globally ordered.
        RuntimeError
            If a requested estimate cannot be represented; no Gaussian fallback is used.
        """
        ...

    def cdar(self, confidence: float) -> pd.Series:
        """
        Average the fractional empirical lower tail of compounded percentage endpoint drawdowns.

        Parameters
        ----------
        confidence : float
            Explicit finite decimal tail confidence strictly between 0.5 and 1; the lower-tail probability is 1 - confidence.

        Returns
        -------
        pd.Series
            Nonpositive percentage-drawdown tail mean; empty history gives NaN and a valid observed no-drawdown path gives zero.

        Raises
        ------
        AnalyticsError
            If confidence is nonfinite or is not strictly between 0.5 and 1.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def m_squared(self, risk_free_rate: float) -> pd.Series:
        """
        Calculate absolute cash-adjusted arithmetic M-squared return at benchmark volatility.

        Parameters
        ----------
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.

        Returns
        -------
        pd.Series
            Absolute annual arithmetic cash-adjusted return, not active performance; fewer than two pairs or exactly zero portfolio volatility gives NaN.

        Raises
        ------
        AnalyticsError
            If the benchmark or declared frequency is absent, or annual-effective cash is nonfinite or not greater than -1.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def modified_sharpe(self, risk_free_rate: float, confidence: float) -> pd.Series:
        """
        Divide observation-period arithmetic cash excess by the negative one-period CF return quantile.

        Parameters
        ----------
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.
        confidence : float
            Explicit finite decimal tail confidence strictly between 0.5 and 1; the lower-tail probability is 1 - confidence.

        Returns
        -------
        pd.Series
            Observation-period cash-excess mean divided by negative one-period CF return quantile, without annualization. An unavailable fit or nonpositive loss denominator gives NaN.

        Raises
        ------
        AnalyticsError
            If confidence, cash or required frequency is invalid, or the CF fit fails its global ordering check.
        RuntimeError
            If numerical calculation fails; no estimator substitution occurs.
        """
        ...

    def sterling_ratio(self, risk_free_rate: float, n: int) -> pd.Series:
        """
        Divide dated CAGR less cash by the mean magnitude of the deepest observed episodes.

        Parameters
        ----------
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.
        n : int
            Explicit positive number of deepest episodes; if fewer exist, all available episodes are used.

        Returns
        -------
        pd.Series
            Dated CAGR less cash divided by mean selected episode magnitude; empty history gives NaN, valid nonzero reward over exact zero risk gives signed infinity and 0/0 gives NaN.

        Raises
        ------
        AnalyticsError
            If the annual-effective cash rate is nonfinite or not greater than -1, or n is not positive.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def burke_ratio(self, risk_free_rate: float) -> pd.Series:
        """
        Calculate modified Burke from every episode, normalized by return observation count.

        Parameters
        ----------
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.

        Returns
        -------
        pd.Series
            Dated CAGR less cash divided by sqrt(sum(all episode depths squared) / N), with N equal to return-observation count. Empty history and 0/0 give NaN; nonzero reward at zero risk gives signed infinity.

        Raises
        ------
        AnalyticsError
            If the annual-effective cash rate is nonfinite or is not greater than -1.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    # -- Vector-per-ticker methods --

    def get_returns(self) -> list[list[float]]:
        """
        Return compact active simple-return histories in ticker order.

        Returns
        -------
        list[list[float]]
            One ragged observed history per ticker, aligned to get_active_dates_for_ticker.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    def get_returns_for_ticker(self, ticker_idx: int | str) -> list[float]:
        """
        Return one compact active observed return history.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index or an exact, case-sensitive loaded ticker name.

        Returns
        -------
        list[float]
            Decimal investment returns aligned to that ticker's active endpoint dates; empty selections produce [].

        Raises
        ------
        AnalyticsError
            If an integer ticker index is outside the loaded columns.
        KeyError
            If a string ticker name is not loaded.
        """
        ...

    def cumulative_returns(self) -> list[list[float]]:
        """
        Compound each active ticker from unit wealth at its actual opening valuation.

        Returns
        -------
        list[list[float]]
            One cumulative decimal return per observed endpoint, without an extra opening baseline; empty histories produce [].

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def get_drawdown_series(self) -> list[list[float]]:
        """
        Return rebased compounded drawdowns for each active ticker.

        Returns
        -------
        list[list[float]]
            Signed percentage drawdowns in [-1, 0], one per observed endpoint; empty histories produce [].

        Raises
        ------
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def correlation_matrix(self) -> CorrelationResult:
        """
        Calculate Pearson correlation on the single all-ticker common active sample.

        Returns
        -------
        CorrelationResult
            Ticker order, finite matrix, common count and observed start/end endpoints; singular positive-semidefinite matrices are accepted.

        Raises
        ------
        AnalyticsError
            If fewer than two common observations exist or a ticker is constant on the common sample.
        RuntimeError
            If stable normalization or final numerical matrix checks fail; no pairwise fallback or matrix repair is applied.
        """
        ...

    def cumulative_returns_outperformance(self) -> list[list[float]]:
        """
        Compound relative wealth against the selected benchmark on each exact paired span.

        Returns
        -------
        list[list[float]]
            Ragged ticker-major relative-wealth returns aligned to get_paired_dates; no extra opening baseline.

        Raises
        ------
        AnalyticsError
            If no benchmark has been selected or paired benchmark wealth reaches a terminal zero.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def drawdown_difference(self) -> list[list[float]]:
        """
        Subtract independently rebased benchmark drawdowns on exact paired spans.

        Returns
        -------
        list[list[float]]
            Signed ticker-minus-benchmark endpoint drawdowns on paired dates.

        Raises
        ------
        AnalyticsError
            If no benchmark has been explicitly selected.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def excess_returns(self, rf: Sequence[float] | npt.NDArray[np.float64] | pd.Series) -> list[list[float]]:
        """
        Subtract annual-effective cash converted geometrically at the declared frequency.

        Parameters
        ----------
        rf : sequence of float, numpy.ndarray or pandas.Series
            One finite annual-effective decimal cash rate greater than -1 for every global active return-ending position, including positions unused by a ticker; positional alignment is required.

        Returns
        -------
        list[list[float]]
            Unannualized arithmetic excess observations in each ticker's active span; these may be below -1 and are not investment wealth.

        Raises
        ------
        AnalyticsError
            If frequency is absent, cash length differs from the global active endpoint grid, or any supplied annual-effective rate is nonfinite or not greater than -1.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    # -- Per-ticker structured methods --

    def beta(self) -> list[BetaResult]:
        """
        Fit paired equal-weight intercept OLS beta with fixed two-sided 95% Student-t uncertainty.

        Returns
        -------
        list[BetaResult]
            One result per ticker; beta needs two pairs with variable benchmark, while uncertainty requires positive residual degrees of freedom.

        Raises
        ------
        AnalyticsError
            If no benchmark has been explicitly selected.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def greeks(self, risk_free_rate: float) -> list[GreeksResult]:
        """
        Fit paired OLS beta, annualized Jensen alpha and field-specific fit statistics.

        Parameters
        ----------
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.

        Returns
        -------
        list[GreeksResult]
            One result per ticker with actual paired counts and unavailable fields preserved as NaN.

        Raises
        ------
        AnalyticsError
            If the benchmark or declared frequency is absent, or annual-effective cash is nonfinite or not greater than -1.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def rolling_greeks(self, ticker_idx: int | str, window: int, risk_free_rate: float) -> RollingGreeks:
        """
        Fit paired Jensen alpha and OLS beta on complete right-labelled windows.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index or an exact, case-sensitive loaded ticker name.
        window : int
            Explicit positive number of consecutive return observations; outputs begin at the first complete window.
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.

        Returns
        -------
        RollingGreeks
            Compact window-end dates and paired estimates; undefined windows preserve native NaN values.

        Raises
        ------
        AnalyticsError
            If an integer ticker index is outside the loaded columns.
        KeyError
            If a string ticker name is not loaded.
        AnalyticsError
            If the ticker, explicit positive window or metric-specific cash/MAR is invalid, or required frequency or benchmark context is absent.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def rolling_volatility(self, ticker_idx: int | str, window: int) -> DatedSeries:
        """
        Calculate sample volatility on complete right-labelled observation windows.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index or an exact, case-sensitive loaded ticker name.
        window : int
            Explicit positive number of consecutive return observations; outputs begin at the first complete window.

        Returns
        -------
        DatedSeries
            One annualized volatility estimate per complete window, dated by its last observation.

        Raises
        ------
        AnalyticsError
            If an integer ticker index is outside the loaded columns.
        KeyError
            If a string ticker name is not loaded.
        AnalyticsError
            If the ticker, explicit positive window or metric-specific cash/MAR is invalid, or required frequency or benchmark context is absent.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def rolling_sortino(self, ticker_idx: int | str, window: int, mar: float) -> DatedSeries:
        """
        Calculate Sortino on complete right-labelled windows with an explicit observation hurdle.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index or an exact, case-sensitive loaded ticker name.
        window : int
            Explicit positive number of consecutive return observations; outputs begin at the first complete window.
        mar : float
            Explicit finite minimum acceptable decimal return per observation; this is not an annual rate.

        Returns
        -------
        DatedSeries
            Window-local annualized Sortino estimates, preserving scalar estimator availability.

        Raises
        ------
        AnalyticsError
            If an integer ticker index is outside the loaded columns.
        KeyError
            If a string ticker name is not loaded.
        AnalyticsError
            If the ticker, explicit positive window or metric-specific cash/MAR is invalid, or required frequency or benchmark context is absent.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def rolling_sharpe(self, ticker_idx: int | str, window: int, risk_free_rate: float) -> DatedSeries:
        """
        Calculate Sharpe on complete right-labelled windows using annual-effective cash.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index or an exact, case-sensitive loaded ticker name.
        window : int
            Explicit positive number of consecutive return observations; outputs begin at the first complete window.
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.

        Returns
        -------
        DatedSeries
            Window-local annualized Sharpe estimates, preserving scalar estimator availability.

        Raises
        ------
        AnalyticsError
            If an integer ticker index is outside the loaded columns.
        KeyError
            If a string ticker name is not loaded.
        AnalyticsError
            If the ticker, explicit positive window or metric-specific cash/MAR is invalid, or required frequency or benchmark context is absent.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def rolling_returns(self, ticker_idx: int | str, window: int) -> DatedSeries:
        """
        Compound returns within complete right-labelled observation windows.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index or an exact, case-sensitive loaded ticker name.
        window : int
            Explicit positive number of consecutive return observations; outputs begin at the first complete window.

        Returns
        -------
        DatedSeries
            Native-period cumulative window returns, without annualization.

        Raises
        ------
        AnalyticsError
            If an integer ticker index is outside the loaded columns.
        KeyError
            If a string ticker name is not loaded.
        AnalyticsError
            If the ticker is out of range or window is not positive.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def drawdown_details(self, ticker_idx: int | str, n: int) -> list[DrawdownEpisode]:
        """
        Select the deepest strictly recovered or open observed drawdown episodes.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index or an exact, case-sensitive loaded ticker name.
        n : int
            Explicit nonnegative display count; zero returns [] after validating the ticker, and larger values return all available episodes.

        Returns
        -------
        list[DrawdownEpisode]
            Deepest-first episodes, with chronological tie-breaking and actual calendar-day durations; open episodes have no recovery date.

        Raises
        ------
        AnalyticsError
            If an integer ticker index is outside the loaded columns.
        KeyError
            If a string ticker name is not loaded.
        AnalyticsError
            If the ticker is out of range or n cannot be represented as a nonnegative display count.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def multi_factor_greeks(
        self, ticker_idx: int | str, factor_returns: list[list[float]], risk_free_rate: float
    ) -> MultiFactorResult:
        """
        Fit the active total-investment ticker after response-only cash subtraction.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index or an exact, case-sensitive loaded ticker name.
        factor_returns : list[list[float]]
            Finite factor-major predictors already aligned exactly to this ticker's active interval sample, in caller-declared units; factors are not cash-adjusted.
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.

        Returns
        -------
        MultiFactorResult
            Annual arithmetic excess intercept, factor loadings and field-specific OLS fit/residual statistics.

        Raises
        ------
        AnalyticsError
            If an integer ticker index is outside the loaded columns.
        KeyError
            If a string ticker name is not loaded.
        AnalyticsError
            If ticker or factor shapes are invalid, frequency is absent, cash is invalid, or the model lacks enough observations or full factor rank.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def lookback_returns(
        self,
        ref_date: object | None = None,
        fiscal_config: FiscalConfig | None = None,
    ) -> LookbackReturns:
        """
        Compound occupied observations within calendar and selected fiscal year-to-date windows.

        Parameters
        ----------
        ref_date : object, optional
            Inclusive reference endpoint; None uses the last globally active endpoint and errors when none exists.
        fiscal_config : FiscalConfig, optional
            Explicit fiscal opening rules for FYTD; None deliberately resolves to calendar-year rules, retained in the result metadata.

        Returns
        -------
        LookbackReturns
            Ticker-aligned MTD/QTD/YTD/FYTD returns and compact sample counts/actual holding bounds; empty samples remain undefined.

        Raises
        ------
        AnalyticsError
            If omitted ref_date cannot resolve from an empty active grid, or a required calendar/fiscal boundary cannot be represented.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def period_stats(
        self, ticker_idx: int | str, aggregation_frequency: str, fiscal_config: FiscalConfig | None = None
    ) -> PeriodStats:
        """
        Group occupied calendar or explicit fiscal buckets and describe observed win/loss statistics.

        Parameters
        ----------
        ticker_idx : int or str
            Zero-based ticker column index or an exact, case-sensitive loaded ticker name.
        aggregation_frequency : str
            Explicit daily, weekly, monthly, quarterly, semi_annual or annual bucket frequency.
        fiscal_config : FiscalConfig, optional
            Explicit fiscal boundaries when supplied; None selects Gregorian periods with ISO weeks.

        Returns
        -------
        PeriodStats
            Bucket counts, strict sign groups, observed-bucket streaks, conditional means, CPC index and loss-normalized Kelly risk fraction.

        Raises
        ------
        AnalyticsError
            If an integer ticker index is outside the loaded columns.
        KeyError
            If a string ticker name is not loaded.
        AnalyticsError
            If the ticker or explicit grouping frequency is invalid, or a required calendar/fiscal boundary cannot be represented.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    def periodic_returns(self, frequency: str) -> list[list[tuple[datetime.date, float]]]:
        """
        Compound the observed returns in each occupied calendar bucket.

        Parameters
        ----------
        frequency : str
            Explicit calendar grouping frequency: daily, weekly, monthly, quarterly, semi_annual or annual; no frequency is inferred.

        Returns
        -------
        list[list[tuple[datetime.date, float]]]
            Ticker-major chronological (last observed endpoint, decimal compounded return) pairs; partial buckets are retained and absent buckets are not synthesized.

        Raises
        ------
        AnalyticsError
            If the explicit grouping frequency is invalid or a calendar boundary cannot be represented.
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
        """
        ...

    # -- DataFrame export --

    def to_dataframe(
        self, risk_free_rate: float, mar: float, omega_threshold: float, confidence: float
    ) -> pd.DataFrame:
        """
        Export the fixed 22-metric summary with explicit inputs and stored sample metadata.

        Parameters
        ----------
        risk_free_rate : float
            Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.
        mar : float
            Explicit finite minimum acceptable decimal return per observation; this is not an annual rate.
        omega_threshold : float
            Explicit finite decimal per-observation Omega hurdle, independent of MAR.
        confidence : float
            Explicit finite decimal tail confidence strictly between 0.5 and 1; the lower-tail probability is 1 - confidence.

        Returns
        -------
        pd.DataFrame
            Ticker-indexed fixed metric columns; annualized arithmetic risk/means, native-period geometric mean and historical VaR/ES. Undefined values and signed infinities retain their meanings.

        Raises
        ------
        AnalyticsError
            If benchmark/frequency context is absent, cash is invalid, MAR/Omega is nonfinite, or confidence is outside the open interval (0.5, 1).
        RuntimeError
            If a required numerical calculation or requested finite result cannot be represented.
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
        >>> from finstack_quant.analytics import Performance, PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=[date(2024, 1, 1), date(2024, 1, 2), date(2024, 1, 3)],
        ...     series=[[0.01, 0.02]],
        ...     kind="returns",
        ...     ticker_names=["FUND"],
        ... )
        >>> perf = Performance.from_panel(panel)
        >>> Performance.from_json(perf.to_json()).get_ticker_names()
        ['FUND']
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

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
    risk_free_rate: float,
    periods_per_year: float,
) -> float:
    """
    Annualized arithmetic cash-excess return divided by sample volatility.

    Parameters
    ----------
    returns : sequence of float, numpy.ndarray or pandas.Series
        Finite decimal simple investment returns in chronological order, each at least -1; an exact terminal -1 may occur only as the last observation.
    risk_free_rate : float
        Explicit annual-effective decimal cash rate, finite and greater than -1; geometric per-observation conversion uses the declared frequency where required.
    periods_per_year : float
        Explicit positive finite observation count per year; the same convention controls cash conversion and risk scaling.

    Returns
    -------
    float
        Native floating estimate; unavailable history gives NaN. Valid nonzero rewards at exactly zero risk yield signed infinity; undefined 0/0 yields NaN.

    Raises
    ------
    AnalyticsError
        If the history domain, required cash/MAR or explicit positive finite annualization factor is invalid.
    RuntimeError
        If a required numerical calculation or requested finite result cannot be represented.
    TypeError
        If returns cannot be read as a numeric sequence.

    Examples
    --------
    >>> from finstack_quant.analytics import sharpe
    >>> sharpe([0.01, -0.02, 0.015, 0.003], 0.0, 252) > 0
    True
    """
    ...

def sortino(
    returns: Sequence[float] | npt.NDArray[np.float64] | pd.Series,
    mar: float,
    periods_per_year: float,
) -> float:
    """
    Annualized arithmetic hurdle-excess return divided by all-observation downside deviation.

    Parameters
    ----------
    returns : sequence of float, numpy.ndarray or pandas.Series
        Finite decimal simple investment returns in chronological order, each at least -1; an exact terminal -1 may occur only as the last observation.
    mar : float
        Explicit finite minimum acceptable decimal return per observation; this is not an annual rate.
    periods_per_year : float
        Explicit positive finite observation count per year for square-root risk scaling.

    Returns
    -------
    float
        Native floating estimate; unavailable history gives NaN. Valid nonzero rewards at exactly zero risk yield signed infinity; undefined 0/0 yields NaN.

    Raises
    ------
    AnalyticsError
        If the history domain, required cash/MAR or explicit positive finite annualization factor is invalid.
    RuntimeError
        If a required numerical calculation or requested finite result cannot be represented.
    TypeError
        If returns cannot be read as a numeric sequence.

    Examples
    --------
    >>> from finstack_quant.analytics import sortino
    >>> sortino([0.01, -0.02, 0.015, 0.003], 0.0, 252) > 0
    True
    """
    ...

def volatility(
    returns: Sequence[float] | npt.NDArray[np.float64] | pd.Series,
    periods_per_year: float,
) -> float:
    """
    Annualized sample standard deviation with n - 1 denominator.

    Parameters
    ----------
    returns : sequence of float, numpy.ndarray or pandas.Series
        Finite decimal simple investment returns in chronological order, each at least -1; an exact terminal -1 may occur only as the last observation.
    periods_per_year : float
        Explicit positive finite observation count per year; scales sample deviation by its square root.

    Returns
    -------
    float
        Native floating estimate; unavailable history gives NaN. Valid nonzero rewards at exactly zero risk yield signed infinity; undefined 0/0 yields NaN.

    Raises
    ------
    AnalyticsError
        If the history domain, required cash/MAR or explicit positive finite annualization factor is invalid.
    RuntimeError
        If a required numerical calculation or requested finite result cannot be represented.
    TypeError
        If returns cannot be read as a numeric sequence.

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
    Most negative compounded percentage drawdown from an opening unit wealth.

    Parameters
    ----------
    returns : sequence of float, numpy.ndarray or pandas.Series
        Finite decimal simple investment returns in chronological order, each at least -1; an exact terminal -1 may occur only as the last observation.

    Returns
    -------
    float
        Signed fraction in [-1, 0], with exact -1 for a terminal wipeout; empty history is NaN and observed no-loss history is zero.

    Raises
    ------
    AnalyticsError
        If a required observation frequency, benchmark, cash rate, threshold, confidence or history domain is invalid for this calculation.
    RuntimeError
        If a required numerical calculation or requested finite result cannot be represented.
    TypeError
        If returns cannot be read as a numeric sequence.

    Examples
    --------
    >>> from finstack_quant.analytics import max_drawdown
    >>> round(max_drawdown([0.10, -0.20, 0.05]), 4)
    -0.2
    """
    ...

def multi_factor_greeks(
    returns: Sequence[float] | npt.NDArray[np.float64] | pd.Series,
    factor_returns: Sequence[Sequence[float]],
    periods_per_year: float,
    return_kind: str,
    risk_free_rate: float | None = None,
) -> MultiFactorResult:
    """
    Fit a raw equal-weight intercept OLS model with explicit response interpretation.

    Factors retain their supplied statistical units and are never compounded or
    cash-adjusted. The caller owns exact timestamp and interval alignment.

    Parameters
    ----------
    returns : sequence of float, numpy.ndarray or pandas.Series
        Finite chronological response observations. Excess responses may be
        below -1; total investment returns are at least -1 and an exact -1
        must end the observed history.
    factor_returns : sequence of sequence of float
        Factor-major finite predictors, each with exactly the response length.
        Do not include an intercept column; the model inserts one.
    periods_per_year : float
        Explicit positive finite annualization factor for alpha and residual
        volatility; it also converts cash for total responses.
    return_kind : str
        Required 'excess' uses the response unchanged; 'total' subtracts cash
        from the response only. No omitted or unknown kind is accepted.
    risk_free_rate : float, optional
        Annual-effective decimal cash rate, finite and greater than -1.
        Required for total responses; excess responses must omit it entirely.

    Returns
    -------
    MultiFactorResult
        Annual arithmetic intercept, factor loadings, actual observation count
        and field-specific fit statistics. Identified saturated fits retain
        coefficients while residual volatility and adjusted R-squared are NaN.

    Raises
    ------
    AnalyticsError
        If shapes, required kind/cash, finite domains, annualization or sample
        identification are invalid, including dependent factor columns.
    RuntimeError
        If stable fitting, coefficient rescaling or a requested result fails
        numerical representability or the final least-squares checks.
    TypeError
        If the response or predictors cannot be converted to numeric sequences.

    Examples
    --------
    >>> from finstack_quant.analytics import multi_factor_greeks
    >>> fit = multi_factor_greeks([0.01, 0.02, 0.03], [[0.0, 0.01, 0.02]], 12.0, "excess")
    >>> round(float(fit.betas[0]), 6)
    1.0
    """
    ...

def validate_summary(table: dict[str, Any] | str) -> None:
    """
    Validate a complete fixed analytics-summary table and its sample metadata.

    Parameters
    ----------
    table : dict[str, Any] or str
        TableEnvelope mapping or JSON string containing the fixed 22 metric
        columns, ordered ticker rows, explicit cash/frequency/MAR/Omega/tail
        inputs and own/paired sample metadata. Result values use canonical
        wire sentinel strings "nan", "inf" and "-inf"; explicit null numeric
        cells are forbidden. Validation checks the supplied report without
        repairing fields or rerunning unavailable source observations.

    Raises
    ------
    ValueError
        If the JSON or mapping cannot be converted into a TableEnvelope,
        including malformed fields, unsupported types or explicit null cells.
    AnalyticsError
        If the report's metric layout, required conventions, sample metadata
        or reported value domains violate the fixed-summary contract.

    Examples
    --------
    >>> from finstack_quant.analytics import validate_summary
    >>> try:
    ...     validate_summary({})
    ... except ValueError:
    ...     print("invalid summary")
    invalid summary
    """
    ...
