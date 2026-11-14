"""Stub declarations mirroring ``finstack-quant-py/src/bindings/analytics/types.rs``."""

from __future__ import annotations

import datetime
from typing import Any
from finstack_quant.core.dates import FiscalConfig
import numpy as np
import numpy.typing as npt
import pandas as pd

# Value-object results

class PeriodStats:
    """
    Equal-weight descriptive statistics of occupied aggregation buckets.

    Counts refer to buckets; neutral buckets affect win rate but not decisive
    Kelly probabilities. CPC is the Cardinal Profitability Construct adaptation
    and Kelly is a loss-normalized binary risk fraction, not a capital allocation.

    Examples
    --------
    >>> from finstack_quant.analytics import PeriodStats
    >>> from finstack_quant.analytics import Performance, PerformancePanel
    >>> panel = PerformancePanel(
    ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
    ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
    ...     kind="returns",
    ...     ticker_names=["FUND", "BENCH"],
    ...     frequency="monthly",
    ...     benchmark_ticker="BENCH",
    ... )
    >>> perf = Performance.from_panel(panel)
    >>> result = perf.period_stats(0, "daily")
    >>> isinstance(result, PeriodStats)
    True
    """

    @property
    def n_obs(self) -> int:
        """
        Number of occupied aggregation buckets, including neutral buckets.

        Returns
        -------
        int
            Number of occupied aggregation buckets, including neutral buckets.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @property
    def wins(self) -> int:
        """
        Number of buckets with strictly positive compounded returns.

        Returns
        -------
        int
            Number of buckets with strictly positive compounded returns.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @property
    def losses(self) -> int:
        """
        Number of buckets with strictly negative compounded returns.

        Returns
        -------
        int
            Number of buckets with strictly negative compounded returns.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PeriodStats:
        """
        Deserialize and validate a canonical PeriodStats result.

        Parameters
        ----------
        json : str
            Complete JSON string with required fields and canonical result sentinels;
            unknown fields and inconsistent metadata are rejected.

        Returns
        -------
        PeriodStats
            Validated result preserving undefined and infinite numeric meanings.

        Raises
        ------
        ValueError
            If JSON is malformed or violates the result structure or value domains.

        Examples
        --------
        >>> from finstack_quant.analytics import PeriodStats
        >>> from finstack_quant.analytics import Performance, PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
        ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
        ...     kind="returns",
        ...     ticker_names=["FUND", "BENCH"],
        ...     frequency="monthly",
        ...     benchmark_ticker="BENCH",
        ... )
        >>> perf = Performance.from_panel(panel)
        >>> result = perf.period_stats(0, "daily")
        >>> restored = PeriodStats.from_json(result.to_json())
        >>> isinstance(restored, PeriodStats)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize validated result state to canonical compact JSON.

        Returns
        -------
        str
            Complete JSON preserving NaN and signed infinity with canonical
            "nan", "inf" and "-inf" sentinel strings.

        Raises
        ------
        ValueError
            If stored state cannot be serialized.
        """
        ...

    @property
    def best(self) -> float:
        """
        Highest single-period return in the sample, as a decimal.

        Returns
        -------
        float
            Highest single-period return.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def worst(self) -> float:
        """
        Lowest single-period return in the sample, as a decimal.

        Returns
        -------
        float
            Lowest single-period return.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def consecutive_wins(self) -> int:
        """
        Longest consecutive winning streak.

        Returns
        -------
        int
            Maximum number of consecutive positive-return periods.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def consecutive_losses(self) -> int:
        """
        Longest consecutive losing streak.

        Returns
        -------
        int
            Maximum number of consecutive negative-return periods.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def win_rate(self) -> float:
        """
        Fraction of positive-return periods.

        Returns
        -------
        float
            Win rate in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def avg_return(self) -> float:
        """
        Average return across all periods.

        Returns
        -------
        float
            Mean periodic return.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def avg_win(self) -> float:
        """
        Average return of positive periods.

        Returns
        -------
        float
            Mean return across winning periods.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def avg_loss(self) -> float:
        """
        Average return of negative periods.

        Returns
        -------
        float
            Mean return across losing periods.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def payoff_ratio(self) -> float:
        """
        Payoff ratio (avg win / |avg loss|).

        Returns
        -------
        float
            Ratio of average win to absolute average loss.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def profit_factor(self) -> float:
        """
        Profit factor (gross profits / gross losses).

        Returns
        -------
        float
            Sum of wins divided by sum of absolute losses.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def cpc_index(self) -> float:
        """
        Cardinal Profitability Construct index adapted to equal-weight bucket returns.

        Returns
        -------
        float
            Profit factor times win rate times payoff, defined only when both positive and negative groups are observed.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """

    @property
    def kelly_risk_fraction(self) -> float:
        """
        Signed loss-normalized binary Kelly risk fraction.

        Returns
        -------
        float
            p - q / payoff from decisive buckets; undefined without both gains and losses. This is not an asset-exposure or portfolio-allocation fraction.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """

    def to_series(self) -> pd.Series:
        """
        The twelve statistics as a ``pandas.Series`` named ``period_stats``.

        Returns
        -------
        pd.Series
            Indexed by statistic name (``best``, ``worst``,
            ``consecutive_wins``, ``consecutive_losses``, ``win_rate``,
            ``avg_return``, ``avg_win``, ``avg_loss``, ``payoff_ratio``,
            ``profit_factor``, ``cpc_index``, ``kelly_risk_fraction``); streak
            counts are cast to ``float``.

        Raises
        ------
        ValueError
            If the values cannot be wrapped as a labelled pandas object.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        The twelve statistics as a single-row ``pandas.DataFrame``.

        Returns
        -------
        pd.DataFrame
            One row, columns in :meth:`to_series` order; stack tickers with
            ``pd.concat``. Non-finite ratios remain native ``NaN`` / ``inf``
            in ``float64`` columns; streak counts remain integers.

        Raises
        ------
        ValueError
            If the result cannot be represented as a pandas object.
        """
        ...

    def __repr__(self) -> str: ...
    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, rendered from :meth:`to_dataframe`.

        Returns
        -------
        str or None
            pandas HTML markup, or ``None`` when the frame cannot be built
            (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise; failures degrade to ``None``.
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

class BetaResult:
    """
    Paired equal-weight OLS beta with fixed two-sided 95% Student-t uncertainty.

    Two varying-benchmark pairs identify beta; uncertainty additionally requires
    positive residual degrees of freedom. Undefined fields remain native NaN.

    Examples
    --------
    >>> from finstack_quant.analytics import BetaResult
    >>> from finstack_quant.analytics import Performance, PerformancePanel
    >>> panel = PerformancePanel(
    ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
    ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
    ...     kind="returns",
    ...     ticker_names=["FUND", "BENCH"],
    ...     frequency="monthly",
    ...     benchmark_ticker="BENCH",
    ... )
    >>> perf = Performance.from_panel(panel)
    >>> result = perf.beta()[0]
    >>> isinstance(result, BetaResult)
    True
    """

    @staticmethod
    def from_json(json: str) -> BetaResult:
        """
        Deserialize and validate a canonical BetaResult result.

        Parameters
        ----------
        json : str
            Complete JSON string with required fields and canonical result sentinels;
            unknown fields and inconsistent metadata are rejected.

        Returns
        -------
        BetaResult
            Validated result preserving undefined and infinite numeric meanings.

        Raises
        ------
        ValueError
            If JSON is malformed or violates the result structure or value domains.

        Examples
        --------
        >>> from finstack_quant.analytics import BetaResult
        >>> from finstack_quant.analytics import Performance, PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
        ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
        ...     kind="returns",
        ...     ticker_names=["FUND", "BENCH"],
        ...     frequency="monthly",
        ...     benchmark_ticker="BENCH",
        ... )
        >>> perf = Performance.from_panel(panel)
        >>> result = perf.beta()[0]
        >>> restored = BetaResult.from_json(result.to_json())
        >>> isinstance(restored, BetaResult)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize validated result state to canonical compact JSON.

        Returns
        -------
        str
            Complete JSON preserving NaN and signed infinity with canonical
            "nan", "inf" and "-inf" sentinel strings.

        Raises
        ------
        ValueError
            If stored state cannot be serialized.
        """
        ...

    @property
    def beta(self) -> float:
        """
        OLS slope of asset returns versus the benchmark.

        Returns
        -------
        float
            OLS regression slope vs benchmark.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def std_err(self) -> float:
        """
        Standard error of the beta estimate.

        Returns
        -------
        float
            Standard error from the OLS fit.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def ci_lower(self) -> float:
        """
        Lower 95% confidence bound.

        Returns
        -------
        float
            Lower bound of the 95% CI for beta.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def ci_upper(self) -> float:
        """
        Upper 95% confidence bound.

        Returns
        -------
        float
            Upper bound of the 95% CI for beta.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def n_obs(self) -> int:
        """
        Number of observations the regression used.

        Returns
        -------
        int
            Paired observations after truncating the portfolio and benchmark
            series to their common length. Reported even when the estimates
            are ``NaN``. Required in serialized results. The interval uses ``n_obs - 2`` degrees
            of freedom.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def confidence_level(self) -> float:
        """
        Two-sided confidence level of ``ci_lower`` / ``ci_upper``.

        Returns
        -------
        float
            Decimal probability (``0.95`` for the 95% interval). The bounds
            are ``beta ± t(n_obs - 2, (1 + confidence_level) / 2) * std_err``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas DataFrame.

        Columns: ``beta``, ``std_err``, ``ci_lower``, ``ci_upper``, ``n_obs``
        (``int64`` observation count), ``confidence_level`` (decimal level of
        the bounds).

        One flat record describes one regression, so a one-row frame is the
        right shape: ``pd.concat([r.to_dataframe() for r in results])`` stacks
        every ticker's beta into one comparison table without reshaping.

        Undefined estimates from a degenerate regression remain ``NaN``
        in numeric ``float64`` columns.

        Returns
        -------
        pd.DataFrame
            Single-row frame with the beta point estimate and its interval.

        Raises
        ------
        ValueError
            If the result cannot be represented as a pandas object.
        """
        ...

    def __repr__(self) -> str: ...
    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, rendered from :meth:`to_dataframe`.

        Returns
        -------
        str or None
            pandas HTML markup, or ``None`` when the frame cannot be built
            (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise; failures degrade to ``None``.
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

class GreeksResult:
    """
    Paired OLS beta, annualized Jensen alpha and field-specific fit statistics.

    Examples
    --------
    >>> from finstack_quant.analytics import GreeksResult
    >>> from finstack_quant.analytics import Performance, PerformancePanel
    >>> panel = PerformancePanel(
    ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
    ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
    ...     kind="returns",
    ...     ticker_names=["FUND", "BENCH"],
    ...     frequency="monthly",
    ...     benchmark_ticker="BENCH",
    ... )
    >>> perf = Performance.from_panel(panel)
    >>> result = perf.greeks(0.0)[0]
    >>> isinstance(result, GreeksResult)
    True
    """

    @staticmethod
    def from_json(json: str) -> GreeksResult:
        """
        Deserialize and validate a canonical GreeksResult result.

        Parameters
        ----------
        json : str
            Complete JSON string with required fields and canonical result sentinels;
            unknown fields and inconsistent metadata are rejected.

        Returns
        -------
        GreeksResult
            Validated result preserving undefined and infinite numeric meanings.

        Raises
        ------
        ValueError
            If JSON is malformed or violates the result structure or value domains.

        Examples
        --------
        >>> from finstack_quant.analytics import GreeksResult
        >>> from finstack_quant.analytics import Performance, PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
        ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
        ...     kind="returns",
        ...     ticker_names=["FUND", "BENCH"],
        ...     frequency="monthly",
        ...     benchmark_ticker="BENCH",
        ... )
        >>> perf = Performance.from_panel(panel)
        >>> result = perf.greeks(0.0)[0]
        >>> restored = GreeksResult.from_json(result.to_json())
        >>> isinstance(restored, GreeksResult)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize validated result state to canonical compact JSON.

        Returns
        -------
        str
            Complete JSON preserving NaN and signed infinity with canonical
            "nan", "inf" and "-inf" sentinel strings.

        Raises
        ------
        ValueError
            If stored state cannot be serialized.
        """
        ...

    @property
    def alpha(self) -> float:
        """
        Annualized Jensen alpha.

        Returns
        -------
        float
            Annualized intercept from the single-index regression.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def beta(self) -> float:
        """
        OLS slope of asset returns versus the benchmark.

        Returns
        -------
        float
            OLS regression slope vs benchmark.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def r_squared(self) -> float:
        """
        Coefficient of determination of the fitted model.

        Returns
        -------
        float
            Coefficient of determination.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def adjusted_r_squared(self) -> float:
        """
        Degrees-of-freedom-adjusted coefficient of determination.

        Returns
        -------
        float
            Degrees-of-freedom-adjusted R².

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def n_obs(self) -> int:
        """
        Number of observations the regression used.

        Returns
        -------
        int
            Paired observations after truncating the portfolio and benchmark
            series to their common length. Reported even when the estimates
            are ``NaN``. Required in serialized results. Adjusted R² uses ``n_obs - 2`` residual
            degrees of freedom.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas DataFrame.

        Columns: ``alpha``, ``beta``, ``r_squared``, ``adjusted_r_squared``,
        ``n_obs`` (``int64`` observation count).

        One flat record describes one regression, so a one-row frame is the
        right shape: ``pd.concat([r.to_dataframe() for r in results])`` stacks
        every ticker's greeks into one comparison table without reshaping.

        Undefined estimates from a degenerate fit remain ``NaN`` in numeric
        ``float64`` columns.

        Returns
        -------
        pd.DataFrame
            Single-row frame with alpha, beta, and goodness-of-fit.

        Raises
        ------
        ValueError
            If the result cannot be represented as a pandas object.
        """
        ...

    def __repr__(self) -> str: ...
    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, rendered from :meth:`to_dataframe`.

        Returns
        -------
        str or None
            pandas HTML markup, or ``None`` when the frame cannot be built
            (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise; failures degrade to ``None``.
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

class RollingGreeks:
    """
    Compact right-labelled rolling Jensen alpha and beta estimates.

    Dates are strictly increasing and aligned exactly with both vectors.
    Undefined window statistics retain native NaN and canonical JSON sentinels.

    Examples
    --------
    >>> from finstack_quant.analytics import RollingGreeks
    >>> from finstack_quant.analytics import Performance, PerformancePanel
    >>> panel = PerformancePanel(
    ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
    ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
    ...     kind="returns",
    ...     ticker_names=["FUND", "BENCH"],
    ...     frequency="monthly",
    ...     benchmark_ticker="BENCH",
    ... )
    >>> perf = Performance.from_panel(panel)
    >>> result = perf.rolling_greeks(0, 2, 0.0)
    >>> isinstance(result, RollingGreeks)
    True
    """

    @staticmethod
    def from_json(json: str) -> RollingGreeks:
        """
        Deserialize and validate a canonical RollingGreeks result.

        Parameters
        ----------
        json : str
            Complete JSON string with required fields and canonical result sentinels;
            unknown fields and inconsistent metadata are rejected.

        Returns
        -------
        RollingGreeks
            Validated result preserving undefined and infinite numeric meanings.

        Raises
        ------
        ValueError
            If JSON is malformed or violates the result structure or value domains.

        Examples
        --------
        >>> from finstack_quant.analytics import RollingGreeks
        >>> from finstack_quant.analytics import Performance, PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
        ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
        ...     kind="returns",
        ...     ticker_names=["FUND", "BENCH"],
        ...     frequency="monthly",
        ...     benchmark_ticker="BENCH",
        ... )
        >>> perf = Performance.from_panel(panel)
        >>> result = perf.rolling_greeks(0, 2, 0.0)
        >>> restored = RollingGreeks.from_json(result.to_json())
        >>> isinstance(restored, RollingGreeks)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize validated result state to canonical compact JSON.

        Returns
        -------
        str
            Complete JSON preserving NaN and signed infinity with canonical
            "nan", "inf" and "-inf" sentinel strings.

        Raises
        ------
        ValueError
            If stored state cannot be serialized.
        """
        ...

    @property
    def dates(self) -> list[datetime.date]:
        """
        Date labels for each rolling window.

        Returns
        -------
        list[datetime.date]
            Window-end dates aligned 1:1 with :attr:`alphas` and :attr:`betas`.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    @property
    def alphas(self) -> npt.NDArray[np.float64]:
        """
        Rolling annualized Jensen alpha for each window.

        Returns
        -------
        npt.NDArray[np.float64]
            Annualized Jensen alpha per window.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def betas(self) -> npt.NDArray[np.float64]:
        """
        Rolling OLS beta versus the benchmark for each window.

        Returns
        -------
        npt.NDArray[np.float64]
            OLS beta per window.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """
        Convert to a pandas DataFrame with a ``DatetimeIndex`` and ``alpha`` /
        ``beta`` columns.

        Returns
        -------
        pd.DataFrame
            DataFrame with columns ``alpha`` and ``beta``, indexed by date.

        Raises
        ------
        ValueError
            If the result cannot be represented as a pandas object.
        """
        ...

    def __repr__(self) -> str: ...
    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, rendered from :meth:`to_dataframe`.

        Returns
        -------
        str or None
            pandas HTML markup, or ``None`` when the frame cannot be built
            (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise; failures degrade to ``None``.
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

class MultiFactorResult:
    """
    Equal-weight intercept OLS coefficients and residual-df-adjusted fit statistics.

    Alpha is an annual arithmetic response intercept, betas retain factor units
    and residual volatility is annualized using N - k - 1 residual degrees of
    freedom. Saturated fits preserve coefficients but lack residual risk.

    Examples
    --------
    >>> from finstack_quant.analytics import MultiFactorResult
    >>> from finstack_quant.analytics import Performance, PerformancePanel
    >>> panel = PerformancePanel(
    ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
    ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
    ...     kind="returns",
    ...     ticker_names=["FUND", "BENCH"],
    ...     frequency="monthly",
    ...     benchmark_ticker="BENCH",
    ... )
    >>> perf = Performance.from_panel(panel)
    >>> result = perf.multi_factor_greeks(0, [[0.02, -0.03, 0.01]], 0.0)
    >>> isinstance(result, MultiFactorResult)
    True
    """

    @staticmethod
    def from_json(json: str) -> MultiFactorResult:
        """
        Deserialize and validate a canonical MultiFactorResult result.

        Parameters
        ----------
        json : str
            Complete JSON string with required fields and canonical result sentinels;
            unknown fields and inconsistent metadata are rejected.

        Returns
        -------
        MultiFactorResult
            Validated result preserving undefined and infinite numeric meanings.

        Raises
        ------
        ValueError
            If JSON is malformed or violates the result structure or value domains.

        Examples
        --------
        >>> from finstack_quant.analytics import MultiFactorResult
        >>> from finstack_quant.analytics import Performance, PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
        ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
        ...     kind="returns",
        ...     ticker_names=["FUND", "BENCH"],
        ...     frequency="monthly",
        ...     benchmark_ticker="BENCH",
        ... )
        >>> perf = Performance.from_panel(panel)
        >>> result = perf.multi_factor_greeks(0, [[0.02, -0.03, 0.01]], 0.0)
        >>> restored = MultiFactorResult.from_json(result.to_json())
        >>> isinstance(restored, MultiFactorResult)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize validated result state to canonical compact JSON.

        Returns
        -------
        str
            Complete JSON preserving NaN and signed infinity with canonical
            "nan", "inf" and "-inf" sentinel strings.

        Raises
        ------
        ValueError
            If stored state cannot be serialized.
        """
        ...

    @property
    def alpha(self) -> float:
        """
        Annual arithmetic intercept in the supplied response model.

        Returns
        -------
        float
            Declared periods per year times the observation-period OLS intercept; cash is subtracted only for an explicit Total-response fit.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """

    @property
    def betas(self) -> npt.NDArray[np.float64]:
        """
        One beta per factor, in factor order.

        Returns
        -------
        npt.NDArray[np.float64]
            One beta per factor, in factor order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def r_squared(self) -> float:
        """
        Coefficient of determination of the fitted model.

        Returns
        -------
        float
            Coefficient of determination; ``nan`` for a constant dependent series.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def adjusted_r_squared(self) -> float:
        """
        Degrees-of-freedom-adjusted coefficient of determination.

        Returns
        -------
        float
            Adjusted R²; ``nan`` when R² or residual degrees of freedom are undefined.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def residual_vol(self) -> float:
        """
        Annualized in-sample residual standard deviation.

        Returns
        -------
        float
            sqrt(SSE / (N - k - 1)) scaled by sqrt(periods_per_year); NaN when residual degrees of freedom are zero.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """

    @property
    def n_obs(self) -> int:
        """
        Number of observations the regression used.

        Returns
        -------
        int
            Common length of the dependent series and every factor series.
            Residual degrees of freedom are ``n_obs - len(betas) - 1``.
            Required in serialized results.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def to_dataframe(self, factor_names: list[str] | None = None) -> pd.DataFrame:
        """
        Export the factor loadings as a pandas DataFrame, one row per factor.

        Columns: ``factor``, ``beta``, ``alpha``, ``r_squared``,
        ``adjusted_r_squared``, ``residual_vol``, ``n_obs`` (``int64``
        observation count).

        The loadings are the per-row payload; the five regression-level
        statistics repeat on every row so a single row carries its own fit
        context after ``pd.concat`` across tickers or ``groupby("factor")``.

        Rows follow the order of the ``factor_returns`` passed to
        :meth:`Performance.multi_factor_greeks`, which is also the order of
        :attr:`betas`. There is always at least one row: the regression rejects
        an empty factor set.

        Parameters
        ----------
        factor_names : list[str], optional
            Labels for the ``factor`` column, positionally aligned with
            :attr:`betas`. Defaults to ``factor_0``, ``factor_1``, ... because
            the regression itself carries no names.

        Returns
        -------
        pd.DataFrame
            One row per fitted factor.

        Raises
        ------
        ValueError
            If ``factor_names`` is supplied and its length differs from the
            number of fitted betas.
        """
        ...

    def __repr__(self) -> str: ...
    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, rendered from :meth:`to_dataframe`.

        Returns
        -------
        str or None
            pandas HTML markup, or ``None`` when the frame cannot be built
            (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise; failures degrade to ``None``.
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

class DrawdownEpisode:
    """
    Strict compounded percentage drawdown episode on a rebased observed path.

    An episode starts at the last preceding observed peak and closes only at
    an observed exact zero drawdown. Open episodes have no recovery date and
    measure duration through the last observed endpoint.

    Examples
    --------
    >>> from finstack_quant.analytics import DrawdownEpisode
    >>> from finstack_quant.analytics import Performance, PerformancePanel
    >>> panel = PerformancePanel(
    ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
    ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
    ...     kind="returns",
    ...     ticker_names=["FUND", "BENCH"],
    ...     frequency="monthly",
    ...     benchmark_ticker="BENCH",
    ... )
    >>> perf = Performance.from_panel(panel)
    >>> result = perf.drawdown_details(0, 1)[0]
    >>> isinstance(result, DrawdownEpisode)
    True
    """

    @staticmethod
    def from_json(json: str) -> DrawdownEpisode:
        """
        Deserialize and validate a canonical DrawdownEpisode result.

        Parameters
        ----------
        json : str
            Complete JSON string with required fields and canonical result sentinels;
            unknown fields and inconsistent metadata are rejected.

        Returns
        -------
        DrawdownEpisode
            Validated result preserving undefined and infinite numeric meanings.

        Raises
        ------
        ValueError
            If JSON is malformed or violates the result structure or value domains.

        Examples
        --------
        >>> from finstack_quant.analytics import DrawdownEpisode
        >>> from finstack_quant.analytics import Performance, PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
        ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
        ...     kind="returns",
        ...     ticker_names=["FUND", "BENCH"],
        ...     frequency="monthly",
        ...     benchmark_ticker="BENCH",
        ... )
        >>> perf = Performance.from_panel(panel)
        >>> result = perf.drawdown_details(0, 1)[0]
        >>> restored = DrawdownEpisode.from_json(result.to_json())
        >>> isinstance(restored, DrawdownEpisode)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize validated result state to canonical compact JSON.

        Returns
        -------
        str
            Complete JSON preserving NaN and signed infinity with canonical
            "nan", "inf" and "-inf" sentinel strings.

        Raises
        ------
        ValueError
            If stored state cannot be serialized.
        """
        ...

    @property
    def start(self) -> datetime.date:
        """
        Start date of the drawdown.

        Returns
        -------
        datetime.date
            Date when the drawdown began.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    @property
    def valley(self) -> datetime.date:
        """
        Date of the maximum drawdown within this episode.

        Returns
        -------
        datetime.date
            Date of the deepest point.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    @property
    def end(self) -> datetime.date | None:
        """
        Recovery date (``None`` if still in drawdown).

        Returns
        -------
        datetime.date or None
            Recovery date, or ``None`` if the episode is ongoing.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    @property
    def duration_days(self) -> int:
        """
        Duration in calendar days.

        Returns
        -------
        int
            Number of calendar days from start to end (or valley if ongoing).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def max_drawdown(self) -> float:
        """
        Maximum drawdown depth (negative).

        Returns
        -------
        float
            Peak-to-trough drawdown as a negative decimal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row ``pandas.DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``start``, ``valley``, ``end`` (``datetime64``; ``NaT``
            while still in drawdown), ``duration_days`` and ``max_drawdown``. Stack
            episodes with ``pd.concat([ep.to_dataframe() for ep in episodes])``.

        Raises
        ------
        ValueError
            If the result cannot be represented as a pandas object.
        """
        ...

    def __repr__(self) -> str: ...
    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, rendered from :meth:`to_dataframe`.

        Returns
        -------
        str or None
            pandas HTML markup, or ``None`` when the frame cannot be built
            (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise; failures degrade to ``None``.
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

class LookbackReturns:
    """
    Calendar/fiscal lookback returns and compact sample provenance per ticker.

    Each value vector matches ticker order and its corresponding sample vector.
    Empty samples retain undefined NaN return values and absent holding bounds.

    Examples
    --------
    >>> from finstack_quant.analytics import LookbackReturns
    >>> from finstack_quant.analytics import Performance, PerformancePanel
    >>> panel = PerformancePanel(
    ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
    ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
    ...     kind="returns",
    ...     ticker_names=["FUND", "BENCH"],
    ...     frequency="monthly",
    ...     benchmark_ticker="BENCH",
    ... )
    >>> perf = Performance.from_panel(panel)
    >>> result = perf.lookback_returns()
    >>> isinstance(result, LookbackReturns)
    True
    """

    @property
    def fiscal_config(self) -> FiscalConfig:
        """
        Resolved fiscal opening configuration used by FYTD; calendar-year rules are retained when no override was supplied.

        Returns
        -------
        FiscalConfig
            Resolved fiscal opening configuration used by FYTD; calendar-year rules are retained when no override was supplied.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @property
    def mtd_samples(self) -> list[LookbackSample]:
        """
        Ticker-aligned raw observation counts and actual holding bounds for MTD.

        Returns
        -------
        list[LookbackSample]
            Ticker-aligned raw observation counts and actual holding bounds for MTD.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @property
    def qtd_samples(self) -> list[LookbackSample]:
        """
        Ticker-aligned raw observation counts and actual holding bounds for QTD.

        Returns
        -------
        list[LookbackSample]
            Ticker-aligned raw observation counts and actual holding bounds for QTD.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @property
    def ytd_samples(self) -> list[LookbackSample]:
        """
        Ticker-aligned raw observation counts and actual holding bounds for YTD.

        Returns
        -------
        list[LookbackSample]
            Ticker-aligned raw observation counts and actual holding bounds for YTD.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @property
    def fytd_samples(self) -> list[LookbackSample]:
        """
        Ticker-aligned raw observation counts and actual holding bounds for FYTD.

        Returns
        -------
        list[LookbackSample]
            Ticker-aligned raw observation counts and actual holding bounds for FYTD.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @staticmethod
    def from_json(json: str) -> LookbackReturns:
        """
        Deserialize and validate a canonical LookbackReturns result.

        Parameters
        ----------
        json : str
            Complete JSON string with required fields and canonical result sentinels;
            unknown fields and inconsistent metadata are rejected.

        Returns
        -------
        LookbackReturns
            Validated result preserving undefined and infinite numeric meanings.

        Raises
        ------
        ValueError
            If JSON is malformed or violates the result structure or value domains.

        Examples
        --------
        >>> from finstack_quant.analytics import LookbackReturns
        >>> from finstack_quant.analytics import Performance, PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
        ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
        ...     kind="returns",
        ...     ticker_names=["FUND", "BENCH"],
        ...     frequency="monthly",
        ...     benchmark_ticker="BENCH",
        ... )
        >>> perf = Performance.from_panel(panel)
        >>> result = perf.lookback_returns()
        >>> restored = LookbackReturns.from_json(result.to_json())
        >>> isinstance(restored, LookbackReturns)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize validated result state to canonical compact JSON.

        Returns
        -------
        str
            Complete JSON preserving NaN and signed infinity with canonical
            "nan", "inf" and "-inf" sentinel strings.

        Raises
        ------
        ValueError
            If stored state cannot be serialized.
        """
        ...

    @property
    def mtd(self) -> npt.NDArray[np.float64]:
        """
        Month-to-date returns per ticker.

        Returns
        -------
        npt.NDArray[np.float64]
            Array of MTD returns, one per ticker.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def qtd(self) -> npt.NDArray[np.float64]:
        """
        Quarter-to-date returns per ticker.

        Returns
        -------
        npt.NDArray[np.float64]
            Array of QTD returns, one per ticker.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def ytd(self) -> npt.NDArray[np.float64]:
        """
        Year-to-date returns per ticker.

        Returns
        -------
        npt.NDArray[np.float64]
            Array of YTD returns, one per ticker.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def fytd(self) -> npt.NDArray[np.float64]:
        """
        Fiscal-year-to-date returns per ticker.

        Returns
        -------
        npt.NDArray[np.float64]
            Array of FYTD returns, one per ticker. The fiscal calendar defaults
            to a January-1 start when no fiscal configuration is supplied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def ref_date(self) -> datetime.date:
        """
        Date on which every lookback window ended.

        Returns
        -------
        datetime.date
            The requested date, else the last active date of the panel.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ticker_names(self) -> list[str]:
        """
        Ticker names aligned with the ``mtd`` / ``qtd`` / ``ytd`` / ``fytd``
        vectors.

        Returns
        -------
        list[str]
            One label per ticker, in panel column order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Convert to a pandas DataFrame indexed by :attr:`ticker_names`.

        Columns: ``mtd``, ``qtd``, ``ytd``, and ``fytd``.

        Returns
        -------
        pd.DataFrame
            DataFrame indexed by ticker name with the four lookback return
            columns, including ``fytd``.

        Raises
        ------
        ValueError
            If the result cannot be represented as a pandas object.
        """
        ...

    def __repr__(self) -> str: ...
    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, rendered from :meth:`to_dataframe`.

        Returns
        -------
        str or None
            pandas HTML markup, or ``None`` when the frame cannot be built
            (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise; failures degrade to ``None``.
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

class DatedSeries:
    """
    Validated compact date-indexed numeric output of a rolling metric.

    One value is stored per complete-window endpoint. Native undefined values
    and intentional signed infinities round-trip as canonical result sentinels.

    Examples
    --------
    >>> from finstack_quant.analytics import DatedSeries
    >>> from finstack_quant.analytics import Performance, PerformancePanel
    >>> panel = PerformancePanel(
    ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
    ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
    ...     kind="returns",
    ...     ticker_names=["FUND", "BENCH"],
    ...     frequency="monthly",
    ...     benchmark_ticker="BENCH",
    ... )
    >>> perf = Performance.from_panel(panel)
    >>> result = perf.rolling_returns(0, 2)
    >>> isinstance(result, DatedSeries)
    True
    """

    @staticmethod
    def from_json(json: str) -> DatedSeries:
        """
        Deserialize and validate a canonical DatedSeries result.

        Parameters
        ----------
        json : str
            Complete JSON string with required fields and canonical result sentinels;
            unknown fields and inconsistent metadata are rejected.

        Returns
        -------
        DatedSeries
            Validated result preserving undefined and infinite numeric meanings.

        Raises
        ------
        ValueError
            If JSON is malformed or violates the result structure or value domains.

        Examples
        --------
        >>> from finstack_quant.analytics import DatedSeries
        >>> from finstack_quant.analytics import Performance, PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
        ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
        ...     kind="returns",
        ...     ticker_names=["FUND", "BENCH"],
        ...     frequency="monthly",
        ...     benchmark_ticker="BENCH",
        ... )
        >>> perf = Performance.from_panel(panel)
        >>> result = perf.rolling_returns(0, 2)
        >>> restored = DatedSeries.from_json(result.to_json())
        >>> isinstance(restored, DatedSeries)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize validated result state to canonical compact JSON.

        Returns
        -------
        str
            Complete JSON preserving NaN and signed infinity with canonical
            "nan", "inf" and "-inf" sentinel strings.

        Raises
        ------
        ValueError
            If stored state cannot be serialized.
        """
        ...

    @property
    def values(self) -> npt.NDArray[np.float64]:
        """
        Rolling values, one per window.

        Returns
        -------
        npt.NDArray[np.float64]
            Metric values aligned with :attr:`dates`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def dates(self) -> list[datetime.date]:
        """
        Window-end dates aligned 1:1 with :attr:`values`.

        Returns
        -------
        list[datetime.date]
            Date labels for each rolling window.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    @property
    def value_column(self) -> str:
        """
        Metric name of the series, also the column name used by
        :meth:`to_dataframe`.

        Returns
        -------
        str
            Rust ``RollingMetric`` name: ``"volatility"``, ``"sortino"``,
            ``"sharpe"`` or ``"return"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """
        Convert to a pandas DataFrame with a ``DatetimeIndex`` and a value column.

        The column is named after :attr:`value_column` (e.g. ``sharpe``,
        ``sortino``, ``volatility``, or ``return``).

        Returns
        -------
        pd.DataFrame
            DataFrame with a date index and one column named
            :attr:`value_column`.

        Raises
        ------
        ValueError
            If the result cannot be represented as a pandas object.
        """
        ...

    def __repr__(self) -> str: ...
    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, rendered from :meth:`to_dataframe`.

        Returns
        -------
        str or None
            pandas HTML markup, or ``None`` when the frame cannot be built
            (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise; failures degrade to ``None``.
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

class LookbackSample:
    """
    Compact actual holding-span provenance for one ticker and one lookback horizon.

    Examples
    --------
    >>> from finstack_quant.analytics import LookbackSample
    >>> from finstack_quant.analytics import Performance, PerformancePanel
    >>> panel = PerformancePanel(
    ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
    ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
    ...     kind="returns",
    ...     ticker_names=["FUND", "BENCH"],
    ...     frequency="monthly",
    ...     benchmark_ticker="BENCH",
    ... )
    >>> perf = Performance.from_panel(panel)
    >>> result = perf.lookback_returns().mtd_samples[0]
    >>> isinstance(result, LookbackSample)
    True
    """

    @property
    def n_obs(self) -> int:
        """
        Number of included raw interval returns, zero for an empty sample.

        Returns
        -------
        int
            Number of included raw interval returns, zero for an empty sample.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @property
    def holding_start(self) -> datetime.date | None:
        """
        Actual valuation immediately before the first included return, absent for an empty sample.

        Returns
        -------
        datetime.date | None
            Actual valuation immediately before the first included return, absent for an empty sample.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @property
    def holding_end(self) -> datetime.date | None:
        """
        Last included return endpoint, absent for an empty sample.

        Returns
        -------
        datetime.date | None
            Last included return endpoint, absent for an empty sample.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @staticmethod
    def from_json(json: str) -> LookbackSample:
        """
        Deserialize and validate a canonical LookbackSample carrier.

        Parameters
        ----------
        json : str
            Complete canonical JSON; all fields are required and unknown fields
            or inconsistent count, date and numerical state are rejected.

        Returns
        -------
        LookbackSample
            Validated result carrier preserving original ordering and metadata.

        Raises
        ------
        ValueError
            If JSON is malformed or the result metadata is inconsistent.

        Examples
        --------
        >>> from finstack_quant.analytics import LookbackSample
        >>> from finstack_quant.analytics import Performance, PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
        ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
        ...     kind="returns",
        ...     ticker_names=["FUND", "BENCH"],
        ...     frequency="monthly",
        ...     benchmark_ticker="BENCH",
        ... )
        >>> perf = Performance.from_panel(panel)
        >>> result = perf.lookback_returns().mtd_samples[0]
        >>> isinstance(result, LookbackSample)
        True
        >>> isinstance(LookbackSample.from_json(result.to_json()), LookbackSample)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the complete validated carrier.

        Returns
        -------
        str
            Canonical compact JSON with all required metadata.

        Raises
        ------
        ValueError
            If serialization of stored state fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support pickle through canonical JSON restoration.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and JSON arguments.

        Raises
        ------
        ValueError
            If serialization of stored state fails.
        """
        ...

class CorrelationResult:
    """
    Common-sample Pearson correlation and its observed sample provenance.

    The matrix is finite, symmetric and unit-diagonal; singular positive-
    semidefinite matrices are allowed. No repair flag or adjusted matrix is stored.

    Examples
    --------
    >>> from finstack_quant.analytics import CorrelationResult
    >>> from finstack_quant.analytics import Performance, PerformancePanel
    >>> panel = PerformancePanel(
    ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
    ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
    ...     kind="returns",
    ...     ticker_names=["FUND", "BENCH"],
    ...     frequency="monthly",
    ...     benchmark_ticker="BENCH",
    ... )
    >>> perf = Performance.from_panel(panel)
    >>> result = perf.correlation_matrix()
    >>> isinstance(result, CorrelationResult)
    True
    """

    @property
    def tickers(self) -> list[str]:
        """
        Original ticker identities in the same order as matrix rows and columns.

        Returns
        -------
        list[str]
            Original ticker identities in the same order as matrix rows and columns.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @property
    def matrix(self) -> list[list[float]]:
        """
        Finite symmetric Pearson coefficients on the one shared sample; no automatic projection or pairwise fallback.

        Returns
        -------
        list[list[float]]
            Finite symmetric Pearson coefficients on the one shared sample; no automatic projection or pairwise fallback.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @property
    def n_obs(self) -> int:
        """
        Number of common observed endpoints, at least two.

        Returns
        -------
        int
            Number of common observed endpoints, at least two.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @property
    def start_date(self) -> datetime.date:
        """
        First included common return endpoint.

        Returns
        -------
        datetime.date
            First included common return endpoint.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @property
    def end_date(self) -> datetime.date:
        """
        Last included common return endpoint.

        Returns
        -------
        datetime.date
            Last included common return endpoint.

        Notes
        -----
        This accessor returns stored validated state and does not raise.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CorrelationResult:
        """
        Deserialize and validate a canonical CorrelationResult carrier.

        Parameters
        ----------
        json : str
            Complete canonical JSON; all fields are required and unknown fields
            or inconsistent count, date and numerical state are rejected.

        Returns
        -------
        CorrelationResult
            Validated result carrier preserving original ordering and metadata.

        Raises
        ------
        ValueError
            If JSON is malformed or the result metadata is inconsistent.

        Examples
        --------
        >>> from finstack_quant.analytics import CorrelationResult
        >>> from finstack_quant.analytics import Performance, PerformancePanel
        >>> panel = PerformancePanel(
        ...     dates=["2024-01-01", "2024-01-02", "2024-01-03", "2024-01-04"],
        ...     series=[[0.10, -0.20, 0.05], [0.02, -0.03, 0.01]],
        ...     kind="returns",
        ...     ticker_names=["FUND", "BENCH"],
        ...     frequency="monthly",
        ...     benchmark_ticker="BENCH",
        ... )
        >>> perf = Performance.from_panel(panel)
        >>> result = perf.correlation_matrix()
        >>> isinstance(result, CorrelationResult)
        True
        >>> isinstance(CorrelationResult.from_json(result.to_json()), CorrelationResult)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the complete validated carrier.

        Returns
        -------
        str
            Canonical compact JSON with all required metadata.

        Raises
        ------
        ValueError
            If serialization of stored state fails.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the correlation matrix as a square ``pandas.DataFrame`` indexed and columned by :attr:`tickers`.

        Row and column labels follow panel column order, preserving the
        common-sample symmetric Pearson coefficients as ``float64`` columns.

        Returns
        -------
        pd.DataFrame
            Square ``DataFrame`` with index and columns equal to :attr:`tickers`.

        Raises
        ------
        ValueError
            If the stored matrix dimensions are inconsistent with :attr:`tickers`
            or the result cannot be represented as a pandas object.
        """
        ...

    def __repr__(self) -> str: ...
    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, rendered from :meth:`to_dataframe`.

        Returns
        -------
        str or None
            pandas HTML markup, or ``None`` when the frame cannot be built
            (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise; failures degrade to ``None``.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support pickle through canonical JSON restoration.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and JSON arguments.

        Raises
        ------
        ValueError
            If serialization of stored state fails.
        """
        ...
