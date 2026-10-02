"""Host contracts for forecast metrics and comparable-company construction."""

from __future__ import annotations

import json
import pickle

import pandas as pd
import pytest

from finstack_quant.statements import ModelBuilder
from finstack_quant.statements_analytics import (
    Exposure,
    ForecastMetrics,
    PeerSet,
    SensitivityConfig,
    Stage,
    StagingConfig,
    backtest_forecast,
    classify_stage,
    evaluate_dcf,
    generate_tornado_entries,
    run_sensitivity,
)


def test_zero_forecast_metrics_round_trip_through_json_and_pickle() -> None:
    result = backtest_forecast([0.0, 0.0], [0.0, 0.0])
    assert result.mape is None
    assert result.smape is None
    assert result.mape_effective_n == 0
    assert result.mae == result.rmse == 0.0
    document = json.loads(result.to_json())
    assert document["mape"] is None
    assert document["smape"] is None
    assert ForecastMetrics.from_json(result.to_json()) == result
    assert pickle.loads(pickle.dumps(result)) == result  # noqa: S301 - trusted in-process round trip
    series = result.to_series()
    assert isinstance(series, pd.Series)
    assert pd.isna(series["mape"])
    assert pd.isna(series["smape"])


def test_zero_actual_preserves_available_symmetric_percentage() -> None:
    result = backtest_forecast([0.0], [1.0])
    assert result.mape is None
    assert result.smape == pytest.approx(200.0)


@pytest.mark.parametrize("value", [float("nan"), float("inf"), -float("inf")])
def test_backtest_rejects_nonfinite_values(value: float) -> None:
    with pytest.raises(ValueError, match="finite"):
        backtest_forecast([value], [0.0])
    with pytest.raises(ValueError, match="finite"):
        backtest_forecast([0.0], [value])


def test_backtest_rejects_arithmetic_overflow() -> None:
    with pytest.raises(ValueError, match="overflow"):
        backtest_forecast([1e200], [0.0])


def test_dataframe_constructor_returns_peer_set() -> None:
    frame = pd.DataFrame({"company": ["SUBJ", "PEER"], "leverage": [3.0, 2.0]})
    result = PeerSet.from_dataframe(frame, "SUBJ", id_column="company")
    assert isinstance(result, PeerSet)
    assert result.subject.id == "SUBJ"
    assert [peer.id for peer in result.peers] == ["PEER"]


def test_disabled_relative_staging_threshold_round_trips() -> None:
    assert StagingConfig().pd_delta_relative == 2.0
    config = StagingConfig(pd_delta_relative=None)
    assert config.pd_delta_relative is None
    assert json.loads(config.to_json())["pd_delta_relative"] is None
    assert StagingConfig.from_json(config.to_json()).pd_delta_relative is None
    restored = pickle.loads(pickle.dumps(config))  # noqa: S301 - trusted in-process round trip
    assert restored.pd_delta_relative is None
    assert restored.to_json() == config.to_json()


@pytest.mark.parametrize("threshold", [float("nan"), float("inf"), -float("inf"), -1.0])
def test_staging_rejects_invalid_enabled_relative_threshold(threshold: float) -> None:
    with pytest.raises(ValueError, match=r"finite|non-negative"):
        StagingConfig(pd_delta_relative=threshold)


def test_staging_json_rejects_invalid_enabled_relative_threshold() -> None:
    document = json.loads(StagingConfig().to_json())
    document["pd_delta_relative"] = -1.0
    with pytest.raises(ValueError, match="non-negative"):
        StagingConfig.from_json(json.dumps(document))


def test_relative_staging_rejects_positive_pd_from_zero_origination() -> None:
    exposure = Exposure("zero-origin", 1_000.0, 0.45, 0.06, 3.0, 0.005, 0.0)
    with pytest.raises(ValueError, match="ratio"):
        classify_stage(exposure)


def test_relative_staging_keeps_both_zero_pds_in_stage_one() -> None:
    exposure = Exposure("both-zero", 1_000.0, 0.45, 0.06, 3.0, 0.0, 0.0)
    assert classify_stage(exposure).stage is Stage.Stage1


def test_disabled_relative_staging_uses_absolute_trigger_from_zero_origination() -> None:
    exposure = Exposure("absolute-only", 1_000.0, 0.45, 0.06, 3.0, 0.005, 0.0)
    config = StagingConfig(pd_delta_relative=None)
    assert classify_stage(exposure, config).stage is Stage.Stage1
    config = StagingConfig(pd_delta_absolute=0.001, pd_delta_relative=None)
    assert classify_stage(exposure, config).stage is Stage.Stage2


def test_growth_terminal_rejects_annual_identifier_with_stub_dates() -> None:
    model = {
        "id": "annual-stub-review",
        "periods": [{"id": "2025", "start": "2025-07-01", "end": "2026-01-01", "is_actual": False}],
        "nodes": {
            "ufcf": {
                "node_id": "ufcf",
                "node_type": "value",
                "values": {"2025": {"amount": "100", "currency": "USD"}},
            }
        },
        "meta": {"currency": "USD"},
        "schema_version": 1,
    }
    with pytest.raises(ValueError, match="complete contiguous history"):
        evaluate_dcf(
            json.dumps(model),
            0.1,
            '{"type":"gordon_growth","stable_growth_rate":0.02}',
            net_debt_override=0.0,
        )


def test_growth_terminal_uses_full_year_across_annual_stub_identifiers() -> None:
    def terminal_pv(actual_id: str, forecast_id: str) -> float:
        model = {
            "id": "complete-year-review",
            "periods": [
                {"id": actual_id, "start": "2024-07-01", "end": "2025-01-01", "is_actual": True},
                {"id": forecast_id, "start": "2025-01-01", "end": "2025-07-01", "is_actual": False},
            ],
            "nodes": {
                "ufcf": {
                    "node_id": "ufcf",
                    "node_type": "value",
                    "values": {
                        actual_id: {"amount": "100", "currency": "USD"},
                        forecast_id: {"amount": "100", "currency": "USD"},
                    },
                }
            },
            "meta": {"currency": "USD"},
            "schema_version": 1,
        }
        result = evaluate_dcf(
            json.dumps(model),
            0.1,
            '{"type":"gordon_growth","stable_growth_rate":0.02}',
            net_debt_override=0.0,
        )
        return result.terminal_value_pv.amount

    assert terminal_pv("2024", "2025") == pytest.approx(terminal_pv("2024H2", "2025H1"))


def test_tornado_rejects_joint_scenarios_mislabeled_as_diagonal_json() -> None:
    builder = ModelBuilder("joint-shocks")
    builder.periods("2025Q1..Q1", None)
    builder.value("revenue", [("2025Q1", 100.0)])
    builder.value("cost", [("2025Q1", 20.0)])
    builder.compute("profit", "revenue - cost")
    result = run_sensitivity(
        builder.build(),
        SensitivityConfig(
            "full_grid",
            [
                ("revenue", "2025Q1", 100.0, [90.0, 110.0]),
                ("cost", "2025Q1", 20.0, [10.0, 30.0]),
            ],
            ["profit"],
        ),
    )
    document = json.loads(result.to_json())
    document["config"]["mode"] = "diagonal"
    with pytest.raises(ValueError, match="exactly one"):
        generate_tornado_entries(json.dumps(document), "profit", "2025Q1")
