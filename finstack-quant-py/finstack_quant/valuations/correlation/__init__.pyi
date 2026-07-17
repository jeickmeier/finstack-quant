"""
Type stubs for ``finstack_quant.valuations.correlation``.

Correlation infrastructure: copulas, factor models, recovery models.

Examples
--------
>>> import finstack_quant.valuations.correlation as correlation
>>> correlation.__name__
'finstack_quant.valuations.correlation'
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
    """
    Copula model specification for configuration and deferred construction.

    Use class methods to create a spec, then call :meth:`build` to obtain
    a concrete :class:`Copula` instance.

    Example
    -------
    >>> from finstack_quant.valuations.correlation import CopulaSpec
    >>> spec = CopulaSpec.gaussian()
    >>> copula = spec.build()
    >>> copula.model_name
    'One-Factor Gaussian Copula'

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import CopulaSpec
    >>> CopulaSpec.__name__
    'CopulaSpec'
    """

    @classmethod
    def gaussian(cls) -> CopulaSpec:
        """
        One-factor Gaussian copula (market standard).

        Returns
        -------
        CopulaSpec
            Gaussian copula specification.

        Examples
        --------
        >>> from finstack_quant.valuations.correlation import CopulaSpec
        >>> callable(CopulaSpec.gaussian)
        True
        """
        ...

    @classmethod
    def student_t(cls, df: float) -> CopulaSpec:
        """
        Student-t copula with specified degrees of freedom.

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

        Examples
        --------
        >>> from finstack_quant.valuations.correlation import CopulaSpec
        >>> callable(CopulaSpec.student_t)
        True
        """
        ...

    @classmethod
    def random_factor_loading(cls, loading_vol: float) -> CopulaSpec:
        """
        Random Factor Loading copula with stochastic correlation.

        Parameters
        ----------
        loading_vol : float
            Volatility of the factor loading, clamped to ``[0, 0.5]``.

        Returns
        -------
        CopulaSpec
            RFL copula specification.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.valuations.correlation import CopulaSpec
        >>> callable(CopulaSpec.random_factor_loading)
        True
        """
        ...

    @classmethod
    def multi_factor(cls, num_factors: int) -> CopulaSpec:
        """
        Multi-factor Gaussian copula with sector structure.

        Parameters
        ----------
        num_factors : int
            Number of systematic factors.

        Returns
        -------
        CopulaSpec
            Multi-factor copula specification.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.valuations.correlation import CopulaSpec
        >>> callable(CopulaSpec.multi_factor)
        True
        """
        ...

    def build(self) -> Copula:
        """
        Build a concrete :class:`Copula` from this specification.

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
        """
        ``True`` if this is a Gaussian spec.

        Returns
        -------
        bool
            Whether gaussian holds for this `CopulaSpec`.
        """
        ...

    @property
    def is_student_t(self) -> bool:
        """
        ``True`` if this is a Student-t spec.

        Returns
        -------
        bool
            Whether student t holds for this `CopulaSpec`.
        """
        ...

    @property
    def is_rfl(self) -> bool:
        """
        ``True`` if this is a Random Factor Loading spec.

        Returns
        -------
        bool
            Whether rfl holds for this `CopulaSpec`.
        """
        ...

    @property
    def is_multi_factor(self) -> bool:
        """
        ``True`` if this is a Multi-factor spec.

        Returns
        -------
        bool
            Whether multi factor holds for this `CopulaSpec`.
        """
        ...

