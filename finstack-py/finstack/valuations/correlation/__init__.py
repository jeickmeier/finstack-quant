"""Correlation infrastructure: copulas, factor models, recovery models.

Bindings for the ``finstack_valuations::correlation`` Rust module. Nested
under :mod:`finstack.valuations` to mirror the Rust crate layout where
correlation lives inside ``finstack-valuations``.
"""

from __future__ import annotations

from finstack.finstack import valuations as _valuations

_corr = _valuations.correlation

CopulaSpec = _corr.CopulaSpec
Copula = _corr.Copula
RecoverySpec = _corr.RecoverySpec
RecoveryModel = _corr.RecoveryModel
LatentFactorSpec = _corr.LatentFactorSpec
LatentFactor = _corr.LatentFactor
LatentSingleFactor = _corr.LatentSingleFactor
LatentTwoFactor = _corr.LatentTwoFactor
LatentMultiFactor = _corr.LatentMultiFactor
CorrelatedBernoulli = _corr.CorrelatedBernoulli
correlation_bounds = _corr.correlation_bounds
joint_probabilities = _corr.joint_probabilities
validate_correlation_matrix = _corr.validate_correlation_matrix
nearest_correlation = _corr.nearest_correlation
cholesky_decompose = _corr.cholesky_decompose

# Deprecated aliases retained for one release cycle.
FactorSpec = LatentFactorSpec
FactorModel = LatentFactor
SingleFactorModel = LatentSingleFactor
TwoFactorModel = LatentTwoFactor
MultiFactorModel = LatentMultiFactor

__all__: list[str] = [
    "Copula",
    "CopulaSpec",
    "CorrelatedBernoulli",
    "FactorModel",
    "FactorSpec",
    "LatentFactor",
    "LatentFactorSpec",
    "LatentMultiFactor",
    "LatentSingleFactor",
    "LatentTwoFactor",
    "MultiFactorModel",
    "RecoveryModel",
    "RecoverySpec",
    "SingleFactorModel",
    "TwoFactorModel",
    "cholesky_decompose",
    "correlation_bounds",
    "joint_probabilities",
    "nearest_correlation",
    "validate_correlation_matrix",
]
