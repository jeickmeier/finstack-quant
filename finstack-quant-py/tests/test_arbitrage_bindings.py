"""Behavioral contracts for volatility-surface arbitrage bindings."""

from finstack_quant.models.volatility import check_surface_grid


def test_surface_report_keys_match_stub_contract() -> None:
    report = check_surface_grid(
        strikes=[90.0, 100.0, 110.0],
        expiries=[0.5, 1.0],
        vols=[[0.20, 0.19, 0.20], [0.21, 0.20, 0.21]],
        forward_prices=[100.0],
    )

    assert report.passed is True
    assert report.total_violations == 0
    assert set(report.by_severity) == {"negligible", "minor", "major", "critical"}
    # Every `ArbitrageType` variant is reported, including the SVI checks.
    assert set(report.by_type) == {
        "butterfly",
        "calendar_spread",
        "local_vol_density",
        "svi_moment_bound",
        "svi_butterfly_condition",
        "svi_calendar_spread",
    }

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
