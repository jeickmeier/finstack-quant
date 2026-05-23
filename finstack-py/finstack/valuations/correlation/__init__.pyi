"""Type stubs for ``finstack.valuations.correlation``.

Correlation infrastructure: copulas, factor models, recovery models.
"""

from __future__ import annotations

from typing import Sequence

__all__ = [
    "CopulaSpec",
    "Copula",
    "RecoverySpec",
    "RecoveryModel",
    "LatentFactorSpec",
    "LatentFactor",
    "LatentSingleFactor",
    "LatentTwoFactor",
    "LatentMultiFactor",
    "CorrelatedBernoulli",
    "correlation_bounds",
    "joint_probabilities",
    "validate_correlation_matrix",
    "nearest_correlation",
    "cholesky_decompose",
]

class CopulaSpec:
    """Copula model specification for configuration and deferred construction.

    Use class methods to create a spec, then call :meth:`build` to obtain
    a concrete :class:`Copula` instance.

    Example
    -------
    >>> from finstack.valuations.correlation import CopulaSpec
    >>> spec = CopulaSpec.gaussian()
    >>> copula = spec.build()
    >>> copula.model_name
    'Gaussian'
    """

    @classmethod
    def gaussian(cls) -> CopulaSpec:
        """One-factor Gaussian copula (market standard).

        Returns
        -------
        CopulaSpec
            Gaussian copula specification.
        """
        ...

    @classmethod
    def student_t(cls, df: float) -> CopulaSpec:
        """Student-t copula with specified degrees of freedom.

        Parameters
        ----------
        df : float
            Degrees of freedom (must be > 2 for finite variance).
            Typical calibration range for CDX tranches is 4–10.

        Returns
        -------
        CopulaSpec
            Student-t copula specification.

        Raises
        ------
        ValueError
            If ``df`` is not finite or is ``<= 2``.
        """
        ...

    @classmethod
    def random_factor_loading(cls, loading_vol: float) -> CopulaSpec:
        """Random Factor Loading copula with stochastic correlation.

        Parameters
        ----------
        loading_vol : float
            Volatility of the factor loading, clamped to ``[0, 0.5]``.

        Returns
        -------
        CopulaSpec
            RFL copula specification.
        """
        ...

    @classmethod
    def multi_factor(cls, num_factors: int) -> CopulaSpec:
        """Multi-factor Gaussian copula with sector structure.

        Parameters
        ----------
        num_factors : int
            Number of systematic factors.

        Returns
        -------
        CopulaSpec
            Multi-factor copula specification.
        """
        ...

    def build(self) -> Copula:
        """Build a concrete :class:`Copula` from this specification.

        Returns
        -------
        Copula
            Concrete copula model.

        Raises
        ------
        ValueError
            If a deserialized Student-t spec has invalid degrees of freedom.
        """
        ...

    @property
    def is_gaussian(self) -> bool:
        """``True`` if this is a Gaussian spec."""
        ...

    @property
    def is_student_t(self) -> bool:
        """``True`` if this is a Student-t spec."""
        ...

    @property
    def is_rfl(self) -> bool:
        """``True`` if this is a Random Factor Loading spec."""
        ...

    @property
    def is_multi_factor(self) -> bool:
        """``True`` if this is a Multi-factor spec."""
        ...

class Copula:
    """Concrete copula model for portfolio default correlation.

    Obtain an instance via :meth:`CopulaSpec.build`.

    Example
    -------
    >>> from finstack.valuations.correlation import CopulaSpec
    >>> copula = CopulaSpec.gaussian().build()
    >>> copula.conditional_default_prob(-2.33, [0.0], 0.3)
    0.009...
    """

    def conditional_default_prob(
        self,
        default_threshold: float,
        factor_realization: Sequence[float],
        correlation: float,
    ) -> float:
        """Conditional default probability given factor realization(s).

        P(default | Z) where the default threshold is typically Φ⁻¹(PD).

        Parameters
        ----------
        default_threshold : float
            Default barrier (e.g. ``norm.ppf(PD)``).
        factor_realization : list[float]
            Systematic factor values.
        correlation : float
            Asset correlation.

        Returns
        -------
        float
            Conditional default probability.
        """
        ...

    @property
    def num_factors(self) -> int:
        """Number of systematic factors in the model."""
        ...

    @property
    def model_name(self) -> str:
        """Model name for diagnostics."""
        ...

    def tail_dependence(self, correlation: float) -> float:
        """Lower-tail dependence coefficient (or proxy) at the given correlation.

        Parameters
        ----------
        correlation : float
            Asset correlation.

        Returns
        -------
        float
            Tail dependence measure.
        """
        ...

