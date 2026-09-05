"""Behavioural parity for the scenarios engine entry points.

Pins the typed-input acceptance (``ScenarioSpec | str``, ``Instrument | str``,
``FinstackConfig | str | None``, date-like ``as_of``), the exception types the
binding maps scenario errors to, and the report / horizon result surfaces.
"""

from __future__ import annotations

from datetime import date, datetime
import json
import pickle

import pandas as pd
import pytest

from finstack_quant.core.config import FinstackConfig
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.scenarios import (
    ApplicationReport,
    ApplicationResult,
    Compounding,
    CurveKind,
    HierarchyTarget,
    HorizonResult,
    OperationSpec,
    RateBindingSpec,
    ScenarioSpec,
    TenorMatchMode,
    TimeRollMode,
    apply_scenario,
    apply_scenario_to_market,
    compute_horizon_return,
    validate_scenario_spec,
)
from finstack_quant.valuations.instruments import Bond

AS_OF = "2025-01-15"


def _market(as_of: str = AS_OF) -> MarketContext:
    market = MarketContext()
    market.insert(
        DiscountCurve(
            "USD-OIS",
            date.fromisoformat(as_of),
            [(0.0, 1.0), (0.5, 0.98), (1.0, 0.96), (2.0, 0.92)],
            day_count="act_365f",
        )
    )
    return market


def _deposit_json() -> str:
    return json.dumps({
        "schema": "finstack_quant.instrument/1",
        "instrument": {
            "type": "deposit",
            "spec": {
                "id": "DEP-0",
                "notional": {"amount": "1000000", "currency": "USD"},
                "start_date": AS_OF,
                "maturity": "2025-07-15",
                "day_count": "act_360",
                "quote_rate": "0.04",
                "discount_curve_id": "USD-OIS",
                "attributes": {},
            },
        },
    })


def _up_25() -> ScenarioSpec:
    return ScenarioSpec("up25", [OperationSpec.curve_parallel_bp("discount", "USD-OIS", 25.0)])


# ---------------------------------------------------------------------------
# Typed-input acceptance
# ---------------------------------------------------------------------------


def test_apply_scenario_to_market_accepts_spec_or_json_and_date_like_as_of() -> None:
    spec = _up_25()
    typed = apply_scenario_to_market(spec, _market(), AS_OF)
    from_json = apply_scenario_to_market(spec.to_json(), _market(), date(2025, 1, 15))
    from_datetime = apply_scenario_to_market(spec, _market(), datetime(2025, 1, 15, 9, 30))
    from_timestamp = apply_scenario_to_market(spec, _market(), pd.Timestamp("2025-01-15"))

    for result in (typed, from_json, from_datetime, from_timestamp):
        assert isinstance(result, ApplicationResult)
        assert result.model is None
        assert result.report.user_operations == 1
        assert result.report.operations_applied >= 1
    assert typed.market.to_json() == from_json.market.to_json()


def test_apply_scenario_accepts_config_object_string_or_none() -> None:
    spec = _up_25()
    default = apply_scenario_to_market(spec, _market(), AS_OF)
    typed = apply_scenario_to_market(spec, _market(), AS_OF, config=FinstackConfig())
    from_json = apply_scenario_to_market(spec, _market(), AS_OF, config=FinstackConfig().to_json())
    assert default.report.meta is not None
    assert typed.report.meta == default.report.meta
    assert from_json.report.meta == default.report.meta
    with pytest.raises(ValueError, match="config"):
        apply_scenario_to_market(spec, _market(), AS_OF, config=42)


def test_apply_scenario_rejects_non_spec_scenario_argument() -> None:
    with pytest.raises(ValueError, match="ScenarioSpec"):
        apply_scenario_to_market(123, _market(), AS_OF)
    with pytest.raises(ValueError, match="Failed to parse ScenarioSpec JSON"):
        apply_scenario_to_market("{not json", _market(), AS_OF)


# ---------------------------------------------------------------------------
# Error mapping
# ---------------------------------------------------------------------------


def test_missing_curve_raises_key_error() -> None:
    spec = ScenarioSpec("missing", [OperationSpec.curve_parallel_bp("discount", "NOPE", 1.0)])
    with pytest.raises(KeyError):
        apply_scenario_to_market(spec, _market(), AS_OF)


