"""Every typed instrument class is priced by ``price_instrument``.

A typed instrument has no pricing method of its own: ``price_instrument`` is
the one verb, and it takes a typed object through a hand-kept cast list
(``extract_instrument`` in ``src/bindings/extract.rs``). A class missing from
that list is rejected with ``TypeError``, so this sweep prices the ``example()``
of every exported instrument class.
"""

from __future__ import annotations

import contextlib
import inspect

import pytest

from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations import instruments as ins


def _typed_classes() -> list[str]:
    """Exported classes with ``example()`` and the ``Instrument`` trait surface."""
    return [
        name
        for name in ins.__all__
        if inspect.isclass(cls := getattr(ins, name))
        and hasattr(cls, "example")
        and hasattr(cls, "market_dependencies")
    ]


TYPED_CLASSES = _typed_classes()


def test_the_sweep_finds_the_typed_instrument_classes() -> None:
    assert {"Bond", "InterestRateSwap", "EquityFutureOption", "StructuredCreditTranche"} <= set(TYPED_CLASSES)


@pytest.mark.parametrize("name", TYPED_CLASSES)
def test_price_instrument_accepts_every_typed_instrument_class(name: str) -> None:
    instrument = getattr(ins, name).example()
    # An empty market cannot price most examples; any of these errors means the
    # instrument was accepted. A ``TypeError`` means it was not, and fails the test.
    with contextlib.suppress(KeyError, ValueError, RuntimeError):
        ins.price_instrument(instrument, MarketContext(), "2024-01-15")
