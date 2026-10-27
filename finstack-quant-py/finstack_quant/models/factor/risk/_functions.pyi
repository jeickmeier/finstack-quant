"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/factor/risk/functions.rs``."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any
import numpy as np
import numpy.typing as npt
import pandas as pd
from finstack_quant.portfolio import SensitivityMatrix
from finstack_quant.models.factor.risk._contributions import PositionRiskDecomposition, RiskDecomposition

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
        VaR confidence strictly inside ``(0.5, 1)``. ``None`` uses ``0.95``,
        as the WASM ``parametricVarDecomposition`` does.
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
        ``0.95``.

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

def decompose_factors(
    sensitivities: SensitivityMatrix,
    covariance_json: str | Mapping[str, Any],
    risk_measure: str | Mapping[str, Any] | None = None,
) -> RiskDecomposition:
    """
    Decompose portfolio risk into factor and position contributions.

    Uses the parametric (covariance-based) Euler decomposition to attribute
    forecasted portfolio risk across factors and individual positions. Binds
    the Rust ``finstack_quant_factor::risk::decompose_factors``; the WASM twin
    is ``models.factor.risk.decomposeFactors``.

    Parameters
    ----------
    sensitivities : SensitivityMatrix
        Weighted position x factor sensitivity matrix, as returned by
        :func:`finstack_quant.portfolio.compute_factor_sensitivities` or
        :meth:`finstack_quant.portfolio.SensitivityMatrix.from_json`.
        Sensitivities are in the matrix's base currency per factor bump unit.
    covariance_json : str or Mapping[str, Any]
        ``FactorCovarianceMatrix`` as JSON text or a dict
        (``{"factor_ids": [...], "n": n, "data": [row-major n x n]}``) of
        factor-return variances and covariances. Must use the same factor IDs
        and ordering as ``sensitivities``.
    risk_measure : str or Mapping[str, Any] or None, optional
        ``RiskMeasure`` wire value: ``"variance"``, ``"volatility"``,
        ``{"var": {"confidence": 0.99}}`` or
        ``{"expected_shortfall": {"confidence": 0.975}}``. ``None`` uses the
        Rust default, ``"variance"``.

    Returns
    -------
    RiskDecomposition
        Portfolio-level risk in the requested measure with factor and
        position detail.

    Raises
    ------
    ValueError
        If the covariance or risk measure is malformed, the factor axes of
        ``sensitivities`` and the covariance disagree, or the decomposition
        produces a non-finite value.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import decompose_factors
    >>> from finstack_quant.portfolio import SensitivityMatrix
    >>> matrix = SensitivityMatrix.from_json(
    ...     '{"base_currency":"USD","position_ids":["P"],"factor_ids":["F"],"data":[[2.0]]}'
    ... )
    >>> covariance = '{"factor_ids":["F"],"n":1,"data":[0.04]}'
    >>> round(decompose_factors(matrix, covariance).total_risk, 6)
    0.16
    >>> round(decompose_factors(matrix, covariance, "volatility").total_risk, 6)
    0.4
    """
    ...
