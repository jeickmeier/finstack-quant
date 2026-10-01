"""Factor and position risk decomposition kernels.

Examples
--------
>>> from finstack_quant.models.factor.risk import DecompositionConfig
>>> DecompositionConfig.parametric_95().confidence
0.95
"""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

import numpy as np
import numpy.typing as npt
import pandas as pd

__all__ = [
    "DEFAULT_UTILIZATION_THRESHOLD",
    "DecompositionConfig",
    "FactorContribution",
    "ParametricEsDecompositionView",
    "PositionBudgetEntry",
    "PositionEsContribution",
    "PositionEsContributionView",
    "PositionFactorContribution",
    "PositionResidualContribution",
    "PositionRiskDecomposition",
    "PositionVarContribution",
    "RiskBudgetResult",
    "RiskDecomposition",
    "StressAttribution",
    "StressPositionEntry",
    "TailScenarioBreakdown",
    "build_stress_attribution",
    "evaluate_risk_budget",
    "historical_var_decomposition",
    "parametric_es_decomposition",
    "parametric_var_decomposition",
    "position_component_var",
]

DEFAULT_UTILIZATION_THRESHOLD: float
"""Default ``actual / target`` utilization ratio (``1.2``) above which
``evaluate_risk_budget`` flags a breach."""

def parametric_var_decomposition(
    position_ids: list[str] | None,
    weights: list[float],
    covariance: list[list[float]] | npt.NDArray[np.float64] | pd.DataFrame,
    confidence: float | None = None,
    compute_incremental: bool = False,
) -> PositionRiskDecomposition:
    """
    Decompose portfolio parametric VaR across positions.

    Parameters
    ----------
    position_ids : list[str] | None
        Position identifiers aligned with ``weights``. May be ``None`` when
        ``covariance`` is a ``pandas.DataFrame`` (its column labels are used).
    weights : list[float]
        Portfolio weights or exposures, one per position.
    covariance : list[list[float]] | numpy.ndarray | pd.DataFrame
        Square covariance matrix aligned with ``position_ids``. C-contiguous
        ``float64`` arrays use the direct buffer path.
    confidence : float | None
        VaR confidence strictly inside ``(0.5, 1)``. ``None`` uses the Rust
        :meth:`DecompositionConfig.parametric_95` preset (``0.95``), as the
        WASM ``parametricVarDecomposition`` does.
    compute_incremental : bool, default False
        Whether to calculate leave-one-out incremental VaR (one full repricing
        per position).

    Returns
    -------
    PositionRiskDecomposition
        Typed portfolio VaR/ES totals and per-position contributions.

    Raises
    ------
    ValueError
        If dimensions do not match, covariance is malformed, or the
        confidence level is invalid.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import parametric_var_decomposition
    >>> result = parametric_var_decomposition(["A", "B"], [1.0, 2.0], [[0.04, 0.0], [0.0, 0.01]])
    >>> round(result.portfolio_var, 6)
    -0.465235
    """
    ...

def parametric_es_decomposition(
    position_ids: list[str] | None,
    weights: list[float],
    covariance: list[list[float]] | npt.NDArray[np.float64] | pd.DataFrame,
    confidence: float | None = None,
) -> ParametricEsDecompositionView:
    """
    Decompose portfolio parametric expected shortfall across positions.

    The ES twin of :func:`parametric_var_decomposition`: runs the same engine
    and returns the ES reporting view (``portfolio_es`` plus per-position
    ``component_es`` / ``pct_contribution`` rows).

    Parameters
    ----------
    position_ids : list[str] | None
        Position identifiers aligned with ``weights``; ``None`` allowed with a
        ``pandas.DataFrame`` covariance.
    weights : list[float]
        Portfolio weights or exposures, one per position.
    covariance : list[list[float]] | numpy.ndarray | pd.DataFrame
        Square covariance matrix aligned with ``position_ids``.
    confidence : float | None
        ES confidence strictly inside ``(0.5, 1)``. ``None`` uses the Rust
        :meth:`DecompositionConfig.parametric_95` preset (``0.95``).

    Returns
    -------
    ParametricEsDecompositionView
        ``portfolio_var``, ``portfolio_es`` (losses negative) and per-position
        ES rows.

    Raises
    ------
    ValueError
        If dimensions do not match, covariance is malformed, or the
        confidence level is invalid.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import parametric_es_decomposition
    >>> result = parametric_es_decomposition(["A", "B"], [1.0, 2.0], [[0.04, 0.0], [0.0, 0.01]])
    >>> round(result.portfolio_es, 6)
    -0.583423
    """
    ...

def historical_var_decomposition(
    position_ids: list[str] | None,
    position_pnls: list[list[float]] | npt.NDArray[np.float64] | pd.DataFrame,
    confidence: float | None = None,
) -> PositionRiskDecomposition:
    """
    Decompose historical VaR from scenario or realized position P&Ls.

    Parameters
    ----------
    position_ids : list[str] | None
        Position identifiers. May be ``None`` when ``position_pnls`` is a
        ``pandas.DataFrame`` (its column labels are used).
    position_pnls : list[list[float]] | numpy.ndarray | pd.DataFrame
        P&L matrix, losses negative. A ``pandas.DataFrame`` is read as rows =
        scenarios, columns = positions. A nested list or 2-D array is read as
        ``n_positions x n_scenarios`` (position-major, one row per position),
        the layout the WASM ``historicalVarDecomposition`` also takes.
    confidence : float | None
        Historical VaR confidence strictly inside ``(0.5, 1)``. ``None`` uses
        the Rust :meth:`DecompositionConfig.historical_95` preset (``0.95``).

    Returns
    -------
    PositionRiskDecomposition
        Typed historical VaR/ES totals and per-position contributions
        (marginal and incremental VaR are ``None``).

    Raises
    ------
    ValueError
        If the P&L matrix is empty, ragged, does not have one row per
        position id, too few scenarios resolve the tail, or the confidence
        level is invalid.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import historical_var_decomposition
    >>> pnl = [[float(i) for i in range(-50, 50)], [2.0 * i for i in range(-50, 50)]]
    >>> historical_var_decomposition(["A", "B"], pnl).portfolio_var < 0.0
    True
    """
    ...

