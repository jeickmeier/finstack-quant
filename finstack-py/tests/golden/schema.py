"""Dataclasses mirroring the Rust `finstack.golden/2` fixture schema."""

from __future__ import annotations

from dataclasses import dataclass, field
import json
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "finstack.golden/2"

_COMMON_TOP_LEVEL_KEYS = {
    "schema_version",
    "metadata",
    "kind",
    "expected",
    "tolerances",
}
_PRICING_BODY_KEYS = {"model", "market", "instrument"}
_SABR_BODY_KEYS = {
    "alpha",
    "beta",
    "nu",
    "rho",
    "shift",
    "forward",
    "time_to_expiry",
    "strikes",
}
_METADATA_KEYS = {
    "name",
    "domain",
    "description",
    "valuation_date",
    "source",
    "source_detail",
    "captured_by",
    "captured_on",
    "last_reviewed_by",
    "last_reviewed_on",
    "review_interval_months",
    "regen_command",
    "screenshots",
}
_SCREENSHOT_KEYS = {"path", "screen", "captured_on", "description"}
_TOLERANCE_KEYS = {"abs", "rel", "tolerance_reason"}
_KINDS = {"pricing", "sabr_smile"}


@dataclass
class Screenshot:
    """Screenshot evidence for manually captured external references."""

    path: str
    screen: str
    captured_on: str
    description: str


@dataclass
class Metadata:
    """Fixture identity, provenance, and review metadata."""

    name: str
    domain: str
    description: str
    valuation_date: str
    source: str
    source_detail: str
    captured_by: str
    captured_on: str
    last_reviewed_by: str
    last_reviewed_on: str
    review_interval_months: int
    regen_command: str
    screenshots: list[Screenshot] = field(default_factory=list)


@dataclass
class ToleranceEntry:
    """Per-metric tolerance entry."""

    abs: float | None = None
    rel: float | None = None
    tolerance_reason: str | None = None


@dataclass
class GoldenFixture:
    """Top-level fixture envelope loaded from one JSON file."""

    schema_version: str
    metadata: Metadata
    kind: str
    body: dict[str, Any]
    expected: dict[str, float]
    tolerances: dict[str, ToleranceEntry]

    @classmethod
    def from_path(cls, path: Path) -> GoldenFixture:
        """Load and parse a golden fixture from disk."""
        raw = json.loads(path.read_text(encoding="utf-8"))
        return cls.from_dict(raw)

    @classmethod
    def from_dict(cls, raw: dict[str, Any]) -> GoldenFixture:
        """Parse a golden fixture from an in-memory mapping."""
        kind = raw.get("kind")
        if kind not in _KINDS:
            msg = f"fixture kind must be one of {sorted(_KINDS)}, got {kind!r}"
            raise ValueError(msg)
        body_keys = _PRICING_BODY_KEYS if kind == "pricing" else _SABR_BODY_KEYS
        _reject_unknown_keys("fixture", raw, _COMMON_TOP_LEVEL_KEYS | body_keys)

        metadata = _parse_metadata(raw["metadata"])
        tolerances = {}
        for metric, tolerance in raw["tolerances"].items():
            _reject_unknown_keys(f"tolerances.{metric}", tolerance, _TOLERANCE_KEYS)
            tolerances[metric] = ToleranceEntry(**tolerance)
        body = {key: raw[key] for key in body_keys if key in raw}
        return cls(
            schema_version=raw["schema_version"],
            metadata=metadata,
            kind=kind,
            body=body,
            expected={metric: float(value) for metric, value in raw["expected"].items()},
            tolerances=tolerances,
        )


def _parse_metadata(raw: dict[str, Any]) -> Metadata:
    _reject_unknown_keys("metadata", raw, _METADATA_KEYS)
    screenshots = []
    for screenshot in raw.get("screenshots", []):
        _reject_unknown_keys("screenshot", screenshot, _SCREENSHOT_KEYS)
        screenshots.append(Screenshot(**screenshot))
    return Metadata(
        name=raw["name"],
        domain=raw["domain"],
        description=raw["description"],
        valuation_date=raw["valuation_date"],
        source=raw["source"],
        source_detail=raw["source_detail"],
        captured_by=raw["captured_by"],
        captured_on=raw["captured_on"],
        last_reviewed_by=raw["last_reviewed_by"],
        last_reviewed_on=raw["last_reviewed_on"],
        review_interval_months=raw["review_interval_months"],
        regen_command=raw["regen_command"],
        screenshots=screenshots,
    )


def _reject_unknown_keys(label: str, value: dict[str, Any], allowed: set[str]) -> None:
    extra = sorted(set(value) - allowed)
    if extra:
        msg = f"{label} has unknown key(s): {extra}"
        raise ValueError(msg)
