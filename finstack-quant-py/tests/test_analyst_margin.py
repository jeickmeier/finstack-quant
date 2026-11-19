"""Run the actual displayed margin proof cells as the prerequisite regression."""

from pathlib import Path
import sys

import nbformat


def test_regulatory_grid_mapping_and_single_bucket_reconciliation() -> None:
    root = Path(__file__).resolve().parents[1] / "examples" / "notebooks"
    sys.path.insert(0, str(root))
    notebook = nbformat.read(root / "07_advanced_quant/margin_collateral_and_xva.ipynb", as_version=4)
    proof_ids = ["margin-proof-1", "margin-proof-2", "margin-proof-3"]
    cells = [
        cell
        for cell in notebook.cells
        if cell.cell_type == "code"
        and cell.metadata.get("analyst_program", {}).get("lesson") == "3.6"
        and cell.metadata.get("analyst_program", {}).get("id") in proof_ids
    ]
    assert [cell.metadata.analyst_program.id for cell in cells] == proof_ids
    namespace = {"__name__": "__main__"}
    for cell in cells:
        exec(compile(cell.source, cell.metadata.analyst_program.id, "exec"), namespace)


def test_standalone_simm_nets_trades_and_rejects_unsupported_inputs() -> None:
    import json

    import pytest

    from finstack_quant.core.market_data import DiscountCurve, ForwardCurve, MarketContext
    from finstack_quant.valuations import calculate_simm
    from finstack_quant.valuations.instruments import Deposit, InterestRateSwap

    as_of = "2023-12-20"
    market = MarketContext()
    market.insert(DiscountCurve.flat("USD-OIS", as_of, 0.04, "act_365f"))
    market.insert(ForwardCurve.flat("USD-SOFR-3M", as_of, 0.045, "act_360"))
    payer = InterestRateSwap.example()
    receiver = json.loads(payer.to_json())
    receiver["instrument"]["spec"]["id"] = "RECEIVER"
    receiver["instrument"]["spec"]["side"] = "receive"
    single = calculate_simm([payer], market, as_of)
    assert single.amount > 0.0
    assert single.approximation
    assert calculate_simm([payer.to_json()], market, as_of).amount == single.amount
    assert calculate_simm([payer, receiver], market, as_of).amount == pytest.approx(0.0, abs=1e-5)
    with pytest.raises(ValueError, match="does not support SIMM"):
        calculate_simm([Deposit.example()], market, as_of)
    with pytest.raises(ValueError, match="must be USD"):
        calculate_simm([payer], market, as_of, currency="EUR")
