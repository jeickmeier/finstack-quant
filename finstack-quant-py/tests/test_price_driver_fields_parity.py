"""Cross-host golden for ``valuations.instruments.price_driver_fields``.

The WASM twin is ``finstack-quant-wasm/tests/facade/price_driver_fields_parity.test.mjs``:
both hosts compare to the same pinned list.
"""

from __future__ import annotations

import json
from pathlib import Path

from finstack_quant.valuations.instruments import price_driver_fields

FIXTURE = Path(__file__).parent / "fixtures" / "price_driver_fields.json"


def test_price_driver_fields_match_the_list_pinned_for_both_hosts() -> None:
    assert price_driver_fields() == json.loads(FIXTURE.read_text())
