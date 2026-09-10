"""Input transcription checks against the archived Bloomberg CDSO screenshot."""

import json
from pathlib import Path


def test_cdso_volatility_matches_archived_screen() -> None:
    """Keep the model override, surface and metadata on the observed quote."""
    # The calculator in screenshots/cdx_ig_46_payer_atm_jun26__main.png
    # displays Spread Vol (%) = 36.63, independently confirmed by local OCR.
    path = (
        Path(__file__).resolve().parents[2]
        / "finstack-quant/valuations/tests/golden/data/pricing/bloomberg/cds_option/cdx_ig_46_payer_atm_jun26.json"
    )
    fixture = json.loads(path.read_text())
    spec = fixture["instrument"]["instrument"]["spec"]
    assert spec["instrument_pricing_overrides"]["market_quotes"]["implied_volatility"] == 0.3663
    assert spec["attributes"]["meta"]["spread_vol_pct"] == "36.63"
    surface = fixture["market"]["envelope"]["prior_market"][0]
    assert all(vol == 0.3663 for vol in surface["vols_row_major"])
