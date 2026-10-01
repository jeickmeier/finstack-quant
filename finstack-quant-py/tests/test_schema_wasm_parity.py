"""Cross-host goldens for the ``*.schema`` namespaces bound to WASM by slice PX1.

Every case is computed here by the Python binding and compared with
``finstack-quant-wasm/tests/facade/schema_parity.golden.json``. The facade test
``finstack-quant-wasm/tests/facade/schema_parity.test.mjs`` computes the same
cases through the WASM binding and asserts the same values, so both hosts are
pinned to one Rust result. Python returns JSON text where WASM returns plain
objects; the cases compare the parsed documents.

Regenerate the golden after an intended change with::

    UPDATE_SCHEMA_PARITY_GOLDEN=1 uv run --no-sync pytest \
        finstack-quant-py/tests/test_schema_wasm_parity.py
    (cd finstack-quant-wasm && npx --no-install prettier --write \
        tests/facade/schema_parity.golden.json)
"""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

import pytest

import finstack_quant
from finstack_quant import (
    attribution,
    calibration,
    cashflows,
    core,
    margin,
    portfolio,
    scenarios,
    statements,
    valuations,
)
from finstack_quant.models import factor

GOLDEN = Path(__file__).parents[2] / "finstack-quant-wasm/tests/facade/schema_parity.golden.json"

# Facade namespace path -> (Python schema module, selector of the artifact exercised).
REGISTRIES: dict[str, tuple[Any, str]] = {
    "core": (core.schema, "period_plan.schema.json"),
    "attribution": (attribution.schema, "attribution.schema.json"),
    "calibration": (calibration.schema, "calibration.schema.json"),
    "cashflows": (cashflows.schema, "amortization_spec.schema.json"),
    "margin": (margin.schema, "schemas/margin/1/ead_result.schema.json"),
    "models.factor": (factor.schema, "factor_model_config.schema.json"),
    "portfolio": (portfolio.schema, "cell_config.schema.json"),
    "scenarios": (scenarios.schema, "scenario.schema.json"),
    "statements": (statements.schema, "financial_model_spec.schema.json"),
    "valuations": (valuations.schema, "bond.schema.json"),
    "": (finstack_quant.schema, "https://finstack_quant.dev/schemas/instrument/1/fixed_income/bond.schema.json"),
}
# A payload no published object schema accepts, sent as JSON text by both hosts.
INVALID_PAYLOAD = '{"not_a_field": 1}'
NAMED_ACCESSORS: dict[str, tuple[Any, list[str]]] = {
    "models.factor": (
        factor.schema,
        [
            "credit_calibration_config_schema",
            "credit_calibration_inputs_schema",
            "credit_factor_model_schema",
            "factor_model_config_schema",
        ],
    ),
    "statements": (
        statements.schema,
        ["financial_model_spec_schema", "normalization_config_schema", "statement_result_schema"],
    ),
}


def _summary(text: str) -> dict[str, Any]:
    """Identity of a schema document: enough to tell both hosts read the same one."""
    document = json.loads(text)
    return {
        "id": document.get("$id"),
        "title": document.get("title"),
        "keys": sorted(document),
        "definitions": sorted(document.get("$defs", {})),
    }


def _index(text: str) -> dict[str, Any]:
    """An index without its prose columns: ``bytes`` pins each rendered document's size."""
    index = json.loads(text)
    keep = ("domain", "path", "$id", "type_name", "kind", "bytes")
    return {
        "schema_index_version": index["schema_index_version"],
        "artifacts": [{key: row[key] for key in keep if key in row} for row in index["artifacts"]],
    }


def _cases() -> dict[str, Any]:
    registries = {}
    for path, (module, selector) in REGISTRIES.items():
        registries[path] = {
            "selector": selector,
            "index": _index(module.index()),
            "canonical": _summary(module.get(selector)),
            "llm": _summary(module.get(selector, "llm")),
            "report": json.loads(module.validate(selector, INVALID_PAYLOAD)),
        }
    bond = json.loads(valuations.schema.instrument_schema("bond"))["examples"][0]
    instrument_json = json.dumps(bond)
    resources = cashflows.schema.resources()
    return {
        "invalid_payload": INVALID_PAYLOAD,
        "registries": registries,
        "domains": finstack_quant.schema.domains(),
        "named": {
            path: {name: _summary(getattr(module, name)()) for name in names}
            for path, (module, names) in NAMED_ACCESSORS.items()
        },
        "cashflows_resources": {uri: _summary(text) for uri, text in resources.items()},
        "instrument": {
            "json": instrument_json,
            "envelope": valuations.schema.validate_instrument_envelope_json(instrument_json),
            "typed": valuations.schema.validate_instrument_type_json("bond", instrument_json),
        },
    }


def test_python_matches_the_cross_host_golden() -> None:
    cases = _cases()
    if os.environ.get("UPDATE_SCHEMA_PARITY_GOLDEN") == "1":
        GOLDEN.write_text(json.dumps(cases, indent=2) + "\n")
    assert cases == json.loads(GOLDEN.read_text())


def test_every_registry_exercises_a_real_failure() -> None:
    """The shared invalid payload must fail each selected schema, or the report case pins nothing."""
    for path, (module, selector) in REGISTRIES.items():
        assert json.loads(module.validate(selector, INVALID_PAYLOAD)), path


def test_domains_come_from_the_rust_registry() -> None:
    index = json.loads(finstack_quant.schema.index())
    assert finstack_quant.schema.domains() == sorted({row["domain"] for row in index["artifacts"]})


def test_unknown_selector_and_profile_raise_the_mapped_exceptions() -> None:
    with pytest.raises(KeyError, match="no_such_schema"):
        core.schema.get("no_such_schema.json")
    with pytest.raises(ValueError, match="unknown schema profile"):
        core.schema.get("period_plan.schema.json", "nope")
