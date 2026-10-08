"""Intermediates behind margin results replay to the reported numbers.

Covers the audit detail exported on ``VmResult`` (threshold, MTA, rounding and
collateral steps), ``ImResult.simm_detail``, the per-bucket ``XvaResult`` rows
and ``FrtbSbaResult.scenario_breakdown``.
"""

from __future__ import annotations

import datetime as dt
import json
import math

import pytest

from finstack_quant.core.market_data.curves import DiscountCurve, HazardCurve
from finstack_quant.margin import (
    CsaSpec,
    ExposureProfile,
    FrtbSensitivities,
    FundingConfig,
    ScheduleImCalculator,
    SimmCalculator,
    SimmSensitivities,
    VmCalculator,
    compute_bilateral_xva,
    frtb_sba_charge,
)


def _csa() -> CsaSpec:
    spec = json.loads(CsaSpec.usd_regulatory().to_json())
    spec["id"] = "USD-AUDIT-CSA"
    spec["vm_params"]["threshold"]["amount"] = "300000"
    spec["vm_params"]["independent_amount"]["amount"] = "100000"
    spec["vm_params"]["mta"]["amount"] = "50000"
    spec["vm_params"]["rounding"]["amount"] = "10000"
    return CsaSpec.from_json(json.dumps(spec))


@pytest.mark.parametrize(
    ("exposure", "collateral", "expected_unrounded", "expected_call"),
    [
        # 1,004,567 - 300,000 + 100,000 = 804,567 required; delivery rounds up.
        (1_004_567.0, 250_000.0, 554_567.0, 560_000.0),
        # Same requirement against 1,200,000 held; the return rounds down.
        (1_004_567.0, 1_200_000.0, -395_433.0, -390_000.0),
        # 20,000 short of the requirement is below the 50,000 MTA.
        (1_000_000.0, 780_000.0, 20_000.0, 0.0),
    ],
)
def test_vm_result_exposes_every_step_of_the_call(
    exposure: float, collateral: float, expected_unrounded: float, expected_call: float
) -> None:
    result = VmCalculator(_csa()).calculate(exposure, collateral, "USD", "2025-06-30")

    assert result.threshold == pytest.approx(300_000.0)
    assert result.independent_amount == pytest.approx(100_000.0)
    assert result.mta == pytest.approx(50_000.0)
    assert result.rounding_increment == pytest.approx(10_000.0)
    assert result.collateral_balance == pytest.approx(collateral)

    excess = math.copysign(max(abs(result.gross_exposure) - result.threshold, 0.0), result.gross_exposure)
    assert result.net_exposure == pytest.approx(excess + result.independent_amount)
    assert result.unrounded_call == pytest.approx(result.net_exposure - result.collateral_balance)
    assert result.unrounded_call == pytest.approx(expected_unrounded)
    assert result.collect_amount - result.post_amount == pytest.approx(expected_call)

    row = result.to_dataframe().iloc[0]
    for column in ("threshold", "independent_amount", "collateral_balance", "unrounded_call", "mta"):
        assert row[column] == pytest.approx(getattr(result, column))
    assert row["rounding_increment"] == pytest.approx(result.rounding_increment)


def _simm_result():
    sens = SimmSensitivities("USD")
    sens.add_ir_delta("USD", "5Y", 100_000.0)
    sens.add_ir_delta("USD", "10Y", -60_000.0)
    sens.add_ir_delta("EUR", "5Y", 40_000.0)
    sens.add_equity_delta("AAPL", 250_000.0)
    sens.add_fx_delta("EUR", 75_000.0)
    return SimmCalculator("v2_6").calculate_from_sensitivities(sens, "USD", "2025-06-30")


def test_simm_detail_rebuilds_the_breakdown() -> None:
    result = _simm_result()
    detail = result.simm_detail

    assert detail is not None
    components = {c["component"]: c for c in detail["components"]}
    assert sorted(components) == result.breakdown_keys()
    for label, component in components.items():
        assert component["margin"] * detail["mpor_scale"] == pytest.approx(result.breakdown_amount(label))
        for bucket in component["buckets"]:
            total = 0.0
            for row in bucket["weighted_sensitivities"]:
                assert row["weighted_sensitivity"] == pytest.approx(
                    row["sensitivity"]
                    * component["historical_volatility_ratio"]
                    * row["risk_weight"]
                    * row["concentration_factor"]
                )
                total += row["weighted_sensitivity"]
            assert bucket["signed_sum"] == pytest.approx(min(max(total, -bucket["k"]), bucket["k"]))

    # Two currencies, with both USD tenors in the USD bucket.
    ir = components["IR_Delta"]
    assert sorted(b["bucket"] for b in ir["buckets"]) == ["EUR", "USD"]
    usd = next(b for b in ir["buckets"] if b["bucket"] == "USD")
    assert sorted(row["tenor"] for row in usd["weighted_sensitivities"]) == ["10Y", "5Y"]

    # Risk-class margins are the component sums.
    by_class: dict[str, float] = {}
    for component in components.values():
        by_class[component["risk_class"]] = by_class.get(component["risk_class"], 0.0) + component["margin"]
    assert detail["risk_class_margins"] == pytest.approx(by_class)


