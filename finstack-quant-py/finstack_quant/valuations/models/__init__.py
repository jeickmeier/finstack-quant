"""Pricing model wrappers for ``finstack_quant.valuations``.

Examples:
--------
>>> import finstack_quant.valuations.models as models
>>> models.__name__
'finstack_quant.valuations.models'
"""

from __future__ import annotations

from finstack_quant.valuations.models import credit as credit

__all__: list[str] = ["credit"]
