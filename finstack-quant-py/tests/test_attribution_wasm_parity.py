"""Cross-host golden for ``ReturnContributionResult``, a plain object in WASM.

``tests/data/attribution_wasm_parity.json`` holds the Python
``attribute_return_contribution`` output for a fixed spec and the result's
property names. This module pins Python to that file and
``finstack-quant-wasm/tests/facade/attribution_parity.test.mjs`` pins
``attribution.attributeReturnContribution`` (which returns the same Rust result
as the TypeScript ``ReturnContributionResult`` object) to the same file.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from finstack_quant.attribution import ReturnContributionResult, attribute_return_contribution

GOLDEN: dict[str, Any] = json.loads((Path(__file__).parent / "data" / "attribution_wasm_parity.json").read_text())


def test_return_contribution_matches_the_shared_golden() -> None:
    result = attribute_return_contribution(GOLDEN["spec"])
    assert json.loads(result.to_json()) == GOLDEN["result"]
    assert ReturnContributionResult.from_json(result.to_json()).to_json() == result.to_json()


def test_return_contribution_properties_match_the_shared_golden() -> None:
    properties = sorted(
        name
        for name in dir(ReturnContributionResult)
        if not name.startswith("_") and not callable(getattr(ReturnContributionResult, name))
    )
    assert properties == GOLDEN["properties"]
    assert set(GOLDEN["result"]) <= set(GOLDEN["properties"])
