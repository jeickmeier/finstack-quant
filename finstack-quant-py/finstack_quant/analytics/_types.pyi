"""Stub declarations mirroring ``finstack-quant-py/src/bindings/analytics/types.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import numpy as np
import numpy.typing as npt
import pandas as pd

# Value-object results

class PeriodStats:
    """
    Aggregated statistics for grouped periodic returns.

    Examples
    --------
    >>> from datetime import date, timedelta
    >>> from finstack_quant.analytics import Performance
    >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(6)]
    >>> perf = Performance.from_returns_arrays(
    ...     dates, [[0.10, -0.05, 0.02, 0.0, 0.03, -0.01]], ["FUND"], frequency="monthly"
    ... )
    >>> round(perf.period_stats(0, aggregation_frequency="monthly").win_rate, 1)
    1.0
    """

    @staticmethod
    def from_json(json: str) -> PeriodStats:
        """
        Deserialize a ``PeriodStats`` from JSON.

        Parameters
        ----------
        json : str
            Complete JSON string produced by :meth:`to_json`; required metadata must be present and unknown fields are rejected.

        Returns
        -------
        PeriodStats
            Parsed ``PeriodStats`` instance.

        Raises
        ------
        ValueError
            If ``json`` is malformed, omits required result fields, contains unknown fields, or does not satisfy the serialized schema.

        Examples
        --------
        >>> from datetime import date
        >>> from finstack_quant.analytics import PeriodStats, Performance
        >>> dates = [date(2024, month, 1) for month in range(1, 7)]
        >>> perf = Performance.from_returns_arrays(
        ...     dates, [[0.10, -0.05, 0.02, 0.0, 0.03, -0.01]], ["FUND"], frequency="monthly"
        ... )
        >>> stats = perf.period_stats(0, aggregation_frequency="monthly")
        >>> PeriodStats.from_json(stats.to_json()).win_rate
        0.5
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
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
    def cpc_ratio(self) -> float:
        """
        CPC index (profit_factor x win_rate x payoff_ratio).

        Returns
        -------
        float
            Composite measure of profitability consistency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def kelly_criterion(self) -> float:
        """
        Kelly criterion optimal fraction.

        Returns
        -------
        float
            Binary win/loss payoff approximation, using probabilities conditional
            on nonzero returns; neutral periods do not affect the fraction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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
            ``profit_factor``, ``cpc_ratio``, ``kelly_criterion``); streak
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
    Regression beta with confidence interval.

    The 95% interval uses Student-t critical values for finite samples and an
    asymptotic normal approximation once ``n - 2 >= 240``.

    Examples
    --------
    >>> from datetime import date, timedelta
    >>> from finstack_quant.analytics import Performance
    >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(6)]
    >>> benchmark = [-0.02, -0.01, 0.0, 0.01, 0.02, 0.03]
    >>> perf = Performance.from_returns_arrays(
    ...     dates,
    ...     [[2.0 * value for value in benchmark], benchmark],
    ...     ["FUND", "BENCH"],
    ...     benchmark_ticker="BENCH",
    ...     frequency="monthly",
    ... )
    >>> perf.beta()[0].beta
    2.0
    """

    @staticmethod
    def from_json(json: str) -> BetaResult:
        """
        Deserialize a ``BetaResult`` from JSON.

        Parameters
        ----------
        json : str
            Complete JSON string produced by :meth:`to_json`; required metadata must be present and unknown fields are rejected.

        Returns
        -------
        BetaResult
            Parsed ``BetaResult`` instance.

        Raises
        ------
        ValueError
            If ``json`` is malformed, omits required result fields, contains unknown fields, or does not satisfy the serialized schema.

        Examples
        --------
        >>> from datetime import date, timedelta
        >>> from finstack_quant.analytics import BetaResult, Performance
        >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(6)]
        >>> benchmark = [-0.02, -0.01, 0.0, 0.01, 0.02, 0.03]
        >>> perf = Performance.from_returns_arrays(
        ...     dates,
        ...     [[2.0 * value for value in benchmark], benchmark],
        ...     ["FUND", "BENCH"],
        ...     benchmark_ticker="BENCH",
        ...     frequency="monthly",
        ... )
        >>> BetaResult.from_json(perf.beta()[0].to_json()).beta
        2.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
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
    Alpha, beta, and goodness-of-fit from a single-index regression.

    Examples
    --------
    >>> from datetime import date, timedelta
    >>> from finstack_quant.analytics import Performance
    >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(6)]
    >>> benchmark = [-0.02, -0.01, 0.0, 0.01, 0.02, 0.03]
    >>> perf = Performance.from_returns_arrays(
    ...     dates,
    ...     [[2.0 * value for value in benchmark], benchmark],
    ...     ["FUND", "BENCH"],
    ...     benchmark_ticker="BENCH",
    ...     frequency="monthly",
    ... )
    >>> round(perf.greeks()[0].alpha, 12)
    0.0
    """

    @staticmethod
    def from_json(json: str) -> GreeksResult:
        """
        Deserialize a ``GreeksResult`` from JSON.

        Parameters
        ----------
        json : str
            Complete JSON string produced by :meth:`to_json`; required metadata must be present and unknown fields are rejected.

        Returns
        -------
        GreeksResult
            Parsed ``GreeksResult`` instance.

        Raises
        ------
        ValueError
            If ``json`` is malformed, omits required result fields, contains unknown fields, or does not satisfy the serialized schema.

        Examples
        --------
        >>> from datetime import date, timedelta
        >>> from finstack_quant.analytics import GreeksResult, Performance
        >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(6)]
        >>> benchmark = [-0.02, -0.01, 0.0, 0.01, 0.02, 0.03]
        >>> perf = Performance.from_returns_arrays(
        ...     dates,
        ...     [[2.0 * value for value in benchmark], benchmark],
        ...     ["FUND", "BENCH"],
        ...     benchmark_ticker="BENCH",
        ...     frequency="monthly",
        ... )
        >>> round(GreeksResult.from_json(perf.greeks()[0].to_json()).alpha, 12)
        0.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
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
    Rolling alpha and beta time series.

    Examples
    --------
    >>> from datetime import date, timedelta
    >>> from finstack_quant.analytics import Performance
    >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(6)]
    >>> benchmark = [-0.02, -0.01, 0.0, 0.01, 0.02, 0.03]
    >>> perf = Performance.from_returns_arrays(
    ...     dates,
    ...     [[2.0 * value for value in benchmark], benchmark],
    ...     ["FUND", "BENCH"],
    ...     benchmark_ticker="BENCH",
    ...     frequency="monthly",
    ... )
    >>> rolling = perf.rolling_greeks(0, window=3)
    >>> len(rolling.dates) == len(rolling.betas)
    True
    """

    @staticmethod
    def from_json(json: str) -> RollingGreeks:
        """
        Deserialize a ``RollingGreeks`` from JSON.

        Parameters
        ----------
        json : str
            Complete JSON string produced by :meth:`to_json`; required metadata must be present and unknown fields are rejected.

        Returns
        -------
        RollingGreeks
            Parsed ``RollingGreeks`` instance.

        Raises
        ------
        ValueError
            If ``json`` is malformed, omits required result fields, contains unknown fields, or does not satisfy the serialized schema.

        Examples
        --------
        >>> from datetime import date, timedelta
        >>> from finstack_quant.analytics import RollingGreeks, Performance
        >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(6)]
        >>> benchmark = [-0.02, -0.01, 0.0, 0.01, 0.02, 0.03]
        >>> perf = Performance.from_returns_arrays(
        ...     dates,
        ...     [[2.0 * value for value in benchmark], benchmark],
        ...     ["FUND", "BENCH"],
        ...     benchmark_ticker="BENCH",
        ...     frequency="monthly",
        ... )
        >>> rolling = RollingGreeks.from_json(perf.rolling_greeks(0, window=3).to_json())
        >>> len(rolling.dates) == len(rolling.betas)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
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
    Multi-factor regression result.

    Examples
    --------
    >>> from datetime import date, timedelta
    >>> from finstack_quant.analytics import Performance
    >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(6)]
    >>> factor = [-0.02, -0.01, 0.0, 0.01, 0.02, 0.03]
    >>> perf = Performance.from_returns_arrays(
    ...     dates, [[2.0 * value for value in factor]], ["FUND"], frequency="monthly"
    ... )
    >>> round(float(perf.multi_factor_greeks(0, [factor]).betas[0]), 1)
    2.0
    """

    @staticmethod
    def from_json(json: str) -> MultiFactorResult:
        """
        Deserialize a ``MultiFactorResult`` from JSON.

        Parameters
        ----------
        json : str
            Complete JSON string produced by :meth:`to_json`; required metadata must be present and unknown fields are rejected.

        Returns
        -------
        MultiFactorResult
            Parsed ``MultiFactorResult`` instance.

        Raises
        ------
        ValueError
            If ``json`` is malformed, omits required result fields, contains unknown fields, or does not satisfy the serialized schema.

        Examples
        --------
        >>> from datetime import date, timedelta
        >>> from finstack_quant.analytics import MultiFactorResult, Performance
        >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(6)]
        >>> factor = [-0.02, -0.01, 0.0, 0.01, 0.02, 0.03]
        >>> perf = Performance.from_returns_arrays(
        ...     dates, [[2.0 * value for value in factor]], ["FUND"], frequency="monthly"
        ... )
        >>> fitted = MultiFactorResult.from_json(perf.multi_factor_greeks(0, [factor]).to_json())
        >>> round(float(fitted.betas[0]), 1)
        2.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def alpha(self) -> float:
        """
        Annualized OLS intercept of the (possibly rf-adjusted) dependent series.

        For ``return_kind="excess"`` this is the intercept of already-excess
        ``y``. For ``return_kind="total"`` it is Jensen-style after subtracting
        the decompounded period risk-free rate from ``y`` only.

        Returns
        -------
        float
            Annualized regression intercept.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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
        Residual (idiosyncratic) volatility of the multi-factor fit.

        Returns
        -------
        float
            Standard deviation of regression residuals.

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
    A single drawdown episode with timing and depth information.

    Examples
    --------
    >>> from datetime import date, timedelta
    >>> from finstack_quant.analytics import Performance
    >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(5)]
    >>> perf = Performance.from_arrays(dates, [[100.0, 90.0, 95.0, 80.0, 100.0]], ["FUND"])
    >>> round(perf.drawdown_details(0, n=1)[0].max_drawdown, 1)
    -0.2
    """

    @staticmethod
    def from_json(json: str) -> DrawdownEpisode:
        """
        Deserialize a ``DrawdownEpisode`` from JSON.

        Parameters
        ----------
        json : str
            Complete JSON string produced by :meth:`to_json`; required metadata must be present and unknown fields are rejected.

        Returns
        -------
        DrawdownEpisode
            Parsed ``DrawdownEpisode`` instance.

        Raises
        ------
        ValueError
            If ``json`` is malformed, omits required result fields, contains unknown fields, or does not satisfy the serialized schema.

        Examples
        --------
        >>> from datetime import date, timedelta
        >>> from finstack_quant.analytics import DrawdownEpisode, Performance
        >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(5)]
        >>> perf = Performance.from_arrays(dates, [[100.0, 90.0, 95.0, 80.0, 100.0]], ["FUND"])
        >>> episode = perf.drawdown_details(0, n=1)[0]
        >>> round(DrawdownEpisode.from_json(episode.to_json()).max_drawdown, 1)
        -0.2
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
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

    @property
    def near_recovery_threshold(self) -> float:
        """
        Near-recovery threshold.

        Returns
        -------
        float
            Price level that would signal near-recovery.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def truncated_at_start(self) -> bool:
        """
        True when the episode began before the first observation (left-censored).

        Returns
        -------
        bool
            ``True`` if the drawdown started before the available data window.

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
            while still in drawdown), ``duration_days``, ``max_drawdown``,
            ``near_recovery_threshold``, ``truncated_at_start``. Stack
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
    Period-to-date returns for each ticker.

    Examples
    --------
    >>> from datetime import date, timedelta
    >>> from finstack_quant.analytics import Performance
    >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(5)]
    >>> perf = Performance.from_arrays(dates, [[100.0, 90.0, 95.0, 80.0, 100.0]], ["FUND"])
    >>> len(perf.lookback_returns(date(2024, 1, 5)).fytd)
    1
    """

    @staticmethod
    def from_json(json: str) -> LookbackReturns:
        """
        Deserialize a ``LookbackReturns`` from JSON.

        Parameters
        ----------
        json : str
            Complete JSON string produced by :meth:`to_json`; required metadata must be present and unknown fields are rejected.

        Returns
        -------
        LookbackReturns
            Parsed ``LookbackReturns`` instance.

        Raises
        ------
        ValueError
            If ``json`` is malformed, omits required result fields, contains unknown fields, or does not satisfy the serialized schema.

        Examples
        --------
        >>> from datetime import date, timedelta
        >>> from finstack_quant.analytics import LookbackReturns, Performance
        >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(5)]
        >>> perf = Performance.from_arrays(dates, [[100.0, 90.0, 95.0, 80.0, 100.0]], ["FUND"])
        >>> lookback = perf.lookback_returns(date(2024, 1, 5))
        >>> len(LookbackReturns.from_json(lookback.to_json()).mtd)
        1
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
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
    Date-indexed numeric series returned by the rolling-window analytics.

    Rolling-window methods return this shared carrier. It wraps Rust
    ``DatedSeries``, whose ``value_column`` (a Rust ``RollingMetric``) names
    the metric; WASM rolling methods return the same ``{values, dates,
    value_column}`` object.

    Examples
    --------
    >>> from datetime import date, timedelta
    >>> from finstack_quant.analytics import Performance
    >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(5)]
    >>> perf = Performance.from_arrays(dates, [[100.0, 90.0, 95.0, 80.0, 100.0]], ["FUND"])
    >>> series = perf.rolling_returns(0, window=2)
    >>> (len(series.values) == len(series.dates), series.value_column)
    (True, 'return')
    """

    @staticmethod
    def from_json(json: str) -> DatedSeries:
        """
        Deserialize a ``DatedSeries`` from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by :meth:`to_json`, carrying ``values``,
            ``dates`` and ``value_column``.

        Returns
        -------
        DatedSeries
            Parsed ``DatedSeries`` instance, including its metric label.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not satisfy the serialized schema,
            including a ``value_column`` other than ``"volatility"``,
            ``"sortino"``, ``"sharpe"`` or ``"return"``.

        Examples
        --------
        >>> from datetime import date, timedelta
        >>> from finstack_quant.analytics import DatedSeries, Performance
        >>> dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(5)]
        >>> perf = Performance.from_arrays(dates, [[100.0, 90.0, 95.0, 80.0, 100.0]], ["FUND"])
        >>> series = DatedSeries.from_json(perf.rolling_returns(0, window=2).to_json())
        >>> series.value_column
        'return'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Emits ``values`` and ``dates`` plus the ``value_column`` label, so the
        metric name survives a round-trip.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
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
