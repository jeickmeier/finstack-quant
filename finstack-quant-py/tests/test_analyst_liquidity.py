"""Verify liquidity quantities and statistical panels share explicit identities."""

from pathlib import Path
import sys

import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "examples/notebooks"))

from _shared.analyst_book import instruments
from _shared.analyst_history import risk_panel
from _shared.analyst_liquidity import liquidity_panel, position_factor_panel


def test_liquidity_units_missing_data_and_alignment() -> None:
    liquidity = liquidity_panel()
    assert set(liquidity.index) == set(instruments("common"))
    assert (liquidity.quantity == 1).all()
    available = liquidity.daily_capacity.dropna()
    assert (available > 0).all()
    assert pd.isna(liquidity.loc["BORROWER-TL", "daily_capacity"])
    assert "unavailable" in liquidity.loc["BORROWER-TL", "source"]
    assert "synthetic OTC" in liquidity.loc["USD-PAYER-IRS", "source"]
    pd.testing.assert_index_equal(position_factor_panel().index, risk_panel().index)
    assert set(instruments("common")) <= set(position_factor_panel().columns)
    liquidity.loc["USD-CORP", "quantity"] = 0
    assert liquidity_panel().loc["USD-CORP", "quantity"] == 1
