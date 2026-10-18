"""Factor-model primitives, calibration, and decomposition.

Bindings for ``finstack_quant_factor`` (Rust path ``finstack_quant::models::factor``). ``FactorCovarianceMatrix`` and
``FactorModelConfig`` are exported here, matching the Rust crate root; credit
hierarchy calibration lives under :mod:`finstack_quant.models.factor.credit`.

Examples:
--------
>>> from finstack_quant.models.factor import credit
>>> try:
...     credit.CreditFactorModel.from_json("{}")
... except ValueError as exc:
...     "missing field" in str(exc)
True
"""

from finstack_quant.finstack_quant import models as _models
from finstack_quant.models.factor import credit as credit, risk as risk

# `schema` is a compiled submodule with no pure-Python shim package; the extension
# registers it as `finstack_quant.models.factor.schema`.
schema = _models.factor.schema

FactorCovarianceMatrix = _models.factor.FactorCovarianceMatrix
FactorModelConfig = _models.factor.FactorModelConfig

__all__: list[str] = [
    "FactorCovarianceMatrix",
    "FactorModelConfig",
    "credit",
    "risk",
    "schema",
]
