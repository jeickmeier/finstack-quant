"""Canonical rating registry ids and version-2 wire validation."""

import json
from typing import Any

import pytest

from finstack_quant.core.rating_scales import RatingScaleRegistry, embedded_registry
from finstack_quant.statements_analytics import ScorecardConfig


def _document() -> dict[str, Any]:
    value = json.loads(embedded_registry().to_json())
    value["scorecard_policy"]["unknown_scale_policy"] = "error"
    return value


def test_registry_normalizes_ids_and_serializes_one_id_per_entry() -> None:
    value = _document()
    assert value["schema"] == "finstack_quant.rating_scales/2"
    value["scorecard_policy"]["default_scale_id"] = " SP "
    value["rating_scales"][0]["id"] = " Sp "
    registry = RatingScaleRegistry.from_json(json.dumps(value))
    assert registry.scale_ids() == ["sp", "moodys"]
    assert registry.default_scale_id() == "sp"
    assert registry.rating_scale(" SP ").scale_name == "S&P"
    assert registry.rating_scale(" MOODYS ").scale_name == "Moody's"
    normalized = json.loads(registry.to_json())
    assert normalized["rating_scales"][0]["id"] == "sp"
    assert "ids" not in normalized["rating_scales"][0]
    assert "aliases" not in normalized["scorecard_policy"]
    assert ScorecardConfig().rating_scale == "sp"


@pytest.mark.parametrize("alias", ["S&P", "Fitch", "standard_and_poors", "Moody's"])
def test_retired_aliases_obey_explicit_unknown_policy(alias: str) -> None:
    value = _document()
    registry = RatingScaleRegistry.from_json(json.dumps(value))
    assert not registry.is_known_rating_scale(alias)
    with pytest.raises(ValueError, match="rating scale registry does not contain scale"):
        registry.rating_scale(alias)
    for policy in ["fallback_to_default", "warn_and_fallback"]:
        value["scorecard_policy"]["unknown_scale_policy"] = policy
        fallback = RatingScaleRegistry.from_json(json.dumps(value))
        assert not fallback.is_known_rating_scale(alias)
        assert fallback.rating_scale(alias).scale_name == "S&P"


@pytest.mark.parametrize(
    ("kind", "reason"),
    [
        ("schema", "unsupported rating scale registry schema version"),
        ("aliases", "unknown field `aliases`"),
        ("ids", "unknown field `ids`"),
        ("duplicate", "duplicate rating scale id"),
        ("blank", "blank rating scale id"),
    ],
)
def test_registry_rejects_legacy_or_invalid_id_documents(kind: str, reason: str) -> None:
    value = _document()
    if kind == "schema":
        value["schema"] = "finstack_quant.rating_scales/1"
    elif kind == "aliases":
        value["scorecard_policy"]["aliases"] = []
    elif kind == "ids":
        value["rating_scales"][0]["ids"] = ["sp"]
    else:
        value["rating_scales"][1]["id"] = " SP " if kind == "duplicate" else "  "
    with pytest.raises(ValueError, match=reason):
        RatingScaleRegistry.from_json(json.dumps(value))
