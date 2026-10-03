"""Instrument pricing and risk metrics.

Bindings for the ``finstack-quant-valuations`` Rust crate. Where things live:

- Market data (``DiscountCurve``, ``ForwardCurve``, ``HazardCurve``,
  ``MarketContext``, ``FxMatrix``): :mod:`finstack_quant.core.market_data`;
  curve bootstrapping and quote ingestion: :mod:`finstack_quant.calibration`.
- Instruments, builders and :func:`~finstack_quant.valuations.instruments.price_instrument`:
  :mod:`finstack_quant.valuations.instruments`.
- Results: :class:`ValuationResult` (here); per-flow cashflow tables come from
  :func:`~finstack_quant.valuations.instruments.instrument_cashflows`.
- Composite instruments, credit-derivative examples, the listed-market
  catalog and JSON schemas: :mod:`~finstack_quant.valuations.composite`,
  :mod:`~finstack_quant.valuations.credit_derivatives`,
  :mod:`~finstack_quant.valuations.market`, :mod:`~finstack_quant.valuations.schema`.

The module-level ``*_coupon_profile``, ``cms_spread_option_intrinsic`` and
``callable_range_accrual_accrued`` functions are deterministic exotic-rates
helpers that need no market data.

Examples:
--------
>>> from finstack_quant.valuations import instruments
>>> hasattr(instruments, "price_instrument")
True

"""

from finstack_quant.finstack_quant import valuations as _valuations
from finstack_quant.valuations import (
    composite as composite,
    credit_derivatives as credit_derivatives,
    instruments as instruments,
    market as market,
)

ValuationResult = _valuations.ValuationResult
tarn_coupon_profile = _valuations.tarn_coupon_profile
snowball_coupon_profile = _valuations.snowball_coupon_profile
inverse_floater_coupon_profile = _valuations.inverse_floater_coupon_profile
cms_spread_option_intrinsic = _valuations.cms_spread_option_intrinsic
callable_range_accrual_accrued = _valuations.callable_range_accrual_accrued
# `schema` is a compiled submodule with no pure-Python shim package; the extension
# registers it as `finstack_quant.valuations.schema`.
schema = _valuations.schema


__all__: list[str] = [
    "ValuationResult",
    "callable_range_accrual_accrued",
    "cms_spread_option_intrinsic",
    "composite",
    "credit_derivatives",
    "instruments",
    "inverse_floater_coupon_profile",
    "market",
    "schema",
    "snowball_coupon_profile",
    "tarn_coupon_profile",
]
