"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/correlation/latent.rs``."""

from __future__ import annotations

from typing import Any, Sequence
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable

class LatentFactorSpec:
    """
    Factor model specification for configuration and deferred construction.

    Example
    -------
    >>> from finstack_quant.models.correlation import LatentFactorSpec
    >>> spec = LatentFactorSpec.single_factor(0.2, 0.05)
    >>> model = spec.build()
    >>> model.num_factors
    1

    Examples
    --------
    >>> from finstack_quant.models.correlation import LatentFactorSpec
    >>> spec = LatentFactorSpec.single_factor(0.2, 0.05)
    >>> (spec.num_factors, spec.build().model_name)
    (1, 'Single Factor Model')

    """

    @classmethod
    def single_factor(cls, volatility: float, mean_reversion: float) -> LatentFactorSpec:
        """
        Single-factor model specification.

        Parameters
        ----------
        volatility : float
            Finite, non-negative decimal volatility of the single latent factor; no upper cap.
        mean_reversion : float
            Finite, non-negative mean-reversion speed per year; no upper cap.

        Returns
        -------
        LatentFactorSpec
            Single-factor specification.

        Raises
        ------
        ValueError
            If the volatility or mean-reversion speed is negative or non-finite.

        Examples
        --------
        >>> from finstack_quant.models.correlation import LatentFactorSpec
        >>> model = LatentFactorSpec.single_factor(0.2, 0.05).build()
        >>> (model.num_factors, model.factor_names)
        (1, ['Market'])
        """
        ...

    @classmethod
    def two_factor(cls, prepay_vol: float, credit_vol: float, correlation: float) -> LatentFactorSpec:
        """
        Two-factor model (prepayment + credit) specification.

        Parameters
        ----------
        prepay_vol : float
            Finite, non-negative decimal prepayment-factor volatility.
        credit_vol : float
            Finite, non-negative decimal credit-factor volatility.
        correlation : float
            Finite Gaussian inter-factor correlation in ``[-1, 1]``, including endpoints.

        Returns
        -------
        LatentFactorSpec
            Two-factor specification.

        Raises
        ------
        ValueError
            If either volatility is negative or non-finite, or the correlation is
            non-finite or outside ``[-1, 1]``.

        Examples
        --------
        >>> from finstack_quant.models.correlation import LatentFactorSpec
        >>> model = LatentFactorSpec.two_factor(0.15, 0.1, -0.2).build()
        >>> (model.num_factors, model.factor_names)
        (2, ['Prepayment', 'Credit'])
        """
        ...

    @property
    def num_factors(self) -> int:
        """
        Number of factors implied by this specification.

        Returns
        -------
        int
            Number of factors implied by this specification.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def build(self) -> LatentFactorKind:
        """
        Build a concrete :class:`LatentFactorKind` from this specification.

        Returns
        -------
        LatentFactorKind
            Concrete factor model.

        Raises
        ------
        ValueError
            If a multi-factor specification contains an invalid volatility
            vector or correlation matrix.
        """
        ...

class LatentFactorKind:
    """
    Concrete factor model for correlated behavior.

    Obtain an instance via :meth:`LatentFactorSpec.build`.

    Examples
    --------
    >>> from finstack_quant.models.correlation import LatentFactorSpec
    >>> model = LatentFactorSpec.single_factor(0.2, 0.05).build()
    >>> (model.model_name, model.volatilities)
    ('Single Factor Model', [0.2])

    """

    @property
    def num_factors(self) -> int:
        """
        Number of factors in the model.

        Returns
        -------
        int
            Number of factors in the model.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def correlation_matrix(self) -> list[float]:
        """
        Factor correlation matrix (flattened row-major).

        Returns
        -------
        list[float]
            Factor correlation matrix (flattened row-major).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def volatilities(self) -> list[float]:
        """
        Annualized volatilities of the latent factors, in factor order.

        Returns
        -------
        list[float]
            Annualized volatilities of the latent factors, in factor order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factor_names(self) -> list[str]:
        """
        Factor names for reporting.

        Returns
        -------
        list[str]
            Factor names for reporting.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model_name(self) -> str:
        """
        Model name for diagnostics.

        Returns
        -------
        str
            Model name for diagnostics.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def diagonal_factor_contribution(self, factor_index: int, z: float) -> float:
        """
        Diagonal factor contribution for a single standard-normal draw.

        Parameters
        ----------
        factor_index : int
            Index of the factor.
        z : float
            Standard normal draw.

        Returns
        -------
        float
            Factor contribution.

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.
        """
        ...

