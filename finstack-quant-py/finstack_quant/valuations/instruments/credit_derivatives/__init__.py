"""CDS-family instrument wrappers.

Mirrors ``finstack_quant_valuations::instruments::credit_derivatives``.
"""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

CreditDefaultSwap = _valuations.instruments.credit_derivatives.CreditDefaultSwap
CDSIndex = _valuations.instruments.credit_derivatives.CDSIndex
CDSTranche = _valuations.instruments.credit_derivatives.CDSTranche
CDSOption = _valuations.instruments.credit_derivatives.CDSOption

__all__ = ["CDSIndex", "CDSOption", "CDSTranche", "CreditDefaultSwap"]