class RecoverySpec:
    """Recovery model specification for configuration and deferred construction.

    Example
    -------
    >>> from finstack.valuations.correlation import RecoverySpec
    >>> spec = RecoverySpec.constant(0.4)
    >>> model = spec.build()
    >>> model.expected_recovery
    0.4
    """

    @classmethod
    def constant(cls, rate: float) -> RecoverySpec:
        """Constant recovery rate.

        Parameters
        ----------
        rate : float
            Fixed recovery rate in ``[0, 1]``.

        Returns
        -------
        RecoverySpec
            Constant recovery specification.
        """
        ...

    @classmethod
    def market_correlated(cls, mean: float, vol: float, correlation: float) -> RecoverySpec:
        """Market-correlated (Andersen-Sidenius) stochastic recovery.

        Parameters
        ----------
        mean : float
            Expected recovery rate.
        vol : float
            Recovery rate volatility.
        correlation : float
            Correlation with market factor.

        Returns
        -------
        RecoverySpec
            Stochastic recovery specification.
        """
        ...

    @classmethod
    def market_standard_stochastic(cls) -> RecoverySpec:
        """Market-standard stochastic recovery (40% mean, 25% vol, −40% corr).

        Returns
        -------
        RecoverySpec
            Standard stochastic recovery specification.
        """
        ...

    @property
    def expected_recovery(self) -> float:
        """Expected (unconditional) recovery rate implied by this spec."""
        ...

    def build(self) -> RecoveryModel:
        """Build a concrete :class:`RecoveryModel` from this specification.

        Returns
        -------
        RecoveryModel
            Concrete recovery model.
        """
        ...

class RecoveryModel:
    """Concrete recovery model for credit portfolio pricing.

    Obtain an instance via :meth:`RecoverySpec.build`.
    """

    @property
    def expected_recovery(self) -> float:
        """Expected (unconditional) recovery rate."""
        ...

    def conditional_recovery(self, market_factor: float) -> float:
        """Recovery conditional on the systematic market factor.

        Parameters
        ----------
        market_factor : float
            Realization of the market factor.

        Returns
        -------
        float
            Conditional recovery rate.
        """
        ...

    @property
    def lgd(self) -> float:
        """Loss given default (1 − recovery)."""
        ...

    def conditional_lgd(self, market_factor: float) -> float:
        """Conditional LGD given market factor.

        Parameters
        ----------
        market_factor : float
            Realization of the market factor.

        Returns
        -------
        float
            Conditional LGD.
        """
        ...

    @property
    def recovery_volatility(self) -> float:
        """Recovery-rate volatility scale (0 for constant models)."""
        ...

    @property
    def is_stochastic(self) -> bool:
        """Whether recovery varies with the market factor."""
        ...

    @property
    def model_name(self) -> str:
        """Model name for diagnostics."""
        ...

class LatentFactorSpec:
    """Factor model specification for configuration and deferred construction.

    Example
    -------
    >>> from finstack.valuations.correlation import LatentFactorSpec
    >>> spec = LatentFactorSpec.single_factor(0.2, 0.05)
    >>> model = spec.build()
    >>> model.num_factors
    1
    """

    @classmethod
    def single_factor(cls, volatility: float, mean_reversion: float) -> LatentFactorSpec:
        """Single-factor model specification.

        Parameters
        ----------
        volatility : float
            Factor volatility.
        mean_reversion : float
            Mean reversion speed.

        Returns
        -------
        LatentFactorSpec
            Single-factor specification.
        """
        ...

    @classmethod
    def two_factor(cls, prepay_vol: float, credit_vol: float, correlation: float) -> LatentFactorSpec:
        """Two-factor model (prepayment + credit) specification.

        Parameters
        ----------
        prepay_vol : float
            Prepayment factor volatility.
        credit_vol : float
            Credit factor volatility.
        correlation : float
            Inter-factor correlation.

        Returns
        -------
        LatentFactorSpec
            Two-factor specification.
        """
        ...

    @property
    def num_factors(self) -> int:
        """Number of factors implied by this specification."""
        ...

    def build(self) -> LatentFactor:
        """Build a concrete :class:`LatentFactor` from this specification.

        Returns
        -------
        LatentFactor
            Concrete factor model.

        Raises
        ------
        ValueError
            If a multi-factor specification contains an invalid volatility
            vector or correlation matrix.
        """
        ...

class LatentFactor:
    """Concrete factor model for correlated behavior.

    Obtain an instance via :meth:`LatentFactorSpec.build`.
    """

    @property
    def num_factors(self) -> int:
        """Number of factors in the model."""
        ...

    @property
    def correlation_matrix(self) -> list[float]:
        """Factor correlation matrix (flattened row-major)."""
        ...

    @property
    def volatilities(self) -> list[float]:
        """Factor volatilities."""
        ...

    @property
    def factor_names(self) -> list[str]:
        """Factor names for reporting."""
        ...

    @property
    def model_name(self) -> str:
        """Model name for diagnostics."""
        ...

    def diagonal_factor_contribution(self, factor_index: int, z: float) -> float:
        """Diagonal factor contribution for a single standard-normal draw.

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
        """
        ...