class Copula:
    """
    Concrete copula model for portfolio default correlation.

    Obtain an instance via :meth:`CopulaSpec.build`.

    Example
    -------
    >>> from finstack_quant.valuations.correlation import CopulaSpec
    >>> copula = CopulaSpec.gaussian().build()
    >>> # P(default | Z=0) = norm.cdf(-2.33 / sqrt(1 - 0.3)) ≈ 0.0027,
    >>> # below the unconditional PD of norm.cdf(-2.33) ≈ 0.0099.
    >>> copula.conditional_default_prob(-2.33, [0.0], 0.3)
    0.002...

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import Copula
    >>> Copula.__name__
    'Copula'
    """

    def conditional_default_prob(
        self,
        default_threshold: float,
        factor_realization: Sequence[float],
        correlation: float,
    ) -> float:
        """
        Conditional default probability given factor realization(s).

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

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def num_factors(self) -> int:
        """
        Number of systematic factors in the model.

        Returns
        -------
        int
            The num factors exposed by this `Copula`.
        """
        ...

    @property
    def model_name(self) -> str:
        """
        Model name for diagnostics.

        Returns
        -------
        str
            The model name exposed by this `Copula`.
        """
        ...

    def tail_dependence(self, correlation: float) -> float:
        """
        Strict lower-tail dependence coefficient ``λ_L`` at the given correlation.

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

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def stress_correlation_proxy(self, correlation: float) -> float:
        """
        Heuristic stress-correlation proxy for the Random Factor Loading copula.

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
    """
    One name in a finite credit portfolio.

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import CreditExposure
    >>> CreditExposure.__name__
    'CreditExposure'
    """

    def __init__(
        self,
        id: str,
        notional: float,
        default_probability: float,
        lgd: float,
        factor_loadings: Sequence[float],
    ) -> None:
        """
        Create one obligor exposure for a correlated portfolio-loss simulation.

        Parameters
        ----------
        id : str
            Stable obligor or position identifier retained in simulation output.
        notional : float
            Positive exposure-at-default amount in the portfolio loss currency.
        default_probability : float
            Marginal default probability over the simulation horizon in ``[0, 1]``.
        lgd : float
            Constant loss-given-default fraction in ``[0, 1]`` when no recovery
            model overrides it.
        factor_loadings : Sequence[float]
            Systematic-factor sensitivities aligned with the selected copula's
            factor dimensions.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...
    @property
    def id(self) -> str:
        """
        Return the id for `CreditExposure`.

        Returns
        -------
        str
            The id exposed by this `CreditExposure`.
        """
        ...

    @property
    def notional(self) -> float:
        """
        Return the notional for `CreditExposure`.

        Returns
        -------
        float
            The notional exposed by this `CreditExposure`.
        """
        ...

    @property
    def default_probability(self) -> float:
        """
        Return the default probability for `CreditExposure`.

        Returns
        -------
        float
            The default probability exposed by this `CreditExposure`.
        """
        ...

    @property
    def lgd(self) -> float:
        """
        Return the lgd for `CreditExposure`.

        Returns
        -------
        float
            The lgd exposed by this `CreditExposure`.
        """
        ...

    @property
    def factor_loadings(self) -> list[float]:
        """
        Return the factor loadings for `CreditExposure`.

        Returns
        -------
        list[float]
            The factor loadings exposed by this `CreditExposure`.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize `CreditExposure` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `CreditExposure`, suitable for a matching `from_json` call.
        """
        ...

