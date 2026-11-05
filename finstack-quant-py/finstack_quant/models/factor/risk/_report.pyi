"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/factor/risk/report.rs``."""

from __future__ import annotations

from typing import Any
from finstack_quant.models.factor.risk._budget import RiskBudgetResult
from finstack_quant.models.factor.risk._contributions import PositionRiskDecomposition
from finstack_quant.models.factor.risk._stress import StressAttribution

class PositionRiskReport:
    """
    Position VaR/ES decomposition with its optional risk budget and tail stress
    attribution, as returned by :func:`position_risk_report`.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import position_risk_report
    >>> pnls = [[float(i) for i in range(-50, 50)], [2.0 * i for i in range(-50, 50)]]
    >>> report = position_risk_report({"position_ids": ["A", "B"], "source": {"kind": "historical", "pnls": pnls}})
    >>> report.stress is not None and report.budget is None
    True
    """

    @property
    def decomposition(self) -> PositionRiskDecomposition:
        """
        Portfolio and per-position VaR/ES (losses negative).

        Returns
        -------
        PositionRiskDecomposition
            The decomposition of the request's source.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def budget(self) -> RiskBudgetResult | None:
        """
        Budget evaluation of the decomposition.

        Returns
        -------
        RiskBudgetResult | None
            The per-position budget comparison, or ``None`` when the request had
            no budget.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def stress(self) -> StressAttribution | None:
        """
        Tail stress attribution of a historical source's scenario P&L.

        Returns
        -------
        StressAttribution | None
            The tail attribution, or ``None`` for a parametric source.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON with the canonical field names.

        Returns
        -------
        str
            JSON object with ``decomposition``, ``budget`` and ``stress``.

        Notes
        -----
        Does not raise.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PositionRiskReport:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON object with ``decomposition``, ``budget`` and ``stress``.

        Returns
        -------
        PositionRiskReport
            Reconstructed report.

        Raises
        ------
        ValueError
            If a field is missing or malformed.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionRiskReport, position_risk_report
        >>> source = {"kind": "parametric", "weights": [1.0], "covariance": [[0.04]]}
        >>> report = position_risk_report({"position_ids": ["A"], "source": source})
        >>> PositionRiskReport.from_json(
        ...     report.to_json()
        ... ).decomposition.portfolio_var == report.decomposition.portfolio_var
        True
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

def position_risk_report(request: str | dict[str, Any]) -> PositionRiskReport:
    """
    Decompose position VaR/ES from a parametric or historical source, with an
    optional risk budget and, for scenario P&L, the tail stress attribution.

    Parameters
    ----------
    request : str | dict[str, Any]
        ``PositionRiskRequest`` as a dict or JSON text with ``position_ids``,
        an optional ``confidence`` (decimal strictly inside ``(0.5, 1)``,
        default ``0.95``), a ``source`` tagged by ``kind`` and an optional
        ``budget``. ``{"kind": "parametric", "weights", "covariance",
        "compute_incremental"}`` takes exposures in portfolio currency, an
        ``n x n`` covariance in position order and an incremental-VaR flag
        (default ``False``); ``{"kind": "historical", "pnls"}`` takes one row of
        scenario P&L per position, losses negative. ``budget`` is
        ``{"targets", "utilization_threshold"}``: target VaR shares in position
        order summing to ~1 and an optional breach ratio (default
        ``DEFAULT_UTILIZATION_THRESHOLD``).

    Returns
    -------
    PositionRiskReport
        The decomposition, the budget evaluation (``None`` without a budget)
        and the stress attribution (``None`` for a parametric source).

    Raises
    ------
    ValueError
        If the request is malformed or has an unknown field or kind, the
        source's dimensions disagree with the position ids, the covariance is
        invalid, too few scenarios resolve the tail, the confidence is outside
        ``(0.5, 1)``, or the budget is malformed.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import position_risk_report
    >>> source = {"kind": "parametric", "weights": [1.0, 2.0], "covariance": [[0.04, 0.0], [0.0, 0.01]]}
    >>> report = position_risk_report({"position_ids": ["A", "B"], "source": source})
    >>> round(report.decomposition.portfolio_var, 6)
    -0.465235
    """
    ...
