"""Shared validation for pricing golden fixture inputs."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jsonschema import validators
from referencing import Registry, Resource

from finstack_quant.valuations.instruments import list_standard_metrics, validate_instrument_json

WORKSPACE_ROOT = Path(__file__).resolve().parents[3]
INSTRUMENT_ENVELOPE_SCHEMA_PATH = (
    WORKSPACE_ROOT / "finstack-quant/valuations/schemas/instruments/1/instrument.schema.json"
)
SCHEMA_RESOURCE_DIRS = (
    WORKSPACE_ROOT / "finstack-quant/valuations/schemas/common/1",
    WORKSPACE_ROOT / "finstack-quant/valuations/schemas/cashflow/1",
    WORKSPACE_ROOT / "finstack-quant/valuations/schemas/instruments/1",
)


def validated_instrument_json(instrument_json: dict[str, Any]) -> str:
    """Validate fixture instrument JSON and return the executable JSON string."""
    if _is_instrument_envelope(instrument_json):
        _validate_instrument_envelope_schema(instrument_json)
        validate_instrument_json(json.dumps(instrument_json["instrument"]))
        return json.dumps(instrument_json)
    return validate_instrument_json(json.dumps(instrument_json))


def requested_metrics(expected: dict[str, float]) -> list[str]:
    """Derive the requested-metric list from expected keys (npv excluded)."""
    metrics: list[str] = []
    for key in expected:
        base = _metric_base(key)
        if base != "npv" and base not in metrics:
            metrics.append(base)
    return metrics


def validate_requested_metrics(metrics: list[str]) -> None:
    """Validate that derived metric names are known standard metrics."""
    standard_metrics = set(list_standard_metrics())
    unknown = [metric for metric in metrics if metric not in standard_metrics]
    assert not unknown, f"expected metric base name(s) are not standard metrics: {unknown}"


def _is_instrument_envelope(instrument_json: dict[str, Any]) -> bool:
    return "schema" in instrument_json and "instrument" in instrument_json


def _validate_instrument_envelope_schema(instrument_json: dict[str, Any]) -> None:
    schema = json.loads(INSTRUMENT_ENVELOPE_SCHEMA_PATH.read_text(encoding="utf-8"))
    validator_cls = validators.validator_for(schema)
    validator_cls.check_schema(schema)
    validator = validator_cls(schema, registry=_schema_registry())
    errors = sorted(validator.iter_errors(instrument_json), key=lambda error: list(error.path))
    if errors:
        details = "\n  ".join(error.message for error in errors)
        msg = f"instrument_json failed {INSTRUMENT_ENVELOPE_SCHEMA_PATH.name} validation:\n  {details}"
        raise ValueError(msg)


def _schema_registry() -> Registry:
    resources: list[tuple[str, Resource]] = []
    for schema_dir in SCHEMA_RESOURCE_DIRS:
        for path in sorted(schema_dir.rglob("*.schema.json")):
            schema = json.loads(path.read_text(encoding="utf-8"))
            schema_id = schema.get("$id")
            if isinstance(schema_id, str):
                resources.append((schema_id, Resource.from_contents(schema)))
    return Registry().with_resources(resources)


def _metric_base(metric: str) -> str:
    return metric.split("::", 1)[0]