def test_simm_detail_is_absent_for_schedule_im() -> None:
    schedule = ScheduleImCalculator.bcbs_standard().calculate_for_notional(
        1_000_000, "USD", "interest_rate", 5.0, "2025-01-15"
    )
    assert schedule.simm_detail is None
    assert "simm_detail" not in json.loads(schedule.to_json())


def test_xva_rows_sum_to_each_reported_leg() -> None:
    base = dt.date(2025, 1, 1)
    discount = DiscountCurve(
        "USD-OIS", base, [(0.5 * i, math.exp(-0.04 * 0.5 * i)) for i in range(9)], interp="log_linear"
    )
    counterparty = HazardCurve("CPTY", base, [(0.0, 0.02), (30.0, 0.02)], recovery_rate=0.40)
    own = HazardCurve("OWN", base, [(0.0, 0.03), (30.0, 0.03)], recovery_rate=0.40)
    times = [0.5, 1.0, 2.0, 3.0]
    profile = ExposureProfile(times, [5e5, 4e5, 3e5, 2e5], [8e5, 7e5, 6e5, 5e5], [3e5, 3e5, 3e5, 3e5])

    result = compute_bilateral_xva(profile, counterparty, own, discount, 0.40, 0.35, FundingConfig(50.0, 30.0))

    for rows, reported, lgd in (
        (result.cva_rows, result.cva, 0.60),
        (result.dva_rows, result.dva, 0.65),
    ):
        assert [row["time"] for row in rows] == times
        for row in rows:
            assert row["loss_given_default"] == pytest.approx(lgd)
            assert row["contribution"] == pytest.approx(
                row["loss_given_default"]
                * row["exposure_mid"]
                * row["marginal_default_probability"]
                * row["discount_factor_mid"]
                * row["survival_weight"]
            )
        assert sum(row["contribution"] for row in rows) == pytest.approx(reported, rel=1e-12)
        assert reported > 0.0

    # Survival probabilities fall along the grid and drive the marginal PDs.
    survival = [row["survival_probability"] for row in result.cva_rows]
    assert survival == sorted(survival, reverse=True)
    assert result.cva_rows[0]["marginal_default_probability"] == pytest.approx(1.0 - survival[0])

    fva_rows = result.fva_rows
    assert [row["time"] for row in fva_rows] == times
    for row in fva_rows:
        assert row["funding_spread"] == pytest.approx(0.005)
        assert row["funding_benefit_spread"] == pytest.approx(0.003)
        assert row["contribution"] == pytest.approx(
            (row["epe_mid"] * row["funding_spread"] - row["ene_mid"] * row["funding_benefit_spread"])
            * row["discount_factor_mid"]
            * row["dt"]
            * row["joint_survival_mid"]
        )
    assert sum(row["contribution"] for row in fva_rows) == pytest.approx(result.fva, rel=1e-12)


def test_frtb_scenario_breakdown_covers_every_scenario() -> None:
    sens = FrtbSensitivities("USD")
    sens.add_girr_delta("USD", "5Y", 25_000.0)
    sens.add_girr_delta("USD", "10Y", -15_000.0)
    sens.add_equity_delta("ACME", 1, 12_000.0)

    result = frtb_sba_charge(sens)
    breakdown = result.scenario_breakdown

    assert sorted(breakdown) == sorted(result.scenario_charges) == ["high", "low", "medium"]
    for scenario, charge in result.scenario_charges.items():
        rows = breakdown[scenario]
        replayed = sum(
            sum(rows[component].values())
            for component in ("delta_by_risk_class", "vega_by_risk_class", "curvature_by_risk_class")
        )
        assert replayed == pytest.approx(charge, rel=1e-12)
    binding = breakdown[result.binding_scenario]
    assert binding["delta_by_risk_class"] == pytest.approx(result.delta_by_risk_class)
    # Offsetting tenors make the GIRR charge depend on the correlation scenario.
    assert breakdown["low"]["delta_by_risk_class"]["girr"] != breakdown["high"]["delta_by_risk_class"]["girr"]
