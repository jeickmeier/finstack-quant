"""Cross-host golden for ``MetricMetadata.coordinates``.

The WASM twin is ``finstack-quant-wasm/tests/facade/metric_coordinates_parity.test.mjs``:
both hosts compare to the same pinned coordinates.
"""

from __future__ import annotations

import json
from pathlib import Path

from finstack_quant.valuations.instruments import metric_metadata

PINNED = json.loads((Path(__file__).parent / "fixtures" / "metric_coordinates.json").read_text())


def test_coordinates_match_the_values_pinned_for_both_hosts() -> None:
    actual = [entry["coordinates"] for entry in metric_metadata(PINNED["keys"])]
    assert actual == PINNED["coordinates"]
