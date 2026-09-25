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
