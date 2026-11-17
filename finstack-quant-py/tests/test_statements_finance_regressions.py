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


def test_dataframe_parses_all_null_column_labels_before_mutation() -> None:
    builder = s.ModelBuilder("bad-label")
    builder.periods("2025Q1..Q2", "2025Q1")
    frame = pd.DataFrame([[100.0, None, 200.0]], index=["revenue"], columns=["2025Q1", "typo", "2025Q2"])
    with pytest.raises(ValueError, match="Period column 'typo'"):
        builder.from_dataframe(frame)
    with pytest.raises(ValueError, match="at least one node"):
        builder.build()


def test_dataframe_import_needs_a_timeline_and_never_infers_one() -> None:
    builder = s.ModelBuilder("no-plan")
    frame = pd.DataFrame([[100.0, 200.0]], index=["revenue"], columns=["2025Q1", "2025Q2"])
    with pytest.raises(ValueError, match="Set the model timeline"):
        builder.from_dataframe(frame)
    builder.periods("2025Q3..Q4", "2025Q3")
    builder.value("revenue", [])
    assert builder.build().periods == ["2025Q3", "2025Q4"]


def test_dataframe_invalid_cell_leaves_the_builder_unchanged() -> None:
    builder = s.ModelBuilder("bad-cell")
    builder.periods("2025Q1..Q2", "2025Q1")
    frame = pd.DataFrame([[100.0, "invalid"]], index=["revenue"], columns=["2025Q1", "2025Q2"])
    with pytest.raises(ValueError, match="is not numeric"):
        builder.from_dataframe(frame)
    builder.value("revenue", [])
    assert builder.build().get_node("revenue").values == {}


def test_dataframe_valid_import_maps_unordered_columns_and_keeps_missing_cells_and_zeros() -> None:
    builder = s.ModelBuilder("valid-import")
    builder.periods("2025Q1..Q3", "2025Q1")
    frame = pd.DataFrame(
        [[300.0, 100.0, None], [0.0, float("nan"), 5.0]],
        index=["revenue", "cash"],
        columns=["2025Q3", "2025Q1", "2025Q2"],
    )
    builder.from_dataframe(frame)
    model = builder.build()
    assert model.actual_periods == ["2025Q1"]
    assert model.forecast_periods == ["2025Q2", "2025Q3"]
    assert model.get_node("revenue").values == {"2025Q3": 300.0, "2025Q1": 100.0}
    assert model.get_node("cash").values == {"2025Q3": 0.0, "2025Q2": 5.0}

    duplicate = s.ModelBuilder("duplicate-column")
    duplicate.periods("2025Q1..Q3", None)
    repeated = pd.DataFrame([[1.0, 2.0]], index=["revenue"], columns=["2025Q1", "2025Q1"])
    with pytest.raises(ValueError, match="more than once"):
        duplicate.from_dataframe(repeated)


def test_named_edits_snapshot_identity_and_observation_cutoff() -> None:
    """STATEMENTS-CONTRACTS-002: revision replay through the Python surface."""
    from finstack_quant.core.market_data import MarketContext

    def snapshot(snapshot_id: str, amount: float, published: str) -> s.FinancialModelSpec:
        builder = s.ModelBuilder("replay")
        builder.periods("2025Q1..Q2", "2025Q1")
        builder.value("revenue", [("2025Q1", amount), ("2025Q2", 125.0)])
        builder.availability_dates("revenue", [("2025Q1", published)])
        builder.source_snapshot(snapshot_id)
        return builder.build()

    original = snapshot("original", 100.0, "2025-04-25")
    revised = original.apply_edits([
        {
            "edit": "set_observation",
            "node_id": "revenue",
            "period": "2025Q1",
            "value": 110.0,
            "availability_date": "2025-05-15",
        }
    ])
    assert original.get_node("revenue").values["2025Q1"] == 100.0, "edits never change the source model"
    assert original.source_snapshot == "original"

    def read(model: s.FinancialModelSpec, cutoff: str) -> float:
        result = s.Evaluator().evaluate_with_market(model, MarketContext(), "2025-06-30", cutoff)
        return result.get("revenue", "2025Q1")

    assert read(original, "2025-04-25") == 100.0
    with pytest.raises(ValueError, match="available on 2025-04-25"):
        read(original, "2025-04-24")
    assert read(revised, "2025-05-15") == 110.0
    with pytest.raises(ValueError, match="available on 2025-05-15"):
        read(revised, "2025-05-14")
    assert read(revised, "full_snapshot") == 110.0

    with pytest.raises(ValueError, match="unknown node 'absent'"):
        original.apply_edits([{"edit": "remove_node", "node_id": "absent"}])
    with pytest.raises(ValueError, match="invalid model edits"):
        original.apply_edits([{"edit": "rename_node", "node_id": "revenue"}])

    builder = s.ModelBuilder("duplicates")
    builder.periods("2025Q1..Q1", None)
    builder.value("revenue", [("2025Q1", 1.0)])
    builder.value("revenue", [("2025Q1", 2.0)])
    with pytest.raises(ValueError, match="defined more than once"):
        builder.build()


def test_wide_export_index_name_is_rejected_during_model_construction() -> None:
    builder = s.ModelBuilder("export-index").periods("2025Q1..Q1", None)
    builder.value_scalar("period_id", {"2025Q1": 100.0})
    with pytest.raises(ValueError, match=r"period_id.*reserved"):
        builder.build()
