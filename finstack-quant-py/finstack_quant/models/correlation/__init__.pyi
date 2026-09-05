"""
Type stubs for ``finstack_quant.models.correlation``.

Correlation infrastructure: copulas, factor models, recovery models.

Examples
--------
>>> from finstack_quant.models.correlation import correlation_bounds
>>> tuple(round(value, 3) for value in correlation_bounds(0.05, 0.03))
(-0.04, 0.767)

"""

from __future__ import annotations

from typing import Any, Sequence

import pandas as pd

__all__ = [
    "Copula",
    "CopulaSpec",
    "CorrelatedBernoulli",
    "CreditExposure",
    "LatentFactorKind",
    "LatentFactorSpec",
    "LatentMultiFactor",
    "LatentSingleFactor",
    "LatentTwoFactor",
    "MAX_PORTFOLIO_LOSS_PATHS",
    "PortfolioLossConfig",
    "PortfolioLossResult",
    "RecoveryModel",
    "RecoverySpec",
    "TrancheLossStatistics",
    "cholesky_decompose",
    "correlation_bounds",
    "joint_probabilities",
    "nearest_correlation",
    "simulate_portfolio_loss",
    "validate_correlation_matrix",
]

MAX_PORTFOLIO_LOSS_PATHS: int

class CopulaSpec:
    """
    Copula model specification for configuration and deferred construction.

    Use class methods to create a spec, then call :meth:`build` to obtain
    a concrete :class:`Copula` instance.

    Example
    -------
    >>> from finstack_quant.models.correlation import CopulaSpec
    >>> spec = CopulaSpec.gaussian()
    >>> copula = spec.build()
    >>> copula.model_name
    'One-Factor Gaussian Copula'

    Examples
    --------
    >>> from finstack_quant.models.correlation import CopulaSpec
    >>> spec = CopulaSpec.gaussian()
    >>> (spec.is_gaussian, spec.build().model_name)
    (True, 'One-Factor Gaussian Copula')

    """

    @classmethod
    def gaussian(cls) -> CopulaSpec:
        """
        One-factor Gaussian copula (market standard).

        Returns
        -------
        CopulaSpec
            Gaussian copula specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec
        >>> (CopulaSpec.gaussian().is_gaussian, CopulaSpec.gaussian().build().num_factors)
        (True, 1)
        """
        ...

    @classmethod
    def student_t(cls, degrees_of_freedom: float) -> CopulaSpec:
        """
        Student-t copula with specified degrees of freedom.

        Parameters
        ----------
        degrees_of_freedom : float
            Degrees of freedom; must be finite and ``> 2`` (finite variance).
            Typical calibration range for CDX tranches is 4-10. Matches the
            Rust ``CopulaSpec::StudentT { degrees_of_freedom }`` field.

        Returns
        -------
        CopulaSpec
            Student-t copula specification.

        Raises
        ------
        ValueError
            If ``degrees_of_freedom`` is not finite or is ``<= 2``.

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec
        >>> copula = CopulaSpec.student_t(degrees_of_freedom=5.0).build()
        >>> (copula.model_name, round(copula.tail_dependence(0.3), 6))
        ('Student-t Copula', 0.122387)

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

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec
        >>> spec = CopulaSpec.random_factor_loading(0.2)
        >>> (spec.is_rfl, spec.build().model_name)
        (True, 'Random Factor Loading Copula')
        """
        ...

    @classmethod
    def multi_factor(cls) -> CopulaSpec:
        """
        Two-factor Gaussian copula with global and shared-sector factors.

        Returns
        -------
        CopulaSpec
            Two-factor copula specification.
        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec
        >>> spec = CopulaSpec.multi_factor()
        >>> (spec.is_multi_factor, spec.build().num_factors)
        (True, 2)
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical tagged JSON wire format (``{"type": ...}``).

        Returns
        -------
        str
            JSON document, e.g. ``{"type":"student_t","degrees_of_freedom":5.0}``.

        Raises
        ------
        ValueError
            If the specification cannot be serialized to JSON
            (raised as ``"CopulaSpec serialization failed"``).

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec
        >>> CopulaSpec.from_json(CopulaSpec.student_t(5.0).to_json()) == CopulaSpec.student_t(5.0)
        True

        """
        ...

    @staticmethod
    def from_json(json: str) -> CopulaSpec:
        """
        Deserialize a spec produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Tagged JSON document produced by :meth:`to_json`.

        Returns
        -------
        CopulaSpec
            The reconstructed specification.

        Raises
        ------
        ValueError
            If the payload is malformed or the tag is unknown.

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec
        >>> CopulaSpec.from_json('{"type":"gaussian"}').is_gaussian
        True

        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str:
        """Python-style repr, e.g. ``CopulaSpec(type='student_t', degrees_of_freedom=5.0)``."""
        ...

class Copula:
    """
    Concrete copula model for portfolio default correlation.

    Obtain an instance via :meth:`CopulaSpec.build`.

    Example
    -------
    >>> from finstack_quant.models.correlation import CopulaSpec
    >>> copula = CopulaSpec.gaussian().build()
    >>> # P(default | Z=0) = norm.cdf(-2.33 / sqrt(1 - 0.3)) ≈ 0.0027,
    >>> # below the unconditional PD of norm.cdf(-2.33) ≈ 0.0099.
    >>> copula.conditional_default_prob(-2.33, [0.0], 0.3)
    0.002...

    Examples
    --------
    >>> from finstack_quant.models.correlation import CopulaSpec
    >>> copula = CopulaSpec.gaussian().build()
    >>> (copula.model_name, round(copula.conditional_default_prob(-2.33, [0.0], 0.3), 6))
    ('One-Factor Gaussian Copula', 0.002677)

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
            If the factor vector length differs from the copula factor count;
            if the threshold, correlation, or any factor is non-finite; if
            correlation is outside ``[0, 1]``; or if the computed probability
            is non-finite or outside ``[0, 1]``.
        """
        ...

    @property
    def num_factors(self) -> int:
        """
        Number of systematic factors in the model.

        Returns
        -------
        int
            Number of systematic factors in the model.

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

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.
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
    >>> from finstack_quant.models.correlation import CreditExposure
    >>> exposure = CreditExposure("ACME", 1_000_000.0, 0.02, 0.6, [0.3])
    >>> (exposure.id, exposure.notional, exposure.default_probability, exposure.factor_loadings)
    ('ACME', 1000000.0, 0.02, [0.3])

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

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable identifier for this exposure.

        Returns
        -------
        str
            Caller-supplied exposure identifier retained on simulated losses.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def notional(self) -> float:
        """
        Exposure at default, in the portfolio currency.

        Returns
        -------
        float
            Exposure at default in portfolio currency units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def default_probability(self) -> float:
        """
        Marginal probability of default over the horizon, in ``[0, 1]``.

        Returns
        -------
        float
            Horizon default probability as a decimal in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def lgd(self) -> float:
        """
        Loss given default, as a fraction of notional in ``[0, 1]``.

        Returns
        -------
        float
            Loss-given-default fraction of notional in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factor_loadings(self) -> list[float]:
        """
        Systematic factor loadings driving correlated defaults.

        Returns
        -------
        list[float]
            Loadings onto the copula factors, one weight per factor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize `CreditExposure` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `CreditExposure`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CreditExposure:
        """
        Deserialize a `CreditExposure` from JSON produced by :meth:`to_json`.

        Completes the wire round-trip, which is also what makes this type
        picklable.

        Parameters
        ----------
        json : str
            Canonical JSON produced by :meth:`to_json`.

        Returns
        -------
        CreditExposure
            Validated exposure reconstructed from the canonical JSON payload.

        Examples
        --------
        >>> from finstack_quant.models.correlation import CreditExposure
        >>> exposure = CreditExposure("ACME", 1_000_000.0, 0.02, 0.6, [0.3])
        >>> CreditExposure.from_json(exposure.to_json()).id
        'ACME'

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized schema.
        """
        ...

