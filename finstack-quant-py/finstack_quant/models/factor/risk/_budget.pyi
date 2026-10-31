"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/factor/risk/budget.rs``."""

from __future__ import annotations

import pandas as pd
from finstack_quant.models.factor.risk._contributions import PositionRiskDecomposition

def evaluate_risk_budget_from_decomposition(
    decomp: PositionRiskDecomposition,
    target_var_pct: list[float],
    utilization_threshold: float | None = None,
) -> RiskBudgetResult:
    """
    Evaluate a risk budget against a position risk decomposition.

    Reads each position's component VaR and the portfolio VaR from ``decomp``,
    then applies ``evaluate_risk_budget``. Rows keep the decomposition's position
    order, which is the order the positions were supplied to it.

    Parameters
    ----------
    decomp : PositionRiskDecomposition
        Decomposition supplying the position ids, component VaRs (losses
        negative) and portfolio VaR.
    target_var_pct : list[float]
        Target fraction of portfolio VaR in ``[0, 1]`` per position, aligned
        with ``decomp.var_contributions``; a non-empty budget must sum to one
        within 0.05.
    utilization_threshold : float, optional
        Actual-to-target ratio above which a breach is flagged. ``None`` uses
        the Rust default ``DEFAULT_UTILIZATION_THRESHOLD`` (1.2).

    Returns
    -------
    RiskBudgetResult
        Per-position utilization and excess, ``total_overbudget`` and ``has_breach``.

    Raises
    ------
    ValueError
        If ``target_var_pct`` does not have one entry per position, a target is
        non-finite or outside ``[0, 1]``, targets do not sum to one, the threshold
        is non-positive, or non-zero component VaR is paired with zero portfolio VaR.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionRiskDecomposition, evaluate_risk_budget_from_decomposition
    >>> doc = '{"portfolio_var":-1.0,"portfolio_es":-1.2,"confidence":0.95,"method":"parametric","var_contributions":[{"position_id":"P1","component_var":-1.0,"relative_var":1.0,"marginal_var":-1.0,"incremental_var":null}],"es_contributions":[],"n_positions":1,"euler_residual":0.0}'
    >>> evaluate_risk_budget_from_decomposition(PositionRiskDecomposition.from_json(doc), [1.0]).has_breach
    False
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
        Finite position component VaR amounts.
    target_var_pct : list[float]
        Finite target share in ``[0, 1]`` per position; shares must sum to one
        within an absolute tolerance of ``0.05``.
    portfolio_var : float
        Finite total portfolio VaR used to convert target percentages
        into target VaR amounts.
    utilization_threshold : float, default 1.2
        Positive finite breach threshold for actual / target utilization. The default is
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

    @staticmethod
    def from_json(json: str) -> PositionBudgetEntry:
        """
        Deserialize a risk-budget entry from canonical JSON.

        Parameters
        ----------
        json : str
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

    @staticmethod
    def from_json(json: str) -> RiskBudgetResult:
        """
        Deserialize a risk-budget result from canonical JSON.

        Parameters
        ----------
        json : str
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