class LatentSingleFactor:
    """
    Single-factor model (common market factor).

    Example
    -------
    >>> from finstack_quant.models.correlation import LatentSingleFactor
    >>> m = LatentSingleFactor(volatility=0.2, mean_reversion=0.05)
    >>> m.num_factors
    1

    Examples
    --------
    >>> from finstack_quant.models.correlation import LatentSingleFactor
    >>> model = LatentSingleFactor(0.2, 0.05)
    >>> (model.volatility, model.mean_reversion, model.num_factors)
    (0.2, 0.05, 1)

    """

    def __init__(self, volatility: float, mean_reversion: float) -> None:
        """
        Create a single-factor model.

        Parameters
        ----------
        volatility : float
            Finite, non-negative decimal volatility of the single latent factor; no upper cap.
        mean_reversion : float
            Finite, non-negative mean-reversion speed per year; no upper cap.

        Raises
        ------
        ValueError
            If a volatility or mean-reversion speed is negative or non-finite.
        """
        ...

    @property
    def volatility(self) -> float:
        """
        Finite, non-negative decimal volatility of the single latent factor; no upper cap.

        Returns
        -------
        float
            Finite, non-negative decimal volatility of the single latent factor; no upper cap.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mean_reversion(self) -> float:
        """
        Finite, non-negative mean-reversion speed per year; no upper cap.

        Returns
        -------
        float
            Finite, non-negative mean-reversion speed per year; no upper cap.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_factors(self) -> int:
        """
        Number of factors (always 1).

        Returns
        -------
        int
            Number of factors (always 1).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class LatentTwoFactor:
    """
    Two-factor model for prepayment and credit.

    Example
    -------
    >>> from finstack_quant.models.correlation import LatentTwoFactor
    >>> m = LatentTwoFactor(prepay_vol=0.15, credit_vol=0.10, correlation=-0.2)
    >>> m.num_factors
    2

    Examples
    --------
    >>> from finstack_quant.models.correlation import LatentTwoFactor
    >>> model = LatentTwoFactor(0.15, 0.1, -0.2)
    >>> (model.prepay_vol, model.credit_vol, model.correlation, model.num_factors)
    (0.15, 0.1, -0.2, 2)

    """

    def __init__(self, prepay_vol: float, credit_vol: float, correlation: float) -> None:
        """
        Create a two-factor model.

        Parameters
        ----------
        prepay_vol : float
            Finite, non-negative decimal prepayment-factor volatility.
        credit_vol : float
            Finite, non-negative decimal credit-factor volatility.
        correlation : float
            Finite Gaussian inter-factor correlation in ``[-1, 1]``, including endpoints.

        Raises
        ------
        ValueError
            If either volatility is negative or non-finite, or correlation is
            non-finite or outside ``[-1, 1]``.
        """
        ...

    @classmethod
    def rmbs_standard(cls) -> LatentTwoFactor:
        """
        Standard RMBS calibration.

        Returns
        -------
        LatentTwoFactor
            Pre-calibrated RMBS model.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.correlation import LatentTwoFactor
        >>> model = LatentTwoFactor.rmbs_standard()
        >>> (model.prepay_vol, model.credit_vol, model.correlation)
        (0.2, 0.25, -0.3)
        """
        ...

    @classmethod
    def clo_standard(cls) -> LatentTwoFactor:
        """
        Standard CLO calibration.

        Returns
        -------
        LatentTwoFactor
            Pre-calibrated CLO model.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.correlation import LatentTwoFactor
        >>> model = LatentTwoFactor.clo_standard()
        >>> (model.prepay_vol, model.credit_vol, model.correlation)
        (0.15, 0.3, -0.2)
        """
        ...

    @property
    def prepay_vol(self) -> float:
        """
        Finite, non-negative decimal prepayment-factor volatility.

        Returns
        -------
        float
            Finite, non-negative decimal prepayment-factor volatility.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def credit_vol(self) -> float:
        """
        Finite, non-negative decimal credit-factor volatility.

        Returns
        -------
        float
            Finite, non-negative decimal credit-factor volatility.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def correlation(self) -> float:
        """
        Correlation between the two latent credit factors.

        Returns
        -------
        float
            Correlation between the two latent credit factors.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_factors(self) -> int:
        """
        Number of factors (always 2).

        Returns
        -------
        int
            Number of factors (always 2).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cholesky_l10(self) -> float:
        """
        Cholesky ``L[1][0]`` for correlated factor generation.

        Returns
        -------
        float
            Cholesky ``L[1][0]`` for correlated factor generation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cholesky_l11(self) -> float:
        """
        Cholesky ``L[1][1]`` for correlated factor generation.

        Returns
        -------
        float
            Cholesky ``L[1][1]`` for correlated factor generation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class LatentMultiFactor:
    """
    Multi-factor model with custom correlation structure.

    Example
    -------
    >>> from finstack_quant.models.correlation import LatentMultiFactor
    >>> m = LatentMultiFactor(
    ...     num_factors=2,
    ...     volatilities=[0.2, 0.15],
    ...     correlations=[1.0, 0.3, 0.3, 1.0],
    ... )
    >>> m.num_factors
    2

    Examples
    --------
    >>> from finstack_quant.models.correlation import LatentMultiFactor
    >>> model = LatentMultiFactor(2, [0.2, 0.15], [1.0, 0.3, 0.3, 1.0])
    >>> (model.num_factors, model.volatilities, model.correlation_matrix)
    (2, [0.2, 0.15], [1.0, 0.3, 0.3, 1.0])

    """

    def __init__(
        self,
        num_factors: int,
        volatilities: Sequence[float],
        correlations: Sequence[float],
    ) -> None:
        """
        Create a validated multi-factor model.

        Parameters
        ----------
        num_factors : int
            Count of latent factors in this multi-factor specification.
        volatilities : list[float]
            Per-factor volatilities (length ``num_factors``).
        correlations : list[float]
            Correlation matrix, flattened row-major (length ``num_factors²``).

        Raises
        ------
        ValueError
            If the correlation matrix is invalid.
        """
        ...

    @classmethod
    def uncorrelated(cls, num_factors: int, volatilities: Sequence[float]) -> LatentMultiFactor:
        """
        Create an uncorrelated (identity) multi-factor model.

        Parameters
        ----------
        num_factors : int
            Positive count of latent factors; zero is rejected.
        volatilities : list[float]
            One finite, non-negative decimal volatility per factor; zero and large
            finite values are preserved without an upper cap.

        Returns
        -------
        LatentMultiFactor
            Uncorrelated factor model.

        Raises
        ------
        ValueError
            If the volatility count does not match the factor count, or a
            volatility is negative or non-finite.

        Examples
        --------
        >>> from finstack_quant.models.correlation import LatentMultiFactor
        >>> model = LatentMultiFactor.uncorrelated(2, [0.2, 0.15])
        >>> model.correlation_matrix
        [1.0, 0.0, 0.0, 1.0]
        """
        ...

    @property
    def num_factors(self) -> int:
        """
        Count of latent factors in this multi-factor specification.

        Returns
        -------
        int
            Count of latent factors in this multi-factor specification.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def correlation_matrix(self) -> list[float]:
        """
        Factor correlation matrix (flattened row-major).

        Returns
        -------
        list[float]
            Factor correlation matrix (flattened row-major).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def volatilities(self) -> list[float]:
        """
        Annualized volatilities of the latent factors, in factor order.

        Returns
        -------
        list[float]
            Annualized volatilities of the latent factors, in factor order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def generate_correlated_factors(self, independent_z: Sequence[float]) -> list[float]:
        """
        Generate correlated factor values from independent standard normal draws.

        Parameters
        ----------
        independent_z : list[float]
            Independent standard normal draws (length ``num_factors``).

        Returns
        -------
        list[float]
            Correlated factor realizations.

        Raises
        ------
        ValueError
            If ``independent_z`` does not contain exactly ``num_factors``
            draws.
        """
        ...
