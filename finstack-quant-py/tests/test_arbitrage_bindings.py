"""Behavioral contracts for volatility-surface arbitrage bindings."""

from finstack_quant.core.market_data.arbitrage import check_surface_grid


def test_surface_report_keys_match_stub_contract() -> None:
    report = check_surface_grid(
        strikes=[90.0, 100.0, 110.0],
        expiries=[0.5, 1.0],
        vols=[[0.20, 0.19, 0.20], [0.21, 0.20, 0.21]],
        forward=100.0,
    )

    assert set(report) == {
        "total_violations",
        "passed",
        "by_severity",
        "by_type",
        "violations",
        "elapsed_us",
    }
    assert set(report["by_severity"]) == {"negligible", "minor", "major", "critical"}
    assert set(report["by_type"]) == {"butterfly", "calendar_spread", "local_vol_density"}

    expected_violation_keys = {
        "type",
        "severity",
        "strike",
        "expiry",
        "adjacent_expiry",
        "magnitude",
        "value",
        "message",
        "description",
    }
    assert all(set(violation) == expected_violation_keys for violation in report["violations"])
