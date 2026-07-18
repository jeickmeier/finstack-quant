"""Correlation infrastructure: copulas, factor models, recovery models.

Bindings for the ``finstack_quant_valuations::correlation`` Rust module. Nested
under :mod:`finstack_quant.valuations` to mirror the Rust crate layout where
correlation lives inside ``finstack-quant-valuations``.

Examples:
--------
>>> import finstack_quant.valuations.correlation as correlation
>>> correlation.__name__
'finstack_quant.valuations.correlation'
"""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

_corr = _valuations.correlation

CopulaSpec = _corr.CopulaSpec
Copula = _corr.Copula
RecoverySpec = _corr.RecoverySpec
RecoveryModel = _corr.RecoveryModel
LatentFactorSpec = _corr.LatentFactorSpec
LatentFactorKind = _corr.LatentFactorKind
LatentSingleFactor = _corr.LatentSingleFactor
LatentTwoFactor = _corr.LatentTwoFactor
LatentMultiFactor = _corr.LatentMultiFactor
CorrelatedBernoulli = _corr.CorrelatedBernoulli
CreditExposure = _corr.CreditExposure
MAX_PORTFOLIO_LOSS_PATHS = _corr.MAX_PORTFOLIO_LOSS_PATHS
PortfolioLossConfig = _corr.PortfolioLossConfig
PortfolioLossResult = _corr.PortfolioLossResult
TrancheLossStatistics = _corr.TrancheLossStatistics
correlation_bounds = _corr.correlation_bounds
joint_probabilities = _corr.joint_probabilities
validate_correlation_matrix = _corr.validate_correlation_matrix
nearest_correlation = _corr.nearest_correlation
cholesky_decompose = _corr.cholesky_decompose
simulate_portfolio_loss = _corr.simulate_portfolio_loss

__all__: list[str] = [
    "MAX_PORTFOLIO_LOSS_PATHS",
    "Copula",
    "CopulaSpec",
    "CorrelatedBernoulli",
    "CreditExposure",
    "LatentFactorKind",
    "LatentFactorSpec",
    "LatentMultiFactor",
    "LatentSingleFactor",
    "LatentTwoFactor",
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
