"""Instrument pricing and risk metrics.

Bindings for the ``finstack-quant-valuations`` Rust crate. Where things live:

- Market data (``DiscountCurve``, ``ForwardCurve``, ``HazardCurve``,
  ``MarketContext``, ``FxMatrix``): :mod:`finstack_quant.core.market_data`;
  curve bootstrapping and quote ingestion: :mod:`finstack_quant.calibration`.
- Instruments, builders and :func:`~finstack_quant.valuations.instruments.price_instrument`:
  :mod:`finstack_quant.valuations.instruments`.
- Results: :class:`ValuationResult` (here); per-flow cashflow tables come from
  :func:`~finstack_quant.valuations.instruments.instrument_cashflows`.
- Composite instruments, the listed-market
  catalog and JSON schemas: :mod:`~finstack_quant.valuations.composite`,
  :mod:`~finstack_quant.valuations.market`, :mod:`~finstack_quant.valuations.schema`.

Examples:
--------
>>> from finstack_quant.valuations import instruments
>>> hasattr(instruments, "price_instrument")
True

"""

from finstack_quant.finstack_quant import valuations as _valuations

ValuationResult = _valuations.ValuationResult
# The submodules are compiled modules with no pure-Python shim; the extension
# registers each under its public path (`finstack_quant.valuations.instruments`
# and so on), and the stubs beside them carry the types and docstrings.
composite = _valuations.composite
instruments = _valuations.instruments
market = _valuations.market
schema = _valuations.schema


__all__: list[str] = [
    "ValuationResult",
    "composite",
    "instruments",
    "market",
    "schema",
]
