"""Runtime and static contract for calibration dependency graphs."""

from typing import assert_type

from finstack_quant import calibration
from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, CalibrationStep, RateQuote, dry_run


def test_dependency_graph_matches_declared_structure() -> None:
    quote = RateQuote.deposit("Q", "USD-SOFR-OIS", "3M", 0.03)
    plan = CalibrationPlan(
        [
            CalibrationStep.discount("D", "USD", "2026-05-08", quote_set="q"),
            CalibrationStep.forward("F", "USD", "2026-05-08", 0.25, "D", quote_set="q"),
        ],
        quote_sets={"q": ["Q"]},
    )
    report = dry_run(CalibrationEnvelope(plan, market_data=[quote]))
    assert report.is_valid
    graph = report.dependency_graph
    assert_type(graph["initial_ids"], list[str])
    node = graph["nodes"][0]
    assert_type(node["step_index"], int)
    assert_type(node["step_id"], str)
    assert_type(node["kind"], str)
    assert_type(node["reads"], list[str])
    assert_type(node["writes"], list[str])
    assert graph == {
        "initial_ids": [],
        "nodes": [
            {"step_index": 0, "step_id": "D", "kind": "discount", "reads": [], "writes": ["D"]},
            {"step_index": 1, "step_id": "F", "kind": "forward", "reads": ["D"], "writes": ["F"]},
        ],
    }
    assert dry_run(CalibrationEnvelope(CalibrationPlan([]))).dependency_graph == {"initial_ids": [], "nodes": []}
    assert not hasattr(calibration, "_DependencyGraph")
    assert not hasattr(calibration, "_DependencyNode")