def test_validation_failure_raises_value_error_with_same_message_everywhere() -> None:
    bad_json = json.dumps({"id": "", "operations": []})
    with pytest.raises(ValueError, match=r"Scenario ID cannot be empty") as from_validate:
        validate_scenario_spec(bad_json)
    with pytest.raises(ValueError, match=r"Scenario ID cannot be empty") as from_from_json:
        ScenarioSpec.from_json(bad_json)
    with pytest.raises(ValueError, match=r"Scenario ID cannot be empty") as from_apply:
        apply_scenario_to_market(bad_json, _market(), AS_OF)
    assert str(from_validate.value) == str(from_from_json.value) == str(from_apply.value)
    assert "Scenario ID cannot be empty" in str(from_validate.value)


def test_instrument_mutating_scenario_without_instruments_raises_value_error() -> None:
    spec = ScenarioSpec("px", [OperationSpec.instrument_price_pct_by_type(["bond"], -5.0)])
    assert spec.mutates_instruments()
    assert spec.requires_instruments()
    with pytest.raises(ValueError, match="instruments"):
        apply_scenario_to_market(spec, _market(), AS_OF)


def test_time_roll_without_instruments_still_rolls_market() -> None:
    spec = ScenarioSpec("roll", [OperationSpec.time_roll_forward("1M", roll_mode="calendar_days")])
    assert spec.requires_instruments()
    assert not spec.mutates_instruments()
    result = apply_scenario_to_market(spec, _market(), AS_OF)
    assert result.report.time_roll is not None
    assert result.report.time_roll["days"] == 31
    assert result.report.changes["as_of_changed"] is True
    assert result.report.carry_to_dataframe().empty


def test_instruments_are_accepted_as_json_envelopes_for_carry() -> None:
    spec = ScenarioSpec("roll", [OperationSpec.time_roll_forward("1M", roll_mode="calendar_days")])
    result = apply_scenario_to_market(spec, _market(), AS_OF, instruments=[_deposit_json()])
    carry = result.report.carry_to_dataframe()
    assert list(carry.columns) == ["instrument_id", "amount", "currency"]
    assert list(carry["instrument_id"]) == ["DEP-0"]
    assert list(carry["currency"]) == ["USD"]


# ---------------------------------------------------------------------------
# Report surface
# ---------------------------------------------------------------------------


def test_report_exposes_structured_warnings_and_counters() -> None:
    spec = ScenarioSpec("eq", [OperationSpec.equity_price_pct(["MISSING"], -10.0)])
    result = apply_scenario_to_market(spec, _market(), AS_OF)
    report = result.report
    assert isinstance(report, ApplicationReport)
    assert report.warning_count == len(report.warnings) == 1
    assert report.warnings[0]["kind"] == "equity_not_found"
    assert json.loads(report.warnings_json) == report.warnings
    assert "warnings=1" in repr(result)
    assert "operations_applied=" in repr(report)

    frame = report.to_dataframe()
    assert list(frame.columns) == [
        "operations_applied",
        "user_operations",
        "expanded_operations",
        "warning_count",
        "as_of_changed",
        "all_dirty",
    ]
    assert int(frame.loc[0, "warning_count"]) == 1
    assert list(result.to_dataframe().columns) == list(frame.columns)


def test_changes_to_dataframe_lists_resolved_targets() -> None:
    result = apply_scenario_to_market(_up_25(), _market(), AS_OF)
    changes = result.report.changes_to_dataframe()
    assert list(changes.columns) == ["kind", "id", "curve_kind"]
    assert changes.to_dict("records") == [{"kind": "curve", "id": "USD-OIS", "curve_kind": "discount"}]


def test_application_result_json_roundtrip_and_pickle() -> None:
    result = apply_scenario_to_market(_up_25(), _market(), AS_OF)
    restored = ApplicationResult.from_json(result.to_json())
    assert restored.report.to_json() == result.report.to_json()
    assert restored.market.to_json() == result.market.to_json()
    unpickled = pickle.loads(pickle.dumps(result))  # noqa: S301
    assert unpickled.report.operations_applied == result.report.operations_applied


# ---------------------------------------------------------------------------
# Horizon analysis
# ---------------------------------------------------------------------------


