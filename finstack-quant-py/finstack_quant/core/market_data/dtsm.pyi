"""Dynamic term-structure model bindings: Diebold-Li and yield-curve PCA.

Provides a function-based API for:

- Diebold-Li (2006) dynamic Nelson-Siegel factor extraction.
- Diebold-Li VAR(1) forecast of the yield curve.
- PCA decomposition of yield-curve changes.
- PCA-based scenario generation (N-sigma shocks along principal components).
"""

from __future__ import annotations

from typing import Any

__all__ = [
    "diebold_li_fit_factors",
    "diebold_li_forecast",
    "yield_pca_fit",
    "yield_pca_scenario",
]

def diebold_li_fit_factors(
    tenors: list[float],
    yields_matrix: list[list[float]],
    lambda_decay: float = 0.7308,
    /,
) -> dict[str, Any]:
    """Extract Nelson-Siegel factors from a yield panel via Diebold-Li.

    The decay parameter is exposed positionally as ``lambda`` in the runtime
    binding; the stub uses ``lambda_decay`` because ``lambda`` is a Python
    keyword.
    """
    ...

def diebold_li_forecast(
    tenors: list[float],
    yields_matrix: list[list[float]],
    horizon: int,
    lambda_decay: float = 0.7308,
    /,
) -> dict[str, Any]:
    """VAR(1) forecast of Diebold-Li factors out to ``horizon`` periods.

    The decay parameter is exposed positionally as ``lambda`` in the runtime
    binding; the stub uses ``lambda_decay`` because ``lambda`` is a Python
    keyword.
    """
    ...

def yield_pca_fit(
    yield_changes: list[list[float]],
    n_components: int = 3,
) -> dict[str, Any]:
    """PCA decomposition of a yield-change panel."""
    ...

def yield_pca_scenario(
    yield_changes: list[list[float]],
    component_index: int,
    sigma_shock: float,
    n_components: int = 3,
) -> list[float]:
    """Single-component N-sigma PCA scenario shift to the yield curve."""
    ...