class LatentSingleFactor:
    """Single-factor model (common market factor).

    Example
    -------
    >>> from finstack.valuations.correlation import LatentSingleFactor
    >>> m = LatentSingleFactor(volatility=0.2, mean_reversion=0.05)
    >>> m.num_factors
    1
    """

    def __init__(self, volatility: float, mean_reversion: float) -> None:
        """Create a single-factor model.

        Parameters
        ----------
        volatility : float
            Factor volatility.
        mean_reversion : float
            Mean reversion speed.
        """
        ...

    @property
    def volatility(self) -> float:
        """Factor volatility."""
        ...

    @property
    def mean_reversion(self) -> float:
        """Mean reversion speed."""
        ...

    @property
    def num_factors(self) -> int:
        """Number of factors (always 1)."""
        ...

class LatentTwoFactor:
    """Two-factor model for prepayment and credit.

    Example
    -------
    >>> from finstack.valuations.correlation import LatentTwoFactor
    >>> m = LatentTwoFactor(prepay_vol=0.15, credit_vol=0.10, correlation=-0.2)
    >>> m.num_factors
    2
    """

    def __init__(self, prepay_vol: float, credit_vol: float, correlation: float) -> None:
        """Create a two-factor model.

        Parameters
        ----------
        prepay_vol : float
            Prepayment factor volatility.
        credit_vol : float
            Credit factor volatility.
        correlation : float
            Inter-factor correlation.
        """
        ...

    @classmethod
    def rmbs_standard(cls) -> LatentTwoFactor:
        """Standard RMBS calibration.

        Returns
        -------
        LatentTwoFactor
            Pre-calibrated RMBS model.
        """
        ...

    @classmethod
    def clo_standard(cls) -> LatentTwoFactor:
        """Standard CLO calibration.

        Returns
        -------
        LatentTwoFactor
            Pre-calibrated CLO model.
        """
        ...

    @property
    def prepay_vol(self) -> float:
        """Prepayment factor volatility."""
        ...

    @property
    def credit_vol(self) -> float:
        """Credit factor volatility."""
        ...

    @property
    def correlation(self) -> float:
        """Factor correlation."""
        ...

    @property
    def num_factors(self) -> int:
        """Number of factors (always 2)."""
        ...

    @property
    def cholesky_l10(self) -> float:
        """Cholesky ``L[1][0]`` for correlated factor generation."""
        ...

    @property
    def cholesky_l11(self) -> float:
        """Cholesky ``L[1][1]`` for correlated factor generation."""
        ...

