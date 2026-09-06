"""Behavioral tests for regulatory-capital Python bindings."""

from __future__ import annotations

import datetime as dt
import json
import pickle

import pandas as pd
import pytest

from finstack_quant.margin import (
    EadResult,
    FrtbSbaEngine,
    FrtbSbaResult,
    FrtbSensitivities,
    NettingSetId,
    SaCcrEngine,
    SaCcrNettingSetConfig,
    SaCcrTrade,
    frtb_sba_charge,
    saccr_ead,
)


def unmargined_config(as_of: str = "2025-01-15") -> SaCcrNettingSetConfig:
    """Zero-collateral bilateral netting set for the shared fixtures."""
    return SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, as_of)


def linear_trade_payload() -> dict[str, object]:
    """Return a complete canonical linear-trade payload."""
    return {
        "trade_id": "IRS-1",
        "asset_class": "interest_rate",
        "notional": 1_000_000.0,
        "start_date": "2025-01-15",
        "end_date": "2030-01-15",
        "underlier": "USD-SOFR",
        "hedging_set": "USD-IR",
        "direction": 1.0,
        "supervisory_delta": 1.0,
        "mtm": 10_000.0,
        "is_option": False,
        "option_type": None,
    }


def test_sa_ccr_trade_keyword_constructor_matches_json() -> None:
    payload = linear_trade_payload()
    from_json = SaCcrTrade.from_json(json.dumps(payload))
    typed = SaCcrTrade(
        trade_id="IRS-1",
        asset_class="interest_rate",
        notional=1_000_000.0,
        start_date=dt.date(2025, 1, 15),
        end_date="2030-01-15",
        underlier="USD-SOFR",
        hedging_set="USD-IR",
        direction=1.0,
        supervisory_delta=1.0,
        mtm=10_000.0,
    )

    assert typed.to_json() == from_json.to_json()
    assert typed.trade_id == "IRS-1"
    assert typed.asset_class == "interest_rate"
    assert typed.notional == pytest.approx(1_000_000.0)
    assert typed.start_date == dt.date(2025, 1, 15)
    assert typed.end_date == dt.date(2030, 1, 15)
    assert typed.underlier == "USD-SOFR"
    assert typed.hedging_set == "USD-IR"
    assert typed.direction == 1.0
    assert typed.supervisory_delta == 1.0
    assert typed.is_option is False
    assert typed.option_type is None

    with pytest.raises(ValueError, match="agree in sign"):
        SaCcrTrade(
            "X",
            "credit",
            1.0,
            "2025-01-15",
            "2026-01-15",
            "ACME",
            "HS",
            1.0,
            -1.0,
            0.0,
            supervisory_category="credit_bbb",
        )


def test_sa_ccr_trade_dataframe_round_trip() -> None:
    trade = SaCcrTrade.from_json(json.dumps(linear_trade_payload()))
    frame = trade.to_dataframe()
    assert list(frame.columns)[:3] == ["trade_id", "asset_class", "notional"]
    assert frame.iloc[0]["start_date"] == "2025-01-15"

    tape = pd.concat([frame, frame.assign(trade_id="IRS-2", direction=-1.0, supervisory_delta=-1.0)])
    trades = SaCcrTrade.from_dataframe(tape)
    assert [t.trade_id for t in trades] == ["IRS-1", "IRS-2"]
    assert trades[1].direction == -1.0


