"""Cross-host golden for ``PnlAttribution.pct_of_total_by_bucket``.

The WASM twin is
``finstack-quant-wasm/tests/facade/pnl_bucket_share_parity.test.mjs``: both
hosts read the same pinned attribution and compare to the same pinned records.

Regenerate with ``UPDATE_PNL_BUCKET_SHARE_PARITY=1 uv run --no-sync pytest <this file>``.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

from finstack_quant.attribution import PnlAttribution

FIXTURE = Path(__file__).parent / "fixtures" / "pnl_bucket_shares.json"
ATTRIBUTION = Path(__file__).parent / "data" / "attribution_bond.json"


def test_buckets_match_the_records_pinned_for_both_hosts() -> None:
    actual = PnlAttribution.from_json(ATTRIBUTION.read_text()).pct_of_total_by_bucket()
    if os.environ.get("UPDATE_PNL_BUCKET_SHARE_PARITY"):
        FIXTURE.write_text(json.dumps(actual, indent=2) + "\n")
    assert actual == json.loads(FIXTURE.read_text())


def test_every_bucket_is_listed_once_in_struct_order() -> None:
    buckets = PnlAttribution.from_json(ATTRIBUTION.read_text()).pct_of_total_by_bucket()
    fields = [bucket["field"] for bucket in buckets]
    assert fields[0] == "carry"
    assert fields[-1] == "residual"
    assert len(fields) == len(set(fields)) == 12