class LatentMultiFactor:
    """Multi-factor model with custom correlation structure.

    Example
    -------
    >>> from finstack.valuations.correlation import LatentMultiFactor
    >>> m = LatentMultiFactor(
    ...     num_factors=2,
    ...     volatilities=[0.2, 0.15],
    ...     correlations=[1.0, 0.3, 0.3, 1.0],
    ... )
    >>> m.num_factors
    2
    """

    def __init__(
        self,
        num_factors: int,
        volatilities: Sequence[float],
        correlations: Sequence[float],
    ) -> None:
        """Create a validated multi-factor model.

        Parameters
        ----------
        num_factors : int
            Number of factors.
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
        """Create an uncorrelated (identity) multi-factor model.

        Parameters
        ----------
        num_factors : int
            Number of factors.
        volatilities : list[float]
            Per-factor volatilities.

        Returns
        -------
        LatentMultiFactor
            Uncorrelated factor model.
        """
        ...

    @property
    def num_factors(self) -> int:
        """Number of factors."""
        ...

    @property
    def correlation_matrix(self) -> list[float]:
        """Factor correlation matrix (flattened row-major)."""
        ...

    @property
    def volatilities(self) -> list[float]:
        """Factor volatilities."""
        ...

    def generate_correlated_factors(self, independent_z: Sequence[float]) -> list[float]:
        """Generate correlated factor values from independent standard normal draws.

        Parameters
        ----------
        independent_z : list[float]
            Independent standard normal draws (length ``num_factors``).

        Returns
        -------
        list[float]
            Correlated factor realizations.
        """
        ...

class CorrelatedBernoulli:
    """Correlated Bernoulli distribution for two binary events.

    Example
    -------
    >>> from finstack.valuations.correlation import CorrelatedBernoulli
    >>> cb = CorrelatedBernoulli(p1=0.05, p2=0.03, correlation=0.3)
    >>> cb.joint_p11  # P(both default)
    0.00...
    """

    def __init__(self, p1: float, p2: float, correlation: float) -> None:
        """Create a correlated Bernoulli distribution.

        Correlation is clamped to the Fréchet-Hoeffding bounds for the
        given marginal probabilities.

        Parameters
        ----------
        p1 : float
            Marginal probability of event 1.
        p2 : float
            Marginal probability of event 2.
        correlation : float
            Desired correlation between events.
        """
        ...

    @property
    def p1(self) -> float:
        """Marginal probability of event 1."""
        ...

    @property
    def p2(self) -> float:
        """Marginal probability of event 2."""
        ...

    @property
    def correlation(self) -> float:
        """Correlation between events."""
        ...

    @property
    def joint_p11(self) -> float:
        """P(X₁=1, X₂=1)."""
        ...

    @property
    def joint_p10(self) -> float:
        """P(X₁=1, X₂=0)."""
        ...

    @property
    def joint_p01(self) -> float:
        """P(X₁=0, X₂=1)."""
        ...

    @property
    def joint_p00(self) -> float:
        """P(X₁=0, X₂=0)."""
        ...

    def joint_probabilities(self) -> tuple[float, float, float, float]:
        """All four joint probabilities ``(p11, p10, p01, p00)``.

        Returns
        -------
        tuple[float, float, float, float]
            ``(p11, p10, p01, p00)`` summing to 1.
        """
        ...

    def conditional_p2_given_x1(self) -> float:
        """Conditional probability P(X₂=1 | X₁=1).

        Returns
        -------
        float
            Conditional probability.
        """
        ...

    def conditional_p1_given_x2(self) -> float:
        """Conditional probability P(X₁=1 | X₂=1).

        Returns
        -------
        float
            Conditional probability.
        """
        ...

    def sample_from_uniform(self, u: float) -> tuple[int, int]:
        """Sample a pair of correlated binary outcomes from a uniform ``[0,1]`` draw.

        Parameters
        ----------
        u : float
            Uniform random variate in ``[0, 1]``.

        Returns
        -------
        tuple[int, int]
            ``(x1, x2)`` where each is 0 or 1.
        """
        ...

def correlation_bounds(p1: float, p2: float) -> tuple[float, float]:
    """Fréchet-Hoeffding correlation bounds for two Bernoulli marginals.

    Parameters
    ----------
    p1 : float
        Marginal probability of event 1.
    p2 : float
        Marginal probability of event 2.

    Returns
    -------
    tuple[float, float]
        ``(rho_min, rho_max)`` — the feasible correlation range.
    """
    ...

def joint_probabilities(p1: float, p2: float, correlation: float) -> tuple[float, float, float, float]:
    """Joint probabilities for two correlated Bernoulli variables.

    Parameters
    ----------
    p1 : float
        Marginal probability of event 1.
    p2 : float
        Marginal probability of event 2.
    correlation : float
        Desired correlation.

    Returns
    -------
    tuple[float, float, float, float]
        ``(p11, p10, p01, p00)`` that sums to 1 and preserves marginals.
    """
    ...

def validate_correlation_matrix(matrix: Sequence[float], n: int) -> None:
    """Validate a correlation matrix (flattened row-major).

    Parameters
    ----------
    matrix : list[float]
        Flattened row-major correlation matrix (length ``n²``).
    n : int
        Dimension of the square matrix.

    Raises
    ------
    ValueError
        If the matrix is invalid (not symmetric, not PSD, etc.).
    """
    ...

def nearest_correlation(
    matrix: Sequence[float],
    n: int,
    max_iter: int = 200,
    tol: float = 1e-10,
) -> list[float]:
    """Nearest correlation matrix (Higham 2002) for a near-PSD input.

    Projects a symmetric, unit-diagonal, near-PSD matrix onto the set of valid
    correlation matrices (symmetric, unit diagonal, positive semi-definite)
    in Frobenius norm. Gross input violations raise rather than being
    silently reshaped.

    Parameters
    ----------
    matrix
        Flattened row-major ``n x n`` input matrix.
    n
        Matrix dimension.
    max_iter
        Maximum alternating-projection iterations.
    tol
        Frobenius-norm tolerance between successive iterates.

    Returns
    -------
    list[float]
        Flattened row-major ``n x n`` correlation matrix.

    Raises
    ------
    ValueError
        If the input is not square, is grossly asymmetric, the diagonal is
        far from 1, or the projection does not converge.
    """
    ...

def cholesky_decompose(matrix: Sequence[float], n: int) -> list[float]:
    """Cholesky decomposition of a correlation matrix (flattened row-major).

    Parameters
    ----------
    matrix : list[float]
        Flattened row-major correlation matrix (length ``n²``).
    n : int
        Dimension of the square matrix.

    Returns
    -------
    list[float]
        Lower-triangular factor L as a flat list (row-major).

    Raises
    ------
    ValueError
        If the matrix is invalid.
    """
    ...