def test_sa_ccr_trade_from_json_validates_regulatory_semantics() -> None:
    option_payload = linear_trade_payload()
    option_payload["is_option"] = True
    option_payload["option_type"] = "call_long"
    option_payload["supervisory_delta"] = 0.6
    option_payload["option_maturity_date"] = "2026-01-15"
    option = SaCcrTrade.from_json(json.dumps(option_payload))
    assert json.loads(option.to_json())["option_type"] == "call_long"

    payload = linear_trade_payload()
    payload["supervisory_delta"] = 0.5
    with pytest.raises(ValueError, match="linear trade supervisory_delta must be"):
        SaCcrTrade.from_json(json.dumps(payload))

    payload = linear_trade_payload()
    payload["is_option"] = True
    payload["option_type"] = None
    payload["option_maturity_date"] = "2026-01-15"
    with pytest.raises(ValueError, match="requires option_type"):
        SaCcrTrade.from_json(json.dumps(payload))

    payload = linear_trade_payload()
    payload["supervisory_factor"] = 0.05
    with pytest.raises(ValueError, match="unknown field"):
        SaCcrTrade.from_json(json.dumps(payload))


def test_sa_ccr_engine_accepts_only_active_configuration() -> None:
    with pytest.raises(TypeError, match="reporting_currency"):
        SaCcrEngine(reporting_currency="EUR")


def test_saccr_ead_returns_typed_result() -> None:
    """`saccr_ead` returns the full `EadResult`, not a lossy 3-tuple."""
    trade = SaCcrTrade.from_json(json.dumps(linear_trade_payload()))

    result = saccr_ead([trade], unmargined_config())

    assert isinstance(result, EadResult)
    assert result.ead == pytest.approx(result.alpha * (result.rc + result.pfe))
    # Fields the old (rc, pfe, ead) tuple dropped entirely.
    assert result.alpha == pytest.approx(1.4)
    assert result.multiplier > 0.0
    assert result.maturity_factor > 0.0
    assert result.add_on_aggregate > 0.0
    assert result.add_on_by_asset_class["interest_rate"] == pytest.approx(result.add_on_aggregate)


def test_ead_result_wire_and_frame_surfaces() -> None:
    trade = SaCcrTrade.from_json(json.dumps(linear_trade_payload()))
    result = saccr_ead([trade], unmargined_config())

    round_tripped = EadResult.from_json(result.to_json())
    assert round_tripped.ead == pytest.approx(result.ead)
    assert pickle.loads(pickle.dumps(result)).ead == pytest.approx(  # noqa: S301
        result.ead
    )
    assert "EadResult(" in repr(result)

    frame = result.to_dataframe()
    assert list(frame.columns) == [
        "ead",
        "rc",
        "pfe",
        "multiplier",
        "add_on_aggregate",
        "alpha",
        "maturity_factor",
    ]
    assert len(frame) == 1
    assert str(frame["ead"].dtype) == "float64"

    add_ons = result.to_add_on_dataframe()
    assert list(add_ons.columns) == ["asset_class", "add_on"]
    assert add_ons["asset_class"].tolist() == ["interest_rate"]
    assert str(add_ons["add_on"].dtype) == "float64"


def test_sa_ccr_engine_calculate_ead_returns_typed_result() -> None:
    config = unmargined_config()

    result = SaCcrEngine(alpha=1.5).calculate_ead(config, [])

    assert isinstance(result, EadResult)
    assert result.alpha == pytest.approx(1.5)
    assert result.ead == pytest.approx(0.0)
    assert result.add_on_by_asset_class == {}


def sample_sensitivities() -> FrtbSensitivities:
    sens = FrtbSensitivities("USD")
    sens.add_girr_delta("5Y", 25_000.0)
    sens.add_equity_delta("ACME", 1, 12_000.0)
    sens.add_rrao_position("EXOTIC-1", 5_000_000.0, True)
    return sens


def test_frtb_sba_charge_returns_typed_result() -> None:
    result = frtb_sba_charge(sample_sensitivities())

    assert isinstance(result, FrtbSbaResult)
    assert result.total > 0.0
    assert result.rrao > 0.0
    assert result.drc >= 0.0
    assert result.binding_scenario in {"low", "medium", "high"}
    assert set(result.scenario_charges) <= {"low", "medium", "high"}
    assert result.delta_by_risk_class["girr"] > 0.0
    assert result.delta_by_risk_class["equity"] > 0.0