class PortfolioLossConfig:
    """
    Settings for deterministic portfolio credit-loss simulation.

    ``num_paths`` must be in ``[1, MAX_PORTFOLIO_LOSS_PATHS]``.

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import PortfolioLossConfig
    >>> PortfolioLossConfig.__name__
    'PortfolioLossConfig'
    """

    def __init__(
        self,
        num_paths: int,
        seed: int,
        confidence: float,
        copula: CopulaSpec,
    ) -> None:
        """
        Configure deterministic correlated portfolio-loss simulation.

        Parameters
        ----------
        num_paths : int
            Number of deterministic Monte Carlo paths, bounded by the library
            safety limit for finite-pool loss simulation.
        seed : int
            Random seed used to derive stable path-indexed RNG streams.
        confidence : float
            VaR and expected-shortfall confidence level in the open interval
            ``(0, 1)``.
        copula : CopulaSpec
            Dependence model and factor configuration for correlated defaults.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...
    @property
    def num_paths(self) -> int:
        """
        Return the num paths for `PortfolioLossConfig`.

        Returns
        -------
        int
            The num paths exposed by this `PortfolioLossConfig`.
        """
        ...

    @property
    def seed(self) -> int:
        """
        Return the seed for `PortfolioLossConfig`.

        Returns
        -------
        int
            The seed exposed by this `PortfolioLossConfig`.
        """
        ...

    @property
    def confidence(self) -> float:
        """
        Return the confidence for `PortfolioLossConfig`.

        Returns
        -------
        float
            The confidence exposed by this `PortfolioLossConfig`.
        """
        ...

    @property
    def copula(self) -> CopulaSpec:
        """
        Return the copula for `PortfolioLossConfig`.

        Returns
        -------
        CopulaSpec
            The copula exposed by this `PortfolioLossConfig`.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize `PortfolioLossConfig` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioLossConfig`, suitable for a matching `from_json` call.
        """
        ...

class PortfolioLossResult:
    """
    Loss distribution and loss-positive VaR/expected shortfall.

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import PortfolioLossResult
    >>> PortfolioLossResult.__name__
    'PortfolioLossResult'
    """

    @property
    def losses(self) -> list[float]:
        """
        Return the losses for `PortfolioLossResult`.

        Returns
        -------
        list[float]
            The losses exposed by this `PortfolioLossResult`.
        """
        ...

    @property
    def expected_loss(self) -> float:
        """
        Return the expected loss for `PortfolioLossResult`.

        Returns
        -------
        float
            The expected loss exposed by this `PortfolioLossResult`.
        """
        ...

    @property
    def var(self) -> float:
        """
        Return the var for `PortfolioLossResult`.

        Returns
        -------
        float
            The var exposed by this `PortfolioLossResult`.
        """
        ...

    @property
    def expected_shortfall(self) -> float:
        """
        Return the expected shortfall for `PortfolioLossResult`.

        Returns
        -------
        float
            The expected shortfall exposed by this `PortfolioLossResult`.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize `PortfolioLossResult` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioLossResult`, suitable for a matching `from_json` call.
        """
        ...

class RecoverySpec:
    """
    Recovery model specification for configuration and deferred construction.

    Example
    -------
    >>> from finstack_quant.valuations.correlation import RecoverySpec
    >>> spec = RecoverySpec.constant(0.4)
    >>> model = spec.build()
    >>> model.expected_recovery
    0.4

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import RecoverySpec
    >>> RecoverySpec.__name__
    'RecoverySpec'
    """

    @classmethod
    def constant(cls, rate: float) -> RecoverySpec:
        """
        Constant recovery rate.

        Parameters
        ----------
        rate : float
            Fixed recovery rate in ``[0, 1]``.

        Returns
        -------
        RecoverySpec
            Constant recovery specification.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.valuations.correlation import RecoverySpec
        >>> callable(RecoverySpec.constant)
        True
        """
        ...

    @classmethod
    def market_correlated(cls, mean: float, vol: float, correlation: float) -> RecoverySpec:
        """
        Market-correlated (Andersen-Sidenius) stochastic recovery.

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

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.valuations.correlation import RecoverySpec
        >>> callable(RecoverySpec.market_correlated)
        True
        """
        ...

    @classmethod
    def market_standard_stochastic(cls) -> RecoverySpec:
        """
        Market-standard stochastic recovery (40% mean, 25% vol, +40% corr).

        Recovery falls in stress under the canonical low-factor-stress
        convention.

        Returns
        -------
        RecoverySpec
            Standard stochastic recovery specification.

        Examples
        --------
        >>> from finstack_quant.valuations.correlation import RecoverySpec
        >>> callable(RecoverySpec.market_standard_stochastic)
        True
        """
        ...

    @property
    def expected_recovery(self) -> float:
        """
        Location-parameter recovery rate of this spec.

        For a constant spec this is the constant rate. For a
        market-correlated spec this returns the ``mean`` input — the target
        recovery at factor ``Z = 0`` — which differs from the
        Jensen-corrected unconditional mean ``E_Z[R(Z)]`` whenever the
        factor sensitivity is non-zero. For the true unconditional mean call
        ``build().expected_recovery``.

        Returns
        -------
        float
            The expected recovery exposed by this `RecoverySpec`.
        """
        ...

    def build(self) -> RecoveryModel:
        """
        Build a concrete :class:`RecoveryModel` from this specification.

        Returns
        -------
        RecoveryModel
            Concrete recovery model.
        """
        ...

class RecoveryModel:
    """
    Concrete recovery model for credit portfolio pricing.

    Obtain an instance via :meth:`RecoverySpec.build`.

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import RecoveryModel
    >>> RecoveryModel.__name__
    'RecoveryModel'
    """

    @property
    def expected_recovery(self) -> float:
        """
        Expected (unconditional) recovery rate.

        Returns
        -------
        float
            The expected recovery exposed by this `RecoveryModel`.
        """
        ...

    def conditional_recovery(self, market_factor: float) -> float:
        """
        Recovery conditional on the systematic market factor.

        Parameters
        ----------
        market_factor : float
            Realization of the market factor.

        Returns
        -------
        float
            Conditional recovery rate.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def lgd(self) -> float:
        """
        Loss given default (1 − recovery).

        Returns
        -------
        float
            The lgd exposed by this `RecoveryModel`.
        """
        ...

    def conditional_lgd(self, market_factor: float) -> float:
        """
        Conditional LGD given market factor.

        Parameters
        ----------
        market_factor : float
            Realization of the market factor.

        Returns
        -------
        float
            Conditional LGD.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def recovery_volatility(self) -> float:
        """
        Recovery-rate volatility scale (0 for constant models).

        Returns
        -------
        float
            The recovery volatility exposed by this `RecoveryModel`.
        """
        ...

    @property
    def is_stochastic(self) -> bool:
        """
        Whether recovery varies with the market factor.

        Returns
        -------
        bool
            Whether stochastic holds for this `RecoveryModel`.
        """
        ...

    @property
    def model_name(self) -> str:
        """
        Model name for diagnostics.

        Returns
        -------
        str
            The model name exposed by this `RecoveryModel`.
        """
        ...

class LatentFactorSpec:
    """
    Factor model specification for configuration and deferred construction.

    Example
    -------
    >>> from finstack_quant.valuations.correlation import LatentFactorSpec
    >>> spec = LatentFactorSpec.single_factor(0.2, 0.05)
    >>> model = spec.build()
    >>> model.num_factors
    1

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import LatentFactorSpec
    >>> LatentFactorSpec.__name__
    'LatentFactorSpec'
    """

    @classmethod
    def single_factor(cls, volatility: float, mean_reversion: float) -> LatentFactorSpec:
        """
        Single-factor model specification.

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

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.valuations.correlation import LatentFactorSpec
        >>> callable(LatentFactorSpec.single_factor)
        True
        """
        ...

    @classmethod
    def two_factor(cls, prepay_vol: float, credit_vol: float, correlation: float) -> LatentFactorSpec:
        """
        Two-factor model (prepayment + credit) specification.

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

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.valuations.correlation import LatentFactorSpec
        >>> callable(LatentFactorSpec.two_factor)
        True
        """
        ...

    @property
    def num_factors(self) -> int:
        """
        Number of factors implied by this specification.

        Returns
        -------
        int
            The num factors exposed by this `LatentFactorSpec`.
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
    >>> from finstack_quant.valuations.correlation import LatentFactorKind
    >>> LatentFactorKind.__name__
    'LatentFactorKind'
    """

    @property
    def num_factors(self) -> int:
        """
        Number of factors in the model.

        Returns
        -------
        int
            The num factors exposed by this `LatentFactorKind`.
        """
        ...

    @property
    def correlation_matrix(self) -> list[float]:
        """
        Factor correlation matrix (flattened row-major).

        Returns
        -------
        list[float]
            The correlation matrix exposed by this `LatentFactorKind`.
        """
        ...

    @property
    def volatilities(self) -> list[float]:
        """
        Factor volatilities.

        Returns
        -------
        list[float]
            The volatilities exposed by this `LatentFactorKind`.
        """
        ...

    @property
    def factor_names(self) -> list[str]:
        """
        Factor names for reporting.

        Returns
        -------
        list[str]
            The factor names exposed by this `LatentFactorKind`.
        """
        ...

    @property
    def model_name(self) -> str:
        """
        Model name for diagnostics.

        Returns
        -------
        str
            The model name exposed by this `LatentFactorKind`.
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

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

