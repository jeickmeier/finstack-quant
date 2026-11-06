"""Behavioral contracts for volatility-surface arbitrage bindings."""

import pytest

from finstack_quant.core.market_data import VolSurface
from finstack_quant.models.volatility import check_surface, check_surface_grid


def test_surface_report_keys_match_stub_contract() -> None:
    report = check_surface_grid(
        strikes=[90.0, 100.0, 110.0],
        expiries=[0.5, 1.0],
        vols=[[0.20, 0.19, 0.20], [0.21, 0.20, 0.21]],
        forward_prices=[100.0],
    )

    assert report.passed is True
    assert report.total_violations == 0
    # The count maps are the sparse Rust `counts_by_*` maps: a clean surface has none.
    assert report.by_severity == {}
    assert report.by_type == {}

    # Violation rows are the serde form of the Rust `ArbitrageViolation`.
    expected_violation_keys = {
        "violation_type",
        "location",
        "severity",
        "magnitude",
        "description",
        "suggested_fix",
    }
    assert all(set(violation) == expected_violation_keys for violation in report.violations)
    assert report.to_dataframe().empty
    assert type(report).from_json(report.to_json()).passed is True


def _surface(vols: list[list[float]], **kwargs: str) -> VolSurface:
    return VolSurface("SPX-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], vols, **kwargs)


def test_check_surface_passes_a_flat_surface_with_a_partial_config() -> None:
    surface = _surface([[0.2, 0.2, 0.2], [0.2, 0.2, 0.2]])

    report = check_surface(surface, {"forward_prices": [100.0]})

    assert report.passed is True
    assert report.total_violations == 0
    assert report.vol_surface_id == "SPX-VOL"
    # JSON text is the same serde shape as the dict.
    assert check_surface(surface, '{"forward_prices": [100.0]}').passed is True


def test_check_surface_reports_a_butterfly_violation() -> None:
    # A spike in the middle strike makes call prices concave in strike.
    surface = _surface([[0.2, 0.6, 0.2], [0.2, 0.6, 0.2]])

    report = check_surface(
        surface,
        {"forward_prices": [100.0], "check_calendar_spread": False, "check_local_vol_density": False},
    )

    assert report.passed is False
    assert report.by_type.get("butterfly", 0) > 0
    assert set(report.by_type) == {"butterfly"}


def test_check_surface_rejects_missing_forwards_normal_quotes_and_unknown_fields() -> None:
    lognormal = _surface([[0.2, 0.2, 0.2], [0.2, 0.2, 0.2]])
    normal = _surface([[0.01, 0.01, 0.01], [0.01, 0.01, 0.01]], quote_type="normal")

    with pytest.raises(ValueError, match="forward prices are required"):
        check_surface(lognormal, {})
    with pytest.raises(ValueError, match="stores 'normal' quotes"):
        check_surface(normal, {"forward_prices": [100.0]})
    with pytest.raises(ValueError, match="unknown field"):
        check_surface(lognormal, {"forward_prices": [100.0], "forwards": [100.0]})