def test_frtb_sba_result_wire_and_frame_surfaces() -> None:
    result = FrtbSbaEngine().calculate(sample_sensitivities())

    assert isinstance(result, FrtbSbaResult)
    round_tripped = FrtbSbaResult.from_json(result.to_json())
    assert round_tripped.total == pytest.approx(result.total)
    assert pickle.loads(pickle.dumps(result)).total == pytest.approx(  # noqa: S301
        result.total
    )
    assert "FrtbSbaResult(" in repr(result)

    frame = result.to_dataframe()
    assert list(frame.columns) == ["total", "drc", "rrao", "binding_scenario"]
    assert len(frame) == 1
    assert str(frame["total"].dtype) == "float64"

    breakdown = result.to_breakdown_dataframe()
    assert list(breakdown.columns) == ["component", "risk_class", "charge"]
    assert set(breakdown["component"]) <= {"delta", "vega", "curvature"}
    assert str(breakdown["charge"].dtype) == "float64"


def test_frtb_scenario_selection_still_honoured() -> None:
    only_low = frtb_sba_charge(sample_sensitivities(), correlation_scenario="low")

    assert only_low.binding_scenario == "low"
    assert set(only_low.scenario_charges) == {"low"}


def test_saccr_ead_is_a_thin_engine_wrapper() -> None:
    trade = SaCcrTrade.from_json(json.dumps(linear_trade_payload()))
    config = SaCcrNettingSetConfig.margined(
        NettingSetId.cleared("LCH"),
        collateral=250_000.0,
        threshold=100_000.0,
        mta=50_000.0,
        nica=0.0,
        mpor_days=5,
        as_of=dt.date(2025, 1, 15),
    )
    assert config.netting_set_id == NettingSetId.cleared("LCH")
    assert config.netting_set_id.is_cleared
    assert config.as_of == dt.date(2025, 1, 15)
    assert (config.threshold, config.mta, config.nica, config.mpor_days) == (100_000.0, 50_000.0, 0.0, 5)
    assert "mpor_days=5" in repr(config)

    via_function = saccr_ead([trade], config, alpha=1.5)
    via_engine = SaCcrEngine(alpha=1.5).calculate_ead(config, [trade])
    assert via_function.to_json() == via_engine.to_json()
    assert SaCcrEngine(alpha=1.5).alpha == 1.5
    assert via_function.meta["numeric_mode"] is not None

    with pytest.raises(ValueError, match="MPOR"):
        SaCcrNettingSetConfig.margined(NettingSetId.bilateral("A", "B"), 0.0, 0.0, 0.0, 0.0, 0, "2025-01-15")


def test_netting_set_id_is_hashable_and_picklable() -> None:
    lch = NettingSetId.cleared("LCH")
    assert lch == NettingSetId.cleared("LCH")
    assert {lch: 1}[NettingSetId.cleared("LCH")] == 1
    assert pickle.loads(pickle.dumps(lch)) == lch  # noqa: S301
    assert NettingSetId.from_json(lch.to_json()) == lch


def test_frtb_engine_configuration_and_scenario_frame() -> None:
    engine = FrtbSbaEngine(scenarios=["high"], risk_classes=["girr", "equity"])
    assert engine.scenarios == ["high"]
    assert engine.risk_classes == ["girr", "equity"]
    result = engine.calculate(sample_sensitivities())
    frame = result.to_scenario_dataframe()
    assert list(frame.columns) == ["scenario", "charge", "binding"]
    assert frame["scenario"].tolist() == ["high"]
    assert bool(frame["binding"].iloc[0]) is True
    assert result.meta["numeric_mode"] is not None

    with pytest.raises(ValueError, match=r"unknown variant `extreme`"):
        FrtbSbaEngine(scenarios=["extreme"])


