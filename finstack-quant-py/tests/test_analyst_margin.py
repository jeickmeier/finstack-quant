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
