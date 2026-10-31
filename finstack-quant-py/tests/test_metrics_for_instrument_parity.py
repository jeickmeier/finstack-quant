"""Cross-host golden for the ungrouped ids of ``metrics_for_instrument``.

The WASM twin is ``finstack-quant-wasm/tests/facade/metrics_for_instrument_parity.test.mjs``:
both hosts compare to the same pinned ids.
"""

from __future__ import annotations

import json
from pathlib import Path

from finstack_quant.valuations.instruments import metric_metadata, metrics_for_instrument

PINNED = json.loads((Path(__file__).parent / "fixtures" / "metrics_for_instrument.json").read_text())


def test_ungrouped_ids_match_the_values_pinned_for_both_hosts() -> None:
    actual = {
        instrument_type: metrics_for_instrument(instrument_type).get(PINNED["group"], [])
        for instrument_type in PINNED["ids"]
    }
    assert actual == PINNED["ids"]


def test_metric_metadata_reports_the_same_group() -> None:
    assert [entry["group"] for entry in metric_metadata(["all_in_rate", "no_such_metric"])] == [
        PINNED["group"],
        None,
    ]