def test_compute_horizon_return_typed_inputs_and_result_surface() -> None:
    spec = ScenarioSpec(
        "hold_1m_up25",
        [
            OperationSpec.time_roll_forward("1M", roll_mode="calendar_days"),
            OperationSpec.curve_parallel_bp(CurveKind.discount(), "USD-OIS", 25.0),
        ],
    )
    result = compute_horizon_return(_deposit_json(), _market(), AS_OF, spec)
    from_json = compute_horizon_return(_deposit_json(), _market(), date(2025, 1, 15), spec.to_json())
    assert isinstance(result, HorizonResult)
    assert result.to_json() == from_json.to_json()
    assert result.currency == "USD"
    assert result.horizon_days == 31
    assert result.total_return == pytest.approx(result.attribution.total_pnl / result.initial_value)
    assert result.annualized_return is not None
    assert isinstance(result.scenario_report, ApplicationReport)
    assert result.scenario_report.user_operations == 2
    assert result.warnings == result.scenario_report.warnings
    assert "total_return" in result.to_dataframe().columns
    assert "total_return_pct" not in result.to_dataframe().columns
    assert result.explain().startswith("Horizon Total Return:")
    assert repr(result).startswith("HorizonResult(total_return=")
    assert result._repr_html_() is not None
    assert HorizonResult.from_json(result.to_json()).total_return == result.total_return


def test_compute_horizon_return_config_and_method_handling() -> None:
    spec = _up_25()
    default = compute_horizon_return(_deposit_json(), _market(), AS_OF, spec)
    with_config = compute_horizon_return(_deposit_json(), _market(), AS_OF, spec, config=FinstackConfig())
    with_json_config = compute_horizon_return(
        _deposit_json(), _market(), AS_OF, spec, config=FinstackConfig().to_json()
    )
    assert with_config.total_return == pytest.approx(default.total_return)
    assert with_json_config.total_return == pytest.approx(default.total_return)
    with pytest.raises(ValueError, match="Unknown attribution method"):
        compute_horizon_return(_deposit_json(), _market(), AS_OF, spec, method="bogus")
    with pytest.raises(KeyError):
        compute_horizon_return(_deposit_json(), _market(), AS_OF, spec, calendar_id="not-a-calendar")


def test_compute_horizon_return_rejects_instrument_scoped_operations() -> None:
    spec = ScenarioSpec("px", [OperationSpec.instrument_price_pct_by_type(["bond"], -5.0)])
    with pytest.raises(ValueError, match="HorizonAnalysis"):
        compute_horizon_return(_deposit_json(), _market(), AS_OF, spec)


def test_compute_horizon_return_missing_curve_is_key_error() -> None:
    spec = ScenarioSpec("missing", [OperationSpec.curve_parallel_bp("discount", "NOPE", 1.0)])
    with pytest.raises(KeyError):
        compute_horizon_return(_deposit_json(), _market(), AS_OF, spec)


# ---------------------------------------------------------------------------
# Spec ergonomics
# ---------------------------------------------------------------------------


def test_enum_wrappers_construct_from_labels_and_hash() -> None:
    assert CurveKind("par_cds") == CurveKind.par_cds()
    assert TenorMatchMode("interpolate") == TenorMatchMode.interpolate()
    assert TimeRollMode("approximate") == TimeRollMode.approximate()
    assert Compounding("semi_annual") == Compounding.semi_annual()
    assert {CurveKind("discount"): 1}[CurveKind.discount()] == 1
    with pytest.raises(ValueError, match="CurveKind"):
        CurveKind("zero")
    with pytest.raises(ValueError, match="TimeRollMode"):
        OperationSpec.time_roll_forward("1M", roll_mode="whenever")


def test_operation_spec_equality_repr_and_validation() -> None:
    op = OperationSpec.curve_parallel_bp("discount", "USD-OIS", 25.0)
    assert op == OperationSpec.curve_parallel_bp(CurveKind.discount(), "USD-OIS", 25.0)
    assert op != OperationSpec.curve_parallel_bp("discount", "USD-OIS", 10.0)
    assert op == OperationSpec.from_json(op.to_json())
    text = repr(op)
    assert text.startswith("OperationSpec(")
    assert 'kind="curve_parallel_bp"' in text
    assert 'curve_id="USD-OIS"' in text
    assert "bp=25.0" in text
    op.validate()
    with pytest.raises(ValueError, match="curve_id"):
        OperationSpec.curve_parallel_bp("discount", "", 25.0).validate()
    assert not op.requires_instruments()
    assert OperationSpec.time_roll_forward("1M").requires_instruments()
    assert not OperationSpec.time_roll_forward("1M").mutates_instruments()


def test_curve_parallel_bp_expands_curve_id_lists() -> None:
    ops = OperationSpec.curve_parallel_bp("discount", ["USD-OIS", "EUR-OIS"], 25.0)
    assert isinstance(ops, list)
    assert len(ops) == 2
    assert [json.loads(o.to_json())["curve_id"] for o in ops] == ["USD-OIS", "EUR-OIS"]
    with pytest.raises(ValueError, match="curve_id"):
        OperationSpec.curve_parallel_bp("discount", 5, 25.0)


