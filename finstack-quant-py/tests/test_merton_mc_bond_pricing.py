"""Downstream valuation coverage for Merton Monte Carlo bond pricing."""

from __future__ import annotations

import datetime
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import StubKind
from finstack_quant.core.money import Money
from finstack_quant.core.types import Rate
from finstack_quant.models.credit import MertonModel
from finstack_quant.valuations.instruments import (
    BarrierCrossing,
    Bond,
    MertonMcConfig,
    PikMode,
    PikSchedule,
)


def test_merton_mc_config_requires_explicit_recovery() -> None:
    merton = MertonModel(100.0, 0.25, 60.0, 0.04)
    with pytest.raises(TypeError):
        MertonMcConfig(merton)  # type: ignore[call-arg]


@pytest.mark.parametrize("recovery", [0.0, 1.0])
def test_merton_mc_config_accepts_recovery_boundaries(recovery: float) -> None:
    merton = MertonModel(100.0, 0.25, 60.0, 0.04)
    MertonMcConfig(merton, recovery)


def test_merton_mc_bond_price_smoke() -> None:
    merton = MertonModel(100.0, 0.25, 60.0, 0.04)
    config = (
        MertonMcConfig(merton, 0.40)
        .pik_schedule(PikSchedule.uniform(PikMode.pik()))
        .barrier_crossing(BarrierCrossing.discrete())
    )
    bond = Bond.fixed(
        "PIK-1",
        Money(100.0, Currency("USD")),
        Rate(0.08),
        datetime.date(2024, 1, 15),
        datetime.date(2029, 1, 15),
        StubKind.SHORT_FRONT,
        "USD-OIS",
    )
    # Path count lives on the bond's model_config, not on MertonMcConfig.
    envelope = json.loads(bond.to_json())
    spec = envelope["instrument"]["spec"]
    spec.setdefault("instrument_pricing_overrides", {}).setdefault("model_config", {})["mc_paths"] = 64
    bond = Bond.from_json(json.dumps(envelope))
    result = bond.price_merton_mc(config, 0.04, datetime.date(2024, 1, 15))
    assert result.num_paths == 64
    assert result.clean_price_pct > 0.0
    assert 0.0 <= result.path_statistics.default_rate <= 1.0


def _merton_bond_with_config(config: MertonMcConfig, as_of: datetime.date) -> Bond:
    """A 5-year 8% bond carrying ``config`` as its ``merton_mc`` model configuration."""
    bond = Bond.fixed(
        "PIK-1",
        Money(100.0, Currency("USD")),
        Rate(0.08),
        as_of,
        datetime.date(2030, 1, 15),
        StubKind.SHORT_FRONT,
        "USD-OIS",
    )
    envelope = json.loads(bond.to_json())
    model_config = (
        envelope["instrument"]["spec"].setdefault("instrument_pricing_overrides", {}).setdefault("model_config", {})
    )
    model_config["mc_paths"] = 2000
    model_config["merton_mc_config"] = config.to_dict()
    return Bond.from_json(json.dumps(envelope))


def test_merton_mc_calibration_spec_is_typed_and_round_trips() -> None:
    """VALB-004: the calibration spec is a typed class with Rust defaults."""
    from finstack_quant.valuations.instruments import MertonMcCalibrationSpec

    spec = MertonMcCalibrationSpec({"z_spread": 0.03}, "asset_vol", low_paths=500, tolerance_pv=0.05)
    assert spec.target == {"z_spread": 0.03}
    assert spec.parameter == "asset_vol"
    assert (spec.low_paths, spec.max_iterations, spec.tolerance_pv) == (500, 40, 0.05)
    assert spec.bracket is None
    assert spec.seed is None
    assert MertonMcCalibrationSpec.from_json(spec.to_json()).to_dict() == spec.to_dict()
    with pytest.raises(ValueError, match="calibration parameter"):
        MertonMcCalibrationSpec({"z_spread": 0.03}, "leverage")


def test_merton_mc_config_typed_calibration_drives_registry_pricer() -> None:
    """VALB-004: ``MertonMcConfig.calibration`` replaces bond-JSON surgery."""
    from finstack_quant.core.market_data import DiscountCurve, MarketContext
    from finstack_quant.valuations.instruments import MertonMcCalibrationSpec

    as_of = datetime.date(2025, 1, 15)
    market = MarketContext()
    market.insert(
        DiscountCurve(
            "USD-OIS",
            as_of,
            [(0.0, 1.0), (1.0, 0.96), (2.0, 0.92), (5.0, 0.80), (10.0, 0.65)],
            day_count="act_365f",
        )
    )
    base = MertonMcConfig(MertonModel(100.0, 0.25, 60.0, 0.04), 0.40)
    spec = MertonMcCalibrationSpec({"z_spread": 0.03}, "asset_vol", low_paths=500, tolerance_pv=0.05)
    calibrated = base.calibration(spec)
    assert calibrated.to_dict()["calibration"] == spec.to_dict()
    assert base.to_dict()["calibration"] is None
    assert "calibration=true" in repr(calibrated).lower()

    plain = _merton_bond_with_config(base, as_of).price(market, as_of, model="merton_mc")
    solved = _merton_bond_with_config(calibrated, as_of).price(market, as_of, model="merton_mc")
    measures = json.loads(solved.to_json())["measures"]
    assert measures["calibration_iterations"] > 0
    assert measures["calibrated_asset_vol"] != pytest.approx(0.25)
    assert solved.value.amount != pytest.approx(plain.value.amount)


def test_merton_mc_config_typed_cashflow_dfs_moves_price() -> None:
    """VALB-004: ``MertonMcConfig.cashflow_dfs`` reaches the engine's discounting."""
    as_of = datetime.date(2025, 1, 15)
    base = MertonMcConfig(MertonModel(100.0, 0.25, 60.0, 0.04), 0.40)
    steep = base.cashflow_dfs([(0.0, 1.0), (1.0, 0.90), (5.0, 0.60)])
    assert steep.to_dict()["cashflow_dfs"] == [[0.0, 1.0], [1.0, 0.90], [5.0, 0.60]]
    bond = _merton_bond_with_config(base, as_of)
    flat_price = bond.price_merton_mc(base, 0.04, as_of).clean_price_pct
    steep_price = bond.price_merton_mc(steep, 0.04, as_of).clean_price_pct
    assert steep_price < flat_price - 1.0
