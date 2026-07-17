"""
Python bindings for the corresponding finstack-quant Rust API.

Examples
--------
>>> import finstack_quant.valuations.models as models
>>> models.__name__
'finstack_quant.valuations.models'
"""

from __future__ import annotations

from finstack_quant.valuations.models import credit as credit

__all__ = ["credit"]