class PortfolioLossConfig:
    """
    Settings for deterministic portfolio credit-loss simulation.

    ``num_paths`` must be in ``[1, MAX_PORTFOLIO_LOSS_PATHS]``.

    Examples
    --------
    >>> from finstack_quant.models.correlation import CopulaSpec, PortfolioLossConfig
    >>> config = PortfolioLossConfig(1000, 42, 0.99, CopulaSpec.gaussian())
    >>> (config.num_paths, config.seed, config.confidence, config.copula.is_gaussian)
    (1000, 42, 0.99, True)

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

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...
    @property
    def num_paths(self) -> int:
        """
        Number of simulated paths.

        Returns
        -------
        int
            Count of Monte Carlo paths used to build the loss distribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seed(self) -> int:
        """
        RNG seed; the same seed reproduces the same paths exactly.

        Returns
        -------
        int
            Unsigned seed that makes the simulated loss paths reproducible.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def confidence(self) -> float:
        """
        Confidence level for VaR and expected shortfall, in ``(0, 1)``.

        Returns
        -------
        float
            VaR/ES quantile as a decimal in ``(0, 1)``, for example ``0.99``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def copula(self) -> CopulaSpec:
        """
        Dependence structure used to couple the marginal defaults.

        Returns
        -------
        CopulaSpec
            Copula specification that couples the names' default indicators.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize `PortfolioLossConfig` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioLossConfig`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PortfolioLossConfig:
        """
        Deserialize a `PortfolioLossConfig` from JSON produced by :meth:`to_json`.

        Completes the wire round-trip, which is also what makes this type
        picklable.

        Parameters
        ----------
        json : str
            Canonical JSON produced by :meth:`to_json`.

        Returns
        -------
        PortfolioLossConfig
            Validated configuration reconstructed from the canonical JSON payload.

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec, PortfolioLossConfig
        >>> config = PortfolioLossConfig(1000, 42, 0.99, CopulaSpec.gaussian())
        >>> PortfolioLossConfig.from_json(config.to_json()).num_paths
        1000

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized schema.
        """
        ...

class PortfolioLossResult:
    """
    Loss distribution and loss-positive VaR/expected shortfall.

    Examples
    --------
    >>> from finstack_quant.models.correlation import (
    ...     CopulaSpec,
    ...     CreditExposure,
    ...     PortfolioLossConfig,
    ...     simulate_portfolio_loss,
    ... )
    >>> exposures = [CreditExposure("A", 100.0, 0.05, 0.6, [0.3]), CreditExposure("B", 100.0, 0.03, 0.6, [0.3])]
    >>> config = PortfolioLossConfig(200, 42, 0.99, CopulaSpec.gaussian())
    >>> result = simulate_portfolio_loss(exposures, config)
    >>> (len(result.losses), result.expected_loss >= 0.0, result.var >= 0.0)
    (200, True, True)

    """

    @property
    def losses(self) -> list[float]:
        """
        Simulated portfolio loss per path, in the ascending path order Rust produced.

        Returns
        -------
        list[float]
            One loss amount per path, in portfolio currency, in path-id order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expected_loss(self) -> float:
        """
        Mean simulated loss.

        Returns
        -------
        float
            Sample mean of simulated portfolio losses, in portfolio currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def var(self) -> float:
        """
        Value at risk at the configured confidence, loss-positive.

        Returns
        -------
        float
            Nearest-rank VaR at ``confidence``; larger values are worse.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expected_shortfall(self) -> float:
        """
        Probability-weighted mean loss in the worst ``1 - confidence`` tail.

        Returns
        -------
        float
            Loss-positive tail mean in portfolio currency, with fractional
            weight on the boundary observation when necessary.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def confidence(self) -> float:
        """
        Return the loss-positive confidence used for `var` and `expected_shortfall`.

        Returns
        -------
        float
            Tail-statistic confidence in ``(0, 1)`` recorded when this
            `PortfolioLossResult` was aggregated.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def tranche_loss_statistics(
        self,
        attachment: float,
        detachment: float,
        pool_notional: float,
    ) -> TrancheLossStatistics:
        """
        Compute tranche loss statistics over this simulated loss distribution.

        Each path's pool loss fraction ``L = loss / pool_notional`` maps through
        the standard tranche loss function
        ``clamp(L - attachment, 0, detachment - attachment) / (detachment - attachment)``,
        and the resulting distribution is aggregated at this result's own
        :attr:`confidence`.

        Parameters
        ----------
        attachment : float
            Lower tranche boundary as a **fraction** of pool notional in
            ``[0, 1)``; losses below this point hit more junior tranches. A
            0-3% equity tranche uses ``0.0``.
        detachment : float
            Upper tranche boundary as a **fraction** of pool notional in
            ``(0, 1]`` and strictly above ``attachment``; losses above this
            point hit more senior tranches. A 0-3% equity tranche uses ``0.03``.
        pool_notional : float
            Total pool notional, finite and strictly positive, in the same
            scalar unit as the simulated losses.

        Returns
        -------
        TrancheLossStatistics
            Expected loss, VaR, expected shortfall, and breach probabilities
            for the requested tranche.

        Raises
        ------
        ValueError
            If a boundary lies outside ``[0, 1]``, ``attachment >= detachment``,
            or ``pool_notional`` is not finite and strictly positive.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Primary table: the simulated loss distribution.

        Alias of :meth:`to_distribution_dataframe`. Every tabular result type in the library
        answers ``to_dataframe()``; the one-row aggregate view stays on :meth:`to_summary_dataframe`.

        Returns
        -------
        pd.DataFrame
            The same frame :meth:`to_distribution_dataframe` returns.

        Examples
        --------
        >>> frame = result.to_dataframe()  # doctest: +SKIP

        Notes
        -----
        This alias does not raise; it delegates to the method named above.
        """
        ...

    def to_distribution_dataframe(self) -> pd.DataFrame:
        """
        Export the simulated loss distribution as a pandas DataFrame.

        Columns: ``loss``.

        One row per simulated path, indexed by path id (a ``RangeIndex``), in
        the ascending path order Rust produced — so repeated exports of the
        same result are identical. Feed it straight to ``df["loss"].hist()`` or
        ``df["loss"].quantile(...)``.

        The aggregate statistics are not repeated per row; see
        :meth:`to_summary_dataframe`.

        Returns
        -------
        pd.DataFrame
            One row per simulated path.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_summary_dataframe(self) -> pd.DataFrame:
        """
        Export the aggregate loss statistics as a single-row pandas DataFrame.

        Columns: ``expected_loss``, ``var``, ``expected_shortfall``,
        ``confidence``, ``num_paths``.

        One simulation is one flat record, so a one-row frame is the right
        shape: ``pd.concat`` over several correlation or recovery assumptions
        gives a comparison table directly.

        ``var`` and ``expected_shortfall`` are loss-positive, matching the Rust
        convention: a larger number is a worse outcome.

        Returns
        -------
        pd.DataFrame
            Single-row frame of the distribution's aggregate statistics.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize `PortfolioLossResult` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioLossResult`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PortfolioLossResult:
        """
        Deserialize a `PortfolioLossResult` from JSON produced by :meth:`to_json`.

        Completes the wire round-trip, which is also what makes this type
        picklable.

        Parameters
        ----------
        json : str
            Canonical JSON produced by :meth:`to_json`.

        Returns
        -------
        PortfolioLossResult
            Validated result reconstructed from the canonical JSON payload.

        Examples
        --------
        >>> from finstack_quant.models.correlation import (
        ...     CopulaSpec,
        ...     CreditExposure,
        ...     PortfolioLossConfig,
        ...     PortfolioLossResult,
        ...     simulate_portfolio_loss,
        ... )
        >>> config = PortfolioLossConfig(1000, 42, 0.99, CopulaSpec.gaussian())
        >>> result = simulate_portfolio_loss([CreditExposure("A", 1e6, 0.02, 0.6, [0.3])], config)
        >>> PortfolioLossResult.from_json(result.to_json()).expected_loss
        14400.0

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized schema.
        """
        ...

