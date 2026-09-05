"""Binding regressions for semantic validation, units, and result persistence."""

import json
import math
import pickle

import pytest

from finstack_quant import statements as s
from finstack_quant.core.currency import Currency
from finstack_quant.core.money import Money


def money_model() -> s.FinancialModelSpec:
    builder = s.ModelBuilder("units")
    builder.periods("2025Q1..Q1", None)
    builder.value("usd", [("2025Q1", Money(100, Currency("USD")))])
    builder.value("eur", [("2025Q1", Money(100, Currency("EUR")))])
    return builder.build()


def test_raw_json_and_typed_model_reject_the_same_dimension_error() -> None:
    payload = json.loads(money_model().to_json())
    payload["nodes"]["total"] = {"node_id": "total", "node_type": "calculated", "formula_text": "usd + eur"}
    text = json.dumps(payload)
    for load in [s.FinancialModelSpec.from_json, s.Evaluator().evaluate]:
        with pytest.raises(ValueError, match="Dimensional mismatch"):
            load(text)


@pytest.mark.parametrize("declared", [{"type": "scalar"}, {"type": "monetary", "currency": "USD"}])
def test_declared_type_cannot_relabel_explicit_money(declared: dict[str, str]) -> None:
    payload = json.loads(money_model().to_json())
    payload["nodes"]["eur"]["value_type"] = declared
    with pytest.raises(ValueError, match=r"declares.*explicit values"):
        s.FinancialModelSpec.from_json(json.dumps(payload))
    with pytest.raises(ValueError, match=r"declares.*explicit values"):
        s.Evaluator().evaluate(json.dumps(payload))


def test_normalization_preserves_currency_and_rejects_foreign_adjustment() -> None:
    result = s.Evaluator().evaluate(money_model())
    payload = {
        "target_node": "usd",
        "adjustments": [
            {"id": "a", "name": "a", "value": {"type": "percentage_of_node", "node_id": "eur", "percentage": 0.1}}
        ],
    }
    with pytest.raises(ValueError, match="incompatible units"):
        s.normalize(result, s.NormalizationConfig.from_json(json.dumps(payload)))
    payload["adjustments"][0]["value"]["node_id"] = "usd"
    normalized = s.normalize(result, s.NormalizationConfig.from_json(json.dumps(payload)))[0]
    assert (normalized.value_type, normalized.currency, normalized.final_value) == ("monetary", "USD", 110.0)
    restored = pickle.loads(pickle.dumps(normalized))  # noqa: S301 - locally constructed fixture
    assert (restored.currency, restored.final_value) == ("USD", 110.0)


def test_ordinary_missing_results_survive_json_and_pickle() -> None:
    builder = s.ModelBuilder("missing")
    builder.periods("2025Q1..Q2", None)
    builder.value("x", [("2025Q1", 1.0), ("2025Q2", 2.0)])
    builder.compute("lagged", "lag(x, 1)")
    result = s.Evaluator().evaluate(builder.build())
    assert json.loads(result.to_json())["nodes"]["lagged"]["2025Q1"] == "nan"
    pickled = pickle.loads(pickle.dumps(result))  # noqa: S301 - locally constructed fixture
    for restored in [s.StatementResult.from_json(result.to_json()), pickled]:
        assert math.isnan(restored.get("lagged", "2025Q1"))
        assert restored.get("lagged", "2025Q2") == 1.0
