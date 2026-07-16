"""Type stubs for ``finstack_quant.valuations.correlation``.

Correlation infrastructure: copulas, factor models, recovery models.
"""

from __future__ import annotations

from typing import Sequence

__all__ = [
    "MAX_PORTFOLIO_LOSS_PATHS",
    "CopulaSpec",
    "Copula",
    "CreditExposure",
    "PortfolioLossConfig",
    "PortfolioLossResult",
    "RecoverySpec",
    "RecoveryModel",
    "LatentFactorSpec",
    "LatentFactorKind",
    "LatentSingleFactor",
    "LatentTwoFactor",
    "LatentMultiFactor",
    "CorrelatedBernoulli",
    "correlation_bounds",
    "joint_probabilities",
    "validate_correlation_matrix",
    "nearest_correlation",
    "cholesky_decompose",
    "simulate_portfolio_loss",
]

MAX_PORTFOLIO_LOSS_PATHS: int

class CopulaSpec:
    """Copula model specification for configuration and deferred construction.

    Use class methods to create a spec, then call :meth:`build` to obtain
    a concrete :class:`Copula` instance.

    Example
    -------
    >>> from finstack_quant.valuations.correlation import CopulaSpec
    >>> spec = CopulaSpec.gaussian()
    >>> copula = spec.build()
    >>> copula.model_name
    'One-Factor Gaussian Copula'
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
    >>> from finstack_quant.valuations.correlation import CopulaSpec
    >>> copula = CopulaSpec.gaussian().build()
    >>> # P(default | Z=0) = norm.cdf(-2.33 / sqrt(1 - 0.3)) ≈ 0.0027,
    >>> # below the unconditional PD of norm.cdf(-2.33) ≈ 0.0099.
    >>> copula.conditional_default_prob(-2.33, [0.0], 0.3)
    0.002...
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
        """Strict lower-tail dependence coefficient ``λ_L`` at the given correlation.

        Returns ``nan`` when the model has no closed-form ``λ_L`` (Random
        Factor Loading); check ``math.isnan()`` before using the result.
        Gaussian and multi-factor Gaussian copulas return ``0.0``; Student-t
        returns the closed-form positive ``λ_L``. For the RFL heuristic
        stress gauge use :meth:`stress_correlation_proxy` instead.

        Parameters
        ----------
        correlation : float
            Asset correlation.

        Returns
        -------
        float
            The strict ``λ_L``, or ``nan`` if the model has no closed form.
        """
        ...

    def stress_correlation_proxy(self, correlation: float) -> float:
        """Heuristic stress-correlation proxy for the Random Factor Loading copula.

        This is **not** the strict copula lower-tail-dependence coefficient
        ``λ_L`` (which has no closed form for RFL — :meth:`tail_dependence`
        returns ``nan``). It gauges the extra correlation mass in the
        high-loading tail and vanishes in the Gaussian (``loading_vol = 0``)
        limit.

        Parameters
        ----------
        correlation : float
            Asset correlation.

        Returns
        -------
        float
            Non-negative stress-correlation proxy.

        Raises
        ------
        ValueError
            If the copula is not a Random Factor Loading copula.
        """
        ...

class CreditExposure:
    """One name in a finite credit portfolio."""

    def __init__(
        self,
        id: str,
        notional: float,
        default_probability: float,
        lgd: float,
        factor_loadings: Sequence[float],
    ) -> None: ...
    @property
    def id(self) -> str: ...
    @property
    def notional(self) -> float: ...
    @property
    def default_probability(self) -> float: ...
    @property
    def lgd(self) -> float: ...
    @property
    def factor_loadings(self) -> list[float]: ...
    def to_json(self) -> str: ...

class PortfolioLossConfig:
    """Settings for deterministic portfolio credit-loss simulation.

    ``num_paths`` must be in ``[1, MAX_PORTFOLIO_LOSS_PATHS]``.
    """

    def __init__(
        self,
        num_paths: int,
        seed: int,
        confidence: float,
        copula: CopulaSpec,
    ) -> None: ...
    @property
    def num_paths(self) -> int: ...
    @property
    def seed(self) -> int: ...
    @property
    def confidence(self) -> float: ...
    @property
    def copula(self) -> CopulaSpec: ...
    def to_json(self) -> str: ...

class PortfolioLossResult:
    """Loss distribution and loss-positive VaR/expected shortfall."""

    @property
    def losses(self) -> list[float]: ...
    @property
    def expected_loss(self) -> float: ...
    @property
    def var(self) -> float: ...
    @property
    def expected_shortfall(self) -> float: ...
    def to_json(self) -> str: ...

class RecoverySpec:
    """Recovery model specification for configuration and deferred construction.

    Example
    -------
    >>> from finstack_quant.valuations.correlation import RecoverySpec
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
        """Market-standard stochastic recovery (40% mean, 25% vol, +40% corr).

        Recovery falls in stress under the canonical low-factor-stress
        convention.

        Returns
        -------
        RecoverySpec
            Standard stochastic recovery specification.
        """
        ...

    @property
    def expected_recovery(self) -> float:
        """Location-parameter recovery rate of this spec.

        For a constant spec this is the constant rate. For a
        market-correlated spec this returns the ``mean`` input — the target
        recovery at factor ``Z = 0`` — which differs from the
        Jensen-corrected unconditional mean ``E_Z[R(Z)]`` whenever the
        factor sensitivity is non-zero. For the true unconditional mean call
        ``build().expected_recovery``.
        """
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
    >>> from finstack_quant.valuations.correlation import LatentFactorSpec
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

    def build(self) -> LatentFactorKind:
        """Build a concrete :class:`LatentFactorKind` from this specification.

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
    >>> from finstack_quant.valuations.correlation import LatentSingleFactor
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
    >>> from finstack_quant.valuations.correlation import LatentTwoFactor
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
    >>> from finstack_quant.valuations.correlation import LatentMultiFactor
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

        Raises
        ------
        ValueError
            If ``independent_z`` does not contain exactly ``num_factors``
            draws.
        """
        ...

