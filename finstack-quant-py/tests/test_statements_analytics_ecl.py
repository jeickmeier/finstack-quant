"""Statements analytics ECL binding tests."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from finstack_quant.statements_analytics import Exposure


def _exposure(**kwargs: object) -> Exposure:
    from finstack_quant.statements_analytics import Exposure

    defaults: dict[str, object] = {
        "id": "loan",
        "ead": 1_000_000.0,
        "lgd": 0.45,
        "eir": 0.06,
        "remaining_maturity": 1.0,
        "current_pd": 0.02,
        "origination_pd": 0.015,
    }
    defaults.update(kwargs)
    return Exposure(**defaults)  # type: ignore[arg-type]


def test_classify_stage_uses_canonical_defaults_and_backstop_toggles() -> None:
    """Exposure and classification defaults are resolved by canonical Rust."""
    from finstack_quant.statements_analytics import Stage, StagingConfig, classify_stage

    exposure = _exposure(ead=100.0, lgd=0.4, eir=0.05, remaining_maturity=5.0)
    assert exposure.dpd == 0
    assert exposure.undrawn == 0.0
    assert exposure.ccf == 0.75

    clean = classify_stage(exposure)
    assert clean.stage is Stage.Stage1
    assert clean.stage.value == "stage1"
    assert clean.triggers == ["no_trigger"]

    exposure.dpd = 30
    assert classify_stage(exposure).stage is Stage.Stage2

    no_dpd = StagingConfig(dpd_stage2_threshold=2**32 - 1, dpd_stage3_threshold=2**32 - 1)
    assert classify_stage(exposure, no_dpd).stage is Stage.Stage1

    exposure.dpd = 91
    assert classify_stage(exposure).stage is Stage.Stage3
    assert classify_stage(exposure, no_dpd).stage is Stage.Stage1


def test_classify_stage_uses_default_or_explicit_pd_threshold() -> None:
    """The SICR PD-delta threshold comes from ``StagingConfig``, not the binding."""
    from finstack_quant.statements_analytics import Stage, StagingConfig, classify_stage

    exposure = _exposure(ead=100.0, lgd=0.4, eir=0.05, remaining_maturity=5.0, current_pd=0.026)
    assert classify_stage(exposure).stage is Stage.Stage2

    relaxed = StagingConfig(pd_delta_absolute=0.02)
    assert classify_stage(exposure, relaxed).stage is Stage.Stage1


def test_classify_stage_result_round_trips_through_json() -> None:
    """``StageResult`` is a serde-backed typed wrapper."""
    from finstack_quant.statements_analytics import StageResult, classify_stage

    result = classify_stage(_exposure())
    assert StageResult.from_json(result.to_json()).stage is result.stage
    assert result.cured is False


def test_classify_then_measure_workflow() -> None:
    """The classify-then-measure workflow composes without a string round-trip."""
    from finstack_quant.statements_analytics import classify_stage, compute_ecl

    exposure = _exposure(remaining_maturity=5.0)
    stage = classify_stage(exposure).stage
    result = compute_ecl(exposure, [(0.0, 0.0), (5.0, 0.10)], stage=stage)
    assert result.stage is stage
    assert result.ecl > 0.0
    # The serde name is accepted interchangeably with the enum.
    assert compute_ecl(exposure, [(0.0, 0.0), (5.0, 0.10)], stage=stage.value).ecl == pytest.approx(result.ecl)


def test_compute_ecl_weighted_validates_scenario_weights() -> None:
    """Weighted ECL rejects malformed scenario probabilities from Rust."""
    from finstack_quant.statements_analytics import compute_ecl_weighted

    scenarios = [
        (0.75, [(0.0, 0.0), (1.0, 0.02)]),
        (0.20, [(0.0, 0.0), (1.0, 0.05)]),
    ]

    with pytest.raises(ValueError, match=r"[Ss]cenario weights must sum to 1\.0"):
        compute_ecl_weighted(_exposure(), scenarios)


def test_compute_ecl_weighted_preserves_public_error_mapping() -> None:
    """Missing scenarios and PD schedules remain public ValueError failures."""
    from finstack_quant.statements_analytics import compute_ecl_weighted

    with pytest.raises(ValueError, match="At least one scenario is required for weighted ECL"):
        compute_ecl_weighted(_exposure(), [])

    with pytest.raises(ValueError, match="At least two data points are required"):
        compute_ecl_weighted(_exposure(), [(1.0, [])])


def test_compute_ecl_weighted_returns_probability_weighted_ecl() -> None:
    """Weighted ECL binding delegates the scenario aggregation to Rust."""
    from finstack_quant.statements_analytics import compute_ecl, compute_ecl_weighted

    base_curve = [(0.0, 0.0), (1.0, 0.02)]
    downside_curve = [(0.0, 0.0), (1.0, 0.05)]
    exposure = _exposure()

    base = compute_ecl(exposure, base_curve).ecl
    downside = compute_ecl(exposure, downside_curve).ecl
    weighted = compute_ecl_weighted(exposure, [(0.70, base_curve), (0.30, downside_curve)])

    assert weighted.ecl == pytest.approx(0.70 * base + 0.30 * downside, rel=1e-12)
    assert len(weighted.scenario_breakdown) == 2
    assert [w for _, w, _ in weighted.scenario_breakdown] == pytest.approx([0.70, 0.30])


def test_weighted_result_exposes_bucket_audit_trail() -> None:
    """``WeightedEclResult`` carries the per-scenario bucket table."""
    from finstack_quant.statements_analytics import WeightedEclResult, compute_ecl_weighted

    result = compute_ecl_weighted(
        _exposure(remaining_maturity=2.0),
        [(1.0, [(0.0, 0.0), (2.0, 0.04)])],
        stage="stage2",
    )
    frame = result.to_dataframe()
    assert list(frame.columns)[:2] == ["scenario", "weight"]
    assert not frame.empty
    assert frame["ecl"].sum() == pytest.approx(result.ecl)

    scenario_result = result.scenario_breakdown[0][2]
    assert scenario_result.buckets
    assert scenario_result.buckets[0].ead > 0.0
    assert WeightedEclResult.from_json(result.to_json()).ecl == pytest.approx(result.ecl)


def test_compute_ecl_prices_undrawn_commitment() -> None:
    """EAD is derived in Rust as ``ead + undrawn * ccf``."""
    from finstack_quant.statements_analytics import compute_ecl

    curve = [(0.0, 0.0), (1.0, 0.02)]
    drawn = compute_ecl(_exposure(), curve).ecl
    revolver = compute_ecl(_exposure(undrawn=1_000_000.0, ccf=0.5), curve).ecl
    assert revolver == pytest.approx(1.5 * drawn, rel=1e-9)


def test_compute_ecl_weighted_anchors_unanchored_schedules() -> None:
    """Both ECL entry points accept schedules without an explicit (0, 0) knot."""
    from finstack_quant.statements_analytics import compute_ecl, compute_ecl_weighted

    anchored = [(0.0, 0.0), (1.0, 0.02)]
    unanchored = [(1.0, 0.02)]
    exposure = _exposure()

    assert compute_ecl(exposure, unanchored).ecl == pytest.approx(compute_ecl(exposure, anchored).ecl)
    assert compute_ecl_weighted(exposure, [(1.0, unanchored)]).ecl == pytest.approx(
        compute_ecl_weighted(exposure, [(1.0, anchored)]).ecl
    )


def test_compute_ecl_ead_schedule_reduces_lifetime_ecl() -> None:
    """An amortizing EAD profile lowers ECL versus a constant balance."""
    from finstack_quant.statements_analytics import compute_ecl

    curve = [(0.0, 0.0), (5.0, 0.10)]
    bullet = compute_ecl(_exposure(remaining_maturity=5.0), curve, stage="stage2").ecl
    amortizing = compute_ecl(
        _exposure(
            remaining_maturity=5.0,
            ead_schedule=[(0.0, 1_000_000.0), (5.0, 0.0)],
        ),
        curve,
        stage="stage2",
    ).ecl
    assert amortizing < bullet


def test_compute_ecl_stage3_time_to_recovery() -> None:
    """Stage 3 allowance is carrying EAD less discounted recovery."""
    from finstack_quant.statements_analytics import compute_ecl

    curve = [(0.0, 0.0), (1.0, 0.02)]
    exposure = _exposure()
    fast = compute_ecl(exposure, curve, stage="stage3", stage3_time_to_recovery_years=0.5).ecl
    slow = compute_ecl(exposure, curve, stage="stage3", stage3_time_to_recovery_years=3.0).ecl
    # Longer time to recovery reduces recovery PV and increases impairment.
    assert slow > fast
    assert fast >= 0.45 * 1_000_000.0


def test_same_rating_pd_deterioration_reaches_stage_two() -> None:
    from finstack_quant.statements_analytics import Stage, classify_stage

    exposure = _exposure(current_pd=0.20, origination_pd=0.01, current_rating="BBB", origination_rating="BBB")
    assert classify_stage(exposure).stage == Stage.Stage2


def test_stage_three_full_loss_and_invalid_bucket_inputs() -> None:
    from finstack_quant.statements_analytics import Stage, compute_ecl

    exposure = _exposure(ead=100.0, lgd=1.0, eir=0.1)
    result = compute_ecl(exposure, [(1.0, 0.1)], stage=Stage.Stage3)
    assert result.ecl == pytest.approx(100.0)
    for invalid in [float("nan"), float("inf")]:
        with pytest.raises(ValueError, match="bucket_width_years"):
            compute_ecl(exposure, [(1.0, 0.1)], stage=Stage.Stage2, bucket_width_years=invalid)
        with pytest.raises(ValueError, match=r"(?i)knot"):
            compute_ecl(exposure, [(0.0, 0.0), (invalid, 0.1)], stage=Stage.Stage2)
