"""Shared validation for pricing golden fixture inputs."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from finstack.valuations.instruments import list_standard_metrics, validate_instrument_json
from jsonschema import validators

WORKSPACE_ROOT = Path(__file__).resolve().parents[3]
INSTRUMENT_ENVELOPE_SCHEMA_PATH = WORKSPACE_ROOT / "finstack/valuations/schemas/instruments/1/instrument.schema.json"


def validated_instrument_json(instrument_json: dict[str, Any]) -> str:
    """Validate fixture instrument JSON and return the executable JSON string."""
    if _is_instrument_envelope(instrument_json):
        _validate_instrument_envelope_schema(instrument_json)
        validate_instrument_json(json.dumps(instrument_json["instrument"]))
        return json.dumps(instrument_json)
    return validate_instrument_json(json.dumps(instrument_json))


def validate_requested_metrics(metrics: list[str], expected_outputs: dict[str, float]) -> None:
    """Validate requested metric names and coverage of expected outputs."""
    standard_metrics = set(list_standard_metrics())
    unknown = [metric for metric in metrics if metric not in standard_metrics]
    assert not unknown, f"pricing fixture inputs.metrics contains unknown metric(s): {unknown}"

    requested = set(metrics)
    missing = [
        metric for metric in expected_outputs if _metric_base(metric) != "npv" and _metric_base(metric) not in requested
    ]
    assert not missing, f"pricing fixture expected_outputs has metric(s) not requested in inputs.metrics: {missing}"


def _is_instrument_envelope(instrument_json: dict[str, Any]) -> bool:
    return "schema" in instrument_json and "instrument" in instrument_json


def _validate_instrument_envelope_schema(instrument_json: dict[str, Any]) -> None:
    schema = json.loads(INSTRUMENT_ENVELOPE_SCHEMA_PATH.read_text(encoding="utf-8"))
    validator_cls = validators.validator_for(schema)
    validator_cls.check_schema(schema)
    validator = validator_cls(schema)
    errors = sorted(validator.iter_errors(instrument_json), key=lambda error: list(error.path))
    if errors:
        details = "\n  ".join(error.message for error in errors)
        msg = f"instrument_json failed {INSTRUMENT_ENVELOPE_SCHEMA_PATH.name} validation:\n  {details}"
        raise ValueError(msg)


def _metric_base(metric: str) -> str:
    return metric.split("::", 1)[0]
