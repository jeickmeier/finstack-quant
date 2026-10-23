# ruff: noqa: D104
# Module help is compiled from the matching stub and delegated below.

from finstack_quant.finstack_quant import models as _models

_corr = _models.correlation

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
validate_correlation_matrix = _corr.validate_correlation_matrix
nearest_correlation = _corr.nearest_correlation
cholesky_correlation = _corr.cholesky_correlation
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
    "cholesky_correlation",
    "correlation_bounds",
    "nearest_correlation",
    "simulate_portfolio_loss",
    "validate_correlation_matrix",
]

__doc__ = _models.correlation.__doc__
