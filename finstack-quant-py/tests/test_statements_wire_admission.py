"""Python loads the shared model documents to the outcomes Rust records.

The cases file is written by the Rust test
``wire_admission::every_host_document_loads_to_its_recorded_outcome``; the
WASM facade test reads the same file.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from finstack_quant.portfolio import ContractLimitExceededError, ContractValidationError
from finstack_quant.statements import FinancialModelSpec, MonteCarloConfig
from finstack_quant.statements_analytics import DependencyTracer

CASES = json.loads(
    (Path(__file__).resolve().parents[2] / "finstack-quant/statements/tests/data/wire_admission.json").read_text()
)["cases"]


def _outcome(document: str) -> dict[str, Any]:
    """Return the content hash of a loaded model, or the error's kind, code and pointer."""
    try:
        model = FinancialModelSpec.from_json(document)
    except ContractLimitExceededError:
        return {"kind": "validation", "code": "limit_exceeded", "pointer": None}
    except ContractValidationError as error:
        first = error.report[0]
        return {"kind": "validation", "code": first["code"], "pointer": first.get("pointer")}
    except KeyError:
        return {"kind": "not_found", "code": None, "pointer": None}
    except RuntimeError:
        return {"kind": "computation", "code": None, "pointer": None}
    except ValueError:
        return {"kind": "validation", "code": None, "pointer": None}
    return {"content_hash": model.content_hash()}


@pytest.mark.parametrize("case", CASES, ids=[case["name"] for case in CASES])
def test_document_loads_to_the_recorded_outcome(case: dict[str, Any]) -> None:
    assert _outcome(case["document"]) == case["expect"]


def test_every_model_taking_entry_point_uses_the_same_loader() -> None:
    duplicate = next(case["document"] for case in CASES if case["name"] == "duplicate_key")
    with pytest.raises(ContractValidationError) as raised:
        FinancialModelSpec.from_json(duplicate)
    assert raised.value.report[0]["pointer"] == "/id"
    # A model-taking entry point takes the loaded model; JSON text is not one.
    with pytest.raises(TypeError, match="FinancialModelSpec"):
        DependencyTracer(duplicate)


def test_monte_carlo_seed_is_a_decimal_string_on_the_wire() -> None:
    config = MonteCarloConfig(n_paths=4, seed=2**64 - 1, metrics=["revenue"])
    assert json.loads(config.to_json())["seed"] == "18446744073709551615"
    assert MonteCarloConfig.from_json(config.to_json()).seed == 2**64 - 1
    with pytest.raises(ValueError, match="decimal string"):
        MonteCarloConfig.from_json('{"n_paths":4,"seed":7,"metrics":["revenue"]}')
