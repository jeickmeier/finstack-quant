"""Comps table, regression scatter and composite score of one subject."""

from __future__ import annotations

import json
import math
import pickle

import pytest

from finstack_quant.statements_analytics import (
    CompanyMetrics,
    PeerSet,
    RelativeValueCompany,
    RelativeValueDimension,
    RelativeValueMultiple,
    RelativeValuePoint,
    RelativeValueReport,
    ScoringDimension,
    relative_value_report,
    score_relative_value,
)


def _company(name: str, sector: str = "energy", **metrics: float) -> CompanyMetrics:
    return CompanyMetrics(name, metrics, meta={"gics_sector": sector})


SUBJECT = _company("SUBJ", leverage=2.5, oas_bp=300.0, enterprise_value=1000.0, ebitda=100.0)
UNIVERSE = [
    _company("P1", leverage=1.0, oas_bp=100.0, enterprise_value=800.0, ebitda=100.0),
    _company("OUT", sector="tech", leverage=1.0, oas_bp=9999.0),
    _company("P2", leverage=2.0, oas_bp=200.0, enterprise_value=900.0, ebitda=100.0),
    SUBJECT,
    _company("P3", leverage=3.0, oas_bp=310.0, enterprise_value=1200.0, ebitda=100.0),
    _company("P4", leverage=4.0, oas_bp=380.0, enterprise_value=500.0),
    _company("P5", oas_bp=250.0, enterprise_value=1100.0, ebitda=100.0),
]
ENERGY = {
    "gics_sectors": ["energy"],
    "gics_industries": [],
    "countries": [],
    "market_cap_min": None,
    "market_cap_max": None,
    "ratings": [],
    "required_tags": [],
    "excluded_tags": [],
    "selectors": [],
}
ON_LEVERAGE = {
    "label": "Spread vs Leverage",
    "y_extractor": {"named": "oas_bp"},
    "x_extractor": {"named": "leverage"},
    "weight": 0.7,
}
EV_EBITDA = {
    "label": "EV/EBITDA",
    "y_extractor": {"multiple": "ev_ebitda"},
    "weight": 0.3,
    "direction": "higher_is_rich",
}
MISSING = {"label": "Nobody has it", "y_extractor": {"custom": "missing"}, "weight": 1.0}
CONFIG = {
    "filter": ENERGY,
    "period_basis": "ltm",
    "dimensions": [ON_LEVERAGE, EV_EBITDA, MISSING],
    "multiples": ["ev_ebitda", "pe"],
}


def test_table_has_the_subject_first_and_one_value_per_multiple() -> None:
    report = relative_value_report(SUBJECT, UNIVERSE, CONFIG)

    assert report.subject_id == "SUBJ"
    assert report.period_basis == "ltm"
    assert (report.peer_count, report.excluded_count) == (5, 1)
    assert [row.id for row in report.companies] == ["SUBJ", "P1", "P2", "P3", "P4", "P5"]
    assert [row.is_subject for row in report.companies] == [True, False, False, False, False, False]
    assert report.companies[0].values == [10.0, None]
    assert report.companies[4].values == [None, None]  # P4 has no EBITDA.

    # EV/EBITDA peers 8, 9, 12, 11: mean 10, two of four at or below the subject's 10.
    ev_ebitda, pe = report.multiples
    assert ev_ebitda.multiple == "ev_ebitda"
    assert ev_ebitda.subject_value == 10.0
    assert ev_ebitda.peer_stats is not None
    assert ev_ebitda.peer_stats.mean == pytest.approx(10.0)
    assert ev_ebitda.percentile == 0.5
    assert ev_ebitda.z_score == pytest.approx(0.0)
    assert (pe.subject_value, pe.peer_stats, pe.percentile, pe.z_score) == (None, None, None, None)


def test_regression_dimension_matches_the_hand_calculation() -> None:
    dimension = relative_value_report(SUBJECT, UNIVERSE, CONFIG).dimensions[0]

    assert (dimension.label, dimension.weight, dimension.direction) == ("Spread vs Leverage", 0.7, "higher_is_cheap")
    assert (dimension.y, dimension.x) == ("oas_bp", "leverage")
    assert (dimension.subject_y, dimension.subject_x) == (300.0, 2.5)
    # Y over all five peers: 100, 200, 310, 380, 250. Mean 248; three at or below 300.
    assert dimension.peer_stats is not None
    assert (dimension.peer_stats.count, dimension.peer_stats.mean) == (5, pytest.approx(248.0))
    assert dimension.percentile == 0.6
    assert dimension.z_score == pytest.approx(52.0 / math.sqrt(11_370.0))
    # P5 has no leverage: the fit is over four pairs, y = 10 + 95x, fitted 247.5 at 2.5.
    assert dimension.regression is not None
    assert dimension.regression.slope == pytest.approx(95.0)
    assert dimension.regression.intercept == pytest.approx(10.0)
    assert dimension.regression.fitted_value == pytest.approx(247.5)
    assert dimension.regression.residual == pytest.approx(52.5)
    assert dimension.signal == pytest.approx(52.5 / math.sqrt(350.0 / 3.0))
    assert dimension.excluded_reason is None
    assert [
        (point.company_id, point.is_subject, point.x, point.y, pytest.approx(point.fitted_y))
        for point in dimension.points
    ] == [
        ("SUBJ", True, 2.5, 300.0, 247.5),
        ("P1", False, 1.0, 100.0, 105.0),
        ("P2", False, 2.0, 200.0, 200.0),
        ("P3", False, 3.0, 310.0, 295.0),
        ("P4", False, 4.0, 380.0, 390.0),
    ]


