"""Factor-model primitives, calibration, and decomposition.

Bindings for the ``finstack-factor-model`` Rust crate. Credit hierarchy
calibration lives under :mod:`finstack.factor_model.credit`.
"""

from __future__ import annotations

from finstack.factor_model import credit as credit

__all__: list[str] = [
    "credit",
]