class TrancheLossStatistics:
    """
    Expected loss, tail statistics, and breach probabilities for one tranche.

    Fraction members are shares of the tranche's own notional; amount members
    are in the pool-notional unit supplied to
    :meth:`PortfolioLossResult.tranche_loss_statistics`.

    Examples
    --------
    >>> from finstack_quant.models.correlation import (
    ...     CopulaSpec,
    ...     CreditExposure,
    ...     PortfolioLossConfig,
    ...     simulate_portfolio_loss,
    ... )
    >>> exposures = [CreditExposure("A", 100.0, 0.05, 0.6, [0.3]), CreditExposure("B", 100.0, 0.03, 0.6, [0.3])]
    >>> config = PortfolioLossConfig(200, 42, 0.99, CopulaSpec.gaussian())
    >>> result = simulate_portfolio_loss(exposures, config)
    >>> statistics = result.tranche_loss_statistics(0.0, 0.1, 200.0)
    >>> (statistics.attachment, statistics.detachment, statistics.tranche_notional)
    (0.0, 0.1, 20.0)

    """

    @property
    def attachment(self) -> float:
        """
        Lower tranche boundary as a fraction of pool notional.

        Returns
        -------
        float
            Lower tranche boundary as a fraction of pool notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def detachment(self) -> float:
        """
        Upper tranche boundary as a fraction of pool notional.

        Returns
        -------
        float
            Upper tranche boundary as a fraction of pool notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tranche_notional(self) -> float:
        """
        Tranche thickness times pool notional.

        Returns
        -------
        float
            ``(detachment - attachment) * pool_notional``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expected_loss_fraction(self) -> float:
        """
        Mean tranche loss as a share of tranche notional, in ``[0, 1]``.

        Returns
        -------
        float
            Mean tranche loss as a share of tranche notional, in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expected_loss_amount(self) -> float:
        """
        Mean tranche loss in pool-notional units.

        Returns
        -------
        float
            Mean tranche loss in pool-notional units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def var_fraction(self) -> float:
        """
        Nearest-rank tranche loss share at the distribution's confidence.

        Returns
        -------
        float
            Nearest-rank tranche loss share at the distribution's confidence.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def var_amount(self) -> float:
        """
        Nearest-rank tranche loss in pool-notional units.

        Returns
        -------
        float
            Nearest-rank tranche loss in pool-notional units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expected_shortfall_fraction(self) -> float:
        """
        Mean tranche loss share in the worst `1 - confidence` probability mass, with fractional boundary weight.

        Returns
        -------
        float
            Mean tranche loss share in the worst `1 - confidence` probability mass, with fractional boundary weight.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expected_shortfall_amount(self) -> float:
        """
        Mean tranche loss amount in the worst `1 - confidence` probability mass, with fractional boundary weight.

        Returns
        -------
        float
            Mean tranche loss amount in the worst `1 - confidence` probability mass, with fractional boundary weight.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def prob_attachment_breached(self) -> float:
        """
        Share of paths whose pool loss fraction strictly exceeds the attachment.

        Returns
        -------
        float
            Share of paths whose pool loss fraction strictly exceeds the attachment.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def prob_full_writedown(self) -> float:
        """
        Share of paths whose pool loss fraction reaches or exceeds the detachment.

        Returns
        -------
        float
            Share of paths whose pool loss fraction reaches or exceeds the detachment.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas DataFrame.

        Columns: ``attachment``, ``detachment``, ``tranche_notional``,
        ``expected_loss_fraction``, ``expected_loss_amount``, ``var_fraction``,
        ``var_amount``, ``expected_shortfall_fraction``,
        ``expected_shortfall_amount``, ``prob_attachment_breached``,
        ``prob_full_writedown``.

        These statistics describe ONE tranche, so a one-row frame is the right
        shape. Build the capital-structure table by stacking tranches::

            pd.concat(
                [
                    result.tranche_loss_statistics(a, d, pool).to_dataframe()
                    for a, d in [(0.0, 0.03), (0.03, 0.07), (0.07, 1.0)]
                ],
                ignore_index=True,
            )

        ``*_fraction`` columns are shares of the tranche's own notional;
        ``*_amount`` columns are in the pool-notional unit passed to
        :meth:`PortfolioLossResult.tranche_loss_statistics`.

        Returns
        -------
        pd.DataFrame
            Single-row frame describing this tranche.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize `TrancheLossStatistics` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `TrancheLossStatistics`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> TrancheLossStatistics:
        """
        Deserialize a `TrancheLossStatistics` from JSON produced by :meth:`to_json`.

        Completes the wire round-trip, which is also what makes this type
        picklable.

        Parameters
        ----------
        json : str
            Canonical JSON produced by :meth:`to_json`.

        Returns
        -------
        TrancheLossStatistics
            Validated tranche statistics reconstructed from the canonical JSON payload.

        Examples
        --------
        >>> from finstack_quant.models.correlation import (
        ...     CopulaSpec,
        ...     CreditExposure,
        ...     PortfolioLossConfig,
        ...     TrancheLossStatistics,
        ...     simulate_portfolio_loss,
        ... )
        >>> config = PortfolioLossConfig(1000, 42, 0.99, CopulaSpec.gaussian())
        >>> result = simulate_portfolio_loss([CreditExposure("A", 1e6, 0.02, 0.6, [0.3])], config)
        >>> stats = result.tranche_loss_statistics(0.0, 0.03, 1e6)
        >>> TrancheLossStatistics.from_json(stats.to_json()).detachment
        0.03

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized schema.
        """
        ...

class RecoverySpec:
    """
    Recovery model specification for configuration and deferred construction.

    Example
    -------
    >>> from finstack_quant.models.correlation import RecoverySpec
    >>> spec = RecoverySpec.constant(0.4)
    >>> model = spec.build()
    >>> model.expected_recovery
    0.4

    Examples
    --------
    >>> from finstack_quant.models.correlation import RecoverySpec
    >>> spec = RecoverySpec.constant(0.4)
    >>> (spec.expected_recovery, spec.build().lgd)
    (0.4, 0.6)

    """

    @classmethod
    def constant(cls, rate: float) -> RecoverySpec:
        """
        Constant recovery rate as a decimal of notional.

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
            If ``rate`` is non-finite or outside ``[0, 1]``.

        Examples
        --------
        >>> from finstack_quant.models.correlation import RecoverySpec
        >>> spec = RecoverySpec.constant(0.4)
        >>> (spec.expected_recovery, spec.build().conditional_recovery(0.0))
        (0.4, 0.4)

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
            If ``mean`` is non-finite or outside ``[0, 1]``, or if ``vol`` or
            ``correlation`` is non-finite. Finite volatility and correlation
            values are clamped to their supported ranges.

        Examples
        --------
        >>> from finstack_quant.models.correlation import RecoverySpec
        >>> model = RecoverySpec.market_correlated(0.4, 0.2, 0.3).build()
        >>> (model.is_stochastic, model.recovery_volatility)
        (True, 0.2)

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

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.correlation import RecoverySpec
        >>> model = RecoverySpec.market_standard_stochastic().build()
        >>> (model.model_name, round(model.expected_recovery, 3))
        ('Market-Correlated Stochastic Recovery', 0.404)
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
            Location-parameter recovery rate of this spec.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def build(self) -> RecoveryModel:
        """
        Build a concrete :class:`RecoveryModel` from this specification.

        Returns
        -------
        RecoveryModel
            Concrete recovery model.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical tagged JSON wire format (``{"type": ...}``).

        Returns
        -------
        str
            JSON document, e.g. ``{"type":"constant","rate":0.4}``.

        Raises
        ------
        ValueError
            If the specification cannot be serialized to JSON
            (raised as ``"RecoverySpec serialization failed"``).

        Examples
        --------
        >>> from finstack_quant.models.correlation import RecoverySpec
        >>> RecoverySpec.from_json(RecoverySpec.constant(0.4).to_json()) == RecoverySpec.constant(0.4)
        True

        """
        ...

    @staticmethod
    def from_json(json: str) -> RecoverySpec:
        """
        Deserialize a spec produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Tagged JSON document produced by :meth:`to_json`.

        Returns
        -------
        RecoverySpec
            The reconstructed specification.

        Raises
        ------
        ValueError
            If the payload is malformed or the tag is unknown.

        Examples
        --------
        >>> from finstack_quant.models.correlation import RecoverySpec
        >>> RecoverySpec.from_json('{"type":"constant","rate":0.4}').expected_recovery
        0.4

        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str:
        """Python-style repr, e.g. ``RecoverySpec(type='constant', rate=0.4)``."""
        ...

class RecoveryModel:
    """
    Concrete recovery model for credit portfolio pricing.

    Obtain an instance via :meth:`RecoverySpec.build`.

    Examples
    --------
    >>> from finstack_quant.models.correlation import RecoverySpec
    >>> model = RecoverySpec.constant(0.4).build()
    >>> (model.expected_recovery, model.lgd, model.is_stochastic)
    (0.4, 0.6, False)

    """

    @property
    def expected_recovery(self) -> float:
        """
        Expected (unconditional) recovery rate.

        Returns
        -------
        float
            Expected (unconditional) recovery rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.
        """
        ...

    @property
    def lgd(self) -> float:
        """
        Loss given default (1 − recovery).

        Returns
        -------
        float
            Loss given default (1 − recovery).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.
        """
        ...

    @property
    def recovery_volatility(self) -> float:
        """
        Recovery-rate volatility scale (0 for constant models).

        Returns
        -------
        float
            Recovery-rate volatility scale (0 for constant models).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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
            Annualized volatility of the single latent factor.
        mean_reversion : float
            Mean-reversion speed of the single latent factor, per year.

        Returns
        -------
        LatentFactorSpec
            Single-factor specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

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
            Prepayment factor volatility.
        credit_vol : float
            Credit factor volatility.
        correlation : float
            Inter-factor correlation.

        Returns
        -------
        LatentFactorSpec
            Two-factor specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

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
            Annualized volatility of the single latent factor.
        mean_reversion : float
            Mean-reversion speed of the single latent factor, per year.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def volatility(self) -> float:
        """
        Annualized volatility of the single latent factor.

        Returns
        -------
        float
            Annualized volatility of the single latent factor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mean_reversion(self) -> float:
        """
        Mean-reversion speed of the single latent factor, per year.

        Returns
        -------
        float
            Mean-reversion speed of the single latent factor, per year.

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
            Prepayment factor volatility.
        credit_vol : float
            Credit factor volatility.
        correlation : float
            Inter-factor correlation.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
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
        Prepayment factor volatility.

        Returns
        -------
        float
            Prepayment factor volatility.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def credit_vol(self) -> float:
        """
        Credit factor volatility.

        Returns
        -------
        float
            Credit factor volatility.

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
            Count of latent factors in this multi-factor specification.
        volatilities : list[float]
            Per-factor volatilities.

        Returns
        -------
        LatentMultiFactor
            Uncorrelated factor model.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

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

class CorrelatedBernoulli:
    """
    Correlated Bernoulli distribution for two binary events.

    Example
    -------
    >>> from finstack_quant.models.correlation import CorrelatedBernoulli
    >>> cb = CorrelatedBernoulli(p1=0.05, p2=0.03, correlation=0.3)
    >>> cb.joint_p11  # P(both default)
    0.012653...

    Examples
    --------
    >>> from finstack_quant.models.correlation import CorrelatedBernoulli
    >>> distribution = CorrelatedBernoulli(0.05, 0.03, 0.3)
    >>> (round(distribution.joint_p11, 6), round(sum(distribution.joint_probabilities()), 6))
    (0.012654, 1.0)

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
            Marginal probability of event 1.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def p2(self) -> float:
        """
        Marginal probability of event 2.

        Returns
        -------
        float
            Marginal probability of event 2.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def correlation(self) -> float:
        """
        Effective correlation after Fréchet-Hoeffding clamping.

        Returns
        -------
        float
            Effective correlation after Fréchet-Hoeffding clamping.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def requested_correlation(self) -> float:
        """
        Caller-requested correlation before Fréchet-Hoeffding clamping.

        Returns
        -------
        float
            Caller-requested correlation before Fréchet-Hoeffding clamping.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def joint_p11(self) -> float:
        """
        Joint probability P(X₁=1, X₂=1) under the fitted Bernoulli pair.

        Returns
        -------
        float
            Model joint probability of both events occurring.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def joint_p10(self) -> float:
        """
        Joint probability P(X₁=1, X₂=0) under the fitted Bernoulli pair.

        Returns
        -------
        float
            Model joint probability of the first event only.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def joint_p01(self) -> float:
        """
        Joint probability P(X₁=0, X₂=1) under the fitted Bernoulli pair.

        Returns
        -------
        float
            Model joint probability of the second event only.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def joint_p00(self) -> float:
        """
        Joint probability P(X₁=0, X₂=0) under the fitted Bernoulli pair.

        Returns
        -------
        float
            Model joint probability of neither event occurring.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def joint_probabilities(self) -> tuple[float, float, float, float]:
        """
        All four joint probabilities ``(p11, p10, p01, p00)``.

        Returns
        -------
        tuple[float, float, float, float]
            ``(p11, p10, p01, p00)`` summing to 1.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def conditional_p2_given_x1(self) -> float:
        """
        Conditional probability P(X₂=1 | X₁=1).

        Returns
        -------
        float
            Conditional probability.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def conditional_p1_given_x2(self) -> float:
        """
        Conditional probability P(X₁=1 | X₂=1).

        Returns
        -------
        float
            Conditional probability.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
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
    exposures: Sequence[CreditExposure] | pd.DataFrame,
    config: PortfolioLossConfig,
    recovery: RecoverySpec | None = None,
) -> PortfolioLossResult:
    """
    Simulate finite-pool losses with deterministic path-indexed RNG streams.

    Losses are positive amounts. VaR is the nearest-rank empirical quantile at
    ``config.confidence``; expected shortfall averages exactly the worst
    ``1 - config.confidence`` probability mass, with fractional boundary weight. If ``recovery`` is provided, its conditional LGD replaces
    each exposure's constant LGD and exactly one systematic factor is required.

    Parameters
    ----------
    exposures : Sequence[CreditExposure] or pandas.DataFrame
        Obligors to simulate, each with exposure, marginal PD, LGD, and factor
        loadings compatible with ``config.copula``. A DataFrame needs columns
        ``id``, ``notional``, ``pd`` (or ``default_probability``), ``lgd`` and
        ``factor_loading`` (one scalar per name) or ``factor_loadings`` (a
        list per name).
    config : PortfolioLossConfig
        Path count, RNG seed, confidence level, and dependence-model settings.
    recovery : RecoverySpec or None, default None
        Optional conditional recovery model replacing constant exposure LGDs;
        it requires a one-factor systematic copula.

    Returns
    -------
    PortfolioLossResult
        Path losses in path-index order together with loss-positive expected
        loss, nearest-rank VaR, expected shortfall, and the configured confidence.

    Raises
    ------
    TypeError
        If ``exposures`` is neither a sequence of ``CreditExposure`` nor a
        DataFrame.
    ValueError
        If a required DataFrame column is missing; the path count or
        confidence level is invalid; the copula is not a
        supported Gaussian or Student-t model; exposure identifiers are blank
        or duplicated; notionals, probabilities, LGDs, or factor loadings are
        non-finite or outside their supported ranges; factor-loading dimensions
        differ or a loading norm exceeds one; conditional recovery is requested
        without exactly one systematic factor; or simulation cannot produce
        finite loss statistics.

    Examples
    --------
    >>> from finstack_quant.models.correlation import (
    ...     CopulaSpec,
    ...     CreditExposure,
    ...     PortfolioLossConfig,
    ...     simulate_portfolio_loss,
    ... )
    >>> exposures = [CreditExposure("A", 100.0, 0.05, 0.6, [0.3])]
    >>> result = simulate_portfolio_loss(exposures, PortfolioLossConfig(200, 42, 0.99, CopulaSpec.gaussian()))
    >>> (len(result.losses), result.expected_loss >= 0.0)
    (200, True)
    >>> import pandas as pd
    >>> frame = pd.DataFrame({"id": ["A"], "notional": [100.0], "pd": [0.05], "lgd": [0.6], "factor_loading": [0.3]})
    >>> simulate_portfolio_loss(
    ...     frame, PortfolioLossConfig(200, 42, 0.99, CopulaSpec.gaussian())
    ... ).losses == result.losses
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
    >>> from finstack_quant.models.correlation import correlation_bounds
    >>> tuple(round(value, 3) for value in correlation_bounds(0.05, 0.03))
    (-0.04, 0.767)

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
    >>> from finstack_quant.models.correlation import joint_probabilities
    >>> probabilities = joint_probabilities(0.05, 0.03, 0.3)
    >>> (round(probabilities[0], 6), round(sum(probabilities), 6))
    (0.012654, 1.0)

    """
    ...

