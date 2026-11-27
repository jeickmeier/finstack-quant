"""Each per-kind ``CalibrationStep`` method takes a dict of wire fields or that kind's params class."""

from __future__ import annotations

import json
from typing import Any

import pytest

from finstack_quant import calibration
from finstack_quant.calibration import CalibrationStep, DiscountCurveParams

BASE_DATE = "2026-05-08"

# Per-kind method, its params class, and one valid set of fields.
KINDS: list[tuple[str, str, dict[str, Any]]] = [
    ("discount", "DiscountCurveParams", {"curve_id": "USD-OIS", "currency": "USD", "base_date": BASE_DATE}),
    (
        "forward",
        "ForwardCurveParams",
        {"curve_id": "USD-SOFR-3M", "base_date": BASE_DATE, "currency": "USD", "discount_curve_id": "USD-OIS"},
    ),
    (
        "hazard",
        "HazardCurveParams",
        {
            "curve_id": "ACME",
            "base_date": BASE_DATE,
            "currency": "USD",
            "discount_curve_id": "USD-OIS",
            "entity": "ACME",
            "seniority": "senior",
            "recovery_rate": 0.4,
        },
    ),
    (
        "inflation",
        "InflationCurveParams",
        {
            "curve_id": "USA-CPI",
            "base_date": BASE_DATE,
            "currency": "USD",
            "discount_curve_id": "USD-OIS",
            "index": "USA-CPI-U",
            "observation_lag": "3M",
            "base_cpi": 310.0,
            "source_base_date": "2026-02-01",
            "source_base_cpi": 310.0,
            "day_count": "act_365f",
            "interpolation": "linear",
            "extrapolation": "extend",
        },
    ),
    (
        "vol_surface",
        "VolSurfaceParams",
        {"vol_surface_id": "AAPL-VOL", "base_date": BASE_DATE, "underlying_ticker": "AAPL"},
    ),
    (
        "swaption_vol",
        "SwaptionVolParams",
        {"vol_surface_id": "SWPT", "base_date": BASE_DATE, "currency": "USD", "discount_curve_id": "USD-OIS"},
    ),
    (
        "base_correlation",
        "BaseCorrelationParams",
        {
            "index_id": "CDX.NA.IG",
            "series": 42,
            "maturity_years": 5.0,
            "base_date": BASE_DATE,
            "currency": "USD",
            "discount_curve_id": "USD-OIS",
            "date_regime": "bespoke",
        },
    ),
    (
        "student_t",
        "StudentTParams",
        {"tranche_instrument_id": "TRANCHE-1", "base_correlation_curve_id": "CDX.NA.IG_CORR"},
    ),
    (
        "hull_white",
        "HullWhiteStepParams",
        {"curve_id": "USD-OIS", "currency": "USD", "base_date": BASE_DATE, "fit_tolerance": 0.0001},
    ),
    (
        "cap_floor_hull_white",
        "CapFloorHullWhiteStepParams",
        {
            "discount_curve_id": "USD-OIS",
            "forward_curve_id": "USD-SOFR-3M",
            "currency": "USD",
            "base_date": BASE_DATE,
            "index_id": "USD-SOFR-3M",
            "fit_tolerance": 0.0001,
        },
    ),
    (
        "svi_surface",
        "SviSurfaceParams",
        {"vol_surface_id": "AAPL-SVI", "base_date": BASE_DATE, "underlying_ticker": "AAPL"},
    ),
    (
        "xccy_basis",
        "XccyBasisParams",
        {
            "curve_id": "EUR-XCCY",
            "currency": "EUR",
            "base_date": BASE_DATE,
            "fx_spot": 1.1,
            "domestic_discount_id": "USD-OIS",
        },
    ),
    ("parametric", "ParametricCurveParams", {"curve_id": "USD-NS", "base_date": BASE_DATE, "model": "ns"}),
]
KIND_IDS = [kind for kind, _, _ in KINDS]


def _wire(step: CalibrationStep) -> dict[str, Any]:
    return json.loads(step.to_json())


def test_the_cases_cover_every_params_class() -> None:
    exported = {name for name in dir(calibration) if name.endswith("Params")}
    assert {class_name for _, class_name, _ in KINDS} == exported
    assert all(callable(getattr(CalibrationStep, kind)) for kind in KIND_IDS)


@pytest.mark.parametrize(("kind", "class_name", "fields"), KINDS, ids=KIND_IDS)
def test_dict_typed_class_and_constructor_build_the_same_step(
    kind: str, class_name: str, fields: dict[str, Any]
) -> None:
    typed = getattr(calibration, class_name)(**fields)
    build = getattr(CalibrationStep, kind)

    from_dict = _wire(build("step", fields))
    assert from_dict["kind"] == kind
    assert _wire(build("step", typed)) == from_dict
    assert _wire(CalibrationStep("step", typed)) == from_dict
    assert _wire(build("step", typed, "other-quotes")) == _wire(build("step", fields, "other-quotes"))


@pytest.mark.parametrize("index", range(len(KINDS)), ids=KIND_IDS)
def test_another_kinds_params_class_is_a_type_error(index: int) -> None:
    kind = KINDS[index][0]
    _, other_class, other_fields = KINDS[(index + 1) % len(KINDS)]
    other = getattr(calibration, other_class)(**other_fields)

    with pytest.raises(TypeError, match=rf"CalibrationStep\.{kind} takes a dict or the {kind} step's params class"):
        getattr(CalibrationStep, kind)("step", other)


def test_only_the_dict_form_takes_an_omitted_identifier_from_the_step_id() -> None:
    fields = {"currency": "USD", "base_date": BASE_DATE}

    assert _wire(CalibrationStep.discount("USD-OIS", fields))["curve_id"] == "USD-OIS"
    with pytest.raises(ValueError, match="missing field `curve_id`"):
        DiscountCurveParams(**fields)
