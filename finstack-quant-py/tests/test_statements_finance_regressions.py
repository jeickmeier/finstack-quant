"""Statement-model JSON and DataFrame boundaries preserve economic inputs."""

import json

import pandas as pd
import pytest

from finstack_quant import statements as s


def _model() -> s.FinancialModelSpec:
    builder = s.ModelBuilder("identity")
    builder.periods("2025Q1..Q2", "2025Q1")
    builder.value_scalar("revenue", {"2025Q1": 100.0})
    builder.forecast("revenue", s.ForecastSpec.growth(0.1))
    builder.value_scalar("cost", {"2025Q1": 40.0})
    builder.forecast("cost", s.ForecastSpec.growth(0.0))
    return builder.build()


def test_json_cannot_alias_another_nodes_forecast_cache() -> None:
    payload = json.loads(_model().to_json())
    payload["nodes"]["cost"]["node_id"] = "revenue"
    text = json.dumps(payload)
    for load in [s.FinancialModelSpec.from_json, s.Evaluator().evaluate]:
        with pytest.raises(ValueError, match="does not match its embedded node_id"):
            load(text)


@pytest.mark.parametrize("cell", [300.0, None, float("nan")])
def test_dataframe_rejects_extra_period_without_overwriting_existing_nodes(cell: float | None) -> None:
    builder = s.ModelBuilder("retained")
    builder.periods("2025Q1..Q2", "2025Q1")
    builder.value_scalar("revenue", {"2025Q1": 7.0})
    frame = pd.DataFrame([[100.0, cell]], index=["revenue"], columns=["2025Q1", "2025Q3"])
    with pytest.raises(ValueError, match=r"2025Q3.*not present in the model timeline"):
        builder.from_dataframe(frame)
    model = builder.build()
    assert model.periods == ["2025Q1", "2025Q2"]
    assert model.get_node("revenue").values == {"2025Q1": 7.0}


@pytest.mark.parametrize("ready", [False, True])
def test_dataframe_parses_all_null_column_labels_before_mutation(ready: bool) -> None:
    builder = s.ModelBuilder("bad-label")
    if ready:
        builder.periods("2025Q1..Q2", "2025Q1")
    frame = pd.DataFrame([[100.0, None, 200.0]], index=["revenue"], columns=["2025Q1", "typo", "2025Q2"])
    with pytest.raises(ValueError, match="invalid period id"):
        builder.from_dataframe(frame)
    if not ready:
        builder.periods("2025Q1..Q2", "2025Q1")
    assert builder.build().node_count == 0


def test_dataframe_invalid_cell_does_not_install_an_inferred_timeline() -> None:
    builder = s.ModelBuilder("bad-cell")
    frame = pd.DataFrame([[100.0, "invalid"]], index=["revenue"], columns=["2025Q1", "2025Q2"])
    with pytest.raises(ValueError, match="is not numeric"):
        builder.from_dataframe(frame)
    builder.periods("2025Q3..Q4", "2025Q3")
    assert builder.build().periods == ["2025Q3", "2025Q4"]


def test_dataframe_valid_import_preserves_actual_cutoff_and_missing_cells() -> None:
    builder = s.ModelBuilder("valid-import")
    frame = pd.DataFrame([[100.0, None, 300.0]], index=["revenue"], columns=["2025Q1", "2025Q2", "2025Q3"])
    builder.from_dataframe(frame, actuals_until="2025Q1")
    model = builder.build()
    assert model.actual_periods == ["2025Q1"]
    assert model.forecast_periods == ["2025Q2", "2025Q3"]
    assert model.get_node("revenue").values == {"2025Q1": 100.0, "2025Q3": 300.0}


def test_wide_export_index_name_is_rejected_during_model_construction() -> None:
    builder = s.ModelBuilder("export-index").periods("2025Q1..Q1", None)
    builder.value_scalar("period_id", {"2025Q1": 100.0})
    with pytest.raises(ValueError, match=r"period_id.*reserved"):
        builder.build()
