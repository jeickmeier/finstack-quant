"""Unit tests for golden fixture schema parsing."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from .schema import SCHEMA_VERSION, GoldenFixture


def test_parse_pricing_fixture(tmp_path: Path) -> None:
    fixture_json = _pricing_fixture_json()
    path = tmp_path / "fixture.json"
    path.write_text(json.dumps(fixture_json), encoding="utf-8")

    fixture = GoldenFixture.from_path(path)

    assert fixture.schema_version == SCHEMA_VERSION
    assert fixture.metadata.name == "test_fixture"
    assert fixture.metadata.valuation_date == "2026-04-30"
    assert fixture.kind == "pricing"
    assert fixture.body["model"] == "discounting"
    assert fixture.expected["npv"] == 100.0
    assert fixture.metadata.screenshots == []


def test_parse_sabr_fixture(tmp_path: Path) -> None:
    fixture_json = _sabr_fixture_json()
    path = tmp_path / "fixture.json"
    path.write_text(json.dumps(fixture_json), encoding="utf-8")

    fixture = GoldenFixture.from_path(path)

    assert fixture.kind == "sabr_smile"
    assert fixture.body["strikes"][0]["key"] == "vol_k0050"
    assert "shift" not in fixture.body


def test_rejects_unknown_top_level_field(tmp_path: Path) -> None:
    fixture_json = _pricing_fixture_json()
    fixture_json["unexpected"] = True
    path = tmp_path / "fixture.json"
    path.write_text(json.dumps(fixture_json), encoding="utf-8")

    with pytest.raises(ValueError, match="fixture has unknown key"):
        GoldenFixture.from_path(path)


def test_rejects_unknown_metadata_field(tmp_path: Path) -> None:
    fixture_json = _pricing_fixture_json()
    fixture_json["metadata"]["unexpected"] = True
    path = tmp_path / "fixture.json"
    path.write_text(json.dumps(fixture_json), encoding="utf-8")

    with pytest.raises(ValueError, match="metadata has unknown key"):
        GoldenFixture.from_path(path)


def test_rejects_unknown_kind(tmp_path: Path) -> None:
    fixture_json = _pricing_fixture_json()
    fixture_json["kind"] = "bogus"
    path = tmp_path / "fixture.json"
    path.write_text(json.dumps(fixture_json), encoding="utf-8")

    with pytest.raises(ValueError, match="fixture kind must be"):
        GoldenFixture.from_path(path)


def _metadata() -> dict:
    return {
        "name": "test_fixture",
        "domain": "rates.irs",
        "description": "Minimal smoke fixture",
        "valuation_date": "2026-04-30",
        "source": "quantlib",
        "source_detail": "QL 1.34",
        "captured_by": "test",
        "captured_on": "2026-04-30",
        "last_reviewed_by": "test",
        "last_reviewed_on": "2026-04-30",
        "review_interval_months": 6,
        "regen_command": "uv run scripts/goldens/regen.py --kind irs-par",
    }


def _pricing_fixture_json() -> dict:
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": _metadata(),
        "kind": "pricing",
        "model": "discounting",
        "market": {"kind": "envelope", "envelope": {"schema": "finstack.calibration"}},
        "instrument": {"foo": 1},
        "expected": {"npv": 100.0},
        "tolerances": {"npv": {"abs": 0.01}},
    }


def _sabr_fixture_json() -> dict:
    metadata = _metadata()
    metadata["domain"] = "volatility.sabr"
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata,
        "kind": "sabr_smile",
        "alpha": 0.05,
        "beta": 0.5,
        "nu": 0.4,
        "rho": -0.1,
        "forward": 0.05,
        "time_to_expiry": 2.0,
        "strikes": [{"key": "vol_k0050", "strike": 0.05}],
        "expected": {"vol_k0050": 0.2292},
        "tolerances": {"vol_k0050": {"abs": 1e-9}},
    }