class CorrelatedBernoulli:
    """Correlated Bernoulli distribution for two binary events.

    Example
    -------
    >>> from finstack_quant.valuations.correlation import CorrelatedBernoulli
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
            Desired finite correlation in ``[-1, 1]``. Values inside that
            domain but outside the feasible Fréchet-Hoeffding interval are
            clamped to the nearest feasible bound.

        Raises
        ------
        ValueError
            If a marginal is not finite and in ``[0, 1]`` or correlation is
            not finite and in ``[-1, 1]``.
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
        """Effective correlation after Fréchet-Hoeffding clamping."""
        ...

    @property
    def requested_correlation(self) -> float:
        """Caller-requested correlation before Fréchet-Hoeffding clamping."""
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

        Raises
        ------
        ValueError
            If ``u`` is not finite and in ``[0, 1]``.
        """
        ...

def simulate_portfolio_loss(
    exposures: Sequence[CreditExposure],
    config: PortfolioLossConfig,
    recovery: RecoverySpec | None = None,
) -> PortfolioLossResult:
    """Simulate finite-pool losses with deterministic path-indexed RNG streams.

    Losses are positive amounts. VaR is the nearest-rank empirical quantile at
    ``config.confidence``; expected shortfall includes the VaR observation and
    every worse path. If ``recovery`` is provided, its conditional LGD replaces
    each exposure's constant LGD and exactly one systematic factor is required.
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

    Raises
    ------
    ValueError
        If either marginal is not finite and in ``[0, 1]``.
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

    Raises
    ------
    ValueError
        If either marginal is not finite and in ``[0, 1]`` or correlation is
        not finite and in ``[-1, 1]``.
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
    max_iter: int | None = None,
    tol: float | None = None,
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
        Maximum alternating-projection iterations. Defaults to the Rust
        ``NearestCorrelationOpts::default()`` value (currently ``200``).
    tol
        Frobenius-norm tolerance between successive iterates. Defaults to
        the Rust ``NearestCorrelationOpts::default()`` value (currently
        ``1e-10``).

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
    """Pivoted Cholesky decomposition of a correlation matrix (flattened row-major).

    Uses diagonal pivoting to handle near-singular and positive-semidefinite
    matrices gracefully.

    Parameters
    ----------
    matrix : list[float]
        Flattened row-major correlation matrix (length ``n²``).
    n : int
        Dimension of the square matrix.

    Returns
    -------
    list[float]
        Factor matrix ``L`` as a flat list (row-major, original variable
        order) satisfying ``L @ L.T == matrix``. Because of pivoting, the
        unpermuted factor is **not** guaranteed to be lower triangular — it
        may contain non-zero entries above the diagonal. The effective
        numerical rank is not surfaced through this function.

    Raises
    ------
    ValueError
        If the matrix shape is wrong or the matrix is indefinite.
    """
    ...