def test_composite_is_the_score_on_the_same_inputs() -> None:
    report = relative_value_report(SUBJECT, UNIVERSE, CONFIG)
    peers = [company for company in UNIVERSE if company.id not in {"SUBJ", "OUT"}]
    score = score_relative_value(
        PeerSet(SUBJECT, peers, "ltm"),
        [
            ScoringDimension("Spread vs Leverage", "oas_bp", "leverage", weight=0.7),
            ScoringDimension("EV/EBITDA", "multiple:ev_ebitda", weight=0.3, direction="higher_is_rich"),
            ScoringDimension("Nobody has it", "missing", weight=1.0),
        ],
    )

    assert report.composite is not None
    assert report.composite.to_json() == score.to_json()
    assert report.composite_excluded_reason is None
    assert [dimension.excluded_reason for dimension in report.dimensions] == [None, None, "no_subject_value"]
    assert report.dimensions[2].signal is None


def test_table_survives_when_no_dimension_is_usable() -> None:
    config = {"period_basis": {"custom": "FY2025E"}, "dimensions": [MISSING], "multiples": ["ev_ebitda"]}
    report = relative_value_report(SUBJECT, UNIVERSE, config)

    assert report.period_basis == "FY2025E"
    # Without a filter the other sector is a peer too.
    assert (report.peer_count, report.excluded_count) == (6, 0)
    assert report.composite is None
    assert report.composite_excluded_reason == "no_usable_dimension"
    assert report.companies[0].values == [10.0]

    no_dimensions = relative_value_report(SUBJECT, UNIVERSE, {**config, "dimensions": []})
    assert no_dimensions.composite_excluded_reason == "no_dimensions"


@pytest.mark.parametrize(
    "config",
    [
        {"period_basis": "ltm", "dimensions": [], "multiples": []},
        {"period_basis": "ltm", "dimensions": [{**ON_LEVERAGE, "weight": -1.0}], "multiples": ["pe"]},
        {"period_basis": "ltm", "dimensions": [], "multiples": ["pe"], "guess": True},
        {"period_basis": "ltm", "dimensions": [], "multiples": ["not_a_multiple"]},
        {"dimensions": [], "multiples": ["pe"]},
        {"filter": {"gics_sector": ["energy"]}, "period_basis": "ltm", "dimensions": [], "multiples": ["pe"]},
    ],
)
def test_invalid_requests_raise_value_error(config: dict[str, object]) -> None:
    with pytest.raises(ValueError, match=r".+"):
        relative_value_report(SUBJECT, UNIVERSE, config)


def test_partial_filter_matches_the_full_filter() -> None:
    partial = {**CONFIG, "filter": {"gics_sectors": ["energy"]}}
    assert relative_value_report(SUBJECT, UNIVERSE, partial).to_json() == (
        relative_value_report(SUBJECT, UNIVERSE, CONFIG).to_json()
    )


def test_misspelt_company_metric_raises_value_error() -> None:
    misspelt = {"id": "TYPO", "attributes": {}, "ebidta": 100.0}
    with pytest.raises(ValueError, match="ebidta"):
        relative_value_report(SUBJECT, [misspelt], CONFIG)


def test_accepts_serde_documents_and_json_text() -> None:
    subject = json.loads(SUBJECT.to_json())
    universe = [json.loads(company.to_json()) for company in UNIVERSE]
    from_documents = relative_value_report(subject, universe, json.dumps(CONFIG))
    assert from_documents.to_json() == relative_value_report(SUBJECT, UNIVERSE, CONFIG).to_json()


def test_to_dataframe_has_one_row_per_company() -> None:
    frame = relative_value_report(SUBJECT, UNIVERSE, CONFIG).to_dataframe()

    assert list(frame.columns) == ["id", "is_subject", "ev_ebitda", "pe"]
    assert list(frame["id"]) == ["SUBJ", "P1", "P2", "P3", "P4", "P5"]
    assert list(frame["is_subject"]) == [True, False, False, False, False, False]
    assert frame["ev_ebitda"].iloc[0] == 10.0
    assert frame["ev_ebitda"].isna().tolist() == [False, False, False, False, True, False]
    assert frame["pe"].isna().all()


def test_wire_round_trips_and_pickles() -> None:
    report = relative_value_report(SUBJECT, UNIVERSE, CONFIG)
    assert RelativeValueReport.from_json(report.to_json()).to_json() == report.to_json()
    assert pickle.loads(pickle.dumps(report)).to_json() == report.to_json()  # noqa: S301
    assert report.to_dict()["subject_id"] == "SUBJ"
    assert "RelativeValueReport" in repr(report)

    parts = [report.companies[0], report.multiples[0], report.dimensions[0], report.dimensions[0].points[0]]
    types = [RelativeValueCompany, RelativeValueMultiple, RelativeValueDimension, RelativeValuePoint]
    for part, kind in zip(parts, types, strict=True):
        assert isinstance(part, kind)
        assert kind.from_json(part.to_json()).to_json() == part.to_json()
        assert pickle.loads(pickle.dumps(part)).to_json() == part.to_json()  # noqa: S301
        assert part.to_dict() == json.loads(part.to_json())
        assert kind.__name__ in repr(part)
        with pytest.raises(ValueError, match=r".+"):
            kind.from_json("{}")
