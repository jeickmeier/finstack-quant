import json

import pytest

from finstack_quant.statements_analytics import ScoringDimension


@pytest.mark.parametrize("predictor", [None, "leverage"])
def test_scoring_dimension_optional_predictor_roundtrip(predictor: str | None) -> None:
    dimension = ScoringDimension("spread", "oas_bp", predictor)
    assert dimension.x == predictor
    wire = json.loads(dimension.to_json())
    assert wire["x_extractor"] == (None if predictor is None else {"named": predictor})
    assert ScoringDimension.from_json(dimension.to_json()).x == predictor


def test_scoring_dimension_rejects_obsolete_predictor_vector() -> None:
    wire = {"label": "spread", "y_extractor": {"named": "oas_bp"}, "weight": 1.0}
    wire["x_extractors"] = [{"named": "leverage"}, {"named": "ebitda_margin"}]
    with pytest.raises(ValueError, match="x_extractors"):
        ScoringDimension.from_json(json.dumps(wire))
    with pytest.raises(TypeError):
        ScoringDimension("spread", "oas_bp", ["leverage"])
