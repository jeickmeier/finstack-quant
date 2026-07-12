"""Validation parity for core configuration bindings."""

import math

import pytest

from finstack_quant.core.config import FinstackConfig, ToleranceConfig


@pytest.mark.parametrize("bad", [0.0, -1.0, math.nan, math.inf])
def test_tolerance_config_rejects_nonpositive_and_nonfinite_values(bad: float) -> None:
    with pytest.raises(ValueError, match="finite and positive"):
        ToleranceConfig(rate_epsilon=bad)
    with pytest.raises(ValueError, match="finite and positive"):
        ToleranceConfig(generic_epsilon=bad)


def test_config_extension_insertion_validates_key() -> None:
    config = FinstackConfig()
    config.set_extension("valuations.calibration.v2", {"enabled": True})
    with pytest.raises(ValueError, match="invalid config extension key"):
        config.set_extension("not namespaced", {"enabled": True})
    assert "not namespaced" not in config.extension_keys()