def evaluate_risk_budget(
    position_ids: list[str],
    actual_var: list[float],
    target_var_pct: list[float],
    portfolio_var: float,
    utilization_threshold: float = 1.2,
) -> RiskBudgetResult:
    """
    Compare actual position VaR against target risk-budget shares.

    Parameters
    ----------
    position_ids : list[str]
        Position identifiers aligned with ``actual_var`` and
        ``target_var_pct``.
    actual_var : list[float]
        Position component VaR amounts.
    target_var_pct : list[float]
        Target share of total portfolio VaR per position.
    portfolio_var : float
        Total portfolio VaR used to convert target percentages
        into target VaR amounts.
    utilization_threshold : float, default 1.2
        Breach threshold for actual / target utilization. The default is
        :data:`DEFAULT_UTILIZATION_THRESHOLD`, the Rust constant shared with
        the WASM binding.

    Returns
    -------
    RiskBudgetResult
        Typed per-position utilization, excess VaR, and breach diagnostics.

    Raises
    ------
    ValueError
        If input lengths differ, a position id is duplicated, or risk-budget
        inputs are invalid.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import evaluate_risk_budget
    >>> result = evaluate_risk_budget(["A", "B"], [1.0, 2.0], [0.5, 0.5], 3.0)
    >>> (result.has_breach, result.total_overbudget)
    (True, 0.5)
    """
    ...

