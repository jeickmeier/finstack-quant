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
import polars as pl

from finstack_quant.core.table import ArrowTable

from finstack_quant.models.correlation._copula import (
    CopulaSpec as CopulaSpec,
    Copula as Copula,
)
from finstack_quant.models.correlation._recovery import (
    RecoverySpec as RecoverySpec,
    RecoveryModel as RecoveryModel,
)
from finstack_quant.models.correlation._latent import (
    LatentFactorSpec as LatentFactorSpec,
    LatentFactorKind as LatentFactorKind,
    LatentSingleFactor as LatentSingleFactor,
    LatentTwoFactor as LatentTwoFactor,
    LatentMultiFactor as LatentMultiFactor,
)
from finstack_quant.models.correlation._matrix import (
    CreditExposure as CreditExposure,
    PortfolioLossConfig as PortfolioLossConfig,
    PortfolioLossResult as PortfolioLossResult,
    TrancheLossStatistics as TrancheLossStatistics,
    CorrelatedBernoulli as CorrelatedBernoulli,
    simulate_portfolio_loss as simulate_portfolio_loss,
    correlation_bounds as correlation_bounds,
    validate_correlation_matrix as validate_correlation_matrix,
    nearest_correlation as nearest_correlation,
    cholesky_correlation as cholesky_correlation,
)

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
    "cholesky_correlation",
    "correlation_bounds",
    "nearest_correlation",
    "simulate_portfolio_loss",
    "validate_correlation_matrix",
]

MAX_PORTFOLIO_LOSS_PATHS: int
