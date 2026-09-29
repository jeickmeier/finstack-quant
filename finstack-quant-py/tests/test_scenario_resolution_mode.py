from __future__ import annotations

import pytest

from finstack_quant.scenarios import (
    CurveKind,
    OperationSpec,
    ScenarioSpec,
    TemplateMetadata,
    build_from_template,
    compose_scenarios,
    list_builtin_template_metadata,
)


def _time_roll_scenario(
    scenario_id: str,
    period: str,
    priority: int,
) -> ScenarioSpec:
    operation = OperationSpec.time_roll_forward(period)
    return ScenarioSpec(
        scenario_id,
        [operation],
        priority=priority,
        resolution_mode="cumulative",
    )


def test_scenario_spec_runtime_doc_describes_resolution_contract() -> None:
    doc = ScenarioSpec.__doc__
    assert doc
    assert "resolution_mode" in doc
    assert "most_specific_wins" in doc
    assert "cumulative" in doc
    assert "ValueError" in doc


def test_scenario_spec_exposes_hazard_bump_mode() -> None:
    defaulted = ScenarioSpec("default-hazard", [])
    first_order = ScenarioSpec(
        "first-order-hazard",
        [],
        hazard_bump_mode="first_order_shift",
    )
    composed = compose_scenarios([
        first_order,
        ScenarioSpec(
            "also-first-order",
            [],
            hazard_bump_mode="first_order_shift",
        ),
    ])

    assert defaulted.hazard_bump_mode == "solve_to_par"
    assert first_order.hazard_bump_mode == "first_order_shift"
    assert composed.hazard_bump_mode == "first_order_shift"
    assert defaulted.with_hazard_bump_mode("first_order_shift").hazard_bump_mode == "first_order_shift"
    assert defaulted.hazard_bump_mode == "solve_to_par", "with_hazard_bump_mode returns a copy"


def test_compose_scenarios_rejects_mixed_hazard_bump_modes() -> None:
    first_order = ScenarioSpec(
        "first-order-hazard",
        [],
        hazard_bump_mode="first_order_shift",
    )
    solve_to_par = ScenarioSpec("solve-to-par-hazard", [])

    with pytest.raises(
        ValueError,
        match=r"first-order-hazard.*first_order_shift.*solve-to-par-hazard.*solve_to_par",
    ):
        compose_scenarios([first_order, solve_to_par])


def test_scenario_spec_exposes_resolution_mode() -> None:
    spec = ScenarioSpec(
        "cumulative-shock",
        [],
        resolution_mode="cumulative",
    )
    assert spec.resolution_mode == "cumulative"
    # Rust `wire::serde_parse` owns the message: it names the bad value and
    # lists the serde variants.
    with pytest.raises(ValueError, match=r'invalid value "nope".*most_specific_wins'):
        ScenarioSpec("bad", [], resolution_mode="nope")


def test_scenario_spec_preserves_typed_operations() -> None:
    operation = OperationSpec.curve_parallel_bp(CurveKind.discount(), "USD-OIS", 25.0)
    spec = ScenarioSpec("rates", [operation])

    assert len(spec.operations) == 1
    assert spec.operations[0].kind == "curve_parallel_bp"
    assert spec.operations[0] == operation
    assert '"curve_kind":"discount"' in spec.to_json()


def test_scenario_spec_json_roundtrip_is_typed_and_equal() -> None:
    built = ScenarioSpec("typed", [], name="Typed")
    parsed = ScenarioSpec.from_json(built.to_json())

    assert isinstance(parsed, ScenarioSpec)
    assert parsed.id == "typed"
    assert parsed.name == "Typed"
    assert parsed.operations == []
    assert parsed == built
    assert parsed != ScenarioSpec("other", [])


def test_template_helpers_return_typed_values() -> None:
    metadata = list_builtin_template_metadata()
    built = build_from_template(metadata[0].id)

    assert metadata
    assert isinstance(metadata[0], TemplateMetadata)
    assert metadata[0] == TemplateMetadata.from_json(metadata[0].to_json())
    assert isinstance(built, ScenarioSpec)
    assert built.id == metadata[0].id


def test_compose_scenarios_rejects_duplicate_time_rolls() -> None:
    specs = [
        _time_roll_scenario("roll_1m", "1M", 0),
        _time_roll_scenario("roll_3m", "3M", 1),
    ]

    with pytest.raises(ValueError, match="TimeRollForward"):
        compose_scenarios(specs)


def test_time_roll_period_is_validated_eagerly() -> None:
    bad = OperationSpec.time_roll_forward("three months")
    with pytest.raises(ValueError, match="tenor"):
        bad.validate()
    with pytest.raises(ValueError, match="Operation 0"):
        ScenarioSpec("bad-roll", [bad])