class FactorContribution:
    """
    Aggregate contribution of a single factor to portfolio risk.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import FactorContribution
    >>> item = FactorContribution.from_json(
    ...     '{"factor_id":"Rates","absolute_risk":1.0,"relative_risk":0.5,"marginal_risk":0.2}'
    ... )
    >>> (item.factor_id, item.relative_risk)
    ('Rates', 0.5)
    """

    @classmethod
    def from_json(cls, json_str: str) -> FactorContribution:
        """
        Deserialize a factor contribution from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized factor contribution, normally produced by
            ``FactorContribution.to_json``.

        Returns
        -------
        FactorContribution
            Validated `FactorContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import FactorContribution
        >>> item = FactorContribution.from_json(
        ...     '{"factor_id":"Rates","absolute_risk":1.0,"relative_risk":0.5,"marginal_risk":0.2}'
        ... )
        >>> item.absolute_risk
        1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this factor contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `FactorContribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def factor_id(self) -> str:
        """
        Identifier of the contributing risk factor in the model.

        Returns
        -------
        str
            Identifier of the contributing risk factor in the model.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def absolute_risk(self) -> float:
        """
        Absolute risk contribution.

        Returns
        -------
        float
            Absolute risk contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def relative_risk(self) -> float:
        """
        Share of total portfolio risk.

        Returns
        -------
        float
            Share of total portfolio risk.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def marginal_risk(self) -> float:
        """
        Marginal risk contribution.

        Returns
        -------
        float
            Marginal risk contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this entry as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row with columns ``factor_id``, ``absolute_risk``, ``relative_risk``, ``marginal_risk``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionFactorContribution:
    """
    Per-position contribution to a specific factor bucket.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionFactorContribution
    >>> item = PositionFactorContribution.from_json('{"position_id":"P1","factor_id":"Rates","risk_contribution":1.0}')
    >>> (item.position_id, item.factor_id)
    ('P1', 'Rates')
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionFactorContribution:
        """
        Deserialize a position-factor contribution from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized position-factor contribution, normally
            produced by ``PositionFactorContribution.to_json``.

        Returns
        -------
        PositionFactorContribution
            Validated `PositionFactorContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionFactorContribution
        >>> item = PositionFactorContribution.from_json(
        ...     '{"position_id":"P1","factor_id":"Rates","risk_contribution":1.0}'
        ... )
        >>> item.risk_contribution
        1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position-factor contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionFactorContribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Portfolio position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factor_id(self) -> str:
        """
        Identifier of the contributing risk factor in the model.

        Returns
        -------
        str
            Identifier of the contributing risk factor in the model.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def risk_contribution(self) -> float:
        """
        Risk contribution for this position-factor pair.

        Returns
        -------
        float
            Risk contribution for this position-factor pair.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this entry as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row with columns ``position_id``, ``factor_id``, ``risk_contribution``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionResidualContribution:
    """
    Annualized residual variance contributed by a single position.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionResidualContribution
    >>> item = PositionResidualContribution.from_json(
    ...     '{"position_id":"P1","residual_variance":0.1,"source":{"kind":"other"}}'
    ... )
    >>> (item.position_id, item.source_kind)
    ('P1', 'other')
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionResidualContribution:
        """
        Deserialize a residual contribution from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized residual contribution, normally produced by
            ``PositionResidualContribution.to_json``.

        Returns
        -------
        PositionResidualContribution
            Validated `PositionResidualContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionResidualContribution
        >>> item = PositionResidualContribution.from_json(
        ...     '{"position_id":"P1","residual_variance":0.1,"source":{"kind":"other"}}'
        ... )
        >>> item.residual_variance
        0.1
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this residual contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionResidualContribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Portfolio position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def residual_variance(self) -> float:
        """
        Residual variance assigned to this position.

        Returns
        -------
        float
            Residual variance assigned to this position.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def source_kind(self) -> str:
        """
        Source category used to derive residual risk.

        Returns
        -------
        str
            Source category used to derive residual risk.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def source_issuer_id(self) -> str | None:
        """
        Issuer identifier for issuer-sourced residual risk, if present.

        Returns
        -------
            Issuer identifier for issuer-sourced residual risk, if present.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this entry as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row with columns ``position_id``, ``residual_variance``, ``source_kind``, ``source_issuer_id``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class RiskDecomposition:
    """
    Portfolio-level risk decomposition across factors and residuals.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import RiskDecomposition
    >>> doc = '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
    >>> decomposition = RiskDecomposition.from_json(doc)
    >>> (decomposition.total_risk, decomposition.residual_risk)
    (1.0, 1.0)
    """

    @classmethod
    def from_json(cls, json_str: str) -> RiskDecomposition:
        """
        Deserialize a risk decomposition from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized factor-and-residual decomposition, normally
            produced by ``RiskDecomposition.to_json``.

        Returns
        -------
        RiskDecomposition
            Validated `RiskDecomposition` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import RiskDecomposition
        >>> doc = '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
        >>> RiskDecomposition.from_json(doc).measure_json
        '"variance"'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this risk decomposition to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `RiskDecomposition`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def total_risk(self) -> float:
        """
        Total portfolio risk under the decomposition measure.

        Returns
        -------
        float
            Total portfolio risk under the decomposition measure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def measure(self) -> str | dict[str, Any]:
        """
        Risk measure used for aggregation, as its canonical Python value.

        Returns
        -------
        str | dict[str, Any]
            ``"variance"``, ``"volatility"``, ``{"var": {"confidence": c}}`` or
            ``{"expected_shortfall": {"confidence": c}}`` — the same shape as
            ``FactorModelConfig.risk_measure``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def measure_json(self) -> str:
        """
        Risk measure specification as JSON.

        Returns
        -------
        str
            Risk measure specification as JSON.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def residual_risk(self) -> float:
        """
        Residual risk not explained by factor contributions.

        Returns
        -------
        float
            Residual risk not explained by factor contributions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factor_contributions(self) -> list[FactorContribution]:
        """
        Factor-level risk contributions.
        Returns
        -------
        list[FactorContribution]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_factor_contributions(self) -> list[PositionFactorContribution]:
        """
        Position-by-factor risk contributions.
        Returns
        -------
        list[PositionFactorContribution]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_residual_contributions(self) -> list[PositionResidualContribution]:
        """
        Per-position residual risk contributions.
        Returns
        -------
        list[PositionResidualContribution]
            Empty unless the decomposer had per-issuer idiosyncratic vol
            estimates (credit-aware position decomposers).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Primary table: the factor-level risk decomposition.

        Alias of :meth:`to_factor_dataframe`. Every tabular result type in the library
        answers ``to_dataframe()``; the position × factor view stays on
        :meth:`to_position_factor_dataframe`.

        Returns
        -------
        pd.DataFrame
            The same frame :meth:`to_factor_dataframe` returns.

        Examples
        --------
        >>> frame = result.to_dataframe()  # doctest: +SKIP

        Notes
        -----
        This alias does not raise; it delegates to the method named above.
        """
        ...

    def to_factor_dataframe(self) -> pd.DataFrame:
        """
        Export factor contributions as a pandas DataFrame.

        Columns: ``factor_id``, ``absolute_risk``, ``relative_risk``,
        ``marginal_risk``.

        Returns
        -------
        pd.DataFrame
            One row per entry of :attr:`factor_contributions`.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_position_factor_dataframe(self) -> pd.DataFrame:
        """
        Export position x factor contributions as a pandas DataFrame.

        Columns: ``position_id``, ``factor_id``, ``risk_contribution``.

        Returns
        -------
        pd.DataFrame
            One row per entry of :attr:`position_factor_contributions`.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_position_residual_dataframe(self) -> pd.DataFrame:
        """
        Export per-position residual variance contributions as a DataFrame.

        Columns: ``position_id``, ``residual_variance`` (annualized variance,
        non-negative), ``source_kind`` (``"from_credit_model"`` or
        ``"other"``), ``source_issuer_id`` (``None`` unless ``source_kind`` is
        ``"from_credit_model"``).

        Returns
        -------
        pd.DataFrame
            One row per entry of :attr:`position_residual_contributions`; zero
            rows — with the columns still present — when the decomposer
            produced no position-level residual allocation.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionVarContribution:
    """
    Per-position component / marginal VaR.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionVarContribution
    >>> item = PositionVarContribution.from_json(
    ...     '{"position_id":"P1","component_var":-1.0,"relative_var":1.0,"marginal_var":-1.0,"incremental_var":null}'
    ... )
    >>> (item.position_id, item.component_var)
    ('P1', -1.0)
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionVarContribution:
        """
        Deserialize a position VaR contribution from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized component and marginal VaR contribution,
            normally produced by ``PositionVarContribution.to_json``.

        Returns
        -------
        PositionVarContribution
            Validated `PositionVarContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionVarContribution
        >>> item = PositionVarContribution.from_json(
        ...     '{"position_id":"P1","component_var":-1.0,"relative_var":1.0,"marginal_var":-1.0,"incremental_var":null}'
        ... )
        >>> item.relative_var
        1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position VaR contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionVarContribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Portfolio position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def component_var(self) -> float:
        """
        Component VaR assigned to this position.

        Returns
        -------
        float
            Component VaR assigned to this position.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def relative_var(self) -> float:
        """
        Share of total portfolio VaR.

        Returns
        -------
        float
            Share of total portfolio VaR.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def marginal_var(self) -> float | None:
        """
        Marginal VaR, if computed.

        Returns
        -------
            Marginal VaR, if computed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def incremental_var(self) -> float | None:
        """
        Incremental VaR, if requested in the decomposition config.

        Returns
        -------
            Incremental VaR, if requested in the decomposition config.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this contribution as a single-row pandas DataFrame.

        Columns: ``position_id``, ``component_var``, ``relative_var``,
        ``marginal_var``, ``incremental_var``.

        Returns
        -------
        pd.DataFrame
            Exactly one row, for symmetry with
            :meth:`PositionRiskDecomposition.to_dataframe`.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionEsContribution:
    """
    Per-position component / marginal ES.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionEsContribution
    >>> item = PositionEsContribution.from_json(
    ...     '{"position_id":"P1","component_es":-1.2,"relative_es":1.0,"marginal_es":-1.2}'
    ... )
    >>> (item.position_id, item.component_es)
    ('P1', -1.2)
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionEsContribution:
        """
        Deserialize a position ES contribution from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized component and marginal expected-shortfall
            contribution, normally produced by ``PositionEsContribution.to_json``.

        Returns
        -------
        PositionEsContribution
            Validated `PositionEsContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionEsContribution
        >>> item = PositionEsContribution.from_json(
        ...     '{"position_id":"P1","component_es":-1.2,"relative_es":1.0,"marginal_es":-1.2}'
        ... )
        >>> item.relative_es
        1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position ES contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionEsContribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Portfolio position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def component_es(self) -> float:
        """
        Component expected shortfall assigned to this position.

        Returns
        -------
        float
            Component expected shortfall assigned to this position.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def relative_es(self) -> float:
        """
        Share of total portfolio expected shortfall.

        Returns
        -------
        float
            Share of total portfolio expected shortfall.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def marginal_es(self) -> float | None:
        """
        Marginal expected shortfall, if computed.

        Returns
        -------
            Marginal expected shortfall, if computed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this entry as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row with columns ``position_id``, ``component_es``, ``relative_es``, ``marginal_es``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionRiskDecomposition:
    """
    Complete position-level risk decomposition.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionRiskDecomposition
    >>> doc = '{"portfolio_var":-1.0,"portfolio_es":-1.2,"confidence":0.95,"method":"parametric","var_contributions":[],"es_contributions":[],"n_positions":0,"euler_residual":0.0}'
    >>> result = PositionRiskDecomposition.from_json(doc)
    >>> (result.portfolio_var, result.method)
    (-1.0, 'parametric')
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionRiskDecomposition:
        """
        Deserialize a position risk decomposition from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized VaR/ES decomposition, normally produced by
            ``PositionRiskDecomposition.to_json``.

        Returns
        -------
        PositionRiskDecomposition
            Validated `PositionRiskDecomposition` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionRiskDecomposition
        >>> doc = '{"portfolio_var":-1.0,"portfolio_es":-1.2,"confidence":0.95,"method":"parametric","var_contributions":[],"es_contributions":[],"n_positions":0,"euler_residual":0.0}'
        >>> PositionRiskDecomposition.from_json(doc).confidence
        0.95
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position risk decomposition to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionRiskDecomposition`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def portfolio_var(self) -> float:
        """
        Total portfolio Value-at-Risk.
        Returns
        -------
        float
            Portfolio-currency amount under the workspace loss convention, so
            losses are reported as **negative** numbers.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def portfolio_es(self) -> float:
        """
        Portfolio expected shortfall.

        Returns
        -------
        float
            Portfolio expected shortfall.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def confidence(self) -> float:
        """
        Confidence level used for VaR/ES.

        Returns
        -------
        float
            Confidence level used for VaR/ES.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_positions(self) -> int:
        """
        Number of positions included in the decomposition.

        Returns
        -------
        int
            Number of positions included in the decomposition.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def method(self) -> str:
        """
        Decomposition method label.

        Returns
        -------
        str
            Decomposition method label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def euler_residual(self) -> float | None:
        """
        Euler allocation residual, if reported.

        Returns
        -------
            Euler allocation residual, if reported.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def var_contributions(self) -> list[PositionVarContribution]:
        """
        Per-position VaR contributions.
        Returns
        -------
        list[PositionVarContribution]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def es_contributions(self) -> list[PositionEsContribution]:
        """
        Per-position expected shortfall contributions.
        Returns
        -------
        list[PositionEsContribution]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the joined per-position VaR and ES decomposition.

        ``var_contributions`` and ``es_contributions`` are both keyed by
        position, so they are joined on ``position_id`` into one frame. Rows
        follow ``var_contributions`` order; an ES column is ``None`` for a
        position that has no matching ES entry.

        The portfolio-level scalars (:attr:`portfolio_var`,
        :attr:`portfolio_es`, :attr:`confidence`, :attr:`method`) are header
        metadata and are deliberately **not** repeated on every row.

        Columns: ``position_id``, ``component_var``, ``relative_var``,
        ``marginal_var``, ``incremental_var``, ``component_es``,
        ``relative_es``, ``marginal_es``.

        Returns
        -------
        pd.DataFrame
            One row per entry of :attr:`var_contributions`.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionBudgetEntry:
    """
    Per-position budget comparison entry.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionBudgetEntry
    >>> item = PositionBudgetEntry.from_json(
    ...     '{"position_id":"P1","actual_component_var":1.0,"target_component_var":0.8,"utilization":1.25,"excess":0.2}'
    ... )
    >>> (item.position_id, item.utilization)
    ('P1', 1.25)
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionBudgetEntry:
        """
        Deserialize a risk-budget entry from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized per-position budget comparison, normally
            produced by ``PositionBudgetEntry.to_json``.

        Returns
        -------
        PositionBudgetEntry
            Validated `PositionBudgetEntry` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionBudgetEntry
        >>> item = PositionBudgetEntry.from_json(
        ...     '{"position_id":"P1","actual_component_var":1.0,"target_component_var":0.8,"utilization":1.25,"excess":0.2}'
        ... )
        >>> item.excess
        0.2
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this risk-budget entry to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionBudgetEntry`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Portfolio position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def actual_component_var(self) -> float:
        """
        Actual component VaR for this position.

        Returns
        -------
        float
            Actual component VaR for this position.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def target_component_var(self) -> float:
        """
        Target component VaR for this position.

        Returns
        -------
        float
            Target component VaR for this position.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def utilization(self) -> float:
        """
        Actual-to-target utilization ratio.

        Returns
        -------
        float
            Actual-to-target utilization ratio.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def excess(self) -> float:
        """
        Actual component VaR less target component VaR.

        Returns
        -------
        float
            Actual component VaR less target component VaR.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this budget entry as a single-row pandas DataFrame.

        Columns: ``position_id``, ``actual_component_var``,
        ``target_component_var``, ``utilization``, ``excess``.

        Returns
        -------
        pd.DataFrame
            Exactly one row, with the same schema
            :meth:`RiskBudgetResult.to_dataframe` emits.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class RiskBudgetResult:
    """
    Budget evaluation result across positions.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import RiskBudgetResult
    >>> result = RiskBudgetResult.from_json('{"positions":[],"total_overbudget":0.0,"has_breach":false}')
    >>> (result.total_overbudget, result.has_breach)
    (0.0, False)
    """

    @classmethod
    def from_json(cls, json_str: str) -> RiskBudgetResult:
        """
        Deserialize a risk-budget result from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized portfolio risk-budget result, normally
            produced by ``RiskBudgetResult.to_json``.

        Returns
        -------
        RiskBudgetResult
            Validated `RiskBudgetResult` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import RiskBudgetResult
        >>> RiskBudgetResult.from_json('{"positions":[],"total_overbudget":0.0,"has_breach":false}').positions
        []
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this risk-budget result to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `RiskBudgetResult`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def total_overbudget(self) -> float:
        """
        Total amount above target risk budgets.

        Returns
        -------
        float
            Total amount above target risk budgets.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def has_breach(self) -> bool:
        """
        Whether any position exceeds the utilization threshold.
        Returns
        -------
        bool
            Whether this `RiskBudgetResult` has breach.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def positions(self) -> list[PositionBudgetEntry]:
        """
        Per-position risk-budget entries.
        Returns
        -------
        list[PositionBudgetEntry]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-position budget comparison as a pandas DataFrame.

        One row per entry of :attr:`positions`. The scalars
        :attr:`total_overbudget` and :attr:`has_breach` are header metadata
        and are not repeated per row.

        Columns: ``position_id``, ``actual_component_var``,
        ``target_component_var``, ``utilization`` (ratio, not percentage),
        ``excess``.

        Returns
        -------
        pd.DataFrame
            One row per budgeted position.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class StressPositionEntry:
    """
    Single position's contribution to tail stress.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import StressPositionEntry
    >>> item = StressPositionEntry.from_json(
    ...     '{"position_id":"P1","avg_tail_pnl":-5.0,"pct_of_tail_loss":1.0,"worst_scenario_pnl":-10.0}'
    ... )
    >>> (item.position_id, item.worst_scenario_pnl)
    ('P1', -10.0)
    """

    @classmethod
    def from_json(cls, json_str: str) -> StressPositionEntry:
        """
        Deserialize a stress position entry from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized per-position tail-stress contribution, normally
            produced by ``StressPositionEntry.to_json``.

        Returns
        -------
        StressPositionEntry
            Validated `StressPositionEntry` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import StressPositionEntry
        >>> item = StressPositionEntry.from_json(
        ...     '{"position_id":"P1","avg_tail_pnl":-5.0,"pct_of_tail_loss":1.0,"worst_scenario_pnl":-10.0}'
        ... )
        >>> item.pct_of_tail_loss
        1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this stress position entry to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `StressPositionEntry`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Portfolio position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def avg_tail_pnl(self) -> float:
        """
        Average P&L across tail scenarios.

        Returns
        -------
        float
            Average P&L across tail scenarios.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pct_of_tail_loss(self) -> float:
        """
        Share of aggregate tail loss.

        Returns
        -------
        float
            Share of aggregate tail loss.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def worst_scenario_pnl(self) -> float:
        """
        Worst single-scenario P&L for this position.

        Returns
        -------
        float
            Worst single-scenario P&L for this position.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this entry as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row with columns ``position_id``, ``avg_tail_pnl``, ``pct_of_tail_loss``, ``worst_scenario_pnl``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class TailScenarioBreakdown:
    """
    Breakdown of a single tail scenario.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import TailScenarioBreakdown
    >>> scenario = TailScenarioBreakdown.from_json('{"scenario_index":0,"portfolio_pnl":-10.0,"position_pnls":[-10.0]}')
    >>> (scenario.scenario_index, scenario.portfolio_pnl)
    (0, -10.0)
    """

    @classmethod
    def from_json(cls, json_str: str) -> TailScenarioBreakdown:
        """
        Deserialize a tail scenario breakdown from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized tail-scenario P&L breakdown, normally
            produced by ``TailScenarioBreakdown.to_json``.

        Returns
        -------
        TailScenarioBreakdown
            Validated `TailScenarioBreakdown` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import TailScenarioBreakdown
        >>> scenario = TailScenarioBreakdown.from_json(
        ...     '{"scenario_index":0,"portfolio_pnl":-10.0,"position_pnls":[-10.0]}'
        ... )
        >>> scenario.position_pnls
        [-10.0]
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this tail scenario breakdown to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `TailScenarioBreakdown`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def scenario_index(self) -> int:
        """
        Scenario index in the source P&L matrix.

        Returns
        -------
        int
            Scenario index in the source P&L matrix.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def portfolio_pnl(self) -> float:
        """
        Portfolio P&L for this tail scenario.

        Returns
        -------
        float
            Portfolio P&L for this tail scenario.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_pnls(self) -> list[float]:
        """
        Per-position P&L for this scenario, index-aligned to
        ``StressAttribution.position_ids`` (entry ``i`` is the P&L for
        ``position_ids[i]``).

        Returns
        -------
        list[float]
            Per-position P&L for this scenario, index-aligned to

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_ids(self) -> list[str] | None:
        """
        Position ordering inherited from the parent ``StressAttribution``.

        Returns
        -------
        list[str] | None
            Identifiers aligned with :attr:`position_pnls`, or ``None`` for a
            breakdown reconstructed from JSON on its own.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self, position_ids: list[str] | None = None) -> pd.DataFrame:
        """
        Export this scenario's per-position P&L as a pandas DataFrame.

        Columns: ``position_id``, ``pnl`` (portfolio-currency amount; a loss
        is negative).

        Parameters
        ----------
        position_ids : list[str] | None
            Position identifiers aligned with :attr:`position_pnls`. Defaults
            to the ordering inherited from the parent
            ``StressAttribution.position_ids`` when this breakdown was read
            from ``tail_scenarios``; required for a breakdown built via
            :meth:`from_json`.

        Returns
        -------
        pd.DataFrame
            One row per position.

        Raises
        ------
        ValueError
            If no identifiers are available or their count does not match
            ``len(position_pnls)``.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class StressAttribution:
    """
    Per-position attribution of portfolio losses in tail scenarios.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import StressAttribution
    >>> doc = '{"var_threshold":-5.0,"n_tail_scenarios":1,"position_ids":["P1"],"position_contributions":[],"tail_scenarios":[]}'
    >>> result = StressAttribution.from_json(doc)
    >>> (result.var_threshold, result.n_tail_scenarios)
    (-5.0, 1)
    """

    @classmethod
    def from_json(cls, json_str: str) -> StressAttribution:
        """
        Deserialize stress attribution from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized tail-loss attribution, normally produced by
            ``StressAttribution.to_json``.

        Returns
        -------
        StressAttribution
            Validated `StressAttribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import StressAttribution
        >>> doc = '{"var_threshold":-5.0,"n_tail_scenarios":1,"position_ids":["P1"],"position_contributions":[],"tail_scenarios":[]}'
        >>> StressAttribution.from_json(doc).position_ids
        ['P1']
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this stress attribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `StressAttribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def var_threshold(self) -> float:
        """
        VaR threshold used to select tail scenarios.

        Returns
        -------
        float
            VaR threshold used to select tail scenarios.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_tail_scenarios(self) -> int:
        """
        Number of scenarios classified as tail scenarios.

        Returns
        -------
        int
            Number of scenarios classified as tail scenarios.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_ids(self) -> list[str]:
        """
        Canonical position ordering shared by every ``tail_scenarios`` entry.
        ``tail_scenarios[k].position_pnls[i]`` is the P&L for ``position_ids[i]``.

        Returns
        -------
        list[str]
            Canonical position ordering shared by every ``tail_scenarios`` entry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_contributions(self) -> list[StressPositionEntry]:
        """
        Per-position tail-loss contributions.
        Returns
        -------
        list[StressPositionEntry]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tail_scenarios(self) -> list[TailScenarioBreakdown]:
        """
        Detailed tail scenario breakdowns.
        Returns
        -------
        list[TailScenarioBreakdown]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-position tail-loss contributions as a DataFrame.

        One row per entry of :attr:`position_contributions`, in the same
        (largest-driver-first) order.

        Columns: ``position_id``, ``avg_tail_pnl`` (portfolio-currency average
        across tail scenarios; a loss is negative), ``pct_of_tail_loss``
        (**fraction**, not percentage, of total portfolio tail loss),
        ``worst_scenario_pnl``.

        Returns
        -------
        pd.DataFrame
            One row per contributing position.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_scenario_dataframe(self) -> pd.DataFrame:
        """
        Export the tail scenario x position P&L matrix as a DataFrame.

        Rows are tail scenarios indexed by
        ``TailScenarioBreakdown.scenario_index``; columns are the position
        identifiers from :attr:`position_ids`, in that order. Every cell is a
        portfolio-currency P&L (a loss is negative).

        Columns: one per entry of :attr:`position_ids`.

        Returns
        -------
        pd.DataFrame
            One row per tail scenario, indexed by scenario index.

        Raises
        ------
        ValueError
            If any tail scenario's ``position_pnls`` width disagrees with
            ``len(position_ids)`` (only reachable through a hand-built
            :meth:`from_json` payload).
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class DecompositionConfig:
    """
    Configuration for position-level VaR / ES decomposition.

    Holds the tail ``confidence`` (decimal probability in ``(0.5, 1)``), the
    ``method`` (``"parametric"`` or ``"historical"``) and whether leave-one-out
    incremental VaR is computed. The decomposition functions take
    ``confidence`` / ``compute_incremental`` directly and resolve an omitted
    confidence to the :meth:`parametric_95` / :meth:`historical_95` presets.
    Instances compare equal field-wise and pickle through JSON.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import DecompositionConfig
    >>> cfg = DecompositionConfig.parametric(0.975).with_incremental()
    >>> (cfg.confidence, cfg.method, cfg.compute_incremental)
    (0.975, 'parametric', True)
    >>> DecompositionConfig.from_json(cfg.to_json()) == cfg
    True
    """

    @classmethod
    def parametric(cls, confidence: float) -> DecompositionConfig:
        """
        Parametric configuration at an arbitrary confidence level.

        Parameters
        ----------
        confidence : float
            Tail confidence as a decimal probability strictly inside
            ``(0.5, 1)``, e.g. ``0.95``.

        Returns
        -------
        DecompositionConfig
            Parametric config with incremental VaR disabled.

        Notes
        -----
        This method does not raise; the confidence is validated when the
        config is used.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import DecompositionConfig
        >>> DecompositionConfig.parametric(0.9).confidence
        0.9
        """
        ...

    @classmethod
    def parametric_95(cls) -> DecompositionConfig:
        """
        Default 95% parametric VaR decomposition config.

        Returns
        -------
        DecompositionConfig
            Parametric 95% config with incremental VaR disabled.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import DecompositionConfig
        >>> (DecompositionConfig.parametric_95().method, DecompositionConfig.parametric_95().confidence)
        ('parametric', 0.95)
        """
        ...

    @classmethod
    def parametric_99(cls) -> DecompositionConfig:
        """
        Default 99% parametric VaR decomposition config.

        Returns
        -------
        DecompositionConfig
            Parametric 99% config with incremental VaR disabled.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import DecompositionConfig
        >>> DecompositionConfig.parametric_99().confidence
        0.99
        """
        ...

    @classmethod
    def historical(cls, confidence: float) -> DecompositionConfig:
        """
        Build a historical VaR decomposition configuration.

        Parameters
        ----------
        confidence : float
            VaR confidence as a decimal probability strictly inside ``(0.5, 1)``.

        Returns
        -------
        DecompositionConfig
            Historical config at the supplied confidence.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import DecompositionConfig
        >>> DecompositionConfig.historical(0.975).method
        'historical'
        """
        ...

    @classmethod
    def historical_95(cls) -> DecompositionConfig:
        """
        Default 95% historical VaR decomposition config.

        The preset :func:`historical_var_decomposition` and
        :func:`build_stress_attribution` use when ``confidence`` is omitted.

        Returns
        -------
        DecompositionConfig
            Historical 95% config with incremental VaR disabled.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import DecompositionConfig
        >>> (DecompositionConfig.historical_95().method, DecompositionConfig.historical_95().confidence)
        ('historical', 0.95)
        """
        ...

    def with_incremental(self) -> DecompositionConfig:
        """
        Return a copy that requests leave-one-out incremental VaR.

        Returns
        -------
        DecompositionConfig
            Copy with ``compute_incremental`` set.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def from_json(json_str: str) -> DecompositionConfig:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json_str : str
            Object with ``confidence``, ``method`` and ``compute_incremental``.

        Returns
        -------
        DecompositionConfig
            Parsed configuration.

        Raises
        ------
        ValueError
            If the JSON is malformed or names an unknown field.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import DecompositionConfig
        >>> DecompositionConfig.from_json(
        ...     '{"confidence":0.99,"method":"historical","compute_incremental":false}'
        ... ).method
        'historical'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Compact JSON with ``confidence``, ``method`` and ``compute_incremental``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def confidence(self) -> float:
        """
        Tail confidence as a decimal probability (``0.95``, not ``95``).

        Returns
        -------
        float
            VaR/ES confidence level.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def method(self) -> str:
        """
        Decomposition method label.

        Returns
        -------
        str
            ``"parametric"`` or ``"historical"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def compute_incremental(self) -> bool:
        """
        Whether leave-one-out incremental VaR is requested.

        Returns
        -------
        bool
            ``True`` when incremental VaR is computed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionEsContributionView:
    """
    One position's row in a :class:`ParametricEsDecompositionView`.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionEsContributionView
    >>> row = PositionEsContributionView.from_json(
    ...     '{"position_id":"A","component_es":-1.5,"marginal_es":null,"pct_contribution":0.6}'
    ... )
    >>> (row.position_id, row.pct_contribution)
    ('A', 0.6)
    """

    @staticmethod
    def from_json(json_str: str) -> PositionEsContributionView:
        """
        Parse from a JSON string.

        Parameters
        ----------
        json_str : str
            Object with ``position_id``, ``component_es``, ``marginal_es`` and
            ``pct_contribution``.

        Returns
        -------
        PositionEsContributionView
            Parsed row.

        Raises
        ------
        ValueError
            If the JSON is malformed.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionEsContributionView
        >>> PositionEsContributionView.from_json(
        ...     '{"position_id":"A","component_es":-1.0,"marginal_es":null,"pct_contribution":1.0}'
        ... ).component_es
        -1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def component_es(self) -> float:
        """
        Component Expected Shortfall allocated to the position.

        Returns
        -------
        float
            Portfolio-currency amount; losses are negative.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def marginal_es(self) -> float | None:
        """
        Marginal ES, when the engine produced a gradient.

        Returns
        -------
        float | None
            Same sign convention as :attr:`component_es`; ``None`` otherwise.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pct_contribution(self) -> float:
        """
        Share of portfolio ES contributed by this position.

        Returns
        -------
        float
            A fraction (not a percentage); sums to ``1.0`` across positions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this row as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Columns ``position_id``, ``component_es``, ``marginal_es``,
            ``pct_contribution``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class ParametricEsDecompositionView:
    """
    Expected-shortfall view of a parametric position decomposition.

    Returned by :func:`parametric_es_decomposition`.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import parametric_es_decomposition
    >>> es = parametric_es_decomposition(["A", "B"], [1.0, 2.0], [[0.04, 0.0], [0.0, 0.01]])
    >>> (es.n_positions, round(es.portfolio_es, 6))
    (2, -0.583423)
    """

    @staticmethod
    def from_json(json_str: str) -> ParametricEsDecompositionView:
        """
        Parse from a JSON string.

        Parameters
        ----------
        json_str : str
            Object with ``portfolio_var``, ``portfolio_es``, ``confidence``,
            ``n_positions`` and ``contributions``.

        Returns
        -------
        ParametricEsDecompositionView
            Parsed view.

        Raises
        ------
        ValueError
            If the JSON is malformed.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import ParametricEsDecompositionView
        >>> doc = '{"portfolio_var":-1.0,"portfolio_es":-1.2,"confidence":0.95,"n_positions":0,"contributions":[]}'
        >>> ParametricEsDecompositionView.from_json(doc).portfolio_es
        -1.2
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def portfolio_var(self) -> float:
        """
        Total portfolio VaR.

        Returns
        -------
        float
            Portfolio-currency amount; losses are negative.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def portfolio_es(self) -> float:
        """
        Total portfolio Expected Shortfall.

        Returns
        -------
        float
            Portfolio-currency amount; losses are negative and
            ``portfolio_es <= portfolio_var``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def confidence(self) -> float:
        """
        Tail confidence as a decimal probability.

        Returns
        -------
        float
            Strictly inside ``(0.5, 1)``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_positions(self) -> int:
        """
        Number of positions in the decomposition.

        Returns
        -------
        int
            Position count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def contributions(self) -> list[PositionEsContributionView]:
        """
        Per-position ES rows.

        Returns
        -------
        list[PositionEsContributionView]
            One entry per position, in input order.

        Notes
        -----
        This accessor does not raise; it returns copies of the stored rows.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-position ES rows as a pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Columns ``position_id``, ``component_es``, ``marginal_es``,
            ``pct_contribution`` (fraction, not percentage). The portfolio
            scalars stay on their properties.

        Raises
        ------
        ValueError
            If pandas cannot build the frame.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def build_stress_attribution(
    position_ids: list[str] | None,
    position_pnls: list[list[float]] | npt.NDArray[np.float64] | pd.DataFrame,
    confidence: float | None = None,
) -> StressAttribution:
    """
    Build tail-scenario stress attribution from position P&Ls.

    Parameters
    ----------
    position_ids : list[str] | None
        Position identifiers. May be ``None`` when ``position_pnls`` is a
        ``pandas.DataFrame`` (its column labels are used).
    position_pnls : list[list[float]] | numpy.ndarray | pd.DataFrame
        P&L matrix, losses negative. A ``pandas.DataFrame`` is read as rows =
        scenarios, columns = positions. A nested list or 2-D array is read as
        ``n_positions x n_scenarios`` (position-major, one row per position).
        C-contiguous ``float64`` arrays use the direct buffer path.
    confidence : float | None
        Tail confidence level in ``(0.5, 1)``. The Rust engine selects
        ``floor((1 - confidence) * n_scenarios)`` tail scenarios. ``None``
        uses the confidence of the Rust
        :meth:`DecompositionConfig.historical_95` preset (``0.95``).

    Returns
    -------
    StressAttribution
        VaR threshold, tail scenario count, per-position tail contributions
        and scenario-level P&L breakdowns.

    Raises
    ------
    ValueError
        If dimensions are inconsistent or the matrix does not have one row per
        position id, confidence is outside ``(0.5, 1)``, the requested tail
        has zero scenarios, or any P&L is non-finite.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import build_stress_attribution
    >>> pnl = [list(range(-10, 10)), [2 * value for value in range(-10, 10)]]
    >>> result = build_stress_attribution(["A", "B"], pnl)
    >>> (result.var_threshold, result.n_tail_scenarios)
    (-30.0, 1)
    """
    ...

def position_component_var(
    decomp: PositionRiskDecomposition,
    position_id: str,
) -> float:
    """
    Look up a position's component VaR inside a decomposition.

    Parameters
    ----------
    decomp : PositionRiskDecomposition
        Typed risk decomposition containing component VaR by position.
    position_id : str
        Position identifier whose component VaR is required; absent IDs raise
        ``KeyError``.

    Returns
    -------
    float
        Loss-signed component VaR for the requested position and portfolio measure.

    Raises
    ------
    KeyError
        If ``position_id`` is absent from ``decomp``.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionRiskDecomposition, position_component_var
    >>> doc = '{"portfolio_var":-1.0,"portfolio_es":-1.2,"confidence":0.95,"method":"parametric","var_contributions":[{"position_id":"P1","component_var":-1.0,"relative_var":1.0,"marginal_var":-1.0,"incremental_var":null}],"es_contributions":[],"n_positions":1,"euler_residual":0.0}'
    >>> position_component_var(PositionRiskDecomposition.from_json(doc), "P1")
    -1.0
    """
    ...
