"""Host regressions for structural credit, recovery, and retained MC workloads."""

from __future__ import annotations

import pytest

from finstack_quant.models.correlation import RecoverySpec
from finstack_quant.models.credit import AssetDynamics, MertonBarrierType, MertonModel
from finstack_quant.models.monte_carlo import LsmcPricer, simulate_gbm_paths


def test_high_intensity_zero_size_jumps_preserve_gbm_default_probability() -> None:
    diffusion = MertonModel(100.0, 0.20, 80.0, 0.05)
    jumps = MertonModel.new_with_dynamics(
        100.0,
        0.20,
        80.0,
        0.05,
        0.0,
        MertonBarrierType.terminal(),
        AssetDynamics.jump_diffusion(746.0, 0.0, 0.0),
    )

    expected = diffusion.default_probability(1.0)
    assert expected > 0.01
    assert jumps.default_probability(1.0) == pytest.approx(expected, abs=1e-11)


@pytest.mark.parametrize("mean", [0.01, 0.99])
def test_zero_volatility_recovery_preserves_extreme_location(mean: float) -> None:
    spec = RecoverySpec.market_correlated(mean, 0.0, 0.0)
    recovery = spec.build()

    assert spec.expected_recovery == pytest.approx(mean, abs=1e-14)
    assert recovery.expected_recovery == pytest.approx(mean, abs=1e-14)
    for factor in [-6.0, 0.0, 6.0]:
        assert recovery.conditional_recovery(factor) == pytest.approx(mean, abs=1e-14)
        assert recovery.conditional_lgd(factor) == pytest.approx(1.0 - mean, abs=1e-14)


@pytest.mark.parametrize(("mean", "expected"), [(0.01, 0.3271433591834951), (0.99, 0.6728566408165049)])
def test_recovery_expectation_resolves_steep_near_boundary_curve(mean: float, expected: float) -> None:
    recovery = RecoverySpec.market_correlated(mean, 0.25, 0.4).build()
    assert recovery.expected_recovery == pytest.approx(expected, abs=1e-10)


@pytest.mark.parametrize(
    ("num_paths", "num_steps"),
    [(10_000_001, 1), (1, 100_001)],
)
def test_lsmc_constructor_rejects_oversized_workload(num_paths: int, num_steps: int) -> None:
    with pytest.raises(ValueError, match="LSMC"):
        LsmcPricer(num_paths=num_paths, num_steps=num_steps)


def test_compact_gbm_rejects_oversized_output_before_grid_allocation() -> None:
    with pytest.raises(ValueError, match="exceed"):
        simulate_gbm_paths(100.0, 0.05, 0.0, 0.20, 1.0, 64_000_000, 1)
