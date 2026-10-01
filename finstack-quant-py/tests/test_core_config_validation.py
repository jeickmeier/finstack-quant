"""Validation parity for core configuration bindings."""

from decimal import Decimal
import json
import math

import pytest

from finstack_quant.core.config import FinstackConfig, ToleranceConfig
from finstack_quant.core.money import Money


@pytest.mark.parametrize("scale", [29, 2**31, 2**32 - 1])
@pytest.mark.parametrize("kind", ["ingest", "output"])
def test_config_scale_rejects_invalid_setter_and_json_values(scale: int, kind: str) -> None:
    config = FinstackConfig()
    setter = getattr(config, f"set_{kind}_scale")
    setter("USD", 4)
    original = config.to_json()
    with pytest.raises(ValueError, match=r"0\.\.=28"):
        setter("USD", scale)
    assert config.to_json() == original
    with pytest.raises(ValueError, match=r"0\.\.=28"):
        setter("EUR", scale)
    assert config.to_json() == original

    payload = json.loads(original)
    payload["rounding"][f"{kind}_scale"]["overrides"]["USD"] = scale
    with pytest.raises(ValueError, match=r"0\.\.=28"):
        FinstackConfig.from_json(json.dumps(payload))


def test_config_maximum_scale_preserves_small_nonzero_money() -> None:
    config = FinstackConfig()
    config.set_ingest_scale("USD", 28)
    config.set_output_scale("USD", 28)
    restored = FinstackConfig.from_json(config.to_json())
    assert restored.ingest_scale("USD") == 28
    assert restored.output_scale("USD") == 28
    money = Money(Decimal("1e-28"), "USD", config=restored)
    assert money.amount_decimal == Decimal("1e-28")


@pytest.mark.parametrize("bad", [0.0, -1.0, math.nan, math.inf])
def test_tolerance_config_rejects_nonpositive_and_nonfinite_values(bad: float) -> None:
    with pytest.raises(ValueError, match="finite and positive"):
        ToleranceConfig(rate_epsilon=bad)
    with pytest.raises(ValueError, match="finite and positive"):
        ToleranceConfig(generic_epsilon=bad)


def test_config_extension_insertion_validates_key() -> None:
    config = FinstackConfig()
    config.set_extension("calibration.config.v1", {"enabled": True})
    with pytest.raises(ValueError, match="invalid config extension key"):
        config.set_extension("not namespaced", {"enabled": True})
    assert "not namespaced" not in config.extension_keys()
