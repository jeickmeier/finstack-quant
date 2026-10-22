"""Comparable-company analytics binding tests."""

from __future__ import annotations

import pytest

from finstack_quant.statements_analytics import (
    compute_multiple,
    percentile_rank,
    regression_fair_value,
    score_relative_value,
    z_score,
)

_COMPANY_METRIC_FIELDS = [
    "enterprise_value",
    "market_cap",
    "share_price",
    "oas_bp",
    "yield_pct",
    "ebitda",
    "revenue",
    "ebit",
    "ufcf",
    "lfcf",
    "net_income",
    "book_value",
    "tangible_book_value",
    "dividends_per_share",
    "leverage",
    "interest_coverage",
    "revenue_growth",
    "ebitda_margin",
]


def _company(cid: str, custom: dict[str, float] | None = None, **metrics: float) -> dict:
    """Build a canonical serde ``CompanyMetrics`` payload."""
    blank: dict = dict.fromkeys(_COMPANY_METRIC_FIELDS)
    return {"id": cid, "attributes": {}, "custom": custom or {}, **blank, **metrics}


class TestCompsBindings:
    def test_compute_multiple(self) -> None:
        metrics = {"enterprise_value": 8_500.0, "ebitda": 1_000.0}
        assert compute_multiple(metrics, "ev_ebitda") == pytest.approx(8.5)

    def test_regression_fair_value(self) -> None:
        result = regression_fair_value([1.0, 2.0, 3.0, 4.0], [3.0, 5.0, 7.0, 9.0], 3.0, 10.0)
        assert result.fitted_value == pytest.approx(7.0)
        assert result.residual == pytest.approx(3.0)

    def test_percentile_rank(self) -> None:
        assert percentile_rank([100.0, 200.0, 300.0, 400.0, 500.0], 250.0) == pytest.approx(0.4)
        assert percentile_rank([], 100.0) is None

    def test_z_score(self) -> None:
        assert z_score([1.0, 2.0, 3.0, 4.0, 5.0], 3.0) == pytest.approx(0.0)
        assert z_score([1.0], 1.0) is None
        assert z_score([5.0, 5.0, 5.0], 5.0) is None

    def test_score_relative_value(self) -> None:
        peer_set = {
            "subject": _company("SUBJ", leverage=2.0, oas_bp=250.0),
            "peers": [
                _company("P1", leverage=1.0, oas_bp=100.0),
                # Regression scoring requires nonzero peer residual dispersion.
                _company("P2", leverage=2.0, oas_bp=210.0),
                _company("P3", leverage=3.0, oas_bp=300.0),
            ],
            "period_basis": "ltm",
        }
        result = score_relative_value(
            peer_set,
            [
                {
                    "label": "Spread vs Leverage",
                    "y_extractor": {"named": "oas_bp"},
                    "x_extractor": {"named": "leverage"},
                    "weight": 1.0,
                }
            ],
        )
        assert result.company_id == "SUBJ"
        assert not hasattr(result, "by_dimension")
        assert result.dimensions[0].label == "Spread vs Leverage"
        assert result.composite_score > 0.0

    def test_score_relative_value_accepts_json_strings(self) -> None:
        import json

        peer_set = {
            "subject": _company("SUBJ", leverage=2.0),
            "peers": [_company("P1", leverage=1.0), _company("P2", leverage=3.0)],
            "period_basis": "ltm",
        }
        dimensions = [
            {
                "label": "Leverage",
                "y_extractor": {"named": "leverage"},
                "x_extractor": None,
                "weight": 1.0,
            }
        ]
        typed = score_relative_value(peer_set, dimensions)
        via_json = score_relative_value(json.dumps(peer_set), json.dumps(dimensions))
        assert via_json.to_json() == typed.to_json()

    def test_peer_stats_uses_rust_field_names(self) -> None:
        from finstack_quant.statements_analytics import peer_stats

        stats = peer_stats([1.0, 2.0, 3.0, 4.0, 5.0])
        assert stats.count == 5
        assert not hasattr(stats, "n")
        assert stats.iqr == pytest.approx(stats.q3 - stats.q1)
        # No-result path returns None, matching the WASM twin's `undefined`.
        assert peer_stats([]) is None

    def test_score_relative_value_direction_flips_sign(self) -> None:
        def pe_peer_set() -> dict:
            return {
                "subject": _company("SUBJ", custom={"pe": 30.0}),
                "peers": [
                    _company("P1", custom={"pe": 10.0}),
                    _company("P2", custom={"pe": 15.0}),
                    _company("P3", custom={"pe": 20.0}),
                ],
                "period_basis": "ltm",
            }

        def dimension(direction: str | None) -> dict:
            spec = {
                "label": "pe",
                "y_extractor": {"custom": "pe"},
                "x_extractor": None,
                "weight": 1.0,
            }
            if direction is not None:
                spec["direction"] = direction
            return spec

        cheap_convention = score_relative_value(pe_peer_set(), [dimension(None)])
        rich_convention = score_relative_value(pe_peer_set(), [dimension("higher_is_rich")])
        # High multiple vs peers: rich (negative) under higher_is_rich, cheap
        # (positive) under the default higher_is_cheap convention.
        assert cheap_convention.composite_score > 0.0
        assert rich_convention.composite_score < 0.0
        assert rich_convention.composite_score == pytest.approx(-cheap_convention.composite_score)

    def test_score_relative_value_rejects_unknown_direction(self) -> None:
        peer_set = {
            "subject": _company("SUBJ", custom={"pe": 30.0}),
            "peers": [_company("P1", custom={"pe": 10.0})],
            "period_basis": "ltm",
        }
        with pytest.raises(ValueError, match="down_is_up"):
            score_relative_value(
                peer_set,
                [
                    {
                        "label": "pe",
                        "y_extractor": {"custom": "pe"},
                        "x_extractor": None,
                        "weight": 1.0,
                        "direction": "down_is_up",
                    }
                ],
            )

    def test_score_relative_value_multiple_extractor(self) -> None:
        peer_set = {
            "subject": _company("SUBJ", enterprise_value=12_000.0, ebitda=1_000.0),
            "peers": [
                _company("P1", enterprise_value=8_000.0, ebitda=1_000.0),
                _company("P2", enterprise_value=9_000.0, ebitda=1_000.0),
                _company("P3", enterprise_value=10_000.0, ebitda=1_000.0),
            ],
            "period_basis": "ltm",
        }
        result = score_relative_value(
            peer_set,
            [
                {
                    "label": "EV/EBITDA",
                    "y_extractor": {"multiple": "ev_ebitda"},
                    "x_extractor": None,
                    "weight": 1.0,
                }
            ],
        )
        assert result.peer_count == 3
        assert result.dimensions[0].label == "EV/EBITDA"

    def test_non_numeric_metric_raises_value_error(self) -> None:
        peer_set = {
            "subject": _company("SUBJ") | {"oas_bp": "wide"},
            "peers": [_company("P1", oas_bp=100.0)],
            "period_basis": "ltm",
        }
        with pytest.raises(ValueError, match="peer_set"):
            score_relative_value(
                peer_set,
                [
                    {
                        "label": "oas_bp",
                        "y_extractor": {"named": "oas_bp"},
                        "x_extractor": None,
                        "weight": 1.0,
                    }
                ],
            )

    def test_none_metric_treated_as_missing(self) -> None:
        metrics = {"enterprise_value": 8_500.0, "ebitda": 1_000.0, "revenue": None}
        assert compute_multiple(metrics, "ev_ebitda") == pytest.approx(8.5)
