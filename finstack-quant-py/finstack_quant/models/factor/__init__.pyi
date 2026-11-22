"""
Factor-model primitives, calibration, and decomposition.

``FactorCovarianceMatrix`` and ``FactorModelConfig`` live here because Rust
exports them from the ``factor`` crate root; credit hierarchy calibration and
factor risk kernels live under :mod:`finstack_quant.models.factor.credit` and
:mod:`finstack_quant.models.factor.risk`.

Examples
--------
>>> from finstack_quant.models.factor import credit
>>> try:
...     credit.CreditFactorModel.from_json("{}")
... except ValueError as exc:
...     "missing field" in str(exc)
True
"""

from __future__ import annotations

from finstack_quant.models.factor import credit as credit
from finstack_quant.models.factor import risk as risk
from finstack_quant.models.factor import schema as schema
from finstack_quant.models.factor._config import (
    FactorCovarianceMatrix as FactorCovarianceMatrix,
    FactorModelConfig as FactorModelConfig,
)
from finstack_quant.models.factor._definitions import (
    FactorDefinition as FactorDefinition,
    MarketMapping as MarketMapping,
)

__all__ = [
    "FactorCovarianceMatrix",
    "FactorModelConfig",
    "FactorDefinition",
    "MarketMapping",
    "credit",
    "risk",
    "schema",
]
