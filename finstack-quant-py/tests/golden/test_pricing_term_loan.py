"""Term-loan pricing goldens."""

from __future__ import annotations

import json

import pytest

from finstack_quant.valuations.instruments import price_instrument_with_metrics

from .conftest import discover_fixtures, fixture_path, run_golden
from .pricing_validation import validated_instrument_json
from .runners.pricing_common import _resolve_market


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/term_loan"))
def test_pricing_term_loan(fixture: str) -> None:
    """Run every term-loan pricing fixture through the Python bindings."""
    run_golden(fixture)


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/term_loan"))
def test_registered_term_loan_metrics_cross_python_json_boundary(fixture: str) -> None:
    """Loan-specific registered metrics remain requestable from Python."""
    body = json.loads(fixture_path(fixture).read_text(encoding="utf-8"))
    result = json.loads(
        price_instrument_with_metrics(
            validated_instrument_json(body["instrument"]),
            _resolve_market(body["market"]),
            body["metadata"]["valuation_date"],
            model=body["model"],
            metrics=["all_in_rate", "yt2y"],
        )
    )
    assert "all_in_rate" in result["measures"]
    assert "yt2y" in result["measures"]
