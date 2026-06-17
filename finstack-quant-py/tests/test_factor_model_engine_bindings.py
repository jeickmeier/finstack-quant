"""Behavioral tests for portfolio factor-model engine binding helpers."""

from __future__ import annotations

import json

import pytest

from finstack_quant.factor_model.credit import CreditFactorModel
from finstack_quant.portfolio import (
    CreditVolReport,
    RiskDecomposition,
    StressAttribution,
    build_credit_vol_report,
    build_stress_attribution,
)


def test_build_stress_attribution_from_position_pnls() -> None:
    # Python input shape is positions x scenarios; binding transposes to the
    # Rust row-major scenarios x positions engine layout.
    position_ids = ["A", "B"]
    pnls_a = [-8.0, -2.0] + [0.5] * 38
    pnls_b = [-2.0, -4.0] + [0.5] * 38

    attr = build_stress_attribution(position_ids, [pnls_a, pnls_b], confidence=0.95)

    assert isinstance(attr, StressAttribution)
    assert attr.n_tail_scenarios == 2
    assert attr.var_threshold == pytest.approx(6.0)
    assert [scenario.scenario_index for scenario in attr.tail_scenarios] == [0, 1]

    by_id = {entry.position_id: entry for entry in attr.position_contributions}
    assert by_id["A"].avg_tail_pnl == pytest.approx(-5.0)
    assert by_id["B"].avg_tail_pnl == pytest.approx(-3.0)
    assert by_id["A"].pct_of_tail_loss == pytest.approx(0.625)
    assert by_id["B"].pct_of_tail_loss == pytest.approx(0.375)


def test_build_stress_attribution_rejects_bad_shape() -> None:
    with pytest.raises(ValueError, match="position_pnls row"):
        build_stress_attribution(["A", "B"], [[1.0, 2.0], [1.0]], confidence=0.95)


def test_build_credit_vol_report_from_typed_inputs() -> None:
    model = CreditFactorModel.from_json(
        json.dumps({
            "schema_version": "finstack_quant.credit_factor_model/1",
            "as_of": "2024-03-29",
            "calibration_window": {"start": "2022-03-29", "end": "2024-03-29"},
            "policy": "globally_off",
            "generic_factor": {"name": "CDX IG", "series_id": "cdx.ig.5y"},
            "hierarchy": {"levels": ["rating", "region"]},
            "config": {
                "factors": [],
                "covariance": {"n": 0, "factor_ids": [], "data": []},
                "matching": {"MappingTable": []},
                "pricing_mode": "delta_based",
            },
            "issuer_betas": [],
            "anchor_state": {"pc": 0.0, "by_level": []},
            "static_correlation": {"factor_ids": [], "data": []},
            "vol_state": {"factors": {}, "idiosyncratic": {}},
            "factor_histories": None,
            "diagnostics": {
                "mode_counts": {},
                "bucket_sizes_per_level": [],
                "fold_ups": [],
                "r_squared_histogram": None,
                "tag_taxonomy": {},
            },
        })
    )
    decomposition = RiskDecomposition.from_json(
        json.dumps({
            "total_risk": 1.0,
            "measure": "variance",
            "factor_contributions": [
                {
                    "factor_id": "credit::generic",
                    "absolute_risk": 0.10,
                    "relative_risk": 0.10,
                    "marginal_risk": 0.0,
                },
                {
                    "factor_id": "credit::level0::rating::IG",
                    "absolute_risk": 0.20,
                    "relative_risk": 0.20,
                    "marginal_risk": 0.0,
                },
                {
                    "factor_id": "credit::level1::rating.region::IG.EU",
                    "absolute_risk": 0.30,
                    "relative_risk": 0.30,
                    "marginal_risk": 0.0,
                },
            ],
            "residual_risk": 0.40,
            "position_factor_contributions": [
                {
                    "position_id": "POS1",
                    "factor_id": "credit::generic",
                    "risk_contribution": 0.10,
                },
                {
                    "position_id": "POS1",
                    "factor_id": "credit::level0::rating::IG",
                    "risk_contribution": 0.20,
                },
            ],
            "position_residual_contributions": [],
        })
    )

    report = build_credit_vol_report(decomposition, model, by_position=True)

    assert isinstance(report, CreditVolReport)
    assert report.total == pytest.approx(1.0)
    assert report.generic == pytest.approx(0.10)
    assert report.idiosyncratic_total == pytest.approx(0.40)
    assert [level.level_name for level in report.by_level] == ["Rating", "Region"]
    assert report.by_level[0].total == pytest.approx(0.20)
    assert report.by_level[0].by_bucket["IG"] == pytest.approx(0.20)
    assert report.by_level[1].total == pytest.approx(0.30)
    assert report.by_position is not None
    assert report.by_position[0].position_id == "POS1"
