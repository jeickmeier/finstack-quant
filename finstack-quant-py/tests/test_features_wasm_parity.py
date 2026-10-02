"""Cross-host goldens for the features members that WASM binds as free functions.

``tests/data/features_wasm_parity.json`` holds the Python outputs for a fixed
panel pipeline and for every operation selector. This module pins Python to
that file and ``finstack-quant-wasm/tests/facade/features_parity.test.mjs``
pins the WASM functions (``transformPanel``, ``panelTransformResultGetColumn``,
``timeSeriesOpValues`` / ``timeSeriesOpParamKeys`` and their cross-sectional and
pairwise twins) to the same file.
"""

from __future__ import annotations

import json
from pathlib import Path
import re
from typing import Any

import pytest

from finstack_quant.features import (
    CrossSectionalOp,
    PairwiseOp,
    PanelTransformSpec,
    TimeSeriesOp,
    transform_panel,
    transform_panel_json,
)

GOLDEN: dict[str, Any] = json.loads((Path(__file__).parent / "data" / "features_wasm_parity.json").read_text())
OPS = {"TimeSeriesOp": TimeSeriesOp, "CrossSectionalOp": CrossSectionalOp, "PairwiseOp": PairwiseOp}


def exact(message: str) -> str:
    """Regex matching exactly ``message``."""
    return f"^{re.escape(message)}$"


def test_transform_panel_matches_the_shared_golden() -> None:
    spec = GOLDEN["spec"]
    result = transform_panel(spec)
    assert json.loads(result.to_json()) == GOLDEN["expected"]
    assert json.loads(transform_panel_json(json.dumps(spec))) == GOLDEN["expected"]
    typed = PanelTransformSpec.from_json(json.dumps(spec))
    assert json.loads(transform_panel(typed).to_json()) == GOLDEN["expected"]
    assert result.columns == list(GOLDEN["columns"])
    for name, values in GOLDEN["columns"].items():
        assert result.get_column(name) == values


def test_transform_panel_errors_match_the_shared_golden() -> None:
    spec = GOLDEN["spec"]
    with pytest.raises(KeyError) as missing:
        transform_panel(spec).get_column("nope")
    assert missing.value.args[0] == GOLDEN["missing_column_error"]["message"]
    reserved = {**spec, "operations": [{"family": "timeseries", "name": "values", "op": "diff"}]}
    with pytest.raises(ValueError, match=exact(GOLDEN["reserved_name_error"]["message"])):
        transform_panel(reserved)


@pytest.mark.parametrize("type_name", sorted(OPS))
def test_operation_selectors_match_the_shared_golden(type_name: str) -> None:
    op_type = OPS[type_name]
    golden = GOLDEN["ops"][type_name]
    assert op_type.values() == golden["values"]
    assert {name: op_type(name).param_keys for name in op_type.values()} == golden["param_keys"]
    with pytest.raises(ValueError, match=exact(golden["unknown_error"]["message"])):
        op_type("nope")