class LatentSingleFactor:
    """
    Single-factor model (common market factor).

    Example
    -------
    >>> from finstack_quant.valuations.correlation import LatentSingleFactor
    >>> m = LatentSingleFactor(volatility=0.2, mean_reversion=0.05)
    >>> m.num_factors
    1

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import LatentSingleFactor
    >>> LatentSingleFactor.__name__
    'LatentSingleFactor'
    """

    def __init__(self, volatility: float, mean_reversion: float) -> None:
        """
        Create a single-factor model.

        Parameters
        ----------
        volatility : float
            Factor volatility.
        mean_reversion : float
            Mean reversion speed.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def volatility(self) -> float:
        """
        Factor volatility.

        Returns
        -------
        float
            The volatility exposed by this `LatentSingleFactor`.
        """
        ...

    @property
    def mean_reversion(self) -> float:
        """
        Mean reversion speed.

        Returns
        -------
        float
            The mean reversion exposed by this `LatentSingleFactor`.
        """
        ...

    @property
    def num_factors(self) -> int:
        """
        Number of factors (always 1).

        Returns
        -------
        int
            The num factors exposed by this `LatentSingleFactor`.
        """
        ...

class LatentTwoFactor:
    """
    Two-factor model for prepayment and credit.

    Example
    -------
    >>> from finstack_quant.valuations.correlation import LatentTwoFactor
    >>> m = LatentTwoFactor(prepay_vol=0.15, credit_vol=0.10, correlation=-0.2)
    >>> m.num_factors
    2

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import LatentTwoFactor
    >>> LatentTwoFactor.__name__
    'LatentTwoFactor'
    """

    def __init__(self, prepay_vol: float, credit_vol: float, correlation: float) -> None:
        """
        Create a two-factor model.

        Parameters
        ----------
        prepay_vol : float
            Prepayment factor volatility.
        credit_vol : float
            Credit factor volatility.
        correlation : float
            Inter-factor correlation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
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

        Examples
        --------
        >>> from finstack_quant.valuations.correlation import LatentTwoFactor
        >>> callable(LatentTwoFactor.rmbs_standard)
        True
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

        Examples
        --------
        >>> from finstack_quant.valuations.correlation import LatentTwoFactor
        >>> callable(LatentTwoFactor.clo_standard)
        True
        """
        ...

    @property
    def prepay_vol(self) -> float:
        """
        Prepayment factor volatility.

        Returns
        -------
        float
            The prepay vol exposed by this `LatentTwoFactor`.
        """
        ...

    @property
    def credit_vol(self) -> float:
        """
        Credit factor volatility.

        Returns
        -------
        float
            The credit vol exposed by this `LatentTwoFactor`.
        """
        ...

    @property
    def correlation(self) -> float:
        """
        Factor correlation.

        Returns
        -------
        float
            The correlation exposed by this `LatentTwoFactor`.
        """
        ...

    @property
    def num_factors(self) -> int:
        """
        Number of factors (always 2).

        Returns
        -------
        int
            The num factors exposed by this `LatentTwoFactor`.
        """
        ...

    @property
    def cholesky_l10(self) -> float:
        """
        Cholesky ``L[1][0]`` for correlated factor generation.

        Returns
        -------
        float
            The cholesky l10 exposed by this `LatentTwoFactor`.
        """
        ...

    @property
    def cholesky_l11(self) -> float:
        """
        Cholesky ``L[1][1]`` for correlated factor generation.

        Returns
        -------
        float
            The cholesky l11 exposed by this `LatentTwoFactor`.
        """
        ...

class LatentMultiFactor:
    """
    Multi-factor model with custom correlation structure.

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

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import LatentMultiFactor
    >>> LatentMultiFactor.__name__
    'LatentMultiFactor'
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
        """
        Create an uncorrelated (identity) multi-factor model.

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

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.valuations.correlation import LatentMultiFactor
        >>> callable(LatentMultiFactor.uncorrelated)
        True
        """
        ...

    @property
    def num_factors(self) -> int:
        """
        Number of factors.

        Returns
        -------
        int
            The num factors exposed by this `LatentMultiFactor`.
        """
        ...

    @property
    def correlation_matrix(self) -> list[float]:
        """
        Factor correlation matrix (flattened row-major).

        Returns
        -------
        list[float]
            The correlation matrix exposed by this `LatentMultiFactor`.
        """
        ...

    @property
    def volatilities(self) -> list[float]:
        """
        Factor volatilities.

        Returns
        -------
        list[float]
            The volatilities exposed by this `LatentMultiFactor`.
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

class CorrelatedBernoulli:
    """
    Correlated Bernoulli distribution for two binary events.

    Example
    -------
    >>> from finstack_quant.valuations.correlation import CorrelatedBernoulli
    >>> cb = CorrelatedBernoulli(p1=0.05, p2=0.03, correlation=0.3)
    >>> cb.joint_p11  # P(both default)
    0.00...

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import CorrelatedBernoulli
    >>> CorrelatedBernoulli.__name__
    'CorrelatedBernoulli'
    """

    def __init__(self, p1: float, p2: float, correlation: float) -> None:
        """
        Create a correlated Bernoulli distribution.

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
        """
        Marginal probability of event 1.

        Returns
        -------
        float
            The p1 exposed by this `CorrelatedBernoulli`.
        """
        ...

    @property
    def p2(self) -> float:
        """
        Marginal probability of event 2.

        Returns
        -------
        float
            The p2 exposed by this `CorrelatedBernoulli`.
        """
        ...

    @property
    def correlation(self) -> float:
        """
        Effective correlation after Fréchet-Hoeffding clamping.

        Returns
        -------
        float
            The correlation exposed by this `CorrelatedBernoulli`.
        """
        ...

    @property
    def requested_correlation(self) -> float:
        """
        Caller-requested correlation before Fréchet-Hoeffding clamping.

        Returns
        -------
        float
            The requested correlation exposed by this `CorrelatedBernoulli`.
        """
        ...

    @property
    def joint_p11(self) -> float:
        """
        Return the joint p11 for `CorrelatedBernoulli`.
        P(X₁=1, X₂=1).

        Returns
        -------
        float
            The joint p11 exposed by this `CorrelatedBernoulli`.
        """
        ...

    @property
    def joint_p10(self) -> float:
        """
        Return the joint p10 for `CorrelatedBernoulli`.
        P(X₁=1, X₂=0).

        Returns
        -------
        float
            The joint p10 exposed by this `CorrelatedBernoulli`.
        """
        ...

    @property
    def joint_p01(self) -> float:
        """
        Return the joint p01 for `CorrelatedBernoulli`.
        P(X₁=0, X₂=1).

        Returns
        -------
        float
            The joint p01 exposed by this `CorrelatedBernoulli`.
        """
        ...

    @property
    def joint_p00(self) -> float:
        """
        Return the joint p00 for `CorrelatedBernoulli`.
        P(X₁=0, X₂=0).

        Returns
        -------
        float
            The joint p00 exposed by this `CorrelatedBernoulli`.
        """
        ...

    def joint_probabilities(self) -> tuple[float, float, float, float]:
        """
        All four joint probabilities ``(p11, p10, p01, p00)``.

        Returns
        -------
        tuple[float, float, float, float]
            ``(p11, p10, p01, p00)`` summing to 1.
        """
        ...

    def conditional_p2_given_x1(self) -> float:
        """
        Conditional probability P(X₂=1 | X₁=1).

        Returns
        -------
        float
            Conditional probability.
        """
        ...

    def conditional_p1_given_x2(self) -> float:
        """
        Conditional probability P(X₁=1 | X₂=1).

        Returns
        -------
        float
            Conditional probability.
        """
        ...

    def sample_from_uniform(self, u: float) -> tuple[int, int]:
        """
        Sample a pair of correlated binary outcomes from a uniform ``[0,1]`` draw.

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
    """
    Simulate finite-pool losses with deterministic path-indexed RNG streams.

    Losses are positive amounts. VaR is the nearest-rank empirical quantile at
    ``config.confidence``; expected shortfall includes the VaR observation and
    every worse path. If ``recovery`` is provided, its conditional LGD replaces
    each exposure's constant LGD and exactly one systematic factor is required.

    Parameters
    ----------
    exposures : Sequence[CreditExposure]
        Obligors to simulate, each with exposure, marginal PD, LGD, and factor
        loadings compatible with ``config.copula``.
    config : PortfolioLossConfig
        Path count, RNG seed, confidence level, and dependence-model settings.
    recovery : RecoverySpec or None, default None
        Optional conditional recovery model replacing constant exposure LGDs;
        it requires a one-factor systematic copula.

    Returns
    -------
    PortfolioLossResult
        Result of simulate portfolio loss for the binding in the annotated representation.

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import simulate_portfolio_loss
    >>> callable(simulate_portfolio_loss)
    True
    """
    ...

def correlation_bounds(p1: float, p2: float) -> tuple[float, float]:
    """
    Fréchet-Hoeffding correlation bounds for two Bernoulli marginals.

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

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import correlation_bounds
    >>> callable(correlation_bounds)
    True
    """
    ...

def joint_probabilities(p1: float, p2: float, correlation: float) -> tuple[float, float, float, float]:
    """
    Joint probabilities for two correlated Bernoulli variables.

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

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import joint_probabilities
    >>> callable(joint_probabilities)
    True
    """
    ...

def validate_correlation_matrix(matrix: Sequence[float], n: int) -> None:
    """
    Validate a correlation matrix (flattened row-major).

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

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import validate_correlation_matrix
    >>> callable(validate_correlation_matrix)
    True
    """
    ...

def nearest_correlation(
    matrix: Sequence[float],
    n: int,
    max_iter: int | None = None,
    tol: float | None = None,
) -> list[float]:
    """
    Nearest correlation matrix (Higham 2002) for a near-PSD input.

    Projects a symmetric, unit-diagonal, near-PSD matrix onto the set of valid
    correlation matrices (symmetric, unit diagonal, positive semi-definite)
    in Frobenius norm. Gross input violations raise rather than being
    silently reshaped.

    Parameters
    ----------
    matrix : Sequence[float]
        Flattened row-major ``n x n`` input matrix.
    n : int
        Matrix dimension.
    max_iter : int or None
        Maximum alternating-projection iterations. Defaults to the Rust
        ``NearestCorrelationOpts::default()`` value (currently ``200``).
    tol : float or None
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

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import nearest_correlation
    >>> callable(nearest_correlation)
    True
    """
    ...

def cholesky_decompose(matrix: Sequence[float], n: int) -> list[float]:
    """
    Pivoted Cholesky decomposition of a correlation matrix (flattened row-major).

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

    Examples
    --------
    >>> from finstack_quant.valuations.correlation import cholesky_decompose
    >>> callable(cholesky_decompose)
    True
    """
    ...