def test_attr_operations_accept_mapping_or_pairs() -> None:
    from_mapping = OperationSpec.instrument_price_pct_by_attr({"sector": "tech", "rating": "BBB"}, -5.0)
    from_pairs = OperationSpec.instrument_price_pct_by_attr([("sector", "tech"), ("rating", "BBB")], -5.0)
    assert from_mapping == from_pairs
    assert OperationSpec.instrument_spread_bp_by_attr({"sector": "tech"}, 20.0).kind == ("instrument_spread_bp_by_attr")


def test_hierarchy_target_pyclass_roundtrip() -> None:
    target = HierarchyTarget(["Credit", "US"], {"sector": "financials"})
    assert target.path == ["Credit", "US"]
    assert target.tag_filter == [("sector", "financials")]
    assert HierarchyTarget.from_json(target.to_json()) == target
    typed = OperationSpec.hierarchy_curve_parallel_bp("par_cds", target, 50.0)
    from_json = OperationSpec.hierarchy_curve_parallel_bp(CurveKind.par_cds(), target.to_json(), 50.0)
    assert typed == from_json
    assert OperationSpec.hierarchy_equity_price_pct(HierarchyTarget(["Equity"]), -5.0).kind == (
        "hierarchy_equity_price_pct"
    )
    with pytest.raises(ValueError, match="HierarchyTarget"):
        OperationSpec.hierarchy_equity_price_pct("{bad", -5.0)


def test_rate_binding_spec_equality_and_validation() -> None:
    binding = RateBindingSpec("rate", "USD-OIS", "1Y", compounding="annual")
    assert binding == RateBindingSpec("rate", "USD-OIS", "1Y", compounding=Compounding.annual())
    assert binding == RateBindingSpec.from_json(binding.to_json())
    binding.validate()
    with pytest.raises(ValueError, match=r"Invalid tenor string"):
        RateBindingSpec("rate", "USD-OIS", "soon").validate()
    with pytest.raises(ValueError, match="Compounding"):
        RateBindingSpec("rate", "USD-OIS", "1Y", compounding="hourly")


@pytest.mark.parametrize("with_model", [False, True])
def test_shocked_instrument_copies_survive_result_round_trips(with_model: bool) -> None:
    original = Bond.example()
    before = original.to_json()
    scenario = ScenarioSpec("losses", [OperationSpec.instrument_price_pct_by_type(["bond"], -60.0)] * 2)
    if with_model:
        result = apply_scenario(
            scenario,
            _market(),
            json.dumps({
                "schema_version": 1,
                "id": "m",
                "periods": [{"id": "2025Q1", "start": "2025-01-01", "end": "2025-04-01", "is_actual": False}],
                "nodes": {},
            }),
            AS_OF,
            instruments=[original],
        )
    else:
        result = apply_scenario_to_market(scenario, _market(), AS_OF, instruments=[original])
    assert original.to_json() == before
    for restored in [result, ApplicationResult.from_json(result.to_json()), pickle.loads(pickle.dumps(result))]:  # noqa: S301 - own serialized result
        assert restored.instruments is not None
        assert len(restored.instruments) == 1
        payload = json.loads(restored.instruments[0])
        shock = payload["instrument"]["spec"]["scenario_pricing_overrides"]["scenario_price_shock_pct"]
        assert 100.0 * (1.0 + shock) == pytest.approx(16.0)
        next_result = apply_scenario_to_market(
            ScenarioSpec("half", [OperationSpec.instrument_price_pct_by_type(["bond"], -50.0)]),
            _market(),
            AS_OF,
            instruments=restored.instruments,
        )
        assert next_result.instruments is not None
        next_shock = json.loads(next_result.instruments[0])["instrument"]["spec"]["scenario_pricing_overrides"][
            "scenario_price_shock_pct"
        ]
        assert 100.0 * (1.0 + next_shock) == pytest.approx(8.0)
    assert "instruments" in json.loads(result.to_json())


def test_empty_inventory_is_distinct_from_absent_inventory() -> None:
    spec = ScenarioSpec("empty", [])
    assert apply_scenario_to_market(spec, _market(), AS_OF).instruments is None
    result = apply_scenario_to_market(spec, _market(), AS_OF, instruments=[])
    assert result.instruments == []
    assert ApplicationResult.from_json(result.to_json()).instruments == []
