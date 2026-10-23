"""PD master-scale bindings."""

from __future__ import annotations

import pytest

from finstack_quant.models.credit import pd


def test_library_scales_load_and_are_ordered() -> None:
    scale = pd.MasterScale.default_calibration()
    assert scale.n_grades == len(scale) == 8

    uppers = [g.upper_pd for g in scale.grades]
    assert uppers == sorted(uppers), "bands must be ascending in PD"
    assert uppers[-1] == pytest.approx(1.0)

    for grade in scale.grades:
        assert grade.central_pd <= grade.upper_pd

    assert pd.MasterScale.from_registry_id("moodys_assumptions").n_grades > 0


def test_map_pd_notches_to_the_owning_band() -> None:
    scale = pd.MasterScale.default_calibration()

    result = scale.map_pd(0.003)
    assert result.grade == "BBB"
    assert result.input_pd == pytest.approx(0.003)
    assert result.central_pd == pytest.approx(0.002)
    assert scale.grades[result.grade_index].label == "BBB"

    # A PD exactly on a band's upper bound belongs to that band, not the next.
    boundary = scale.grades[3].upper_pd
    assert scale.map_pd(boundary).grade == scale.grades[3].label


def test_map_pd_rejects_non_finite() -> None:
    scale = pd.MasterScale.default_calibration()
    for bad in (float("nan"), float("inf"), float("-inf")):
        with pytest.raises(ValueError, match="non-finite"):
            scale.map_pd(bad)


def test_map_pd_rejects_out_of_range_probabilities() -> None:
    """A sign error or a percent/decimal mix-up (5.0 for "5%") raises."""
    scale = pd.MasterScale.default_calibration()

    for bad in (-0.1, 1.5, 5.0):
        with pytest.raises(ValueError, match=r"outside \[0, 1\]"):
            scale.map_pd(bad)
    # The closed unit interval itself is accepted.
    assert scale.map_pd(0.0).grade == scale.grades[0].label
    assert scale.map_pd(1.0).grade == scale.grades[-1].label


def test_map_pds_builds_a_grading_table() -> None:
    scale = pd.MasterScale.default_calibration()
    table = scale.map_pds([0.0005, 0.003, 0.05])
    assert list(table.columns) == ["grade", "grade_index", "input_pd", "central_pd"]
    assert list(table["grade"]) == ["AA", "BBB", "B"]
    assert list(scale.map_pds([]).columns) == ["grade", "grade_index", "input_pd", "central_pd"]


def test_master_scale_json_pickle_and_frame() -> None:
    import pickle

    scale = pd.MasterScale.default_calibration()
    assert pd.MasterScale.from_json(scale.to_json()).n_grades == scale.n_grades
    assert pickle.loads(pickle.dumps(scale))  # noqa: S301 - trusted in-process round trip.grades == scale.grades
    frame = scale.to_dataframe()
    assert list(frame.columns) == ["label", "upper_pd", "central_pd"]
    assert len(frame) == scale.n_grades
    grade = scale.grades[0]
    assert pd.MasterScaleGrade.from_json(grade.to_json()) == grade
    assert repr(grade).startswith("MasterScaleGrade(")
    assert 'label="AAA"' in repr(grade)
    assert pd.apply_basel_irb_pd_floor(0.0001) == pd.BASEL_IRB_PD_FLOOR


def test_custom_scale_round_trips() -> None:
    grades = [
        pd.MasterScaleGrade("GOOD", 0.01, 0.005),
        pd.MasterScaleGrade("BAD", 1.0, 0.30),
    ]
    scale = pd.MasterScale(grades)
    assert scale.n_grades == 2
    assert scale.map_pd(0.002).grade == "GOOD"
    assert scale.map_pd(0.5).grade == "BAD"
