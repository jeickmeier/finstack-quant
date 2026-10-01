"""Cross-host goldens for the scenarios members that WASM binds as free functions.

``tests/data/scenarios_wasm_parity.json`` holds the Python outputs for a fixed
set of inputs. This module pins Python to that file and
``finstack-quant-wasm/tests/facade/scenarios_parity.test.mjs`` pins the WASM
free functions (``operationSpec*``, ``curveKind*``, ``rateBindingSpecValidate``,
``scenarioSpec*``, ``horizonResult*``) to the same file, so both hosts are held
to one set of numbers and wire shapes.
"""

from __future__ import annotations

import json
from pathlib import Path
import re
from typing import Any

import pytest

from finstack_quant.scenarios import (
    Compounding,
    CurveKind,
    OperationSpec,
    RateBindingSpec,
    ScenarioSpec,
    TenorMatchMode,
    TimeRollMode,
    compute_horizon_return,
)

GOLDEN: dict[str, Any] = json.loads((Path(__file__).parent / "data" / "scenarios_wasm_parity.json").read_text())
ENUMS = {"CurveKind": CurveKind, "TenorMatchMode": TenorMatchMode, "TimeRollMode": TimeRollMode}


def exact(message: str) -> str:
    """Regex matching exactly ``message``."""
    return f"^{re.escape(message)}$"


def python_args(name: str, args: list[Any]) -> list[Any]:
    """Host conversion of the shared JSON arguments into the Python call shape."""

    def convert(arg: Any) -> Any:
        if name == "rate_binding":
            return RateBindingSpec.from_json(json.dumps(arg))
        if isinstance(arg, dict) and "path" in arg:
            return json.dumps(arg)
        if isinstance(arg, list) and arg and isinstance(arg[0], list):
            return [tuple(pair) for pair in arg]
        return arg

    return [convert(arg) for arg in args]


def wire(value: Any) -> Any:
    """Canonical wire object(s) of an operation or a list of operations."""
    if isinstance(value, list):
        return [json.loads(item.to_json()) for item in value]
    return json.loads(value.to_json())


@pytest.mark.parametrize("case", GOLDEN["constructors"], ids=lambda case: case["py"])
def test_operation_constructors_match_the_shared_golden(case: dict[str, Any]) -> None:
    built = getattr(OperationSpec, case["py"])(*python_args(case["py"], case["args"]))
    assert wire(built) == case["expected"]
    if not isinstance(built, list):
        assert built.requires_instruments() is case["requires_instruments"]
        assert built.mutates_instruments() is case["mutates_instruments"]
        assert built.validate() is None


def test_every_operation_constructor_has_a_golden_case() -> None:
    predicates = {"from_json", "to_json", "validate", "requires_instruments", "mutates_instruments"}
    constructors = {name for name in dir(OperationSpec) if not name.startswith("_")} - predicates - {"kind"}
    assert constructors == {case["py"] for case in GOLDEN["constructors"]}


@pytest.mark.parametrize("case", GOLDEN["invalid_operations"], ids=lambda case: case["operation"]["kind"])
def test_operation_validate_errors_match_the_shared_golden(case: dict[str, Any]) -> None:
    operation = OperationSpec.from_json(json.dumps(case["operation"]))
    with pytest.raises(ValueError, match=exact(case["error"]["message"])):
        operation.validate()


def test_enum_classmethods_match_the_shared_wire_values() -> None:
    for type_name, variants in GOLDEN["enums"].items():
        for variant, expected in variants.items():
            if type_name == "Compounding":
                binding = RateBindingSpec("n", "c", "1Y", compounding=getattr(Compounding, variant)())
                assert json.loads(binding.to_json())["compounding"] == expected
            else:
                assert getattr(ENUMS[type_name], variant)().value == expected


@pytest.mark.parametrize("case", GOLDEN["invalid_bindings"], ids=lambda case: case["binding"]["tenor"])
def test_rate_binding_validate_errors_match_the_shared_golden(case: dict[str, Any]) -> None:
    binding = RateBindingSpec.from_json(json.dumps(case["binding"]))
    with pytest.raises(ValueError, match=exact(case["error"]["message"])):
        binding.validate()


def test_scenario_spec_predicates_and_hazard_mode_match_the_shared_golden() -> None:
    golden = GOLDEN["scenario_spec"]
    for case in golden["cases"]:
        spec = ScenarioSpec.from_json(json.dumps(case["spec"]))
        assert spec.requires_instruments() is case["requires_instruments"]
        assert spec.mutates_instruments() is case["mutates_instruments"]
        assert spec.validate() is None
    spec = ScenarioSpec.from_json(json.dumps(golden["cases"][0]["spec"]))
    bump = golden["with_hazard_bump_mode"]
    assert json.loads(spec.with_hazard_bump_mode(bump["mode"]).to_json()) == bump["expected"]
    assert spec.hazard_bump_mode == "solve_to_par"
    with pytest.raises(ValueError, match=exact(golden["bad_mode_error"]["message"])):
        spec.with_hazard_bump_mode("nope")


def test_horizon_explain_and_factor_contribution_match_the_shared_golden() -> None:
    golden = GOLDEN["horizon"]
    result = compute_horizon_return(
        json.dumps(golden["instrument"]),
        json.dumps(golden["market"]),
        golden["as_of"],
        json.dumps(golden["scenario"]),
    )
    assert result.explain() == golden["explain"]
    for factor, expected in golden["factor_contribution"].items():
        assert result.factor_contribution(factor) == pytest.approx(expected, abs=1e-12)
    with pytest.raises(ValueError, match=exact(golden["bad_factor_error"]["message"])):
        result.factor_contribution("nope")