def test_frtb_sensitivities_adders_and_dataframe_round_trip() -> None:
    sens = FrtbSensitivities("USD")
    sens.add_girr_delta("5Y", 25_000.0)
    sens.add_girr_inflation_delta(1_000.0)
    sens.add_girr_xccy_basis_delta(500.0, "EUR")
    sens.add_girr_vega("1Y", "5Y", 2_000.0)
    sens.add_girr_curvature(300.0, -200.0)
    sens.add_csr_nonsec_delta("ACME", 3, "5Y", "bond", 4_000.0)
    sens.add_csr_nonsec_vega("ACME", 3, "1Y", 400.0)
    sens.add_csr_nonsec_curvature("ACME", 3, 50.0, -40.0)
    sens.add_csr_sec_ctp_delta("CDX-T", 1, "5Y", "bond", 1_000.0)
    sens.add_csr_sec_ctp_vega("CDX-T", 1, "1Y", 100.0)
    sens.add_csr_sec_ctp_curvature("CDX-T", 1, 10.0, -8.0)
    sens.add_csr_sec_nonctp_delta("ABS-1", 1, "5Y", "bond", 1_000.0)
    sens.add_csr_sec_nonctp_vega("ABS-1", 1, "1Y", 100.0)
    sens.add_csr_sec_nonctp_curvature("ABS-1", 1, 10.0, -8.0)
    sens.add_equity_delta("ACME", 1, 12_000.0)
    sens.add_equity_vega("ACME", 1, "1Y", 600.0)
    sens.add_equity_curvature("ACME", 1, 70.0, -60.0)
    sens.add_commodity_delta("WTI", 2, "1Y", "location", 3_000.0)
    sens.add_commodity_vega("WTI", 2, "1Y", 300.0)
    sens.add_commodity_curvature("WTI", 2, 30.0, -20.0)
    sens.add_fx_delta("EUR", "USD", 9_000.0)
    sens.add_fx_vega("EUR", "USD", "1Y", 900.0)
    sens.add_fx_curvature("EUR", "USD", 90.0, -80.0)
    sens.add_rrao_position("EXOTIC-1", 5_000_000.0, True)
    sens.add_rrao_position("PLAIN-1", 5_000_000.0)
    sens.validate()

    restored = FrtbSensitivities.from_dataframe(sens.to_dataframe(), "USD")
    assert restored.to_json() == sens.to_json()
    assert "delta=9" in repr(sens)

    sens.add_drc_position("ACME", 1_000_000.0, 3, "corporate", "senior_unsecured", "corporate", 1.0)
    charged = frtb_sba_charge(sens)
    assert charged.drc > 0.0
    with pytest.raises(ValueError, match="add_drc_position"):
        FrtbSensitivities.from_dataframe(sens.to_dataframe())

    empty = frtb_sba_charge(FrtbSensitivities("USD"))
    assert repr(empty.rrao) == "0.0"


def test_frtb_basis_repo_and_option_categories_round_trip() -> None:
    sens = FrtbSensitivities("USD")
    sens.add_csr_nonsec_delta("ACME", 3, "5Y", "bond", 100.0)
    sens.add_csr_nonsec_delta("ACME", 3, "5Y", "cds", -100.0)
    sens.add_commodity_delta("WTI", 2, "1Y", "Cushing", 100.0)
    sens.add_commodity_delta("WTI", 2, "1Y", "Houston", -100.0)
    sens.add_equity_repo_delta("ACME", 3, 1_000.0)
    assert FrtbSensitivities.from_dataframe(sens.to_dataframe()).to_json() == sens.to_json()
    assert frtb_sba_charge(sens).total > 0
    option = SaCcrTrade(
        "OPT",
        "equity",
        1e6,
        "2025-01-15",
        "2030-01-15",
        "ACME",
        "EQ",
        1.0,
        0.6,
        0.0,
        is_option=True,
        option_type="call_long",
        supervisory_category="equity_single_name",
        option_maturity_date="2026-01-15",
    )
    assert SaCcrTrade.from_dataframe(option.to_dataframe())[0].to_json() == option.to_json()
    assert option.option_maturity_date == dt.date(2026, 1, 15)
    assert option.supervisory_category == "equity_single_name"
