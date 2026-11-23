"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/correlation/matrix.rs``."""

from __future__ import annotations

from typing import Any, Sequence
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable
from finstack_quant.models.correlation._copula import Copula, CopulaSpec
from finstack_quant.models.correlation._recovery import RecoverySpec

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

    @staticmethod
    def from_losses(losses: list[float], confidence: float) -> PortfolioLossResult:
        """
        Aggregate a finite loss distribution under loss-positive conventions.

        The WASM ``PortfolioLossResult.fromLosses`` twin: both hosts call the
        Rust ``PortfolioLossResult::from_losses``.

        Parameters
        ----------
        losses : list[float]
            Loss-positive path losses in one caller-defined unit, one entry per
            simulated path.
        confidence : float
            Loss-positive VaR and expected-shortfall confidence strictly
            between 0 and 1.

        Returns
        -------
        PortfolioLossResult
            Losses with the nearest-rank VaR at ``confidence`` and the expected
            shortfall over exactly the worst ``1 - confidence`` probability
            mass (fractional weight on the boundary observation).

        Raises
        ------
        ValueError
            If ``losses`` is empty, a loss is non-finite or negative, or
            ``confidence`` is outside ``(0, 1)``.

        Examples
        --------
        >>> from finstack_quant.models.correlation import PortfolioLossResult
        >>> result = PortfolioLossResult.from_losses([0.0, 1.0, 2.0, 5.0, 10.0], 0.75)
        >>> (result.var, result.expected_shortfall)
        (5.0, 9.0)
        """
        ...

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
        attach_pct: float,
        detach_pct: float,
        pool_notional: float,
    ) -> TrancheLossStatistics:
        """
        Compute tranche loss statistics over this simulated loss distribution.

        Each path's pool loss fraction ``L = loss / pool_notional`` maps through
        the standard tranche loss function
        ``clamp(L - attachment, 0, detachment - attachment) / (detachment - attachment)``,
        where the boundaries in the formula are fractions. The resulting
        distribution is aggregated at this result's own
        :attr:`confidence`.

        Parameters
        ----------
        attach_pct : float
            Lower tranche boundary in percentage points of pool notional.
            Must be in ``[0, 100)``. A 0–3% equity tranche uses ``0.0``.
        detach_pct : float
            Upper tranche boundary in percentage points of pool notional.
            Must exceed ``attach_pct`` and be at most 100. A 0–3% equity
            tranche uses ``3.0``.
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
            If a boundary lies outside ``[0, 100]``, ``attach_pct >= detach_pct``,
            or ``pool_notional`` is not finite and strictly positive.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export the simulated loss distribution as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Single-column ``loss`` table with one row per simulated path.

        Raises
        ------
        ValueError
            If the result cannot be represented as an Arrow table.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export the simulated loss distribution as a pandas ``DataFrame``.

        Columns: ``loss``. Aggregate statistics live on :meth:`to_summary_table`.

        Returns
        -------
        pd.DataFrame
            One row per simulated path in deterministic path order.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If the result cannot be represented as a pandas DataFrame.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export the simulated loss distribution as a Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            One row per simulated path with column ``loss``.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If the result cannot be represented as a Polars DataFrame.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export the simulated loss distribution as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row per simulated path with column ``loss``.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        ValueError
            If the result cannot be represented as a PyArrow table.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def to_summary_table(self) -> ArrowTable:
        """
        Export aggregate loss statistics as a single-row :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``expected_loss``, ``var``, ``expected_shortfall``,
        ``confidence``, ``num_paths``.

        ``var`` and ``expected_shortfall`` are loss-positive, matching the Rust
        convention: a larger number is a worse outcome.

        Returns
        -------
        ArrowTable
            Single-row table of the distribution's aggregate statistics.

        Raises
        ------
        ValueError
            If the summary table cannot be constructed.
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
        >>> PortfolioLossResult.from_json(result.to_json()).expected_loss == result.expected_loss
        True

        Raises
        ------
        ValueError
            If ``json`` is malformed, does not match the serialized schema, has
            an invalid loss or confidence, or carries aggregates that disagree
            with its losses (deserialization reruns :meth:`from_losses`).
        """
        ...

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
    >>> statistics = result.tranche_loss_statistics(0.0, 10.0, 200.0)
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

    def to_arrow(self) -> ArrowTable:
        """
        Export tranche loss statistics as a single-row :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``attachment``, ``detachment``, ``tranche_notional``,
        ``expected_loss_fraction``, ``expected_loss_amount``, ``var_fraction``,
        ``var_amount``, ``expected_shortfall_fraction``,
        ``expected_shortfall_amount``, ``prob_attachment_breached``,
        ``prob_full_writedown``.

        Returns
        -------
        ArrowTable
            Single-row table describing this tranche.

        Raises
        ------
        ValueError
            If the result cannot be represented as an Arrow table.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export tranche loss statistics as a single-row pandas ``DataFrame``.

        Stack multiple tranches with::

            pd.concat(
                [
                    result.tranche_loss_statistics(100 * a, 100 * d, pool).to_pandas()
                    for a, d in [(0.0, 0.03), (0.03, 0.07), (0.07, 1.0)]
                ],
                ignore_index=True,
            )

        Returns
        -------
        pd.DataFrame
            Single-row frame describing this tranche.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If the result cannot be represented as a pandas DataFrame.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export tranche loss statistics as a single-row Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            Single-row frame describing this tranche.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If the result cannot be represented as a Polars DataFrame.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export tranche loss statistics as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Single-row table describing this tranche.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        ValueError
            If the result cannot be represented as a PyArrow table.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
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
        >>> stats = result.tranche_loss_statistics(0.0, 3.0, 1e6)
        >>> TrancheLossStatistics.from_json(stats.to_json()).detachment
        0.03

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized schema.
        """
        ...

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

        Correlation must satisfy the Fréchet-Hoeffding bounds for the
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
            rejected. For a deterministic marginal supply zero.

        Raises
        ------
        ValueError
            If a marginal is not finite and in ``[0, 1]`` or correlation is
            outside the feasible interval for these marginals.
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
        Feasible correlation supplied at construction.

        Returns
        -------
        float
            Feasible correlation supplied at construction.

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
    exposures: Sequence[CreditExposure] | pd.DataFrame | pl.DataFrame | Any,
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
    exposures : Sequence[CreditExposure] or pandas.DataFrame or polars.DataFrame or Any
        Obligors to simulate, each with exposure, marginal PD, LGD, and factor
        loadings compatible with ``config.copula``. A pandas/Polars DataFrame
        or Arrow table's columns are the ``CreditExposure`` field names:
        ``id``, ``notional``, ``default_probability``, ``lgd`` and
        ``factor_loadings`` (a list per name).
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
        If a DataFrame record does not match ``CreditExposure``; the path count or
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
    >>> frame = pd.DataFrame({
    ...     "id": ["A"],
    ...     "notional": [100.0],
    ...     "default_probability": [0.05],
    ...     "lgd": [0.6],
    ...     "factor_loadings": [[0.3]],
    ... })
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

def validate_correlation_matrix(matrix: Sequence[float] | Sequence[Sequence[float]], n: int) -> None:
    """
    Validate a correlation matrix.

    Parameters
    ----------
    matrix : Sequence[float] or Sequence[Sequence[float]]
        Flat row-major correlation matrix (length ``n * n``) or a 2-D
        ``n x n`` list/array of rows.
    n : int
        Non-negative square-matrix dimension representable as the platform's
        unsigned pointer-sized integer; ``n * n`` must also fit that range.

    Raises
    ------
    ValueError
        If ``n * n`` overflows the platform's unsigned pointer-sized integer
        range, the shape does not match ``n`` (the Rust shape check names the
        row count or the first row of the wrong width), or the matrix is
        invalid (diagonal not one, entry out of ``[-1, 1]``, not symmetric,
        not PSD).
    OverflowError
        If ``n`` is negative or exceeds the platform's unsigned
        pointer-sized integer range.
    TypeError
        If ``n`` is not an integer.

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
        Non-negative matrix dimension representable as the platform's
        unsigned pointer-sized integer; ``n * n`` must also fit that range.
    max_iter : int or None
        Non-negative maximum alternating-projection iterations,
        representable as the platform's unsigned pointer-sized integer.
        Defaults to the Rust ``NearestCorrelationOpts::default()`` value
        (currently ``200``).
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
        If ``n * n`` overflows the platform's unsigned pointer-sized integer
        range, the input shape does not match ``n``, is grossly asymmetric,
        or the diagonal is far from 1.
    OverflowError
        If ``n`` or ``max_iter`` is negative or exceeds the platform's
        unsigned pointer-sized integer range.
    TypeError
        If ``n`` or a supplied ``max_iter`` is not an integer.
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

def cholesky_correlation(matrix: Sequence[float] | Sequence[Sequence[float]], n: int) -> list[float]:
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
        Non-negative square-matrix dimension representable as the platform's
        unsigned pointer-sized integer; ``n * n`` must also fit that range.

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
        If ``n * n`` overflows the platform's unsigned pointer-sized integer
        range, the matrix shape is wrong, an entry is non-finite, or the
        matrix is indefinite. The message includes the mismatched dimensions
        or the offending position and value.
    OverflowError
        If ``n`` is negative or exceeds the platform's unsigned
        pointer-sized integer range.
    TypeError
        If ``n`` is not an integer.

    Examples
    --------
    >>> from finstack_quant.models.correlation import cholesky_correlation
    >>> factor = cholesky_correlation([1.0, 0.3, 0.3, 1.0], 2)
    >>> (len(factor), round(sum(value * value for value in factor), 2))
    (4, 2.0)

    """
    ...
