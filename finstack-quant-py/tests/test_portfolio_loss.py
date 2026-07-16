"""Behavioral tests for Python portfolio credit-loss simulation."""

from __future__ import annotations

import json

import pytest

from finstack_quant.valuations.correlation import (
    MAX_PORTFOLIO_LOSS_PATHS,
    CopulaSpec,
    CreditExposure,
    PortfolioLossConfig,
    PortfolioLossResult,
    RecoverySpec,
    simulate_portfolio_loss,
)


def test_portfolio_loss_binding_is_reproducible_and_loss_positive() -> None:
    exposures = [
        CreditExposure("a", 100.0, 0.05, 0.6, [0.3]),
        CreditExposure("b", 80.0, 0.10, 0.5, [0.2]),
    ]
    config = PortfolioLossConfig(2_000, 42, 0.99, CopulaSpec.gaussian())

    first = simulate_portfolio_loss(exposures, config)
    second = simulate_portfolio_loss(exposures, config)

    assert isinstance(first, PortfolioLossResult)
    assert first.losses == second.losses
    assert first.expected_loss == second.expected_loss
    assert first.expected_shortfall >= first.var >= 0.0
    assert len(first.losses) == config.num_paths
    assert json.loads(first.to_json())["expected_loss"] == first.expected_loss


def test_portfolio_loss_binding_accepts_canonical_recovery_spec() -> None:
    exposures = [CreditExposure("a", 100.0, 0.10, 0.6, [0.3])]
    config = PortfolioLossConfig(1_000, 7, 0.95, CopulaSpec.gaussian())
    recovery = RecoverySpec.constant(0.4)

    direct = simulate_portfolio_loss(exposures, config)
    modeled = simulate_portfolio_loss(exposures, config, recovery)

    assert modeled.losses == direct.losses


def test_portfolio_loss_binding_maps_validation_errors() -> None:
    config = PortfolioLossConfig(100, 1, 0.99, CopulaSpec.gaussian())
    invalid = CreditExposure("bad", 100.0, 0.05, 0.6, [0.9, 0.9])

    with pytest.raises(ValueError, match="factor-loading norm"):
        simulate_portfolio_loss([invalid], config)


def test_portfolio_loss_binding_rejects_paths_above_public_maximum() -> None:
    exposure = CreditExposure("ok", 100.0, 0.05, 0.6, [0.3])
    config = PortfolioLossConfig(
        MAX_PORTFOLIO_LOSS_PATHS + 1,
        1,
        0.99,
        CopulaSpec.gaussian(),
    )

    with pytest.raises(ValueError, match="num_paths must not exceed"):
        simulate_portfolio_loss([exposure], config)


def test_portfolio_loss_binding_rejects_duplicate_trimmed_ids() -> None:
    exposures = [
        CreditExposure("duplicate", 100.0, 0.05, 0.6, [0.3]),
        CreditExposure(" duplicate ", 100.0, 0.05, 0.6, [0.3]),
    ]
    config = PortfolioLossConfig(1, 1, 0.99, CopulaSpec.gaussian())

    with pytest.raises(
        ValueError,
        match="duplicate credit exposure id after trimming: 'duplicate'",
    ):
        simulate_portfolio_loss(exposures, config)