def validate_correlation_matrix(matrix: Sequence[float] | Sequence[Sequence[float]], n: int) -> None:
    """
    Validate a correlation matrix.

    Parameters
    ----------
    matrix : Sequence[float] or Sequence[Sequence[float]]
        Flat row-major correlation matrix (length ``n * n``) or a 2-D
        ``n x n`` list/array of rows.
    n : int
        Dimension of the square matrix.

    Raises
    ------
    ValueError
        If the shape does not match ``n`` (the message states the rows and
        widths found), or the matrix is invalid (diagonal not one, entry out
        of ``[-1, 1]``, not symmetric, not PSD).

    Examples
    --------
    >>> from finstack_quant.models.correlation import validate_correlation_matrix
    >>> validate_correlation_matrix([1.0, 0.3, 0.3, 1.0], 2) is None
    True

    """
    ...

def nearest_correlation(
    matrix: Sequence[float] | Sequence[Sequence[float]],
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
    matrix : Sequence[float] or Sequence[Sequence[float]]
        Flat row-major ``n x n`` input matrix, or a 2-D ``n x n`` list/array.
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
        If the input shape does not match ``n``, is grossly asymmetric, or
        the diagonal is far from 1.
    RuntimeError
        If the projection does not converge within ``max_iter`` iterations.

    Examples
    --------
    >>> from finstack_quant.models.correlation import nearest_correlation, validate_correlation_matrix
    >>> matrix = nearest_correlation([1.0, 1.01, 1.01, 1.0], 2)
    >>> validate_correlation_matrix(matrix, 2) is None
    True

    """
    ...

def cholesky_decompose(matrix: Sequence[float] | Sequence[Sequence[float]], n: int) -> list[float]:
    """
    Pivoted Cholesky decomposition of a correlation matrix.

    Uses diagonal pivoting to handle near-singular and positive-semidefinite
    matrices gracefully.

    Parameters
    ----------
    matrix : Sequence[float] or Sequence[Sequence[float]]
        Flat row-major correlation matrix (length ``n * n``) or a 2-D
        ``n x n`` list/array of rows.
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
        If the matrix shape is wrong, an entry is non-finite, or the matrix is
        indefinite. The message includes the mismatched dimensions or the
        offending position and value.

    Examples
    --------
    >>> from finstack_quant.models.correlation import cholesky_decompose
    >>> factor = cholesky_decompose([1.0, 0.3, 0.3, 1.0], 2)
    >>> (len(factor), round(sum(value * value for value in factor), 2))
    (4, 2.0)

    """
    ...
