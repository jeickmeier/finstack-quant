"""Binding regressions for semantic validation, units, and result persistence."""

import json
import pickle
from typing import Any

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
    with pytest.raises(ValueError, match="Dimensional mismatch"):
        s.FinancialModelSpec.from_json(text)
    # Model JSON text is not a model: it goes through the loader.
    with pytest.raises(TypeError, match="FinancialModelSpec"):
        s.Evaluator().evaluate(text)


@pytest.mark.parametrize("declared", [{"type": "scalar"}, {"type": "monetary", "currency": "USD"}])
def test_declared_type_cannot_relabel_explicit_money(declared: dict[str, str]) -> None:
    payload = json.loads(money_model().to_json())
    payload["nodes"]["eur"]["value_type"] = declared
    with pytest.raises(ValueError, match=r"declares.*explicit values"):
        s.FinancialModelSpec.from_json(json.dumps(payload))
    # Model JSON text is not a model: it goes through the loader.
    with pytest.raises(TypeError, match="FinancialModelSpec"):
        s.Evaluator().evaluate(json.dumps(payload))


def test_normalization_preserves_currency_and_rejects_foreign_adjustment() -> None:
    result = s.Evaluator().evaluate(money_model())
    payload = {
        "target_node": "usd",
        "output_node": "adjusted_usd",
        "adjustments": [
            {"id": "a", "name": "a", "value": {"type": "percentage_of_node", "node_id": "eur", "percentage": 0.1}}
        ],
    }
    with pytest.raises(ValueError, match="different units"):
        s.normalize(result, s.NormalizationConfig.from_json(json.dumps(payload)))
    payload["adjustments"][0]["value"]["node_id"] = "usd"
    normalized = s.normalize(result, s.NormalizationConfig.from_json(json.dumps(payload)))[0]
    assert (normalized.value_type, normalized.currency, normalized.final_value) == ("monetary", "USD", 110.0)
    assert normalized.adjustments[0].adjusted_value == normalized.final_value
    restored = pickle.loads(pickle.dumps(normalized))  # noqa: S301 - locally constructed fixture
    assert (restored.currency, restored.final_value) == ("USD", 110.0)


def test_ordinary_missing_results_survive_json_and_pickle() -> None:
    builder = s.ModelBuilder("missing")
    builder.periods("2025Q1..Q2", None)
    builder.value("x", [("2025Q1", 1.0), ("2025Q2", 2.0)])
    builder.compute("lagged", "lag(x, 1)")
    result = s.Evaluator().evaluate(builder.build())
    unavailable = {"status": "unavailable", "reason": "missing_input", "source": "formula"}
    assert json.loads(result.to_json())["cells"]["lagged"]["2025Q1"] == unavailable
    pickled = pickle.loads(pickle.dumps(result))  # noqa: S301 - locally constructed fixture
    for restored in [s.StatementResult.from_json(result.to_json()), pickled]:
        assert restored.get("lagged", "2025Q1") is None
        assert restored.get_cell("lagged", "2025Q1") == unavailable
        assert restored.get("lagged", "2025Q2") == 1.0


def test_capital_structure_warnings_round_trip_and_rescaling_warnings_are_rejected() -> None:
    document = json.loads(s.Evaluator().evaluate(money_model()).to_json())
    warning = {
        "capital_structure": {
            "period": "2025Q1",
            "warning": {"kind": "interest_shortfall", "instrument_id": "TL", "amount": 12.5},
        }
    }
    document["meta"]["warnings"] = [warning]
    restored = s.StatementResult.from_json(json.dumps(document))
    assert restored.warnings == [warning]
    assert json.loads(restored.to_json())["meta"]["warnings"] == [warning]
    # An opening-balance mismatch is an error, so no result carries a rescaling warning.
    for obsolete in (
        {"kind": "residual_scale_mismatch", "applied_ratio": 10_000_000.0},
        {"kind": "scale_clamped", "raw_ratio": 10_000_000.0, "clamped_ratio": 1.0},
    ):
        document["meta"]["warnings"][0]["capital_structure"]["warning"] = obsolete
        with pytest.raises(ValueError, match=obsolete["kind"]):
            s.StatementResult.from_json(json.dumps(document))


@pytest.mark.parametrize("method", ["normal", "log_normal", "mean_reverting"])
@pytest.mark.parametrize("rho", [-1.0, 0.0, 0.6, 1.0])
def test_correlated_paths_keep_shocks_when_peer_levels_round_flat(method: str, rho: float) -> None:
    def run(tiny: bool) -> dict[str, Any]:
        builder = s.ModelBuilder("direct-shocks")
        builder.periods("2025Q1..Q4", "2025Q1")
        builder.value("peer", [("2025Q1", 1e16 if tiny else 100.0)])
        params = {"std_dev": 1e-20 if tiny else 0.1, "seed": "42"}
        if method == "mean_reverting":
            params.update({"long_run_mean": 1e16 if tiny else 100.0, "reversion_speed": 0.1})
        else:
            params["mean"] = 0.0
        builder.forecast("peer", s.ForecastSpec.from_json(json.dumps({"method": method, "params": params})))
        for node, peer, correlation, seed in [("follower", "peer", rho, 19), ("child", "follower", 0.6, 23)]:
            mixed = builder.mixed(node)
            mixed.values([("2025Q1", 100.0)])
            mixed.formula(f"{peer} * 0 + 100")
            mixed.forecast(
                s.ForecastSpec.from_json(
                    json.dumps({
                        "method": "normal",
                        "params": {
                            "mean": 0.0,
                            "std_dev": 1.0,
                            "seed": str(seed),
                            "correlation_with": peer,
                            "correlation": correlation,
                        },
                    })
                )
            )
            builder = mixed.build()
        result = s.Evaluator().evaluate_monte_carlo(
            builder.build(), s.MonteCarloConfig(16, 7, ["peer", "follower", "child"], [0.5], True)
        )
        return json.loads(result.to_json())

    def paths(result: dict[str, Any], metric: str) -> list[float]:
        columns = result["path_data"]["columns"]
        return [
            v for m, v in zip(columns[2]["data"]["values"], columns[3]["data"]["values"], strict=True) if m == metric
        ]

    baseline, rounded = run(False), run(True)
    # A log-normal level is restored from its logarithm, so it is flat to rounding.
    assert all(abs(v / 1e16 - 1.0) < 1e-12 for v in paths(rounded, "peer"))
    for metric in ["follower", "child"]:
        assert paths(rounded, metric) == paths(baseline, metric)
