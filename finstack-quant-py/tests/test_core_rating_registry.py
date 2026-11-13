"""Caller-supplied rating registry: strict ids, version-2 wire and band rules."""

import json
from pathlib import Path
from typing import Any

import pytest

from finstack_quant.core.config import FinstackConfig
from finstack_quant.core.rating_scales import (
    RatingLevel,
    RatingScaleEntry,
    RatingScaleRegistry,
    ScorecardScale,
    registry_from_config,
)

FIXTURE = Path(__file__).parent / "data" / "agency_style_rating_scales.json"


def _document() -> dict[str, Any]:
    return json.loads(FIXTURE.read_text())


def test_registry_normalizes_ids_and_serializes_one_id_per_entry() -> None:
    value = _document()
    assert value["schema"] == "finstack_quant.rating_scales/2"
    value["rating_scales"][0]["id"] = " Sp "
    registry = RatingScaleRegistry.from_json(json.dumps(value))
    assert registry.scale_ids() == ["sp", "moodys"]
    assert registry.rating_scale(" SP ").scale_name == "S&P-style test scale"
    assert registry.entry(" MOODYS ").source.startswith("Test fixture")
    normalized = json.loads(registry.to_json())
    assert normalized["rating_scales"][0]["id"] == "sp"
    assert "scorecard_policy" not in normalized


@pytest.mark.parametrize("alias", ["S&P", "Fitch", "standard_and_poors", "Moody's"])
def test_unknown_ids_are_rejected_without_fallback(alias: str) -> None:
    registry = RatingScaleRegistry.from_json(json.dumps(_document()))
    assert not registry.is_known_rating_scale(alias)
    with pytest.raises(ValueError, match="rating scale registry does not contain scale"):
        registry.rating_scale(alias)
    with pytest.raises(ValueError, match="rating scale registry does not contain scale"):
        registry.entry(alias)


@pytest.mark.parametrize(
    ("kind", "reason"),
    [
        ("schema", "unsupported rating scale registry schema version"),
        ("policy", "unknown field `scorecard_policy`"),
        ("ids", "unknown field `ids`"),
        ("duplicate", "duplicate rating scale id"),
        ("blank", "blank scale id"),
        ("source", "blank source"),
        ("empty", "contains no scales"),
    ],
)
def test_registry_rejects_invalid_documents(kind: str, reason: str) -> None:
    value = _document()
    if kind == "schema":
        value["schema"] = "finstack_quant.rating_scales/1"
    elif kind == "policy":
        value["scorecard_policy"] = {"default_score": 50.0}
    elif kind == "ids":
        value["rating_scales"][0]["ids"] = ["sp"]
    elif kind == "source":
        value["rating_scales"][0]["source"] = "  "
    elif kind == "empty":
        value["rating_scales"] = []
    else:
        value["rating_scales"][1]["id"] = " SP " if kind == "duplicate" else "  "
    with pytest.raises(ValueError, match=reason):
        RatingScaleRegistry.from_json(json.dumps(value))


def test_scale_bands_cover_the_domain_and_hold_their_representatives() -> None:
    scale = ScorecardScale("Internal", [RatingLevel("A", 90.0, 80.0), RatingLevel("B", 70.0, 0.0)])
    assert scale.classify(80.0).name == "A"
    assert scale.classify(0.0).name == "B"
    assert scale.level_position(" b ") == 1
    for bad in (-0.1, 100.1, float("nan")):
        with pytest.raises(ValueError, match="must be finite in"):
            scale.classify(bad)
    with pytest.raises(ValueError, match="must have min_score 0"):
        ScorecardScale("gap", [RatingLevel("A", 90.0, 80.0)])
    with pytest.raises(ValueError, match="not below the preceding minimum"):
        ScorecardScale("outside", [RatingLevel("AAA", 100.0, 95.0), RatingLevel("AA+", 95.0, 0.0)])
    with pytest.raises(ValueError, match="duplicate level"):
        ScorecardScale("collide", [RatingLevel("Aa", 90.0, 80.0), RatingLevel(" AA ", 70.0, 0.0)])
    with pytest.raises(ValueError, match="unrated status"):
        RatingLevel("Not Rated", 50.0, 0.0)


def test_registry_constructor_and_config_extension_are_strict() -> None:
    scale = ScorecardScale("Internal", [RatingLevel("A", 90.0, 80.0), RatingLevel("B", 70.0, 0.0)])
    registry = RatingScaleRegistry([RatingScaleEntry(" Internal ", "credit policy 2026", scale)])
    assert registry.scale_ids() == ["internal"]
    assert [entry.id for entry in registry.rating_scales] == ["internal"]
    with pytest.raises(ValueError, match="contains no scales"):
        RatingScaleRegistry([])
    with pytest.raises(ValueError, match="no rating scale registry"):
        registry_from_config(FinstackConfig())
